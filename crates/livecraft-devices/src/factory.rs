//! Builds the `DeviceProcessor` of a device from its model (M05 item 5).

use crate::device_processor::DeviceProcessor;
use crate::effects::{
    AutoFilterEffect, AutoPanEffect, ChorusEffect, CompressorEffect, DelayEffect, EqEight,
    EqThreeEffect, GateEffect, LimiterEffect, ReduxEffect, ReverbEffect, SaturatorEffect,
    UtilityEffect,
};
use crate::instruments::{DrumRackInstrument, OperatorSynth, SimplerInstrument};
use livecraft_model::model::{Device, DeviceType};

/// The processor for a device, with the device's stored parameter values applied by id. Returns
/// None for kinds without a native processor (`DeviceType::Custom`, such as plug-ins).
pub fn create_device_processor(
    device: &Device,
    sample_rate: f32,
) -> Option<Box<dyn DeviceProcessor>> {
    let mut processor: Box<dyn DeviceProcessor> = match device.device_type {
        DeviceType::EqEight => Box::new(EqEight::new(sample_rate)),
        DeviceType::EqThree => Box::new(EqThreeEffect::new(sample_rate)),
        DeviceType::Compressor => Box::new(CompressorEffect::new(sample_rate)),
        DeviceType::Limiter => Box::new(LimiterEffect::new(sample_rate)),
        DeviceType::Utility => Box::new(UtilityEffect::new()),
        DeviceType::Delay => Box::new(DelayEffect::new(sample_rate)),
        DeviceType::Reverb => Box::new(ReverbEffect::new(sample_rate)),
        DeviceType::Saturator => Box::new(SaturatorEffect::new(sample_rate)),
        DeviceType::AutoFilter => Box::new(AutoFilterEffect::new(sample_rate)),
        DeviceType::Chorus => Box::new(ChorusEffect::new(sample_rate)),
        DeviceType::AutoPan => Box::new(AutoPanEffect::new(sample_rate)),
        DeviceType::Gate => Box::new(GateEffect::new(sample_rate)),
        DeviceType::Redux => Box::new(ReduxEffect::new(sample_rate)),
        DeviceType::Operator => Box::new(OperatorSynth::new(sample_rate)),
        DeviceType::Simpler => Box::new(SimplerInstrument::new(sample_rate)),
        DeviceType::DrumRack => Box::new(DrumRackInstrument::new(sample_rate)),
        DeviceType::Custom(_) => return None,
    };
    for (id, value) in device.parameters.iter() {
        processor.set_param(id, value, 0);
    }
    Some(processor)
}

/// True for the devices that make sound from note events rather than from their input.
pub fn is_instrument(device_type: &DeviceType) -> bool {
    matches!(
        device_type,
        DeviceType::Operator | DeviceType::Simpler | DeviceType::DrumRack
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use livecraft_model::ids::DeviceId;

    #[test]
    fn every_native_kind_has_a_processor_and_custom_kinds_do_not() {
        let native = Device::new(DeviceId(1), "Utility", DeviceType::Utility);
        assert!(create_device_processor(&native, 44100.0).is_some());
        let plugin = Device::new(
            DeviceId(2),
            "Plug-in",
            DeviceType::Custom("ext".to_string()),
        );
        assert!(create_device_processor(&plugin, 44100.0).is_none());
    }

    #[test]
    fn stored_parameter_values_reach_the_processor() {
        let mut utility = Device::new(DeviceId(1), "Utility", DeviceType::Utility);
        utility.set_param("mute", 1.0);
        let mut processor = create_device_processor(&utility, 44100.0).expect("utility");
        let mut left = [0.5f32; 4];
        let mut right = [0.5f32; 4];
        let mut io = crate::AudioIo {
            left: &mut left,
            right: &mut right,
        };
        processor.process(
            &crate::ProcessContext {
                sample_rate: 44100.0,
                frames: 4,
                bpm: 120.0,
                playhead_beats: 0.0,
                beats_per_frame: 0.0,
            },
            &mut io,
            &crate::EventList::default(),
            &mut crate::EventSink::default(),
        );
        assert_eq!(left, [0.0; 4]);
    }
}
