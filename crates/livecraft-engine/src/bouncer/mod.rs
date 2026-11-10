mod report;

use crate::compiled::StemTarget;
use crate::driver::{AudioDriver, VirtualDriver};
use crate::executor::{Engine, EngineCommand};
use crate::output::Playback;
pub use livecraft_dsp::dither::DitherMode;
use livecraft_dsp::dither::Ditherer;
pub use livecraft_dsp::metrics::AudioMetrics;
use livecraft_dsp::metrics::analyze_audio;
pub use livecraft_formats::audio_encode::{AudioFileFormat, BitDepth};
use livecraft_formats::audio_encode::{write_mp3_320, write_pcm_f32, write_pcm_i16, write_pcm_i24};
use livecraft_formats::project_dir::ProjectContext;
use livecraft_model::ids::TrackId;
use livecraft_model::model::{Project, Track, TrackType};
use livecraft_time::Beats;
pub use report::{
    ANALYSIS_FILE_VERSION, ANALYSIS_OVERVIEW_BLOCK_FRAMES, AnalysisFile, PeakFrame,
    RenderOutputReport, RenderReport,
};
use report::{
    audio_file_format_serde, dither_mode_serde, ensure_parent_dir, sha256_file, sidecar_path,
    write_analysis_sidecar, write_report_file,
};
use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Resampler};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Frames the bouncer asks `VirtualDriver` for at a time (fixed for determinism).
const BLOCK_FRAMES: usize = 512;

/// Which track(s) to render, matching Live's Rendered Track chooser (manual 5.1.3.1, p. 127).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RenderedTrack {
    #[default]
    Main,
    AllIndividualTracks,
    SelectedTracks(Vec<TrackId>),
    Track(TrackId),
}

/// Export options matching Live 12.4's Export Audio/Video dialog (manual 5.1.3, p. 126-130).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RenderOptions {
    pub rendered_track: RenderedTrack,
    pub sample_rate: u32,
    /// When `Some(src)` and `src != sample_rate`, renders at `src` and resamples via `rubato::Fft`.
    pub source_sample_rate: Option<u32>,
    /// Zero-based bar offset where rendering begins (ignored when `render_start_beats` is `Some`).
    pub start_bars: u32,
    /// Render length in bars (ignored when `render_length_beats` is `Some`).
    pub duration_bars: u32,
    /// Optional explicit render start position in quarter-note beats.
    pub render_start_beats: Option<f64>,
    /// Optional explicit render length in quarter-note beats.
    pub render_length_beats: Option<f64>,
    /// PCM bit depth: `16`, `24`, or `32` (32-bit IEEE float, unclamped and undithered).
    pub bits_per_sample: u16,
    #[serde(with = "dither_mode_serde")]
    pub dither: DitherMode,
    #[serde(with = "audio_file_format_serde")]
    pub file_type: AudioFileFormat,
    pub encode_pcm: bool,
    pub encode_mp3: bool,
    pub include_return_and_main_effects: bool,
    pub render_as_loop: bool,
    pub convert_to_mono: bool,
    pub normalize: bool,
    pub create_analysis_file: bool,
    pub write_report: bool,
    /// Optional project context for resolving relative audio clip paths during export.
    #[serde(skip)]
    pub project_context: Option<ProjectContext>,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            rendered_track: RenderedTrack::Main,
            sample_rate: 44100,
            source_sample_rate: None,
            start_bars: 0,
            duration_bars: 8,
            render_start_beats: None,
            render_length_beats: None,
            bits_per_sample: 24,
            dither: DitherMode::Triangular,
            file_type: AudioFileFormat::Wav,
            encode_pcm: true,
            encode_mp3: false,
            include_return_and_main_effects: false,
            render_as_loop: false,
            convert_to_mono: false,
            normalize: false,
            create_analysis_file: false,
            write_report: false,
            project_context: None,
        }
    }
}

/// Starts an engine on the Set for a render, with the first clip of each track that has one launched
/// and the transport playing from the start. The caller drives the returned `Engine` with
/// `process_offline` (or `VirtualDriver`).
pub fn start_render(project: &Project, sample_rate: f32) -> anyhow::Result<(Playback, Engine)> {
    start_render_in_context(project, sample_rate, ProjectContext::default())
}

/// As [`start_render`], using `context` to resolve relative audio clip references.
pub fn start_render_in_context(
    project: &Project,
    sample_rate: f32,
    context: ProjectContext,
) -> anyhow::Result<(Playback, Engine)> {
    let (mut playback, engine) = Playback::start_in_context(project, sample_rate, context)?;
    for trk in project.tracks.iter() {
        if let Some(slot) = trk.clip_slots.first()
            && slot.clip.is_some()
        {
            playback.handle.send(EngineCommand::TriggerClip {
                track: trk.id,
                slot: 0,
            })?;
        }
    }
    playback.handle.send(EngineCommand::SetPlaying(true))?;
    Ok((playback, engine))
}

pub struct OfflineBouncer;

impl OfflineBouncer {
    /// Exports a project according to `options`.
    ///
    /// For [`RenderedTrack::Main`] or [`RenderedTrack::Track`], `output` is the target audio file
    /// path (when `encode_pcm` is false and `encode_mp3` is true, an `.mp3` path is used).
    /// For [`RenderedTrack::AllIndividualTracks`] or [`RenderedTrack::SelectedTracks`], `output` is
    /// the destination directory receiving one file per exported stem.
    pub fn export<P: AsRef<Path>>(
        project: &Project,
        output: P,
        options: RenderOptions,
    ) -> anyhow::Result<RenderReport> {
        validate_options(&options)?;
        let render_rate = options.source_sample_rate.unwrap_or(options.sample_rate);
        let (start_frames, render_frames) = compute_frame_range(project, &options, render_rate)?;
        let stems = resolve_stems(project, &options)?;

        let output_path = output.as_ref();
        let is_multi_stem = matches!(
            options.rendered_track,
            RenderedTrack::AllIndividualTracks | RenderedTrack::SelectedTracks(_)
        );
        if is_multi_stem {
            fs::create_dir_all(output_path)?;
        }

        let mut outputs = Vec::new();
        for (idx, stem) in stems.iter().enumerate() {
            let base_path = if is_multi_stem {
                let sanitized = sanitize_track_name(&stem.name);
                let ext = if options.encode_pcm {
                    pcm_extension(options.file_type)
                } else {
                    "mp3"
                };
                output_path.join(format!("{:02}_{}.{}", idx + 1, sanitized, ext))
            } else {
                output_path.to_path_buf()
            };

            let stem_outputs = render_and_write_stem(
                project,
                stem.target,
                &base_path,
                &options,
                render_rate,
                start_frames,
                render_frames,
            )?;
            outputs.extend(stem_outputs);
        }

        let report = RenderReport {
            livecraft_version: env!("CARGO_PKG_VERSION").to_string(),
            git_commit: option_env!("LIVECRAFT_GIT_COMMIT")
                .unwrap_or("unknown")
                .to_string(),
            settings: options.clone(),
            outputs,
        };

        if options.write_report {
            let report_path = if is_multi_stem {
                output_path.join("render.report.json")
            } else {
                sidecar_path(output_path, "report.json")
            };
            write_report_file(&report_path, &report)?;
        }

        Ok(report)
    }

    /// Renders the project to `output_path` using `options`.
    pub fn render_project<P: AsRef<Path>>(
        project: &Project,
        output_path: P,
        options: RenderOptions,
    ) -> anyhow::Result<()> {
        Self::export(project, output_path, options)?;
        Ok(())
    }

    /// Renders the project to `output_path` and returns the full [`RenderReport`].
    pub fn render_project_with_report<P: AsRef<Path>>(
        project: &Project,
        output_path: P,
        options: RenderOptions,
    ) -> anyhow::Result<RenderReport> {
        Self::export(project, output_path, options)
    }

    /// Renders individual track stems into `output_dir` and returns the written file paths.
    ///
    /// When `options.rendered_track` is [`RenderedTrack::Main`], it exports
    /// [`RenderedTrack::AllIndividualTracks`] (Audio, Group, MIDI with instrument or audio-FX,
    /// Return, and Main tracks; skipping MIDI tracks with an empty `device_chain`).
    pub fn render_stems<P: AsRef<Path>>(
        project: &Project,
        output_dir: P,
        mut options: RenderOptions,
    ) -> anyhow::Result<Vec<String>> {
        if options.rendered_track == RenderedTrack::Main {
            options.rendered_track = RenderedTrack::AllIndividualTracks;
        }
        let report = Self::export(project, output_dir, options)?;
        Ok(report.outputs.into_iter().map(|out| out.path).collect())
    }
}

struct StemSpec {
    name: String,
    target: StemTarget,
}

fn validate_options(options: &RenderOptions) -> anyhow::Result<()> {
    if !options.encode_pcm && !options.encode_mp3 {
        anyhow::bail!("at least one of encode_pcm or encode_mp3 must be enabled");
    }
    if options.sample_rate == 0 {
        anyhow::bail!("sample_rate must be greater than zero");
    }
    if options.source_sample_rate == Some(0) {
        anyhow::bail!("source_sample_rate must be greater than zero");
    }
    if options.encode_pcm {
        bit_depth_from_u16(options.bits_per_sample)?;
        if options.file_type == AudioFileFormat::Flac && options.bits_per_sample == 32 {
            anyhow::bail!("FLAC does not support 32-bit float samples");
        }
    }
    Ok(())
}

fn bit_depth_from_u16(bits: u16) -> anyhow::Result<BitDepth> {
    match bits {
        16 => Ok(BitDepth::Bits16),
        24 => Ok(BitDepth::Bits24),
        32 => Ok(BitDepth::Float32),
        other => anyhow::bail!("unsupported bit depth {other}; expected 16, 24, or 32"),
    }
}

fn compute_frame_range(
    project: &Project,
    options: &RenderOptions,
    render_rate: u32,
) -> anyhow::Result<(usize, usize)> {
    let ts = &project.transport.time_signature;
    let tempo = &project.transport.tempo;

    let start_beat = match options.render_start_beats {
        Some(beats) => {
            if !beats.is_finite() || beats < 0.0 {
                anyhow::bail!("render_start_beats must be finite and non-negative");
            }
            beats
        }
        None => ts.bar_start(options.start_bars as i32).0,
    };

    let end_beat = match options.render_length_beats {
        Some(len) => {
            if !len.is_finite() || len <= 0.0 {
                anyhow::bail!("render_length_beats must be finite and greater than zero");
            }
            start_beat + len
        }
        None => {
            if options.duration_bars == 0 {
                anyhow::bail!("duration_bars must be greater than zero");
            }
            let bar_base = ts.bar_start(options.start_bars as i32).0;
            let bar_end = ts
                .bar_start((options.start_bars + options.duration_bars) as i32)
                .0;
            start_beat + (bar_end - bar_base).max(0.0)
        }
    };

    let start_frame = tempo
        .beats_to_samples(Beats(start_beat), f64::from(render_rate))
        .round()
        .max(0.0) as usize;
    let end_frame = tempo
        .beats_to_samples(Beats(end_beat), f64::from(render_rate))
        .round()
        .max(0.0) as usize;
    let render_frames = end_frame.saturating_sub(start_frame);
    if render_frames == 0 {
        anyhow::bail!("render range produces zero frames");
    }
    Ok((start_frame, render_frames))
}

fn stem_target_for_track(track: &Track, include_return_and_main: bool) -> StemTarget {
    if track.track_type == TrackType::Main {
        StemTarget::Main
    } else if include_return_and_main {
        StemTarget::WetNode(track.id)
    } else {
        StemTarget::DryNode(track.id)
    }
}

fn should_skip_in_multi_stem(track: &Track) -> bool {
    track.track_type == TrackType::Midi && track.device_chain.is_empty()
}

fn resolve_stems(project: &Project, options: &RenderOptions) -> anyhow::Result<Vec<StemSpec>> {
    match &options.rendered_track {
        RenderedTrack::Main => Ok(vec![StemSpec {
            name: project.main_track.name.clone(),
            target: StemTarget::Main,
        }]),
        RenderedTrack::Track(id) => {
            let track = project
                .track(*id)
                .ok_or_else(|| anyhow::anyhow!("track {} not found in project", id.0))?;
            Ok(vec![StemSpec {
                name: track.name.clone(),
                target: stem_target_for_track(track, options.include_return_and_main_effects),
            }])
        }
        RenderedTrack::AllIndividualTracks => Ok(project
            .all_tracks()
            .filter(|track| !should_skip_in_multi_stem(track))
            .map(|track| StemSpec {
                name: track.name.clone(),
                target: stem_target_for_track(track, options.include_return_and_main_effects),
            })
            .collect()),
        RenderedTrack::SelectedTracks(ids) => {
            for id in ids {
                if project.track(*id).is_none() {
                    anyhow::bail!("selected track {} not found in project", id.0);
                }
            }
            let selected: HashSet<TrackId> = ids.iter().copied().collect();
            Ok(project
                .all_tracks()
                .filter(|track| selected.contains(&track.id) && !should_skip_in_multi_stem(track))
                .map(|track| StemSpec {
                    name: track.name.clone(),
                    target: stem_target_for_track(track, options.include_return_and_main_effects),
                })
                .collect())
        }
    }
}

fn sanitize_track_name(name: &str) -> String {
    let cleaned = name.replace([' ', '/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
    if cleaned.is_empty() {
        "Track".to_string()
    } else {
        cleaned
    }
}

fn pcm_extension(format: AudioFileFormat) -> &'static str {
    match format {
        AudioFileFormat::Wav => "wav",
        AudioFileFormat::Aiff => "aiff",
        AudioFileFormat::Flac => "flac",
    }
}

fn format_label(format: AudioFileFormat) -> &'static str {
    match format {
        AudioFileFormat::Wav => "wav",
        AudioFileFormat::Aiff => "aiff",
        AudioFileFormat::Flac => "flac",
    }
}

fn mp3_output_path(base_path: &Path) -> PathBuf {
    match base_path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("mp3") => base_path.to_path_buf(),
        Some(ext)
            if ext.eq_ignore_ascii_case("wav")
                || ext.eq_ignore_ascii_case("aif")
                || ext.eq_ignore_ascii_case("aiff")
                || ext.eq_ignore_ascii_case("flac") =>
        {
            base_path.with_extension("mp3")
        }
        None => base_path.with_extension("mp3"),
        Some(_) => sidecar_path(base_path, "mp3"),
    }
}

fn render_stem_f32(
    project: &Project,
    stem_target: StemTarget,
    render_rate: u32,
    start_frames: usize,
    render_frames: usize,
    render_as_loop: bool,
    project_context: &ProjectContext,
) -> anyhow::Result<Vec<f32>> {
    let (mut playback, mut engine) =
        start_render_in_context(project, render_rate as f32, project_context.clone())?;
    engine.set_stem_target(Some(stem_target));
    let mut driver = VirtualDriver::new(render_rate, BLOCK_FRAMES as u32, 2);
    driver.start(engine).map_err(|e| anyhow::anyhow!("{e}"))?;

    if start_frames > 0 {
        driver.skip_frames(start_frames);
    }

    let mut audio = vec![0.0f32; render_frames * 2];
    driver.render(&mut audio);

    if render_as_loop {
        let tail_frames = driver
            .engine_mut()
            .map_or(0, |engine| engine.tail_samples() as usize);
        if tail_frames > 0 {
            playback.handle.send(EngineCommand::SetPlaying(false))?;
            let mut tail = vec![0.0f32; tail_frames * 2];
            driver.render(&mut tail);

            drop(driver);
            drop(playback);

            let (_playback2, mut engine2) =
                start_render_in_context(project, render_rate as f32, project_context.clone())?;
            engine2.set_stem_target(Some(stem_target));
            let mut driver2 = VirtualDriver::new(render_rate, BLOCK_FRAMES as u32, 2);
            driver2.start(engine2).map_err(|e| anyhow::anyhow!("{e}"))?;
            if start_frames > 0 {
                driver2.skip_frames(start_frames);
            }
            audio.fill(0.0);
            driver2.render(&mut audio);

            for frame in 0..tail_frames {
                let dst = (frame % render_frames) * 2;
                let src = frame * 2;
                audio[dst] += tail[src];
                audio[dst + 1] += tail[src + 1];
            }
        }
    }

    Ok(audio)
}

fn resample_interleaved(
    samples: &[f32],
    channels: usize,
    src_rate: u32,
    dst_rate: u32,
) -> anyhow::Result<Vec<f32>> {
    let input_frames = samples.len() / channels;
    let mut resampler = Fft::<f32>::new(
        src_rate as usize,
        dst_rate as usize,
        1024,
        channels,
        FixedSync::Both,
    )
    .map_err(|e| {
        anyhow::anyhow!("resampler construction failed ({src_rate} -> {dst_rate}): {e}")
    })?;
    let input = InterleavedSlice::new(samples, channels, input_frames)
        .map_err(|e| anyhow::anyhow!("resampler input buffer invalid: {e}"))?;
    let output = resampler
        .process_all(&input, input_frames, None)
        .map_err(|e| anyhow::anyhow!("resampling failed ({src_rate} -> {dst_rate}): {e}"))?;
    Ok(output.take_data())
}

fn fold_to_mono(stereo: &[f32]) -> Vec<f32> {
    let mut mono = Vec::with_capacity(stereo.len() / 2);
    for frame in stereo.as_chunks::<2>().0 {
        mono.push((frame[0] + frame[1]) * 0.5);
    }
    mono
}

fn normalize_in_place(samples: &mut [f32]) {
    let mut peak = 0.0f32;
    for &sample in samples.iter() {
        if sample.is_finite() {
            let abs = sample.abs();
            if abs > peak {
                peak = abs;
            }
        }
    }
    if peak > 0.0 && peak.is_finite() {
        let scale = 1.0 / f64::from(peak);
        for sample in samples.iter_mut() {
            *sample = if sample.is_finite() {
                (f64::from(*sample) * scale) as f32
            } else {
                0.0
            };
        }
    }
}

fn render_and_write_stem(
    project: &Project,
    stem_target: StemTarget,
    base_path: &Path,
    options: &RenderOptions,
    render_rate: u32,
    start_frames: usize,
    render_frames: usize,
) -> anyhow::Result<Vec<RenderOutputReport>> {
    let project_context = options.project_context.clone().unwrap_or_default();
    let mut samples = render_stem_f32(
        project,
        stem_target,
        render_rate,
        start_frames,
        render_frames,
        options.render_as_loop,
        &project_context,
    )?;

    if let Some(src_rate) = options.source_sample_rate
        && src_rate != options.sample_rate
    {
        samples = resample_interleaved(&samples, 2, src_rate, options.sample_rate)?;
    }

    let channels: usize = if options.convert_to_mono {
        samples = fold_to_mono(&samples);
        1
    } else {
        2
    };

    if options.normalize {
        normalize_in_place(&mut samples);
    }

    let frames = (samples.len() / channels) as u64;
    let metrics = analyze_audio(&samples, channels, options.sample_rate);
    let ch_u16 = channels as u16;
    let mut reports = Vec::new();

    if options.encode_pcm {
        ensure_parent_dir(base_path)?;
        let bit_depth = bit_depth_from_u16(options.bits_per_sample)?;
        match bit_depth {
            BitDepth::Bits16 => {
                let mut ditherer = Ditherer::with_default_seed(options.dither, channels);
                let quantized = ditherer.quantize_i16(&samples, channels);
                write_pcm_i16(
                    base_path,
                    options.file_type,
                    options.sample_rate,
                    ch_u16,
                    &quantized,
                )?;
            }
            BitDepth::Bits24 => {
                let mut ditherer = Ditherer::with_default_seed(options.dither, channels);
                let quantized = ditherer.quantize_i24(&samples, channels);
                write_pcm_i24(
                    base_path,
                    options.file_type,
                    options.sample_rate,
                    ch_u16,
                    &quantized,
                )?;
            }
            BitDepth::Float32 => {
                // 32-bit float is never dithered and never clamped.
                write_pcm_f32(
                    base_path,
                    options.file_type,
                    options.sample_rate,
                    ch_u16,
                    &samples,
                )?;
            }
        }

        if options.create_analysis_file {
            write_analysis_sidecar(
                base_path,
                options.sample_rate,
                ch_u16,
                frames,
                metrics,
                &samples,
            )?;
        }

        reports.push(RenderOutputReport {
            path: base_path.to_string_lossy().into_owned(),
            format: format_label(options.file_type).to_string(),
            bit_depth: options.bits_per_sample,
            sample_rate: options.sample_rate,
            channels: ch_u16,
            frames,
            sha256: sha256_file(base_path)?,
            metrics,
        });
    }

    if options.encode_mp3 {
        let mp3_path = mp3_output_path(base_path);
        ensure_parent_dir(&mp3_path)?;
        write_mp3_320(&mp3_path, options.sample_rate, ch_u16, &samples)?;

        if options.create_analysis_file {
            write_analysis_sidecar(
                &mp3_path,
                options.sample_rate,
                ch_u16,
                frames,
                metrics,
                &samples,
            )?;
        }

        reports.push(RenderOutputReport {
            path: mp3_path.to_string_lossy().into_owned(),
            format: "mp3".to_string(),
            bit_depth: 0,
            sample_rate: options.sample_rate,
            channels: ch_u16,
            frames,
            sha256: sha256_file(&mp3_path)?,
            metrics,
        });
    }

    Ok(reports)
}

#[cfg(test)]
mod tests;
