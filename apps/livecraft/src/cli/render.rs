use clap::{Args, ValueEnum};
use livecraft_engine::bouncer::{
    AudioFileFormat, DitherMode, OfflineBouncer, RenderOptions, RenderReport, RenderedTrack,
};
use livecraft_formats::project_dir::ProjectContext;
use livecraft_model::ids::TrackId;
use livecraft_model::model::Project;
use std::path::{Path, PathBuf};

#[derive(Args, Debug)]
pub struct RenderArgs {
    /// Path to .als or .json project
    pub path: PathBuf,
    /// Output audio file path (or destination directory when --track is all or selected)
    #[arg(short, long, default_value = "output.wav", value_name = "PATH")]
    pub output: PathBuf,
    /// Render Length in musical bars (overridden by --length-beats)
    #[arg(short, long, default_value_t = 8, value_name = "BARS")]
    pub duration_bars: u32,
    /// Render Start in zero-based musical bars (overridden by --start-beats)
    #[arg(long, default_value_t = 0, value_name = "BARS")]
    pub start_bars: u32,
    /// Render Start in quarter-note beats (overrides --start-bars)
    #[arg(long, value_name = "BEATS")]
    pub start_beats: Option<f64>,
    /// Render Length in quarter-note beats (overrides --duration-bars)
    #[arg(long, value_name = "BEATS")]
    pub length_beats: Option<f64>,
    /// Rendered Track: main, all, selected, track numeric ID, or track name
    #[arg(long, default_value = "main", value_name = "TARGET")]
    pub track: String,
    /// Comma-separated track IDs or names used when --track selected
    #[arg(long, value_delimiter = ',', value_name = "IDS")]
    pub selected_tracks: Vec<String>,
    /// Include Return and Main effects on individual or selected track stems
    #[arg(long)]
    pub include_returns_main: bool,
    /// Render as Loop (two-pass render wrapping effect tails onto the start)
    #[arg(long)]
    pub render_as_loop: bool,
    /// Convert to Mono instead of stereo
    #[arg(long)]
    pub mono: bool,
    /// Normalize peak amplitude to 0 dBFS
    #[arg(long)]
    pub normalize: bool,
    /// Create .lcanalysis sidecar analysis file beside each exported audio file
    #[arg(long)]
    pub analysis_file: bool,
    /// Target Sample Rate in Hz (e.g. 44100, 48000, 96000)
    #[arg(short, long, default_value_t = 44100, value_name = "HZ")]
    pub sample_rate: u32,
    /// Internal project sample rate in Hz before resampling to --sample-rate
    #[arg(long, value_name = "HZ")]
    pub source_sample_rate: Option<u32>,
    /// Disable Encode PCM when --mp3 is enabled
    #[arg(long)]
    pub no_pcm: bool,
    /// PCM File Type (wav, aiff, or flac)
    #[arg(
        short,
        long,
        value_enum,
        default_value_t = CliAudioFormat::Wav,
        value_name = "wav|aiff|flac"
    )]
    pub format: CliAudioFormat,
    /// PCM Bit Depth: 16, 24, or 32 (32-bit float is unclamped and undithered)
    #[arg(
        short,
        long,
        default_value_t = 24,
        value_parser = parse_bit_depth,
        value_name = "16|24|32"
    )]
    pub bits: u16,
    /// Dither Options applied below 32-bit (No Dither, Rectangular, Triangular, POW-r 1, POW-r 2, POW-r 3)
    #[arg(
        long,
        value_enum,
        default_value_t = CliDitherMode::Triangular,
        value_name = "none|rectangular|triangular|pow-r-1|pow-r-2|pow-r-3"
    )]
    pub dither: CliDitherMode,
    /// Encode CBR 320 kbps MP3
    #[arg(long)]
    pub mp3: bool,
    /// Disable .report.json manifest output (enabled by default)
    #[arg(long)]
    pub no_report: bool,
}

#[derive(Args, Debug)]
pub struct StemsArgs {
    /// Path to .als or .json project
    pub path: PathBuf,
    /// Output directory for stem files
    #[arg(short, long, default_value = "./stems", value_name = "PATH")]
    pub output_dir: PathBuf,
    /// Render Length in musical bars
    #[arg(short, long, default_value_t = 8, value_name = "BARS")]
    pub duration_bars: u32,
    /// Target Sample Rate in Hz (e.g. 44100, 48000, 96000)
    #[arg(short, long, default_value_t = 44100, value_name = "HZ")]
    pub sample_rate: u32,
    /// PCM Bit Depth: 16, 24, or 32 (32-bit float is unclamped and undithered)
    #[arg(
        short,
        long,
        default_value_t = 24,
        value_parser = parse_bit_depth,
        value_name = "16|24|32"
    )]
    pub bits: u16,
    /// PCM File Type (wav, aiff, or flac)
    #[arg(
        short,
        long,
        value_enum,
        default_value_t = CliAudioFormat::Wav,
        value_name = "wav|aiff|flac"
    )]
    pub format: CliAudioFormat,
    /// Dither Options applied below 32-bit (No Dither, Rectangular, Triangular, POW-r 1, POW-r 2, POW-r 3)
    #[arg(
        long,
        value_enum,
        default_value_t = CliDitherMode::Triangular,
        value_name = "none|rectangular|triangular|pow-r-1|pow-r-2|pow-r-3"
    )]
    pub dither: CliDitherMode,
    /// Include Return and Main effects on individual track stems
    #[arg(long)]
    pub include_returns_main: bool,
    /// Convert to Mono instead of stereo
    #[arg(long)]
    pub mono: bool,
    /// Normalize peak amplitude to 0 dBFS
    #[arg(long)]
    pub normalize: bool,
    /// Render as Loop (two-pass render wrapping effect tails onto the start)
    #[arg(long)]
    pub render_as_loop: bool,
    /// Disable .report.json manifest output (enabled by default)
    #[arg(long)]
    pub no_report: bool,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum CliAudioFormat {
    Wav,
    #[value(alias = "aif")]
    Aiff,
    Flac,
}

impl From<CliAudioFormat> for AudioFileFormat {
    fn from(format: CliAudioFormat) -> Self {
        match format {
            CliAudioFormat::Wav => AudioFileFormat::Wav,
            CliAudioFormat::Aiff => AudioFileFormat::Aiff,
            CliAudioFormat::Flac => AudioFileFormat::Flac,
        }
    }
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum CliDitherMode {
    #[value(name = "none", alias = "no-dither")]
    None,
    #[value(name = "rectangular", alias = "rpdf")]
    Rectangular,
    #[value(name = "triangular", alias = "tpdf")]
    Triangular,
    #[value(name = "pow-r-1", alias = "powr1")]
    PowR1,
    #[value(name = "pow-r-2", alias = "powr2")]
    PowR2,
    #[value(name = "pow-r-3", alias = "powr3")]
    PowR3,
}

impl From<CliDitherMode> for DitherMode {
    fn from(mode: CliDitherMode) -> Self {
        match mode {
            CliDitherMode::None => DitherMode::NoDither,
            CliDitherMode::Rectangular => DitherMode::Rectangular,
            CliDitherMode::Triangular => DitherMode::Triangular,
            CliDitherMode::PowR1 => DitherMode::PowR1,
            CliDitherMode::PowR2 => DitherMode::PowR2,
            CliDitherMode::PowR3 => DitherMode::PowR3,
        }
    }
}

fn parse_bit_depth(s: &str) -> Result<u16, String> {
    match s.trim() {
        "16" => Ok(16),
        "24" => Ok(24),
        "32" => Ok(32),
        other => Err(format!(
            "invalid bit depth '{other}'; expected 16, 24, or 32"
        )),
    }
}

pub fn run_render(proj: &Project, args: RenderArgs) -> anyhow::Result<()> {
    let rendered_track = resolve_rendered_track(proj, &args.track, &args.selected_tracks)?;
    let options = RenderOptions {
        rendered_track,
        sample_rate: args.sample_rate,
        source_sample_rate: args.source_sample_rate,
        start_bars: args.start_bars,
        duration_bars: args.duration_bars,
        render_start_beats: args.start_beats,
        render_length_beats: args.length_beats,
        bits_per_sample: args.bits,
        dither: args.dither.into(),
        file_type: args.format.into(),
        encode_pcm: !args.no_pcm,
        encode_mp3: args.mp3,
        include_return_and_main_effects: args.include_returns_main,
        render_as_loop: args.render_as_loop,
        convert_to_mono: args.mono,
        normalize: args.normalize,
        create_analysis_file: args.analysis_file,
        write_report: !args.no_report,
        project_context: Some(ProjectContext::from_set_path(&args.path)),
    };
    execute_export(proj, &args.output, options)
}

pub fn run_stems(proj: &Project, args: StemsArgs) -> anyhow::Result<()> {
    let options = RenderOptions {
        rendered_track: RenderedTrack::AllIndividualTracks,
        sample_rate: args.sample_rate,
        duration_bars: args.duration_bars,
        bits_per_sample: args.bits,
        dither: args.dither.into(),
        file_type: args.format.into(),
        include_return_and_main_effects: args.include_returns_main,
        render_as_loop: args.render_as_loop,
        convert_to_mono: args.mono,
        normalize: args.normalize,
        write_report: !args.no_report,
        project_context: Some(ProjectContext::from_set_path(&args.path)),
        ..RenderOptions::default()
    };
    execute_export(proj, &args.output_dir, options)
}

fn execute_export(
    proj: &Project,
    target_path: &Path,
    options: RenderOptions,
) -> anyhow::Result<()> {
    let track_desc = describe_rendered_track(proj, &options.rendered_track);
    println!(
        "Exporting '{}' ({track_desc}) to '{}'...",
        proj.name,
        target_path.display()
    );
    print_render_config(&options);

    let start = std::time::Instant::now();
    let report = OfflineBouncer::export(proj, target_path, options)?;
    let elapsed = start.elapsed();

    print_render_summary(&report, target_path, elapsed);
    Ok(())
}

fn resolve_rendered_track(
    project: &Project,
    target: &str,
    selected_tracks: &[String],
) -> anyhow::Result<RenderedTrack> {
    let trimmed = target.trim();
    if trimmed.eq_ignore_ascii_case("selected") {
        if selected_tracks.is_empty() {
            anyhow::bail!("--selected-tracks is required when --track selected is used");
        }
        let mut ids = Vec::with_capacity(selected_tracks.len());
        for selector in selected_tracks {
            ids.push(resolve_single_track(project, selector)?);
        }
        return Ok(RenderedTrack::SelectedTracks(ids));
    }

    if !selected_tracks.is_empty() {
        anyhow::bail!("--selected-tracks can only be used with --track selected");
    }

    if trimmed.eq_ignore_ascii_case("main") {
        return Ok(RenderedTrack::Main);
    }
    if trimmed.eq_ignore_ascii_case("all") {
        return Ok(RenderedTrack::AllIndividualTracks);
    }

    let id = resolve_single_track(project, trimmed)?;
    Ok(RenderedTrack::Track(id))
}

pub(crate) fn resolve_single_track(project: &Project, selector: &str) -> anyhow::Result<TrackId> {
    let selector = selector.trim();
    if selector.is_empty() {
        anyhow::bail!("track selector must not be empty");
    }

    if let Ok(raw_id) = selector.parse::<u64>() {
        let id = TrackId(raw_id);
        if project.track(id).is_some() {
            return Ok(id);
        }
    }

    let exact_matches: Vec<TrackId> = project
        .all_tracks()
        .filter(|track| track.name == selector)
        .map(|track| track.id)
        .collect();
    if exact_matches.len() == 1 {
        return Ok(exact_matches[0]);
    }
    if exact_matches.len() > 1 {
        anyhow::bail!(
            "track name '{}' is ambiguous ({} matching tracks); specify numeric track ID",
            selector,
            exact_matches.len()
        );
    }

    let ci_matches: Vec<TrackId> = project
        .all_tracks()
        .filter(|track| track.name.eq_ignore_ascii_case(selector))
        .map(|track| track.id)
        .collect();
    if ci_matches.len() == 1 {
        return Ok(ci_matches[0]);
    }
    if ci_matches.len() > 1 {
        anyhow::bail!(
            "track name '{}' is ambiguous ({} matching tracks); specify numeric track ID",
            selector,
            ci_matches.len()
        );
    }

    if selector.eq_ignore_ascii_case("main") {
        return Ok(project.main_track.id);
    }

    let available: Vec<String> = project
        .all_tracks()
        .map(|t| format!("{}:{}", t.id, t.name))
        .collect();
    anyhow::bail!(
        "track '{}' not found in project (available: {})",
        selector,
        available.join(", ")
    );
}

fn describe_rendered_track(project: &Project, target: &RenderedTrack) -> String {
    match target {
        RenderedTrack::Main => "Main track".to_string(),
        RenderedTrack::AllIndividualTracks => "All Individual Tracks".to_string(),
        RenderedTrack::SelectedTracks(ids) => {
            let names: Vec<String> = ids
                .iter()
                .map(|id| {
                    project
                        .track(*id)
                        .map_or_else(|| format!("Track {id}"), |t| format!("{id}:{}", t.name))
                })
                .collect();
            format!("Selected Tracks [{}]", names.join(", "))
        }
        RenderedTrack::Track(id) => project.track(*id).map_or_else(
            || format!("Track {id}"),
            |t| format!("Track {id} ({})", t.name),
        ),
    }
}

fn format_label(format: AudioFileFormat) -> &'static str {
    match format {
        AudioFileFormat::Wav => "WAV",
        AudioFileFormat::Aiff => "AIFF",
        AudioFileFormat::Flac => "FLAC",
    }
}

fn print_render_config(options: &RenderOptions) {
    let range = match (options.render_start_beats, options.render_length_beats) {
        (Some(start), Some(len)) => format!("start {start} beats, length {len} beats"),
        (Some(start), None) => format!("start {start} beats, {} bars", options.duration_bars),
        (None, Some(len)) => format!("start bar {}, length {len} beats", options.start_bars),
        (None, None) => format!(
            "start bar {}, {} bars",
            options.start_bars, options.duration_bars
        ),
    };

    let rate = match options.source_sample_rate {
        Some(src) if src != options.sample_rate => {
            format!("{} Hz (from {} Hz)", options.sample_rate, src)
        }
        _ => format!("{} Hz", options.sample_rate),
    };

    let channels = if options.convert_to_mono {
        "mono"
    } else {
        "stereo"
    };

    let mut codecs = Vec::new();
    if options.encode_pcm {
        let fmt = format_label(options.file_type);
        if options.bits_per_sample == 32 {
            codecs.push(format!("32-bit float {fmt}"));
        } else {
            codecs.push(format!(
                "{}-bit {fmt} ({})",
                options.bits_per_sample,
                options.dither.label()
            ));
        }
    }
    if options.encode_mp3 {
        codecs.push("MP3 320 kbps".to_string());
    }

    let mut flags = Vec::new();
    if options.include_return_and_main_effects {
        flags.push("include returns/Main FX");
    }
    if options.render_as_loop {
        flags.push("render as loop");
    }
    if options.normalize {
        flags.push("normalize");
    }
    if options.create_analysis_file {
        flags.push("analysis file");
    }
    if options.write_report {
        flags.push("report JSON");
    }

    println!(
        "Configuration: {range} @ {rate}, {channels}, {}",
        codecs.join(" + ")
    );
    if !flags.is_empty() {
        println!("Options: {}", flags.join(", "));
    }
}

fn report_sidecar_path(output: &Path, target: &RenderedTrack) -> PathBuf {
    let is_multi_stem = matches!(
        target,
        RenderedTrack::AllIndividualTracks | RenderedTrack::SelectedTracks(_)
    );
    if is_multi_stem {
        output.join("render.report.json")
    } else {
        let mut os = output.as_os_str().to_os_string();
        os.push(".report.json");
        PathBuf::from(os)
    }
}

fn print_render_summary(report: &RenderReport, output_target: &Path, elapsed: std::time::Duration) {
    let count = report.outputs.len();
    let noun = if count == 1 { "file" } else { "files" };
    println!("Render complete in {elapsed:.2?} ({count} {noun} written):");
    for out in &report.outputs {
        let audio_seconds = if out.sample_rate > 0 {
            out.frames as f64 / f64::from(out.sample_rate)
        } else {
            0.0
        };
        let depth = if out.format == "mp3" {
            "320 kbps".to_string()
        } else if out.bit_depth == 32 {
            "32-bit float".to_string()
        } else {
            format!("{}-bit", out.bit_depth)
        };
        let channels = match out.channels {
            1 => "mono".to_string(),
            2 => "stereo".to_string(),
            n => format!("{n}ch"),
        };
        println!(
            "  - {} [{}, {}, {} Hz, {}, {:.3}s ({} frames), sha256: {}]",
            out.path,
            out.format,
            depth,
            out.sample_rate,
            channels,
            audio_seconds,
            out.frames,
            out.sha256
        );
        if report.settings.create_analysis_file {
            println!("    analysis: {}.lcanalysis", out.path);
        }
    }
    if report.settings.write_report {
        let report_path = report_sidecar_path(output_target, &report.settings.rendered_track);
        println!("  report: {}", report_path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_rendered_track_supports_keywords_ids_names_and_selected() {
        let project = Project::new_default("ResolveTest");

        assert_eq!(
            resolve_rendered_track(&project, "main", &[]).expect("main"),
            RenderedTrack::Main
        );
        assert_eq!(
            resolve_rendered_track(&project, "all", &[]).expect("all"),
            RenderedTrack::AllIndividualTracks
        );
        assert_eq!(
            resolve_rendered_track(&project, "1", &[]).expect("by id"),
            RenderedTrack::Track(TrackId(1))
        );
        assert_eq!(
            resolve_rendered_track(&project, "A-Reverb", &[]).expect("by name"),
            RenderedTrack::Track(TrackId(5))
        );
        assert_eq!(
            resolve_rendered_track(
                &project,
                "selected",
                &["1".to_string(), "A-Reverb".to_string()]
            )
            .expect("selected"),
            RenderedTrack::SelectedTracks(vec![TrackId(1), TrackId(5)])
        );
        assert!(resolve_rendered_track(&project, "selected", &[]).is_err());
        assert!(resolve_rendered_track(&project, "main", &["1".to_string()]).is_err());
        assert!(resolve_rendered_track(&project, "NonExistent", &[]).is_err());
    }
}
