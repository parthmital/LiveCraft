//! The audio hosts and devices of this machine (M06 item 3), for the settings window and the
//! `livecraft devices` command (manual 41.2 Audio Setup).
//!
//! Listing reads what each host reports and opens no stream. A host or device that cannot be read is
//! skipped with a warning, so one broken driver does not hide the others.

use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{DeviceId, HostId, SupportedBufferSize, SupportedStreamConfigRange};
use serde::Serialize;

/// The sample rates the settings offer, in hertz (M06 item 3).
const OFFERED_SAMPLE_RATES: [u32; 6] = [44_100, 48_000, 88_200, 96_000, 176_400, 192_000];

/// Every audio host on this machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeviceList {
    pub hosts: Vec<HostInfo>,
}

/// One audio host, which is the driver type of Live's Audio settings, and its devices.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HostInfo {
    /// The host's name, such as `WASAPI`.
    pub name: String,
    /// Whether the system uses this host by default.
    pub is_default: bool,
    /// The output devices of the host.
    pub outputs: Vec<DeviceInfo>,
    /// The input devices of the host.
    pub inputs: Vec<DeviceInfo>,
}

/// One input or output device, as its host describes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeviceInfo {
    /// The name the host gives the device.
    pub name: String,
    /// Whether the host uses this device by default for its direction.
    pub is_default: bool,
    /// The most channels that any of its configurations offer.
    pub channels: u16,
    /// The offered sample rates that the device supports, in ascending order.
    pub sample_rates: Vec<u32>,
    /// The smallest and largest buffer size in frames, or `None` when the host reports no range.
    pub buffer_frames: Option<(u32, u32)>,
}

/// Lists every available host and the devices of each. A host or device that cannot be read is
/// skipped with a warning.
pub fn list_devices() -> DeviceList {
    let default_host = cpal::default_host().id();
    let hosts = cpal::available_hosts()
        .into_iter()
        .filter_map(|id| list_host(id, id == default_host))
        .collect();
    DeviceList { hosts }
}

/// Which side of the audio device a list describes.
#[derive(Debug, Clone, Copy)]
enum Direction {
    Input,
    Output,
}

impl Direction {
    fn configs(
        self,
        device: &cpal::Device,
    ) -> Result<Vec<SupportedStreamConfigRange>, cpal::Error> {
        Ok(match self {
            Self::Input => device.supported_input_configs()?.collect(),
            Self::Output => device.supported_output_configs()?.collect(),
        })
    }
}

/// The devices of one host, or `None` when the host cannot be opened.
fn list_host(id: HostId, is_default: bool) -> Option<HostInfo> {
    let host = match cpal::host_from_id(id) {
        Ok(host) => host,
        Err(error) => {
            log::warn!(
                "Audio host {} could not be opened, so it is skipped: {error}",
                id.name()
            );
            return None;
        }
    };
    let default_output = host
        .default_output_device()
        .and_then(|device| device.id().ok());
    let default_input = host
        .default_input_device()
        .and_then(|device| device.id().ok());
    Some(HostInfo {
        name: id.name().to_string(),
        is_default,
        outputs: list_side(
            id.name(),
            host.output_devices(),
            default_output.as_ref(),
            Direction::Output,
        ),
        inputs: list_side(
            id.name(),
            host.input_devices(),
            default_input.as_ref(),
            Direction::Input,
        ),
    })
}

/// The devices of one direction of a host, skipping the ones that cannot be read.
fn list_side(
    host: &str,
    devices: Result<impl Iterator<Item = cpal::Device>, cpal::Error>,
    default: Option<&DeviceId>,
    direction: Direction,
) -> Vec<DeviceInfo> {
    match devices {
        Ok(devices) => devices
            .filter_map(|device| describe(&device, default, direction))
            .collect(),
        Err(error) => {
            log::warn!("The devices of audio host {host} could not be listed: {error}");
            Vec::new()
        }
    }
}

/// Describes one device, or `None` when its name or its configurations cannot be read.
fn describe(
    device: &cpal::Device,
    default: Option<&DeviceId>,
    direction: Direction,
) -> Option<DeviceInfo> {
    let name = match device.description() {
        Ok(description) => description.name().to_string(),
        Err(error) => {
            log::warn!("An audio device has no readable name, so it is skipped: {error}");
            return None;
        }
    };
    let configs = match direction.configs(device) {
        Ok(configs) => configs,
        Err(error) => {
            log::warn!("Audio device {name} lists no configurations, so it is skipped: {error}");
            return None;
        }
    };
    let id = device.id().ok();
    let rates: Vec<(u32, u32)> = configs
        .iter()
        .map(|config| (config.min_sample_rate(), config.max_sample_rate()))
        .collect();
    Some(DeviceInfo {
        name,
        is_default: id.is_some() && id.as_ref() == default,
        channels: configs
            .iter()
            .map(SupportedStreamConfigRange::channels)
            .max()
            .unwrap_or(0),
        sample_rates: supported_rates(&rates),
        buffer_frames: buffer_frames(&configs),
    })
}

/// The offered sample rates that at least one of the `(min, max)` ranges contains, in ascending
/// order.
fn supported_rates(ranges: &[(u32, u32)]) -> Vec<u32> {
    OFFERED_SAMPLE_RATES
        .into_iter()
        .filter(|rate| {
            ranges
                .iter()
                .any(|&(min, max)| min <= *rate && *rate <= max)
        })
        .collect()
}

/// The smallest and largest buffer size over the configurations that report a range, or `None`
/// when none does.
fn buffer_frames(configs: &[SupportedStreamConfigRange]) -> Option<(u32, u32)> {
    configs
        .iter()
        .filter_map(|config| match config.buffer_size() {
            SupportedBufferSize::Range { min, max } => Some((*min, *max)),
            SupportedBufferSize::Unknown => None,
        })
        .reduce(|(low, high), (min, max)| (low.min(min), high.max(max)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cpal::SampleFormat;

    fn config(
        channels: u16,
        rates: (u32, u32),
        buffer: SupportedBufferSize,
    ) -> SupportedStreamConfigRange {
        SupportedStreamConfigRange::new(channels, rates.0, rates.1, buffer, SampleFormat::F32)
    }

    #[test]
    fn a_rate_is_offered_when_a_range_contains_it() {
        assert_eq!(supported_rates(&[(44_100, 48_000)]), vec![44_100, 48_000]);
    }

    #[test]
    fn the_rates_of_several_ranges_are_merged_in_ascending_order() {
        let ranges = [(192_000, 192_000), (44_100, 44_100), (88_200, 96_000)];
        assert_eq!(
            supported_rates(&ranges),
            vec![44_100, 88_200, 96_000, 192_000]
        );
    }

    #[test]
    fn a_rate_outside_the_offered_list_is_not_listed() {
        assert_eq!(
            supported_rates(&[(8_000, 8_000), (22_050, 22_050)]),
            Vec::<u32>::new()
        );
    }

    #[test]
    fn a_range_spanning_every_offered_rate_lists_all_of_them() {
        assert_eq!(
            supported_rates(&[(8_000, 384_000)]),
            OFFERED_SAMPLE_RATES.to_vec()
        );
    }

    #[test]
    fn a_device_without_configurations_offers_no_rates() {
        assert_eq!(supported_rates(&[]), Vec::<u32>::new());
    }

    #[test]
    fn the_buffer_range_spans_the_configurations_that_report_one() {
        let configs = [
            config(
                2,
                (44_100, 48_000),
                SupportedBufferSize::Range { min: 64, max: 1024 },
            ),
            config(
                8,
                (44_100, 48_000),
                SupportedBufferSize::Range { min: 32, max: 512 },
            ),
            config(2, (44_100, 48_000), SupportedBufferSize::Unknown),
        ];
        assert_eq!(buffer_frames(&configs), Some((32, 1024)));
    }

    #[test]
    fn a_device_that_reports_no_buffer_range_has_none() {
        let configs = [config(2, (44_100, 48_000), SupportedBufferSize::Unknown)];
        assert_eq!(buffer_frames(&configs), None);
    }
}
