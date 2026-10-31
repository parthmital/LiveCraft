use crate::graph::AudioGraph;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use crossbeam_channel::{Receiver, Sender, bounded};
use livecraft_model::ids::{DeviceId, SceneId, TrackId};
use livecraft_model::model::Project;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub enum AudioParamMsg {
    SetTempo(f32),
    SetTrackVolume {
        track: TrackId,
        volume_db: f32,
    },
    SetTrackPan {
        track: TrackId,
        pan: f32,
    },
    SetTrackMute {
        track: TrackId,
        muted: bool,
    },
    SetTrackSolo {
        track: TrackId,
        soloed: bool,
    },
    SetTrackSend {
        track: TrackId,
        send_idx: usize,
        amount: f32,
    },
    SetMasterVolume {
        volume_db: f32,
    },
    SetMasterPan {
        pan: f32,
    },
    SetDeviceParam {
        track: TrackId,
        device: DeviceId,
        param_key: String,
        value: f32,
    },
    TriggerClip {
        track: TrackId,
        slot_idx: usize,
    },
    StopClip {
        track: TrackId,
    },
    TriggerScene {
        scene: SceneId,
    },
    StopAll,
    SetPlayhead(f64),
    SetPlaying(bool),
}

pub struct AudioEngine {
    _stream: Option<Stream>,
    pub graph: Arc<Mutex<AudioGraph>>,
    pub is_running: Arc<AtomicBool>,
    pub sample_rate: f32,
    param_tx: Sender<AudioParamMsg>,
}

impl AudioEngine {
    pub fn new(project: &Project) -> anyhow::Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow::anyhow!("No default audio output device found"))?;

        let supported_config = device.default_output_config()?;
        let sample_rate = supported_config.sample_rate() as f32;
        let channels = supported_config.channels() as usize;
        let sample_format = supported_config.sample_format();

        let graph = Arc::new(Mutex::new(AudioGraph::new(project, sample_rate)));
        let is_running = Arc::new(AtomicBool::new(false));

        let (param_tx, param_rx) = bounded::<AudioParamMsg>(1024);

        let config: StreamConfig = supported_config.into();

        let graph_cb = Arc::clone(&graph);
        let param_rx_cb = param_rx;

        let err_fn = |err| log::error!("Audio stream error: {err}");

        let stream = match sample_format {
            SampleFormat::F32 => device.build_output_stream(
                config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    write_audio(data, channels, &graph_cb, &param_rx_cb);
                },
                err_fn,
                None,
            )?,
            SampleFormat::I16 => device.build_output_stream(
                config,
                move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                    write_audio_i16(data, channels, &graph_cb, &param_rx_cb);
                },
                err_fn,
                None,
            )?,
            SampleFormat::U16 => device.build_output_stream(
                config,
                move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                    write_audio_u16(data, channels, &graph_cb, &param_rx_cb);
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
            param_tx,
        })
    }

    pub fn send_param(&self, msg: AudioParamMsg) {
        if let Err(e) = self.param_tx.try_send(msg) {
            log::warn!("Failed to send audio parameter update: {e}");
        }
    }

    pub fn play(&self) {
        self.send_param(AudioParamMsg::SetPlaying(true));
    }

    pub fn stop(&self) {
        self.send_param(AudioParamMsg::SetPlaying(false));
        self.send_param(AudioParamMsg::StopAll);
    }

    pub fn trigger_clip(&self, track: TrackId, slot_idx: usize) {
        self.send_param(AudioParamMsg::TriggerClip { track, slot_idx });
    }

    pub fn stop_clip(&self, track: TrackId) {
        self.send_param(AudioParamMsg::StopClip { track });
    }

    pub fn trigger_scene(&self, scene: SceneId) {
        self.send_param(AudioParamMsg::TriggerScene { scene });
    }

    pub fn set_tempo(&self, bpm: f32) {
        self.send_param(AudioParamMsg::SetTempo(bpm));
    }

    pub fn set_track_volume(&self, track: TrackId, volume_db: f32) {
        self.send_param(AudioParamMsg::SetTrackVolume { track, volume_db });
    }

    pub fn set_track_pan(&self, track: TrackId, pan: f32) {
        self.send_param(AudioParamMsg::SetTrackPan { track, pan });
    }

    pub fn set_track_mute(&self, track: TrackId, muted: bool) {
        self.send_param(AudioParamMsg::SetTrackMute { track, muted });
    }

    pub fn set_track_solo(&self, track: TrackId, soloed: bool) {
        self.send_param(AudioParamMsg::SetTrackSolo { track, soloed });
    }

    pub fn set_track_send(&self, track: TrackId, send_idx: usize, amount: f32) {
        self.send_param(AudioParamMsg::SetTrackSend {
            track,
            send_idx,
            amount,
        });
    }

    pub fn set_master_volume(&self, volume_db: f32) {
        self.send_param(AudioParamMsg::SetMasterVolume { volume_db });
    }

    pub fn set_master_pan(&self, pan: f32) {
        self.send_param(AudioParamMsg::SetMasterPan { pan });
    }

    pub fn set_device_param(
        &self,
        track: TrackId,
        device: DeviceId,
        param_key: impl Into<String>,
        value: f32,
    ) {
        self.send_param(AudioParamMsg::SetDeviceParam {
            track,
            device,
            param_key: param_key.into(),
            value,
        });
    }

    pub fn set_playhead(&self, beats: f64) {
        self.send_param(AudioParamMsg::SetPlayhead(beats));
    }

    pub fn sync_project(&self, project: &Project) {
        if let Ok(mut g) = self.graph.try_lock() {
            g.sync_project(project);
        }
    }
}

fn write_audio(
    data: &mut [f32],
    channels: usize,
    graph_arc: &Arc<Mutex<AudioGraph>>,
    param_rx: &Receiver<AudioParamMsg>,
) {
    match graph_arc.try_lock() {
        Ok(mut graph) => {
            while let Ok(msg) = param_rx.try_recv() {
                graph.apply_param_msg(msg);
            }
            graph.process_buffer_interleaved(data, channels);
        }
        _ => {
            data.fill(0.0);
        }
    }
}

fn write_audio_i16(
    data: &mut [i16],
    channels: usize,
    graph_arc: &Arc<Mutex<AudioGraph>>,
    param_rx: &Receiver<AudioParamMsg>,
) {
    match graph_arc.try_lock() {
        Ok(mut graph) => {
            while let Ok(msg) = param_rx.try_recv() {
                graph.apply_param_msg(msg);
            }
            graph.process_buffer_interleaved_i16(data, channels);
        }
        _ => {
            data.fill(0);
        }
    }
}

fn write_audio_u16(
    data: &mut [u16],
    channels: usize,
    graph_arc: &Arc<Mutex<AudioGraph>>,
    param_rx: &Receiver<AudioParamMsg>,
) {
    match graph_arc.try_lock() {
        Ok(mut graph) => {
            while let Ok(msg) = param_rx.try_recv() {
                graph.apply_param_msg(msg);
            }
            graph.process_buffer_interleaved_u16(data, channels);
        }
        _ => {
            data.fill(32768);
        }
    }
}
