//! The host audio driver (M06 items 2 and 5): plays the engine through a `cpal` output stream, and
//! captures audio input through a `cpal` input stream when the config asks for one.
//!
//! `open` picks the host, the devices, the sample rate, and the buffer size. `start` builds and plays
//! the streams. The output callback calls `Engine::process` and records the output delay in an atomic.
//! The input callback pushes its samples into a ring that the engine reads, and records the input
//! delay. The output error callback records a lost device, which `status` reports.

use super::{AudioDriver, DriverConfig, DriverError, DriverStatus, Latency};
use crate::executor::Engine;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{
    BufferSize, ErrorKind, FromSample, HostId, Sample, SampleFormat, SizedSample, Stream,
    StreamConfig, SupportedStreamConfig, SupportedStreamConfigRange,
};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

/// Frames converted per step when the device does not take `f32` samples. A callback buffer longer
/// than this is converted in steps, so the scratch buffer never grows on the audio thread.
const SCRATCH_FRAMES: usize = 1024;

/// The input ring holds this many frames per channel: eight callbacks of 2048 frames. Input that does
/// not fit is dropped (see `build_capture`).
const INPUT_RING_FRAMES: usize = 8 * 2048;

/// A `cpal` output stream on one host and device, and an input stream when the config names one.
pub struct CpalDriver {
    host: &'static str,
    device: cpal::Device,
    config: StreamConfig,
    format: SampleFormat,
    /// The input device, when the driver captures input.
    input: Option<InputSetup>,
    stream: Option<Stream>,
    input_stream: Option<Stream>,
    /// The message of the error that lost the device. It is set once.
    lost: Arc<OnceLock<String>>,
    /// The output delay in frames, measured by the output callback.
    output_frames: Arc<AtomicU32>,
    /// The input delay in frames, measured by the input callback.
    input_frames: Arc<AtomicU32>,
}

/// An input device and the config its stream opens with. `required` is set when the config named the
/// device, so a named device that cannot open is an error. The default input is best effort.
struct InputSetup {
    device: cpal::Device,
    config: StreamConfig,
    format: SampleFormat,
    required: bool,
}

impl CpalDriver {
    /// Resolves the host, devices, sample rate, and buffer size that `config` asks for. A host or
    /// device name that is not found falls back to the default, with a warning. A sample rate that
    /// the output device does not support is an error. The input runs at the output's rate. Nothing
    /// plays until `start`.
    pub fn open(config: &DriverConfig) -> Result<Self, DriverError> {
        let host = resolve_host(config.host.as_deref());
        let device = resolve_output_device(&host, config.output_device.as_deref())?;
        let default = device.default_output_config()?;
        let ranges: Vec<SupportedStreamConfigRange> = device.supported_output_configs()?.collect();
        let requested = config.sample_rate;
        let supported = select_stream_config(&ranges, &default, requested).ok_or_else(|| {
            DriverError::UnsupportedSampleRate {
                device: device_name(&device),
                rate: requested.unwrap_or_default(),
            }
        })?;
        let format = supported.sample_format();
        if !supports_format(format) {
            return Err(DriverError::UnsupportedSampleFormat(format));
        }
        let buffer_size = config
            .buffer_frames
            .map_or(BufferSize::Default, BufferSize::Fixed);
        let input = open_input(&host, config, supported.sample_rate(), buffer_size)?;
        Ok(Self {
            host: host.id().name(),
            device,
            config: StreamConfig {
                channels: supported.channels().max(1),
                sample_rate: supported.sample_rate(),
                buffer_size,
            },
            format,
            input,
            stream: None,
            input_stream: None,
            lost: Arc::new(OnceLock::new()),
            output_frames: Arc::new(AtomicU32::new(0)),
            input_frames: Arc::new(AtomicU32::new(0)),
        })
    }

    /// Opens the input stream of `setup` and gives the engine the reading end of the ring that the
    /// stream fills. The writing end moves into the stream's callback.
    fn start_input(&self, setup: &InputSetup, engine: &mut Engine) -> Result<Stream, DriverError> {
        let channels = usize::from(setup.config.channels);
        let (producer, consumer) = rtrb::RingBuffer::new(INPUT_RING_FRAMES * channels);
        engine.set_input(consumer, channels);
        let stream = match setup.format {
            SampleFormat::F32 => self.build_capture::<f32>(setup, producer)?,
            SampleFormat::I16 => self.build_capture::<i16>(setup, producer)?,
            SampleFormat::U16 => self.build_capture::<u16>(setup, producer)?,
            other => return Err(DriverError::UnsupportedSampleFormat(other)),
        };
        stream.play()?;
        Ok(stream)
    }

    /// Builds the input stream for samples of type `T`. Its callback converts each sample to `f32` and
    /// pushes whole frames into the ring. A frame that does not fit is dropped, so the callback never
    /// waits.
    fn build_capture<T>(
        &self,
        setup: &InputSetup,
        mut producer: rtrb::Producer<f32>,
    ) -> Result<Stream, DriverError>
    where
        T: SizedSample,
        f32: FromSample<T>,
    {
        let channels = usize::from(setup.config.channels);
        let rate = setup.config.sample_rate;
        let input_frames = Arc::clone(&self.input_frames);
        let stream = setup.device.build_input_stream(
            setup.config,
            move |data: &[T], info: &cpal::InputCallbackInfo| {
                let timing = info.timestamp();
                let delay = timing.callback.saturating_duration_since(timing.capture);
                input_frames.store(frames_in(delay, rate), Ordering::Relaxed);
                for frame in data.chunks_exact(channels) {
                    if producer.slots() < channels {
                        break;
                    }
                    for sample in frame {
                        // The room for the whole frame was checked above, so the push cannot fail.
                        let _ = producer.push(f32::from_sample(*sample));
                    }
                }
            },
            |err: cpal::Error| log::error!("Audio input error: {err}"),
            None,
        )?;
        Ok(stream)
    }

    /// Builds the output stream for samples of type `T`. The callback converts each engine block
    /// from `f32` into the device buffer, through a scratch buffer allocated here.
    fn build_stream<T>(&self, mut engine: Engine) -> Result<Stream, DriverError>
    where
        T: SizedSample + FromSample<f32>,
    {
        let channels = usize::from(self.config.channels);
        let rate = self.config.sample_rate;
        let mut scratch = vec![0.0f32; SCRATCH_FRAMES * channels];
        let output_frames = Arc::clone(&self.output_frames);
        let lost = Arc::clone(&self.lost);
        let stream = self.device.build_output_stream(
            self.config,
            move |data: &mut [T], info: &cpal::OutputCallbackInfo| {
                let timing = info.timestamp();
                let delay = timing.playback.saturating_duration_since(timing.callback);
                output_frames.store(frames_in(delay, rate), Ordering::Relaxed);
                for chunk in data.chunks_mut(scratch.len()) {
                    let rendered = &mut scratch[..chunk.len()];
                    engine.process(rendered, channels);
                    for (out, sample) in chunk.iter_mut().zip(rendered.iter()) {
                        *out = T::from_sample(*sample);
                    }
                }
            },
            move |err: cpal::Error| {
                log::error!("Audio stream error: {err}");
                if err.kind() == ErrorKind::DeviceNotAvailable {
                    let _ = lost.set(err.to_string());
                }
            },
            None,
        )?;
        Ok(stream)
    }
}

impl AudioDriver for CpalDriver {
    fn name(&self) -> &str {
        self.host
    }

    fn sample_rate(&self) -> u32 {
        self.config.sample_rate
    }

    fn buffer_frames(&self) -> u32 {
        match self.config.buffer_size {
            BufferSize::Fixed(frames) => frames,
            BufferSize::Default => self
                .stream
                .as_ref()
                .and_then(|stream| stream.buffer_size().ok())
                .unwrap_or(0),
        }
    }

    fn channels(&self) -> usize {
        usize::from(self.config.channels)
    }

    fn latency(&self) -> Latency {
        Latency {
            input_frames: self.input_frames.load(Ordering::Relaxed),
            output_frames: self.output_frames.load(Ordering::Relaxed),
        }
    }

    fn output_delay(&self) -> Arc<AtomicU32> {
        Arc::clone(&self.output_frames)
    }

    fn start(&mut self, mut engine: Engine) -> Result<(), DriverError> {
        if self.stream.is_some() {
            return Err(DriverError::AlreadyStarted);
        }
        if let Some(setup) = &self.input {
            match self.start_input(setup, &mut engine) {
                Ok(stream) => self.input_stream = Some(stream),
                Err(err) if setup.required => return Err(err),
                Err(err) => log::warn!("Audio input is off: {err}"),
            }
        }
        let stream = match self.format {
            SampleFormat::F32 => self.build_stream::<f32>(engine)?,
            SampleFormat::I16 => self.build_stream::<i16>(engine)?,
            SampleFormat::U16 => self.build_stream::<u16>(engine)?,
            other => return Err(DriverError::UnsupportedSampleFormat(other)),
        };
        stream.play()?;
        self.stream = Some(stream);
        Ok(())
    }

    fn status(&self) -> DriverStatus {
        if let Some(message) = self.lost.get() {
            return DriverStatus::Lost(message.clone());
        }
        if self.stream.is_some() {
            DriverStatus::Running
        } else {
            DriverStatus::Stopped
        }
    }
}

/// The host named `name`, or the default host when there is no name or no such host.
fn resolve_host(name: Option<&str>) -> cpal::Host {
    let Some(name) = name else {
        return cpal::default_host();
    };
    match name.parse::<HostId>().and_then(cpal::host_from_id) {
        Ok(host) => host,
        Err(err) => {
            log::warn!("Audio host {name} is not available ({err}), so the default host is used");
            cpal::default_host()
        }
    }
}

/// The output device named `name`, or the default output device when there is no name or no such
/// device.
fn resolve_output_device(
    host: &cpal::Host,
    name: Option<&str>,
) -> Result<cpal::Device, DriverError> {
    if let Some(name) = name {
        let found = host
            .output_devices()?
            .find(|device| device_name(device) == name);
        if let Some(device) = found {
            return Ok(device);
        }
        log::warn!(
            "Audio output device {name} was not found, so the default output device is used"
        );
    }
    host.default_output_device()
        .ok_or(DriverError::NoOutputDevice)
}

/// The input device the config asks for, and whether the config named it. A named device that is not
/// found is reported, and the default input is used when `input_enabled` is set.
fn resolve_input_device(
    host: &cpal::Host,
    config: &DriverConfig,
) -> Result<Option<(cpal::Device, bool)>, DriverError> {
    if let Some(name) = config.input_device.as_deref() {
        let found = host
            .input_devices()?
            .find(|device| device_name(device) == name);
        if let Some(device) = found {
            return Ok(Some((device, true)));
        }
        log::warn!("Audio input device {name} was not found");
    }
    if !config.input_enabled {
        return Ok(None);
    }
    Ok(host.default_input_device().map(|device| (device, false)))
}

/// Resolves the input the config asks for, at the output's sample rate. A named input that cannot open
/// is an error. The default input is best effort: when it cannot open, input is off, with a warning.
fn open_input(
    host: &cpal::Host,
    config: &DriverConfig,
    sample_rate: u32,
    buffer_size: BufferSize,
) -> Result<Option<InputSetup>, DriverError> {
    let Some((device, required)) = resolve_input_device(host, config)? else {
        return Ok(None);
    };
    match input_setup(device, sample_rate, buffer_size, required) {
        Ok(setup) => Ok(Some(setup)),
        Err(err) if required => Err(err),
        Err(err) => {
            log::warn!("Audio input is off: {err}");
            Ok(None)
        }
    }
}

/// The config an input device opens with at `sample_rate`.
fn input_setup(
    device: cpal::Device,
    sample_rate: u32,
    buffer_size: BufferSize,
    required: bool,
) -> Result<InputSetup, DriverError> {
    let default = device.default_input_config()?;
    let ranges: Vec<SupportedStreamConfigRange> = device.supported_input_configs()?.collect();
    let supported =
        select_stream_config(&ranges, &default, Some(sample_rate)).ok_or_else(|| {
            DriverError::UnsupportedSampleRate {
                device: device_name(&device),
                rate: sample_rate,
            }
        })?;
    let format = supported.sample_format();
    if !supports_format(format) {
        return Err(DriverError::UnsupportedSampleFormat(format));
    }
    Ok(InputSetup {
        device,
        config: StreamConfig {
            channels: supported.channels().max(1),
            sample_rate,
            buffer_size,
        },
        format,
        required,
    })
}

/// The name a device is listed under, or an empty string when the host cannot describe it.
fn device_name(device: &cpal::Device) -> String {
    device
        .description()
        .map(|description| description.name().to_string())
        .unwrap_or_default()
}

/// Whether the stream code can convert samples of `format`.
fn supports_format(format: SampleFormat) -> bool {
    matches!(
        format,
        SampleFormat::F32 | SampleFormat::I16 | SampleFormat::U16
    )
}

/// The config a stream opens with. Without a requested rate it is the device default. With one, it
/// is the supported range that contains the rate, preferring the default's channel count and sample
/// format. It is `None` when no supported range contains the rate.
fn select_stream_config(
    ranges: &[SupportedStreamConfigRange],
    default: &SupportedStreamConfig,
    requested: Option<u32>,
) -> Option<SupportedStreamConfig> {
    let Some(rate) = requested else {
        return Some(*default);
    };
    ranges
        .iter()
        .filter(|range| supports_format(range.sample_format()) && range.contains_rate(rate))
        .min_by_key(|range| {
            (
                range.channels() != default.channels(),
                range.sample_format() != default.sample_format(),
            )
        })
        .and_then(|range| range.try_with_sample_rate(rate))
}

/// The number of frames in `duration` at `sample_rate`, rounded to the nearest frame.
fn frames_in(duration: Duration, sample_rate: u32) -> u32 {
    (duration.as_secs_f64() * f64::from(sample_rate)).round() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(
        channels: u16,
        min: u32,
        max: u32,
        format: SampleFormat,
    ) -> SupportedStreamConfigRange {
        SupportedStreamConfigRange::new(channels, min, max, Default::default(), format)
    }

    fn default_stereo() -> SupportedStreamConfig {
        SupportedStreamConfig::new(2, 48000, Default::default(), SampleFormat::F32)
    }

    #[test]
    fn a_requested_rate_inside_a_supported_range_is_opened_at_that_rate() {
        let ranges = [range(2, 44100, 96000, SampleFormat::F32)];
        let chosen = select_stream_config(&ranges, &default_stereo(), Some(44100))
            .expect("44100 Hz is supported");
        assert_eq!(chosen.sample_rate(), 44100);
    }

    #[test]
    fn a_requested_rate_outside_every_supported_range_is_refused() {
        let ranges = [range(2, 44100, 48000, SampleFormat::F32)];
        assert_eq!(
            select_stream_config(&ranges, &default_stereo(), Some(96000)),
            None
        );
    }

    #[test]
    fn without_a_requested_rate_the_device_default_is_kept() {
        let ranges = [range(2, 44100, 48000, SampleFormat::F32)];
        assert_eq!(
            select_stream_config(&ranges, &default_stereo(), None),
            Some(default_stereo())
        );
    }

    #[test]
    fn the_range_with_the_default_channels_and_format_is_preferred() {
        let ranges = [
            range(8, 44100, 48000, SampleFormat::F32),
            range(2, 44100, 48000, SampleFormat::I16),
            range(2, 44100, 48000, SampleFormat::F32),
        ];
        let chosen = select_stream_config(&ranges, &default_stereo(), Some(48000))
            .expect("48000 Hz is supported");
        assert_eq!(chosen.channels(), 2);
        assert_eq!(chosen.sample_format(), SampleFormat::F32);
    }

    #[test]
    fn a_range_in_a_sample_format_the_stream_cannot_convert_is_skipped() {
        let ranges = [range(2, 44100, 48000, SampleFormat::I32)];
        assert_eq!(
            select_stream_config(&ranges, &default_stereo(), Some(48000)),
            None
        );
    }

    #[test]
    fn a_delay_of_ten_milliseconds_is_441_frames_at_44100_hz() {
        assert_eq!(frames_in(Duration::from_millis(10), 44100), 441);
    }
}
