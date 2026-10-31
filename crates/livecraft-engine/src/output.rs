//! The audio output (M05 item 12, TARGET-ARCHITECTURE "Threads" and "Engine Decisions").
//!
//! The cpal stream calls `Engine::process` for each driver buffer and does nothing else. `Playback`
//! is the session side: it compiles the Set, submits the graphs, sends commands, and reads the
//! snapshots. It owns the stream, so dropping it stops the output.

use crate::compiler::{GraphCompiler, ProcessorFactory};
use crate::executor::{Engine, EngineCommand, EngineHandle};
use anyhow::anyhow;
use basedrop::Collector;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, Stream, StreamConfig};
use livecraft_devices::create_device_processor;
use livecraft_model::model::Project;

/// Frames converted per step when the device does not take `f32` samples. A callback buffer longer
/// than this is converted in steps, so the scratch buffer never grows on the audio thread.
const SCRATCH_FRAMES: usize = 1024;

/// The session side of a running engine, and the output stream when there is a device.
pub struct Playback {
    /// Keeps the output running. None when the Set renders without an audio device.
    stream: Option<Stream>,
    pub sample_rate: f32,
    pub compiler: GraphCompiler,
    pub handle: EngineHandle,
}

impl Playback {
    /// Opens the default output device and starts rendering `project` through it.
    pub fn open(project: &Project) -> anyhow::Result<Self> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or_else(|| anyhow!("No default audio output device found"))?;
        let supported = device.default_output_config()?;
        let sample_rate = supported.sample_rate() as f32;
        let channels = supported.channels().max(1) as usize;
        let format = supported.sample_format();
        let config: StreamConfig = supported.into();
        let (mut playback, engine) = Self::start(project, sample_rate)?;
        let stream = match format {
            SampleFormat::F32 => output_stream::<f32>(&device, config, engine, channels)?,
            SampleFormat::I16 => output_stream::<i16>(&device, config, engine, channels)?,
            SampleFormat::U16 => output_stream::<u16>(&device, config, engine, channels)?,
            other => anyhow::bail!("Unsupported CPAL sample format: {other:?}"),
        };
        stream.play()?;
        playback.stream = Some(stream);
        Ok(playback)
    }

    /// Starts an engine rendering `project` at `sample_rate`, without an audio device. The caller
    /// drives the returned `Engine` with `Engine::process`, as the offline bouncer does.
    pub fn start(project: &Project, sample_rate: f32) -> anyhow::Result<(Self, Engine)> {
        Self::start_with(project, sample_rate, create_device_processor)
    }

    /// As `start`, with each device's processor built by `factory`.
    pub fn start_with(
        project: &Project,
        sample_rate: f32,
        factory: ProcessorFactory,
    ) -> anyhow::Result<(Self, Engine)> {
        let collector = Collector::new();
        let mut compiler = GraphCompiler::with_factory(sample_rate, collector.handle(), factory);
        let graph = compiler.compile(project);
        let (engine, handle) = Engine::start(graph, sample_rate, collector)?;
        let mut playback = Self {
            stream: None,
            sample_rate,
            compiler,
            handle,
        };
        playback
            .handle
            .send(EngineCommand::SetTempo(project.transport.bpm()))?;
        Ok((playback, engine))
    }
}

/// Builds the output stream for samples of type `T`. The callback converts each engine block from
/// `f32` in place of the device buffer, using a scratch buffer allocated here.
fn output_stream<T>(
    device: &cpal::Device,
    config: StreamConfig,
    mut engine: Engine,
    channels: usize,
) -> Result<Stream, cpal::Error>
where
    T: SizedSample + FromSample<f32>,
{
    let mut scratch = vec![0.0f32; SCRATCH_FRAMES * channels];
    device.build_output_stream(
        config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            for chunk in data.chunks_mut(scratch.len()) {
                let rendered = &mut scratch[..chunk.len()];
                engine.process(rendered, channels);
                for (out, sample) in chunk.iter_mut().zip(rendered.iter()) {
                    *out = T::from_sample(*sample);
                }
            }
        },
        |err| log::error!("Audio stream error: {err}"),
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use livecraft_model::model::Project;

    #[test]
    fn the_tempo_of_the_set_reaches_the_engine() {
        let mut project = Project::new_default("Tempo");
        project.transport.set_bpm(140.0);
        let (mut playback, mut engine) = Playback::start(&project, 44100.0).expect("start");
        let mut out = vec![0.0f32; 64];
        engine.process(&mut out, 2);
        assert_eq!(playback.handle.snapshot().transport.bpm, 140.0);
    }
}
