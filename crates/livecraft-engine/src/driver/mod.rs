//! Audio drivers (M06 item 2): the device side of the engine.
//!
//! A driver takes the engine once and calls `Engine::process` for each buffer it needs. `CpalDriver`
//! plays through the host's audio API. `NullDriver` keeps the transport moving in real time with no
//! device. `VirtualDriver` renders on demand, for offline work (M08).

mod cpal_driver;
mod devices;
mod null;
mod virtual_driver;

pub use cpal_driver::CpalDriver;
pub use devices::{DeviceInfo, DeviceList, HostInfo, list_devices};
pub use null::NullDriver;
pub use virtual_driver::VirtualDriver;

use crate::executor::Engine;
use cpal::SampleFormat;

/// Plays or renders the engine. A driver is created stopped and takes the engine once, in `start`.
/// It is not `Send`: a cpal stream may not leave the thread that opened it (CoreAudio), so the
/// session keeps its driver on one thread.
pub trait AudioDriver {
    /// The driver's name, such as the host API for `CpalDriver`.
    fn name(&self) -> &str;
    /// The sample rate the engine must be built for.
    fn sample_rate(&self) -> u32;
    /// Frames per buffer, or 0 while a device has not reported its buffer size.
    fn buffer_frames(&self) -> u32;
    /// Channels per frame in each buffer.
    fn channels(&self) -> usize;
    /// The delay between the engine's output and the device. Drivers without a device report zero.
    fn latency(&self) -> Latency;
    /// Takes the engine and starts calling `Engine::process`.
    fn start(&mut self, engine: Engine) -> Result<(), DriverError>;
    /// Whether the driver runs, and why it does not when it has stopped on its own.
    fn status(&self) -> DriverStatus;
}

/// What the session asks of a driver. `None` means the system default.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DriverConfig {
    /// The host API by name, such as `wasapi`. The name is matched without regard to case.
    pub host: Option<String>,
    /// The output device by its name.
    pub output_device: Option<String>,
    /// The input device by its name. Input capture uses it from M06 item 5.
    pub input_device: Option<String>,
    /// The sample rate in hertz.
    pub sample_rate: Option<u32>,
    /// Frames per buffer.
    pub buffer_frames: Option<u32>,
}

/// The delay of a driver's input and output, in frames.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Latency {
    pub input_frames: u32,
    pub output_frames: u32,
}

impl Latency {
    /// The input delay in milliseconds at `sample_rate`.
    pub fn input_ms(&self, sample_rate: u32) -> f32 {
        frames_to_ms(self.input_frames, sample_rate)
    }

    /// The output delay in milliseconds at `sample_rate`.
    pub fn output_ms(&self, sample_rate: u32) -> f32 {
        frames_to_ms(self.output_frames, sample_rate)
    }
}

fn frames_to_ms(frames: u32, sample_rate: u32) -> f32 {
    frames as f32 * 1000.0 / sample_rate.max(1) as f32
}

/// Whether a driver runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriverStatus {
    /// The driver has not started, or it has stopped.
    Stopped,
    /// The driver is calling the engine.
    Running,
    /// The device went away. The message says why. The driver does not recover by itself.
    Lost(String),
}

/// Why a driver could not open, start, or keep running.
#[derive(Debug, thiserror::Error)]
pub enum DriverError {
    #[error("no audio output device is available")]
    NoOutputDevice,
    #[error("the output device {device} does not support {rate} Hz")]
    UnsupportedSampleRate { device: String, rate: u32 },
    #[error("the device sample format {0:?} is not supported")]
    UnsupportedSampleFormat(SampleFormat),
    #[error("the driver is already started")]
    AlreadyStarted,
    #[error(transparent)]
    Audio(#[from] cpal::Error),
    #[error("the driver thread could not start: {0}")]
    Thread(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latency_converts_frames_to_milliseconds_at_the_sample_rate() {
        let latency = Latency {
            input_frames: 441,
            output_frames: 882,
        };
        assert!((latency.input_ms(44100) - 10.0).abs() < 1e-4);
        assert!((latency.output_ms(44100) - 20.0).abs() < 1e-4);
    }
}
