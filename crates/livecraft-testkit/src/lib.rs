use livecraft_engine::bouncer::start_render;
use livecraft_model::model::Project;

#[derive(Debug, Clone, PartialEq)]
pub struct RenderSummary {
    pub frames: usize,
    pub peak_l: f32,
    pub peak_r: f32,
    pub rms_l: f32,
    pub rms_r: f32,
}

/// Renders `bars` bars of the Set through the engine, as the bouncer does, and measures the output.
pub fn render_project_summary(
    project: &Project,
    bars: u32,
    sample_rate: u32,
) -> anyhow::Result<RenderSummary> {
    let (_playback, mut engine) = start_render(project, sample_rate as f32)?;
    let beats_total = (bars * project.transport.time_signature().0 as u32) as f64;
    let total_seconds = (beats_total / project.transport.bpm() as f64) * 60.0;
    let total_samples = (total_seconds * sample_rate as f64) as usize;

    const BLOCK_FRAMES: usize = 512;
    let mut interleaved = vec![0.0f32; BLOCK_FRAMES * 2];
    let mut rendered_samples = 0;

    let mut peak_l = 0.0f32;
    let mut peak_r = 0.0f32;
    let mut sum_sq_l = 0.0f64;
    let mut sum_sq_r = 0.0f64;

    while rendered_samples < total_samples {
        let frames = BLOCK_FRAMES.min(total_samples - rendered_samples);
        let block = &mut interleaved[..frames * 2];
        engine.process(block, 2);

        for &[l, r] in block.as_chunks::<2>().0 {
            if l.abs() > peak_l {
                peak_l = l.abs();
            }
            if r.abs() > peak_r {
                peak_r = r.abs();
            }
            sum_sq_l += (l as f64) * (l as f64);
            sum_sq_r += (r as f64) * (r as f64);
        }

        rendered_samples += frames;
    }

    let rms_l = if total_samples > 0 {
        (sum_sq_l / total_samples as f64).sqrt() as f32
    } else {
        0.0
    };
    let rms_r = if total_samples > 0 {
        (sum_sq_r / total_samples as f64).sqrt() as f32
    } else {
        0.0
    };

    Ok(RenderSummary {
        frames: total_samples,
        peak_l,
        peak_r,
        rms_l,
        rms_r,
    })
}
