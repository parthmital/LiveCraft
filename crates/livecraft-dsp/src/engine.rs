use crate::graph::AudioGraph;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use livecraft_core::model::Project;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};

pub struct AudioEngine {
    _stream: Option<Stream>,
    pub graph: Arc<Mutex<AudioGraph>>,
    pub project: Arc<RwLock<Project>>,
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
        let project_arc = Arc::new(RwLock::new(project.clone()));
        let is_running = Arc::new(AtomicBool::new(false));

        let config: StreamConfig = supported_config.into();

        let graph_cb = Arc::clone(&graph);
        let project_cb = Arc::clone(&project_arc);

        let err_fn = |err| eprintln!("Audio stream error: {}", err);

        let stream = match sample_format {
            SampleFormat::F32 => device.build_output_stream(
                &config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    write_audio(data, channels, &graph_cb, &project_cb);
                },
                err_fn,
                None,
            )?,
            SampleFormat::I16 => device.build_output_stream(
                &config,
                move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                    write_audio_i16(data, channels, &graph_cb, &project_cb);
                },
                err_fn,
                None,
            )?,
            SampleFormat::U16 => device.build_output_stream(
                &config,
                move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                    write_audio_u16(data, channels, &graph_cb, &project_cb);
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
            project: project_arc,
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

    pub fn sync_project(&self, project: &Project) {
        if let Ok(mut p) = self.project.try_write() {
            *p = project.clone();
        }
        if let Ok(mut g) = self.graph.try_lock() {
            g.sync_project(project);
        }
    }
}

fn write_audio(
    data: &mut [f32],
    channels: usize,
    graph_arc: &Arc<Mutex<AudioGraph>>,
    project_arc: &Arc<RwLock<Project>>,
) {
    if let Ok(mut graph) = graph_arc.try_lock() {
        if let Ok(project) = project_arc.try_read() {
            graph.process_buffer_interleaved(data, channels, &project);
        } else {
            data.fill(0.0);
        }
    } else {
        data.fill(0.0);
    }
}

fn write_audio_i16(
    data: &mut [i16],
    channels: usize,
    graph_arc: &Arc<Mutex<AudioGraph>>,
    project_arc: &Arc<RwLock<Project>>,
) {
    if let Ok(mut graph) = graph_arc.try_lock() {
        if let Ok(project) = project_arc.try_read() {
            graph.process_buffer_interleaved_i16(data, channels, &project);
        } else {
            data.fill(0);
        }
    } else {
        data.fill(0);
    }
}

fn write_audio_u16(
    data: &mut [u16],
    channels: usize,
    graph_arc: &Arc<Mutex<AudioGraph>>,
    project_arc: &Arc<RwLock<Project>>,
) {
    if let Ok(mut graph) = graph_arc.try_lock() {
        if let Ok(project) = project_arc.try_read() {
            graph.process_buffer_interleaved_u16(data, channels, &project);
        } else {
            data.fill(32768);
        }
    } else {
        data.fill(32768);
    }
}
