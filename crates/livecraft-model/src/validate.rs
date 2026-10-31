//! Referential integrity of a Live Set (M03 item 8).
//!
//! `Project::validate` checks that every id the Set refers to exists, that ids are unique, and
//! the structural rules of the manual: group parents are group tracks, groups nest without
//! cycles, group tracks hold no clips (manual 18.3, page 400), return and Main tracks hold no
//! clips because they cannot play them (manual 18.4, page 402), and every track has one send per
//! return track.

use crate::ids::{ClipId, DeviceId, LaneId, SceneId, TrackId};
use crate::model::*;
use crate::routing::{AudioInput, AudioOutput};
use std::collections::HashSet;
use std::hash::Hash;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ValidationError {
    #[error("track id {0} is used by more than one track")]
    DuplicateTrack(TrackId),
    #[error("scene id {0} is used by more than one scene")]
    DuplicateScene(SceneId),
    #[error("clip id {0} is used by more than one clip")]
    DuplicateClip(ClipId),
    #[error("device id {0} is used by more than one device")]
    DuplicateDevice(DeviceId),
    #[error("automation lane id {0} is used by more than one lane")]
    DuplicateLane(LaneId),
    #[error("track {track} is in group {parent}, which does not exist")]
    MissingParent { track: TrackId, parent: TrackId },
    #[error("track {track} is in {parent}, which is not a group track")]
    ParentNotGroup { track: TrackId, parent: TrackId },
    #[error("track {0} is its own ancestor through its group parents")]
    ParentCycle(TrackId),
    #[error("group track {0} holds clips, but group tracks hold no clips")]
    GroupHasClips(TrackId),
    #[error(
        "track {0} is a return or Main track, which cannot play clips, but it has clip slots or arrangement clips"
    )]
    ReturnOrMainHoldsClips(TrackId),
    #[error("track {track} has {sends} sends, but the Set has {returns} return tracks")]
    SendCount {
        track: TrackId,
        sends: usize,
        returns: usize,
    },
    #[error("track {track} routes audio to track {target}, which does not exist")]
    MissingRouteTarget { track: TrackId, target: TrackId },
    #[error("track {track} automates device {device}, which does not exist")]
    MissingAutomationDevice { track: TrackId, device: DeviceId },
}

impl Project {
    /// Every rule the Set must satisfy. An empty list means the Set is valid.
    pub fn validate(&self) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        let tracks: Vec<&Track> = self.all_tracks().collect();
        let returns = self.return_tracks.len();

        report_duplicates(
            tracks.iter().map(|track| track.id),
            ValidationError::DuplicateTrack,
            &mut errors,
        );
        report_duplicates(
            self.scenes.iter().map(|scene| scene.id),
            ValidationError::DuplicateScene,
            &mut errors,
        );
        let clips: Vec<&Clip> = tracks
            .iter()
            .flat_map(|track| {
                let slot_clips = track
                    .clip_slots
                    .iter()
                    .filter_map(|slot| slot.clip.as_ref());
                let arranged = track
                    .arrangement_clips
                    .iter()
                    .map(|arranged| &arranged.clip);
                slot_clips.chain(arranged)
            })
            .collect();
        report_duplicates(
            clips.iter().map(|clip| clip.id),
            ValidationError::DuplicateClip,
            &mut errors,
        );
        let lanes: Vec<LaneId> = tracks
            .iter()
            .flat_map(|track| track.automation_lanes.iter().map(|lane| lane.id))
            .chain(
                clips
                    .iter()
                    .flat_map(|clip| clip.automation_lanes.iter().map(|lane| lane.id)),
            )
            .collect();
        report_duplicates(lanes, ValidationError::DuplicateLane, &mut errors);
        let devices: Vec<DeviceId> = tracks
            .iter()
            .flat_map(|track| track.device_chain.iter().map(|device| device.id))
            .collect();
        report_duplicates(
            devices.iter().copied(),
            ValidationError::DuplicateDevice,
            &mut errors,
        );

        for track in &tracks {
            if let Some(parent) = track.parent {
                match self.track(parent) {
                    None => errors.push(ValidationError::MissingParent {
                        track: track.id,
                        parent,
                    }),
                    Some(group) if group.track_type != TrackType::Group => {
                        errors.push(ValidationError::ParentNotGroup {
                            track: track.id,
                            parent,
                        })
                    }
                    Some(_) if self.reaches(parent, track.id) => {
                        errors.push(ValidationError::ParentCycle(track.id))
                    }
                    Some(_) => {}
                }
            }
            let holds_clips = track.clip_slots.iter().any(|slot| slot.clip.is_some())
                || !track.arrangement_clips.is_empty();
            if track.track_type == TrackType::Group && holds_clips {
                errors.push(ValidationError::GroupHasClips(track.id));
            }
            let has_slots_or_arranged =
                !track.clip_slots.is_empty() || !track.arrangement_clips.is_empty();
            if matches!(track.track_type, TrackType::Return | TrackType::Main)
                && has_slots_or_arranged
            {
                errors.push(ValidationError::ReturnOrMainHoldsClips(track.id));
            }
            if track.sends.len() != returns {
                errors.push(ValidationError::SendCount {
                    track: track.id,
                    sends: track.sends.len(),
                    returns,
                });
            }
            if let AudioOutput::Track(target) = track.routing.audio_to
                && self.track(target).is_none()
            {
                errors.push(ValidationError::MissingRouteTarget {
                    track: track.id,
                    target,
                });
            }
            if let AudioInput::Track { track: target, .. } = track.routing.audio_from
                && self.track(target).is_none()
            {
                errors.push(ValidationError::MissingRouteTarget {
                    track: track.id,
                    target,
                });
            }
            for lane in &track.automation_lanes {
                if let AutomationTarget::DeviceParam { device_id, .. } = lane.target
                    && !devices.contains(&device_id)
                {
                    errors.push(ValidationError::MissingAutomationDevice {
                        track: track.id,
                        device: device_id,
                    });
                }
            }
        }
        errors
    }
}

/// Reports every id that appears more than once.
fn report_duplicates<T: Copy + Eq + Hash>(
    ids: impl IntoIterator<Item = T>,
    report: impl Fn(T) -> ValidationError,
    errors: &mut Vec<ValidationError>,
) {
    let mut seen = HashSet::new();
    for id in ids {
        if !seen.insert(id) {
            errors.push(report(id));
        }
    }
}
