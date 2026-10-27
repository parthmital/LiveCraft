use livecraft_core::model::Project;
use livecraft_dsp::graph::AudioGraph;

#[derive(Debug, Clone, PartialEq)]
pub struct RenderSummary {
    pub frames: usize,
    pub peak_l: f32,
    pub peak_r: f32,
    pub rms_l: f32,
    pub rms_r: f32,
}

pub fn render_project_summary(project: &Project, bars: u32, sample_rate: u32) -> RenderSummary {
    let mut graph = AudioGraph::new(project, sample_rate as f32);
    graph.is_playing = true;
    for (trk_idx, trk) in project.tracks.iter().enumerate() {
        if let Some(slot) = trk.clip_slots.first()
            && slot.clip.is_some()
        {
            graph.trigger_clip(trk_idx, 0);
        }
    }
    let beats_total = (bars * project.transport.time_signature.0 as u32) as f64;
    let total_seconds = (beats_total / project.transport.bpm as f64) * 60.0;
    let total_samples = (total_seconds * sample_rate as f64) as usize;

    let block_size = 512;
    let mut buf_l = vec![0.0f32; block_size];
    let mut buf_r = vec![0.0f32; block_size];
    let mut rendered_samples = 0;

    let mut peak_l = 0.0f32;
    let mut peak_r = 0.0f32;
    let mut sum_sq_l = 0.0f64;
    let mut sum_sq_r = 0.0f64;

    while rendered_samples < total_samples {
        let to_render = block_size.min(total_samples - rendered_samples);
        let active_l = &mut buf_l[..to_render];
        let active_r = &mut buf_r[..to_render];

        graph.process_buffer(active_l, active_r);

        for i in 0..to_render {
            let s_l = active_l[i].abs();
            let s_r = active_r[i].abs();
            if s_l > peak_l {
                peak_l = s_l;
            }
            if s_r > peak_r {
                peak_r = s_r;
            }
            sum_sq_l += (active_l[i] as f64) * (active_l[i] as f64);
            sum_sq_r += (active_r[i] as f64) * (active_r[i] as f64);
        }

        rendered_samples += to_render;
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

    RenderSummary {
        frames: total_samples,
        peak_l,
        peak_r,
        rms_l,
        rms_r,
    }
}
