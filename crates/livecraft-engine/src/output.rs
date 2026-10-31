//! The audio output (M05 item 12, TARGET-ARCHITECTURE "Threads" and "Engine Decisions"; M06 item 2).
//!
//! The driver calls `Engine::process` for each buffer and does nothing else. `Playback` is the
//! session side: it compiles the Set, submits the graphs, sends commands, and reads the snapshots.
//! It holds the driver, so dropping it stops the output.

use crate::compiler::{GraphCompiler, ProcessorFactory};
use crate::driver::{AudioDriver, CpalDriver, DriverConfig, DriverStatus, Latency};
use crate::executor::{Engine, EngineCommand, EngineHandle};
use basedrop::Collector;
use livecraft_devices::create_device_processor;
use livecraft_model::model::Project;

/// The session side of a running engine, and the driver that plays it when there is one.
pub struct Playback {
    /// Keeps the output running. None when the Set renders without an audio driver.
    driver: Option<Box<dyn AudioDriver>>,
    pub sample_rate: f32,
    pub compiler: GraphCompiler,
    pub handle: EngineHandle,
}

impl Playback {
    /// Opens the output that `config` names and starts rendering `project` through it.
    pub fn open(project: &Project, config: &DriverConfig) -> anyhow::Result<Self> {
        Self::open_with_driver(project, Box::new(CpalDriver::open(config)?))
    }

    /// Starts rendering `project` through `driver`, at the driver's sample rate.
    pub fn open_with_driver(
        project: &Project,
        mut driver: Box<dyn AudioDriver>,
    ) -> anyhow::Result<Self> {
        let (mut playback, engine) = Self::start(project, driver.sample_rate() as f32)?;
        driver.start(engine)?;
        playback.driver = Some(driver);
        Ok(playback)
    }

    /// Whether the driver runs. It is `Stopped` when the Set renders without a driver.
    pub fn driver_status(&self) -> DriverStatus {
        self.driver
            .as_ref()
            .map_or(DriverStatus::Stopped, |driver| driver.status())
    }

    /// The output latency of the driver. It is zero when the Set renders without a driver.
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

        let playback =
            Playback::open_with_driver(&project, Box::new(NullDriver::new(44100, 512, 2)))
                .expect("open with a null driver");
        assert_eq!(playback.driver_status(), DriverStatus::Running);
        assert_eq!(playback.sample_rate, 44100.0);
    }
}
