use crate::graph::AudioGraph;
use hound::{WavSpec, WavWriter};
use livecraft_model::model::Project;
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
        let mut graph = AudioGraph::new(project, options.sample_rate as f32);
        graph.is_playing = true;

        // Auto-trigger slot 0 on all tracks that have clips
        for (trk_idx, trk) in project.tracks.iter().enumerate() {
            if let Some(slot) = trk.clip_slots.first()
                && slot.clip.is_some()
            {
                graph.trigger_clip(trk_idx, 0);
            }
        }

        // Calculate total samples from duration_bars and BPM
        let beats_total =
            (options.duration_bars * project.transport.time_signature.0 as u32) as f64;
        let total_seconds = (beats_total / project.transport.bpm as f64) * 60.0;
        let total_samples = (total_seconds * options.sample_rate as f64) as usize;

        let block_size = 512;
        let mut buf_l = vec![0.0f32; block_size];
        let mut buf_r = vec![0.0f32; block_size];

        let mut rendered_samples = 0;
        while rendered_samples < total_samples {
            let to_render = block_size.min(total_samples - rendered_samples);
            let active_l = &mut buf_l[..to_render];
            let active_r = &mut buf_r[..to_render];

            graph.process_buffer(active_l, active_r);

            for i in 0..to_render {
                let s_l = crate::graph::sanitize_output_sample(active_l[i]).clamp(-1.0, 1.0);
                let s_r = crate::graph::sanitize_output_sample(active_r[i]).clamp(-1.0, 1.0);

                if options.bits_per_sample == 16 {
                    writer.write_sample((s_l * 32767.0) as i16)?;
                    writer.write_sample((s_r * 32767.0) as i16)?;
                } else if options.bits_per_sample == 24 {
                    writer.write_sample((s_l * 8388607.0) as i32)?;
                    writer.write_sample((s_r * 8388607.0) as i32)?;
                } else {
                    writer.write_sample(s_l)?;
                    writer.write_sample(s_r)?;
                }
            }

            rendered_samples += to_render;
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
            for (i, t) in solo_proj.tracks.iter_mut().enumerate() {
                t.is_soloed = i == trk_idx;
            }

            Self::render_project(&solo_proj, &stem_file, options.clone())?;
            stem_paths.push(stem_file.to_string_lossy().to_string());
        }

        Ok(stem_paths)
    }
}
