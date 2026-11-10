//! The audio output (M05 item 12, TARGET-ARCHITECTURE "Threads" and "Engine Decisions"; M06 items 2
//! and 6 and 7).
//!
//! The driver calls `Engine::process` for each buffer and does nothing else. `Playback` is the
//! session side: it compiles the Set, submits the graphs, sends commands, and reads the snapshots.
//! It holds the driver and the MIDI ports, so dropping it stops the output and closes the ports.

use crate::compiler::{GraphCompiler, ProcessorFactory};
use crate::driver::{AudioDriver, CpalDriver, DriverConfig, DriverStatus, Latency};
use crate::executor::{Engine, EngineHandle};
use crate::midi_io::{MidiInputs, MidiOutputs, MidiPorts};
use crate::streamer::DiskStreamer;
use basedrop::Collector;
use livecraft_devices::create_device_processor;
use livecraft_formats::project_dir::ProjectContext;
use livecraft_model::model::Project;
use std::path::PathBuf;
use std::sync::Arc;

/// The session side of a running engine, and the driver that plays it when there is one.
pub struct Playback {
    /// Keeps the output running. None when the Set renders without an audio driver.
    driver: Option<Box<dyn AudioDriver>>,
    /// Keeps the MIDI input ports open, so they feed the engine. None when there is no driver.
    _midi_inputs: Option<MidiInputs>,
    /// Keeps the MIDI output thread and its ports running. None when there is no driver.
    _midi_outputs: Option<MidiOutputs>,
    pub sample_rate: f32,
    pub compiler: GraphCompiler,
    pub handle: EngineHandle,
}

impl Playback {
    /// Opens the output that `config` names and starts rendering `project` through it, with no MIDI
    /// ports.
    pub fn open(project: &Project, config: &DriverConfig) -> anyhow::Result<Self> {
        Self::open_with_driver(
            project,
            Box::new(CpalDriver::open(config)?),
            &MidiPorts::default(),
        )
    }

    /// Starts rendering `project` through `driver`, at the driver's sample rate. The MIDI input ports
    /// named in `midi.inputs` feed the engine, and the output ports named in `midi.outputs` receive the
    /// notes of the tracks routed to them. A port that cannot be opened is skipped with a warning.
    pub fn open_with_driver(
        project: &Project,
        driver: Box<dyn AudioDriver>,
        midi: &MidiPorts,
    ) -> anyhow::Result<Self> {
        Self::open_with_driver_in_context(project, driver, midi, ProjectContext::default())
    }

    /// As [`Self::open_with_driver`], using `context` to resolve relative audio clip references.
    pub fn open_with_driver_in_context(
        project: &Project,
        mut driver: Box<dyn AudioDriver>,
        midi: &MidiPorts,
        context: ProjectContext,
    ) -> anyhow::Result<Self> {
        let sample_rate = driver.sample_rate() as f32;
        let (mut playback, mut engine) = Self::start_in_context(project, sample_rate, context)?;
        let (midi_inputs, feed) = MidiInputs::open(&midi.inputs);
        let clock = feed.clock;
        engine.set_midi_input(feed.messages, clock);
        let (midi_outputs, out) =
            MidiOutputs::open(&midi.outputs, clock, sample_rate, driver.output_delay())?;
        engine.set_midi_output(out.events, out.anchor);
        driver.start(engine)?;
        playback.driver = Some(driver);
        playback._midi_inputs = Some(midi_inputs);
        playback._midi_outputs = Some(midi_outputs);
        Ok(playback)
    }

    /// Whether the driver runs. It is `Stopped` when the Set renders without a driver.
    pub fn driver_status(&self) -> DriverStatus {
        self.driver
            .as_ref()
            .map_or(DriverStatus::Stopped, |driver| driver.status())
    }

    /// The driver's buffer size in frames, or None when the Set renders without a driver.
    pub fn buffer_frames(&self) -> Option<u32> {
        self.driver.as_ref().map(|driver| driver.buffer_frames())
    }

    /// The driver's output channels, or None when the Set renders without a driver.
    pub fn channels(&self) -> Option<usize> {
        self.driver.as_ref().map(|driver| driver.channels())
    }

    /// The input and output latency of the driver. It is zero when the Set renders without a driver.
    pub fn latency(&self) -> Latency {
        self.driver
            .as_ref()
            .map_or_else(Latency::default, |driver| driver.latency())
    }

    /// Starts an engine rendering `project` at `sample_rate`, without an audio device. The caller
    /// drives the returned `Engine` with `Engine::process`, as the offline bouncer does.
    pub fn start(project: &Project, sample_rate: f32) -> anyhow::Result<(Self, Engine)> {
        Self::start_with_context(
            project,
            sample_rate,
            create_device_processor,
            ProjectContext::default(),
        )
    }

    /// As [`Self::start`], using `context` to resolve relative audio clip references.
    pub fn start_in_context(
        project: &Project,
        sample_rate: f32,
        context: ProjectContext,
    ) -> anyhow::Result<(Self, Engine)> {
        Self::start_with_context(project, sample_rate, create_device_processor, context)
    }

    /// Starts an offline-render engine with a deterministic [`Manual`](crate::streamer::StreamerMode::Manual)
    /// [`DiskStreamer`], using `context` to resolve relative audio clip references.
    pub fn start_render_in_context(
        project: &Project,
        sample_rate: f32,
        context: ProjectContext,
    ) -> anyhow::Result<(Self, Engine)> {
        Self::start_with_mode(project, sample_rate, create_device_processor, context, true)
    }

    /// As `start`, with each device's processor built by `factory`.
    pub fn start_with(
        project: &Project,
        sample_rate: f32,
        factory: ProcessorFactory,
    ) -> anyhow::Result<(Self, Engine)> {
        Self::start_with_context(project, sample_rate, factory, ProjectContext::default())
    }

    /// Updates the [`ProjectContext`] used by the compiler for subsequent graph compiles.
    pub fn set_project_context(&mut self, context: ProjectContext) {
        self.compiler.set_project_context(context);
    }

    /// Enqueues a RAM-mode audio clip reference for background decoding and resampling.
    pub fn enqueue_ram_decode(
        &mut self,
        file_path: &str,
        relative_path: Option<&str>,
    ) -> Option<PathBuf> {
        self.compiler.enqueue_ram_decode(file_path, relative_path)
    }

    /// Polls background RAM decodes into the compiler's cache, returning how many completed.
    pub fn poll_decoded(&mut self) -> usize {
        self.compiler.poll_decoded()
    }

    /// Returns the compiler's [`DiskStreamer`].
    #[must_use]
    pub fn streamer(&self) -> &Arc<DiskStreamer> {
        self.compiler.streamer()
    }

    fn start_with_context(
        project: &Project,
        sample_rate: f32,
        factory: ProcessorFactory,
        context: ProjectContext,
    ) -> anyhow::Result<(Self, Engine)> {
        Self::start_with_mode(project, sample_rate, factory, context, false)
    }

    fn start_with_mode(
        project: &Project,
        sample_rate: f32,
        factory: ProcessorFactory,
        context: ProjectContext,
        manual_streamer: bool,
    ) -> anyhow::Result<(Self, Engine)> {
        let collector = Collector::new();
        let mut compiler = GraphCompiler::with_factory(sample_rate, collector.handle(), factory)
            .with_project_context(context);
        if manual_streamer {
            compiler = compiler.with_manual_streamer();
        }
        let graph = compiler.compile(project);
        let (engine, handle) = Engine::start(graph, sample_rate, collector)?;
        let playback = Self {
            driver: None,
            _midi_inputs: None,
            _midi_outputs: None,
            sample_rate,
            compiler,
            handle,
        };
        Ok((playback, engine))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::NullDriver;
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

    #[test]
    fn a_playback_reports_the_status_of_the_driver_it_holds() {
        let project = Project::new_default("Driver");
        let (playback, _engine) = Playback::start(&project, 44100.0).expect("start");
        assert_eq!(playback.driver_status(), DriverStatus::Stopped);
        assert_eq!(playback.latency(), Latency::default());

        let playback = Playback::open_with_driver(
            &project,
            Box::new(NullDriver::new(44100, 512, 2)),
            &MidiPorts::default(),
        )
        .expect("open with a null driver");
        assert_eq!(playback.driver_status(), DriverStatus::Running);
        assert_eq!(playback.sample_rate, 44100.0);
    }

    #[test]
    fn missing_midi_ports_do_not_stop_the_playback() {
        let project = Project::new_default("Missing port");
        let ports = MidiPorts {
            inputs: vec!["No Such MIDI Input Port".to_string()],
            outputs: vec!["No Such MIDI Output Port".to_string()],
        };
        let playback =
            Playback::open_with_driver(&project, Box::new(NullDriver::new(44100, 512, 2)), &ports)
                .expect("open with a null driver and missing MIDI ports");
        assert_eq!(playback.driver_status(), DriverStatus::Running);
    }

    #[test]
    fn start_in_context_and_set_project_context_resolve_relative_clips() {
        use crate::executor::EngineCommand;
        use livecraft_model::ids::TrackId;
        use livecraft_model::model::{Clip, ClipPayload};

        let dir = std::env::temp_dir().join(format!("livecraft_output_ctx_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let root = dir.join("LiveSet Project");
        livecraft_formats::project_dir::create_project_layout(&root).expect("layout");
        let wav_path = root.join("Samples/Imported/tone.wav");
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 44_100,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(&wav_path, spec).expect("create wav");
        for _ in 0..4096 {
            writer.write_sample(0.5f32).expect("write L");
            writer.write_sample(0.5f32).expect("write R");
        }
        writer.finalize().expect("finalize wav");

        let mut project = Project::new_default("OutputCtx");
        project.tracks[2].device_chain.clear();
        project.main_track.device_chain.clear();
        let mut clip = Clip::new_audio(
            901,
            "Tone",
            "C:/definitely_nonexistent_dir_12345/Samples/Imported/tone.wav",
            4.0,
        );
        if let ClipPayload::Audio(ref mut audio) = clip.payload {
            audio.relative_path = Some("Samples/Imported/tone.wav".to_string());
        }
        project.tracks[2].clip_slots[0].clip = Some(clip);

        let ctx = ProjectContext::for_project(&root);
        let (mut playback, mut engine) =
            Playback::start_in_context(&project, 44100.0, ctx.clone()).expect("start in context");
        assert_eq!(playback.compiler.project_context(), &ctx);

        playback
            .handle
            .send(EngineCommand::TriggerClip {
                track: TrackId(3),
                slot: 0,
            })
            .expect("trigger clip");
        playback
            .handle
            .send(EngineCommand::SetPlaying(true))
            .expect("play");
        let mut out = vec![0.0f32; 512];
        engine.process(&mut out, 2);
        assert!(out.iter().any(|&s| s.abs() > 0.1));

        // Resetting ProjectContext to empty and recompiling makes the broken clip silent.
        playback.set_project_context(ProjectContext::default());
        assert_eq!(
            playback.compiler.project_context(),
            &ProjectContext::default()
        );
        let silent_graph = playback.compiler.compile(&project);
        playback
            .handle
            .submit_graph(silent_graph)
            .expect("submit graph");
        out.fill(0.0);
        engine.process(&mut out, 2);
        assert!(out.iter().all(|&s| s == 0.0));

        // open_with_driver_in_context wires context cleanly through driver startup.
        let driver_playback = Playback::open_with_driver_in_context(
            &project,
            Box::new(NullDriver::new(44100, 512, 2)),
            &MidiPorts::default(),
            ctx.clone(),
        )
        .expect("open_with_driver_in_context");
        assert_eq!(driver_playback.driver_status(), DriverStatus::Running);
        assert_eq!(driver_playback.compiler.project_context(), &ctx);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
