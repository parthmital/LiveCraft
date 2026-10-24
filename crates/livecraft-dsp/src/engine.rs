use crate::graph::AudioGraph;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use livecraft_core::model::Project;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub struct AudioEngine {
    _stream: Option<Stream>,
    pub graph: Arc<Mutex<AudioGraph>>,
    pub is_running: Arc<AtomicBool>,
    pub sample_rate: f32,
}

impl AudioEngine {
    pub fn new(project: &Project) -> anyhow::Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow::anyhow!("No default audio output device found"))?;

        let supported_config = device.default_output_config()?;
        let sample_rate = supported_config.sample_rate().0 as f32;
        let channels = supported_config.channels() as usize;
        let sample_format = supported_config.sample_format();

        let graph = Arc::new(Mutex::new(AudioGraph::new(project, sample_rate)));
        let is_running = Arc::new(AtomicBool::new(false));

        let config: StreamConfig = supported_config.into();

        let graph_cb = Arc::clone(&graph);
        let project_clone = project.clone();

        let err_fn = |err| eprintln!("Audio stream error: {}", err);

        let stream = match sample_format {
            SampleFormat::F32 => device.build_output_stream(
                &config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    write_audio(data, channels, &graph_cb, &project_clone);
                },
                err_fn,
                None,
            )?,
            SampleFormat::I16 => device.build_output_stream(
                &config,
                move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                    let mut float_buf = vec![0.0f32; data.len()];
                    write_audio(&mut float_buf, channels, &graph_cb, &project_clone);
                    for (i, s) in float_buf.iter().enumerate() {
                        data[i] = (s * 32767.0).clamp(-32768.0, 32767.0) as i16;
                    }
                },
                err_fn,
                None,
            )?,
            SampleFormat::U16 => device.build_output_stream(
                &config,
                move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                    let mut float_buf = vec![0.0f32; data.len()];
                    write_audio(&mut float_buf, channels, &graph_cb, &project_clone);
                    for (i, s) in float_buf.iter().enumerate() {
                        data[i] = ((s * 0.5 + 0.5) * 65535.0).clamp(0.0, 65535.0) as u16;
                    }
                },
                err_fn,
                None,
            )?,
            _ => anyhow::bail!("Unsupported CPAL sample format"),
        };

        stream.play()?;
        is_running.store(true, Ordering::SeqCst);

        Ok(Self {
            _stream: Some(stream),
            graph,
            is_running,
            sample_rate,
        })
    }

    pub fn play(&self) {
        if let Ok(mut g) = self.graph.lock() {
            g.is_playing = true;
        }
    }

    pub fn stop(&self) {
        if let Ok(mut g) = self.graph.lock() {
            g.stop_all();
        }
    }

    pub fn trigger_clip(&self, track_idx: usize, slot_idx: usize) {
        if let Ok(mut g) = self.graph.lock() {
            g.trigger_clip(track_idx, slot_idx);
        }
    }
}

fn write_audio(
    data: &mut [f32],
    channels: usize,
    graph_arc: &Arc<Mutex<AudioGraph>>,
    project: &Project,
) {
    if let Ok(mut graph) = graph_arc.try_lock() {
        let frames = data.len() / channels;
        let mut buf_l = vec![0.0f32; frames];
        let mut buf_r = vec![0.0f32; frames];

        graph.process_buffer(&mut buf_l, &mut buf_r, project);

        for (frame_idx, chunk) in data.chunks_mut(channels).enumerate() {
            let l = buf_l[frame_idx];
            let r = buf_r[frame_idx];
            if channels >= 2 {
                chunk[0] = l;
                chunk[1] = r;
            } else if channels == 1 {
                chunk[0] = (l + r) * 0.5;
            }
        }
    } else {
        // If mutex is locked, output silence to prevent audio glitches
        data.fill(0.0);
    }
}
