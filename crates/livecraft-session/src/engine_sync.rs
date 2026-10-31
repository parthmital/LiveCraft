//! Sends the changes to a Set to the audio engine (M05 item 12, TARGET-ARCHITECTURE "Engine Decisions").
//!
//! A change to the structure of the Set compiles and submits a new graph. A change to a mixer level,
//! a send, the tempo, or a device parameter is a command, so the engine ramps it without rebuilding the
//! graph. A parameter that changes its device's latency recompiles, because the latency compensation
//! (manual 23.3) is worked out when a graph is compiled.

use livecraft_engine::executor::EngineCommand;
use livecraft_engine::output::Playback;
use livecraft_model::model::{Device, Project};
use livecraft_model::param::ParamId;

/// What a change to the Set did to the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Push {
    /// The change was sent as commands, and the running graph was kept.
    Commands,
    /// A new graph was compiled and submitted.
    Recompiled,
}

/// Pushes the difference between two states of the Set to the engine.
pub(crate) fn push_changes(playback: &mut Playback, before: &Project, after: &Project) -> Push {
    if !same_shape(before, after) || latency_changed(playback, before, after) {
        match playback
            .handle
            .submit_graph(playback.compiler.compile(after))
        {
            Ok(()) => {}
            Err(error) => {
                log::warn!(
                    "The new graph was not submitted, so the engine keeps the old one: {error}"
                )
            }
        }
        return Push::Recompiled;
    }
    send_values(playback, before, after);
    Push::Commands
}

/// Sends a command to the engine. A full queue is logged, and the engine keeps its old value.
pub(crate) fn send_command(playback: &mut Playback, command: EngineCommand) {
    if playback.handle.send(command).is_err() {
        log::warn!("The engine command queue is full, so {command:?} was not applied");
    }
}

/// Sends the mixer levels, sends, tempo, and device parameters that changed.
fn send_values(playback: &mut Playback, before: &Project, after: &Project) {
    if before.transport.bpm() != after.transport.bpm() {
        send_command(playback, EngineCommand::SetTempo(after.transport.bpm()));
    }
    for (old, new) in before.all_tracks().zip(after.all_tracks()) {
        let track = new.id;
        if old.volume_db != new.volume_db {
            send_command(
                playback,
                EngineCommand::SetTrackVolume {
                    track,
                    volume_db: new.volume_db,
                },
            );
        }
        if old.pan != new.pan {
            send_command(
                playback,
                EngineCommand::SetTrackPan {
                    track,
                    pan: new.pan,
                },
            );
        }
        if old.is_muted != new.is_muted {
            send_command(
                playback,
                EngineCommand::SetTrackMute {
                    track,
                    muted: new.is_muted,
                },
            );
        }
        if old.is_soloed != new.is_soloed {
            send_command(
                playback,
                EngineCommand::SetTrackSolo {
                    track,
                    soloed: new.is_soloed,
                },
            );
        }
        for (send, (old_level, new_level)) in old.sends.iter().zip(&new.sends).enumerate() {
            if old_level != new_level {
                send_command(
                    playback,
                    EngineCommand::SetTrackSend {
                        track,
                        send,
                        amount: *new_level,
                    },
                );
            }
        }
        for (old_device, new_device) in old.device_chain.iter().zip(&new.device_chain) {
            for (param, value) in changed_parameters(old_device, new_device) {
                send_command(
                    playback,
                    EngineCommand::SetDeviceParam {
                        track,
                        device: new_device.id,
                        param,
                        value,
                        offset: 0,
                    },
                );
            }
        }
    }
}

/// True when a device whose parameters changed now has a different latency from the last graph's.
fn latency_changed(playback: &Playback, before: &Project, after: &Project) -> bool {
    before
        .all_tracks()
        .zip(after.all_tracks())
        .any(|(old, new)| {
            old.device_chain
                .iter()
                .zip(&new.device_chain)
                .any(|(old_device, new_device)| {
                    changed_parameters(old_device, new_device).next().is_some()
                        && playback.compiler.latency_changed(new_device)
                })
        })
}

/// The parameters of `new` whose values differ from those of `old`, with their new values.
fn changed_parameters<'a>(
    old: &'a Device,
    new: &'a Device,
) -> impl Iterator<Item = (ParamId, f32)> + 'a {
    new.parameters
        .iter()
        .filter(move |&(param, value)| old.parameters.get(param) != Some(value))
}

/// True when two Sets differ only in values the engine takes as commands: mixer levels and sends,
/// device parameters, and the transport. Anything else, such as a clip, a device, or a track, changes
/// the graph.
fn same_shape(before: &Project, after: &Project) -> bool {
    matches!((shape(before), shape(after)), (Some(a), Some(b)) if a == b)
}

/// The Set as JSON with those values cleared. None when it cannot be serialised, which counts as a
/// change of shape.
fn shape(project: &Project) -> Option<serde_json::Value> {
    let mut shape = project.clone();
    shape.transport = Default::default();
    for track in shape
        .tracks
        .iter_mut()
        .chain(shape.return_tracks.iter_mut())
        .chain(std::iter::once(&mut shape.main_track))
    {
        track.volume_db = 0.0;
        track.pan = 0.0;
        track.is_muted = false;
        track.is_soloed = false;
        track.sends.iter_mut().for_each(|level| *level = 0.0);
        for device in &mut track.device_chain {
            device.parameters = Default::default();
        }
    }
    serde_json::to_value(&shape).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use livecraft_devices::{AudioIo, DeviceProcessor, EventList, EventSink, ProcessContext};
    use livecraft_model::ids::DeviceId;
    use livecraft_model::model::{Clip, DeviceType, MidiNote};

    /// A processor that delays nothing but reports the frames in the first parameter as its latency,
    /// so a parameter change can change the latency.
    struct Latent(u32);

    impl DeviceProcessor for Latent {
        fn process(
            &mut self,
            _: &ProcessContext,
            _: &mut AudioIo<'_>,
            _: &EventList,
            _: &mut EventSink,
        ) {
        }

        fn set_param(&mut self, _: ParamId, _: f32, _: u32) {}

        fn latency_samples(&self) -> u32 {
            self.0
        }

        fn reset(&mut self) {}
    }

    /// The processor factory of the test: a Utility device reports its first parameter as latency.
    fn latent(device: &Device, sample_rate: f32) -> Option<Box<dyn DeviceProcessor>> {
        if device.device_type == DeviceType::Utility {
            let frames = device.parameters.get(ParamId(0)).unwrap_or(0.0);
            Some(Box::new(Latent(frames as u32)))
        } else {
            livecraft_devices::create_device_processor(device, sample_rate)
        }
    }

    /// A Set whose first track has a Utility device with the given latency and a second parameter.
    fn set_with_utility(latency: f32) -> Project {
        let mut project = Project::new_default("Sync");
        let mut utility = Device::new(DeviceId(500), "Utility", DeviceType::Utility);
        utility.parameters.set(ParamId(0), latency);
        utility.parameters.set(ParamId(1), 1.0);
        project.tracks[0].device_chain.push(utility);
        project
    }

    fn playback(project: &Project) -> Playback {
        let (playback, _engine) =
            Playback::start_with(project, 44100.0, latent).expect("start the engine");
        playback
    }

    #[test]
    fn a_mixer_change_is_sent_without_a_recompile() {
        let before = Project::new_default("Mixer");
        let mut after = before.clone();
        after.tracks[0].volume_db = -6.0;
        after.tracks[0].is_muted = true;
        assert_eq!(
            push_changes(&mut playback(&before), &before, &after),
            Push::Commands
        );
    }

    #[test]
    fn a_tempo_or_playhead_change_is_sent_without_a_recompile() {
        let before = Project::new_default("Transport");
        let mut after = before.clone();
        after.transport.set_bpm(150.0);
        after.transport.playhead_beats = 8.0;
        assert_eq!(
            push_changes(&mut playback(&before), &before, &after),
            Push::Commands
        );
    }

    #[test]
    fn a_parameter_that_keeps_the_latency_is_sent_without_a_recompile() {
        let before = set_with_utility(0.0);
        let mut after = before.clone();
        after.tracks[0].device_chain[1]
            .parameters
            .set(ParamId(1), 0.25);
        assert_eq!(
            push_changes(&mut playback(&before), &before, &after),
            Push::Commands
        );
    }

    #[test]
    fn a_parameter_that_changes_the_latency_recompiles_the_graph() {
        let before = set_with_utility(0.0);
        let mut after = before.clone();
        after.tracks[0].device_chain[1]
            .parameters
            .set(ParamId(0), 128.0);
        assert_eq!(
            push_changes(&mut playback(&before), &before, &after),
            Push::Recompiled
        );
    }

    #[test]
    fn a_change_of_clips_or_devices_recompiles_the_graph() {
        let before = Project::new_default("Structure");
        let mut with_clip = before.clone();
        let mut clip = Clip::new_midi(900, "Chord", 4.0);
        if let livecraft_model::model::ClipPayload::Midi(ref mut data) = clip.payload {
            data.notes.push(MidiNote::new(60, 0.0, 2.0, 100));
        }
        with_clip.tracks[0].clip_slots[0].clip = Some(clip);
        assert_eq!(
            push_changes(&mut playback(&before), &before, &with_clip),
            Push::Recompiled
        );

        let mut with_device = before.clone();
        with_device.tracks[0].device_chain.push(Device::new(
            DeviceId(501),
            "Utility",
            DeviceType::Utility,
        ));
        assert_eq!(
            push_changes(&mut playback(&before), &before, &with_device),
            Push::Recompiled
        );
    }
}
