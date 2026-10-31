//! The audio output (M05 item 12, TARGET-ARCHITECTURE "Threads" and "Engine Decisions"; M06 items 2
//! and 6 and 7).
//!
//! The driver calls `Engine::process` for each buffer and does nothing else. `Playback` is the
//! session side: it compiles the Set, submits the graphs, sends commands, and reads the snapshots.
//! It holds the driver and the MIDI ports, so dropping it stops the output and closes the ports.

use crate::compiler::{GraphCompiler, ProcessorFactory};
use crate::driver::{AudioDriver, CpalDriver, DriverConfig, DriverStatus, Latency};
use crate::executor::{Engine, EngineCommand, EngineHandle};
use crate::midi_io::{MidiInputs, MidiOutputs, MidiPorts};
use basedrop::Collector;
use livecraft_devices::create_device_processor;
use livecraft_model::model::Project;

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
        mut driver: Box<dyn AudioDriver>,
        midi: &MidiPorts,
    ) -> anyhow::Result<Self> {
        let sample_rate = driver.sample_rate() as f32;
        let (mut playback, mut engine) = Self::start(project, sample_rate)?;
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

    /// The input and output latency of the driver. It is zero when the Set renders without a driver.
    pub fn latency(&self) -> Latency {
        self.driver
            .as_ref()
            .map_or_else(Latency::default, |driver| driver.latency())
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
            driver: None,
            _midi_inputs: None,
            _midi_outputs: None,
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
}
