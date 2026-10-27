use livecraft_core::model::{Clip, ClipPayload, MidiNote, Project};
use livecraft_dsp::graph::AudioGraph;

fn compute_render_metrics(
    project: &Project,
    bars: u32,
    sample_rate: u32,
) -> (usize, f32, f32, f32, f32) {
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

    (total_samples, peak_l, peak_r, rms_l, rms_r)
}

#[test]
fn test_characterisation_render_metrics() {
    let project = Project::new_default("demo");
    let (frames, peak_l, peak_r, rms_l, rms_r) = compute_render_metrics(&project, 4, 44100);
    assert_eq!(frames, 352800);
    assert!((peak_l - 0.0).abs() < 1e-6);
    assert!((peak_r - 0.0).abs() < 1e-6);
    assert!((rms_l - 0.0).abs() < 1e-6);
    assert!((rms_r - 0.0).abs() < 1e-6);

    let mut chords_project = Project::new_default("ChordsDemo");
    let mut clip = Clip::new_midi(1, "Lead Chords", 4.0);
    if let ClipPayload::Midi(ref mut mdata) = clip.payload {
        mdata.notes.push(MidiNote::new(60, 0.0, 2.0, 100));
        mdata.notes.push(MidiNote::new(64, 0.0, 2.0, 100));
        mdata.notes.push(MidiNote::new(67, 0.0, 2.0, 100));
    }
    chords_project.tracks[0].clip_slots[0].clip = Some(clip);
    let (frames2, peak2_l, peak2_r, rms2_l, rms2_r) =
        compute_render_metrics(&chords_project, 4, 44100);
    assert_eq!(frames2, 352800);
    assert!((peak2_l - 0.26114187).abs() < 1e-6);
    assert!((peak2_r - 0.26114187).abs() < 1e-6);
    assert!((rms2_l - 0.06759972).abs() < 1e-6);
    assert!((rms2_r - 0.06759972).abs() < 1e-6);
}

#[test]
fn test_characterisation_inspect_json() {
    let project = Project::new_default("demo");
    let val: serde_json::Value = serde_json::to_value(&project).expect("serialize to value");
    let json = serde_json::to_string_pretty(&val).expect("pretty print sorted json");
    insta::assert_snapshot!("demo_project_inspect", json);
}
