//! The silent driver (M06 item 2): it calls `Engine::process` once per buffer period, on its own
//! thread, with no device.
//!
//! The transport moves in real time, so the playhead and the snapshot behave as they do with a
//! device. Nothing is played.

use super::{AudioDriver, DriverError, DriverStatus, Latency};
use crate::executor::Engine;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Calls the engine once per buffer period on a thread of its own, with no device.
pub struct NullDriver {
    sample_rate: u32,
    buffer_frames: u32,
    channels: usize,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl NullDriver {
    /// A driver that runs at `sample_rate`, in buffers of `buffer_frames` frames of `channels`
    /// channels.
    pub fn new(sample_rate: u32, buffer_frames: u32, channels: usize) -> Self {
        Self {
            sample_rate: sample_rate.max(1),
            buffer_frames: buffer_frames.max(1),
            channels: channels.max(1),
            stop: Arc::new(AtomicBool::new(false)),
            thread: None,
        }
    }
}

impl AudioDriver for NullDriver {
    fn name(&self) -> &str {
        "Null"
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn buffer_frames(&self) -> u32 {
        self.buffer_frames
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn latency(&self) -> Latency {
        Latency::default()
    }

    fn start(&mut self, engine: Engine) -> Result<(), DriverError> {
        if self.thread.is_some() {
            return Err(DriverError::AlreadyStarted);
        }
        let stop = Arc::clone(&self.stop);
        let (sample_rate, buffer_frames, channels) =
            (self.sample_rate, self.buffer_frames, self.channels);
        let thread = thread::Builder::new()
            .name("livecraft-null-driver".to_string())
            .spawn(move || run(engine, &stop, sample_rate, buffer_frames, channels))?;
        self.thread = Some(thread);
        Ok(())
    }

    fn status(&self) -> DriverStatus {
        match &self.thread {
            Some(thread) if !thread.is_finished() => DriverStatus::Running,
            _ => DriverStatus::Stopped,
        }
    }
}

impl Drop for NullDriver {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            log::error!("The null audio driver thread panicked");
        }
    }
}

/// The driver thread. It renders one buffer, then sleeps until the deadline of the next. Deadlines
/// are counted from the start, so the rate does not drift.
fn run(
    mut engine: Engine,
    stop: &AtomicBool,
    sample_rate: u32,
    buffer_frames: u32,
    channels: usize,
) {
    let mut out = vec![0.0f32; buffer_frames as usize * channels];
    let origin = Instant::now();
    let mut buffers = 0u64;
    while !stop.load(Ordering::Acquire) {
        engine.process(&mut out, channels);
        buffers += 1;
        let period = buffers as f64 * f64::from(buffer_frames) / f64::from(sample_rate);
        let deadline = origin + Duration::from_secs_f64(period);
        thread::sleep(deadline.saturating_duration_since(Instant::now()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::EngineCommand;
    use crate::output::Playback;
    use livecraft_model::model::Project;

    /// A playback of the default Set with the transport playing, and its tempo.
    fn playing() -> (Playback, Engine, f32) {
        let project = Project::new_default("Null");
        let (mut playback, engine) = Playback::start(&project, 44100.0).expect("start the engine");
        playback
            .handle
            .send(EngineCommand::SetPlaying(true))
            .expect("start the transport");
        (playback, engine, project.transport.bpm())
    }

    #[test]
    fn the_playhead_moves_in_real_time_while_the_null_driver_runs() {
        let (mut playback, engine, bpm) = playing();
        let mut driver = NullDriver::new(44100, 512, 2);
        driver.start(engine).expect("start the driver");
        assert_eq!(driver.status(), DriverStatus::Running);
        std::thread::sleep(Duration::from_millis(200));
        let beats = playback.handle.snapshot().transport.playhead_beats;
        let expected = 0.2 * f64::from(bpm) / 60.0;
        assert!(
            beats >= 0.75 * expected,
            "the playhead moved {beats} beats in 200 ms, expected about {expected}"
        );
    }

    #[test]
    fn dropping_the_null_driver_stops_the_transport() {
        let (mut playback, engine, _) = playing();
        let mut driver = NullDriver::new(44100, 512, 2);
        driver.start(engine).expect("start the driver");
        std::thread::sleep(Duration::from_millis(50));
        drop(driver);
        let stopped = playback.handle.snapshot().transport.playhead_beats;
        assert!(
            stopped > 0.0,
            "the transport should have moved before the drop"
        );
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(playback.handle.snapshot().transport.playhead_beats, stopped);
    }

    #[test]
    fn a_null_driver_refuses_a_second_engine() {
        let (_first, engine, _) = playing();
        let (_second, other, _) = playing();
        let mut driver = NullDriver::new(44100, 512, 2);
        driver.start(engine).expect("start the driver");
        assert!(matches!(
            driver.start(other),
            Err(DriverError::AlreadyStarted)
        ));
    }

    #[test]
    fn a_null_driver_reports_stopped_until_it_starts() {
        let (_playback, engine, _) = playing();
        let mut driver = NullDriver::new(48000, 256, 2);
        assert_eq!(driver.status(), DriverStatus::Stopped);
        assert_eq!(driver.sample_rate(), 48000);
        assert_eq!(driver.buffer_frames(), 256);
        assert_eq!(driver.channels(), 2);
        assert_eq!(driver.latency(), Latency::default());
        driver.start(engine).expect("start the driver");
        assert_eq!(driver.status(), DriverStatus::Running);
    }
}
