use crate::executor::{Engine, EngineCommand};
use crate::output::Playback;
use hound::{WavSpec, WavWriter};
use livecraft_model::model::Project;
use livecraft_time::seconds_for_beats;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct RenderOptions {
    pub sample_rate: u32,
    pub duration_bars: u32,
    pub bits_per_sample: u16,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            sample_rate: 44100,
            duration_bars: 8,
            bits_per_sample: 24,
        }
    }
}

/// Frames the bouncer asks the engine for at a time.
const BLOCK_FRAMES: usize = 512;

/// Starts an engine on the Set for a render, with the first clip of each track that has one launched
/// and the transport playing from the start. The caller drives the returned `Engine` with `process`.
pub fn start_render(project: &Project, sample_rate: f32) -> anyhow::Result<(Playback, Engine)> {
    let (mut playback, engine) = Playback::start(project, sample_rate)?;
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
    /// Render the entire project mixdown to a WAV file
    pub fn render_project<P: AsRef<Path>>(
        project: &Project,
        output_path: P,
        options: RenderOptions,
    ) -> anyhow::Result<()> {
        let spec = WavSpec {
            channels: 2,
            sample_rate: options.sample_rate,
            bits_per_sample: options.bits_per_sample,
            sample_format: if options.bits_per_sample == 32 {
                hound::SampleFormat::Float
            } else {
                hound::SampleFormat::Int
            },
        };

        let mut writer = WavWriter::create(output_path, spec)?;
        let (_playback, mut engine) = start_render(project, options.sample_rate as f32)?;

        // Calculate total samples from duration_bars and BPM
        let beats_total =
            (options.duration_bars * project.transport.time_signature().0 as u32) as f64;
        let total_seconds = seconds_for_beats(beats_total, project.transport.bpm() as f64);
        let total_samples = (total_seconds * options.sample_rate as f64) as usize;

        let mut interleaved = vec![0.0f32; BLOCK_FRAMES * 2];
        let mut rendered_samples = 0;
        while rendered_samples < total_samples {
            let frames = BLOCK_FRAMES.min(total_samples - rendered_samples);
            let block = &mut interleaved[..frames * 2];
            engine.process(block, 2);

            for sample in block.iter() {
                let s = sample.clamp(-1.0, 1.0);
                if options.bits_per_sample == 16 {
                    writer.write_sample((s * 32767.0) as i16)?;
                } else if options.bits_per_sample == 24 {
                    writer.write_sample((s * 8388607.0) as i32)?;
                } else {
                    writer.write_sample(s)?;
                }
            }

            rendered_samples += frames;
        }

        writer.finalize()?;
        Ok(())
    }

    /// Render individual track stems to an output folder
    pub fn render_stems<P: AsRef<Path>>(
        project: &Project,
        output_dir: P,
        options: RenderOptions,
    ) -> anyhow::Result<Vec<String>> {
        fs::create_dir_all(output_dir.as_ref())?;
        let mut stem_paths = Vec::new();

        for (trk_idx, trk) in project.tracks.iter().enumerate() {
            let sanitized_name = trk
                .name
                .replace([' ', '/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
            let file_name = format!("{:02}_{}.wav", trk_idx + 1, sanitized_name);
            let stem_file = output_dir.as_ref().join(&file_name);

            // Clone project with only this track unmuted/soloed
            let mut solo_proj = project.clone();
            for t in solo_proj.tracks.iter_mut() {
                t.is_soloed = t.id == trk.id;
            }

            Self::render_project(&solo_proj, &stem_file, options.clone())?;
            stem_paths.push(stem_file.to_string_lossy().to_string());
        }

        Ok(stem_paths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use livecraft_model::model::{Clip, ClipPayload, MidiNote};

    #[test]
    fn a_bounce_writes_a_stereo_wav_at_the_requested_rate() {
        let mut project = Project::new_default("BounceTest");
        let mut clip = Clip::new_midi(1, "Lead Chords", 4.0);
        if let ClipPayload::Midi(ref mut mdata) = clip.payload {
            mdata.notes.push(MidiNote::new(60, 0.0, 2.0, 100));
            mdata.notes.push(MidiNote::new(64, 0.0, 2.0, 100));
            mdata.notes.push(MidiNote::new(67, 0.0, 2.0, 100));
        }
        project.tracks[0].clip_slots[0].clip = Some(clip);

        let out_dir = std::env::temp_dir().join("livecraft_test");
        fs::create_dir_all(&out_dir).expect("create the test folder");
        let wav_path = out_dir.join("test_bounce.wav");

        let options = RenderOptions {
            sample_rate: 44100,
            duration_bars: 2,
            bits_per_sample: 16,
        };

        OfflineBouncer::render_project(&project, &wav_path, options)
            .expect("Failed to render project");
        assert!(wav_path.exists());

        let reader = hound::WavReader::open(&wav_path).expect("Failed to read rendered WAV");
        assert_eq!(reader.spec().channels, 2);
        assert_eq!(reader.spec().sample_rate, 44100);
        assert!(reader.len() > 0);

        let _ = fs::remove_file(wav_path);
    }

    #[test]
    fn a_stem_export_writes_one_wav_per_track() {
        let project = Project::new_default("StemTest");
        let out_dir = std::env::temp_dir().join("livecraft_stem_test");

        let options = RenderOptions {
            sample_rate: 44100,
            duration_bars: 1,
            bits_per_sample: 16,
        };

        let stems = OfflineBouncer::render_stems(&project, &out_dir, options)
            .expect("Failed to render stems");
        assert_eq!(stems.len(), project.tracks.len());

        for stem in &stems {
            assert!(Path::new(stem).exists());
        }

        let _ = fs::remove_dir_all(out_dir);
    }
}
