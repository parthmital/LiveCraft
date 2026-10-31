//! Edit operations on a Live Set (M03 item 8).
//!
//! Each operation checks its preconditions before it changes anything, so an edit that succeeds
//! leaves the Set valid, and a rejected edit leaves it as it was. Commands (M04) wrap these with
//! undo.

use crate::automation::AutomationTarget;
use crate::clip::Clip;
use crate::device::Device;
use crate::ids::{ClipId, DeviceId, SceneId, TrackId};
use crate::model::*;
use crate::routing::{AudioInput, AudioOutput, Routing};

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum EditError {
    #[error("no track with id {0}")]
    NoSuchTrack(TrackId),
    #[error("no scene with id {0}")]
    NoSuchScene(SceneId),
    #[error("track {track} has no device {device}")]
    NoSuchDevice { track: TrackId, device: DeviceId },
    #[error("track {track} has no clip slot {slot}")]
    NoSuchSlot { track: TrackId, slot: usize },
    #[error("id {0} is already used in the Set")]
    IdInUse(u64),
    #[error("track {0} is not a group track")]
    NotGroup(TrackId),
    #[error("track {0} would become its own ancestor")]
    Cycle(TrackId),
    #[error("group tracks hold no clips")]
    GroupHoldsClips,
    #[error("the Main track cannot be removed")]
    MainTrackFixed,
    #[error("the track cannot route to track {0}, which is not another track in the Set")]
    NoSuchRouteTarget(TrackId),
    #[error("a track of type {0:?} cannot be added with this operation")]
    FixedTrackKind(TrackType),
    #[error("the track has {sends} sends for {returns} return tracks")]
    SendCount { sends: usize, returns: usize },
}

impl Project {
    /// Adds a track, which must have a new id. Its group parent must be a group track, and it must
    /// have one send per return track. Return and Main tracks are created with the Set, not added.
    pub fn add_track(&mut self, track: Track) -> Result<(), EditError> {
        if self.track(track.id).is_some() {
            return Err(EditError::IdInUse(track.id.0));
        }
        if matches!(track.track_type, TrackType::Return | TrackType::Main) {
            return Err(EditError::FixedTrackKind(track.track_type));
        }
        if let Some(parent) = track.parent {
            match self.track(parent) {
                None => return Err(EditError::NoSuchTrack(parent)),
                Some(group) if group.track_type != TrackType::Group => {
                    return Err(EditError::NotGroup(parent));
                }
                Some(_) => {}
            }
        }
        if track.track_type == TrackType::Group
            && (track.clip_slots.iter().any(|slot| slot.clip.is_some())
                || !track.arrangement_clips.is_empty())
        {
            return Err(EditError::GroupHoldsClips);
        }
        let returns = self.return_tracks.len();
        if track.sends.len() != returns {
            return Err(EditError::SendCount {
                sends: track.sends.len(),
                returns,
            });
        }
        self.tracks.push(track);
        Ok(())
    }

    /// Removes a track, which is not the Main track. Its group children move up to its own group
    /// parent, and routes to it fall back to Main or to no input. A removed return track also
    /// takes its send slot out of every track.
    pub fn remove_track(&mut self, id: TrackId) -> Result<(), EditError> {
        if self.main_track.id == id {
            return Err(EditError::MainTrackFixed);
        }
        let parent = self.track(id).ok_or(EditError::NoSuchTrack(id))?.parent;

        if let Some(index) = self.return_tracks.iter().position(|track| track.id == id) {
            self.return_tracks.remove(index);
            for track in self.all_tracks_mut() {
                if index < track.sends.len() {
                    track.sends.remove(index);
                }
            }
        } else {
            self.tracks.retain(|track| track.id != id);
        }

        for track in self.all_tracks_mut() {
            if track.parent == Some(id) {
                track.parent = parent;
            }
            if track.routing.audio_to == AudioOutput::Track(id) {
                track.routing.audio_to = AudioOutput::Main;
            }
            if matches!(track.routing.audio_from, AudioInput::Track { track: from, .. } if from == id)
            {
                track.routing.audio_from = AudioInput::None;
            }
        }
        Ok(())
    }

    /// Puts a track in a group, or takes it out of its group with `None`. The group must be a group
    /// track, and it cannot be the track itself or one of the track's own group children.
    pub fn set_parent(&mut self, track: TrackId, parent: Option<TrackId>) -> Result<(), EditError> {
        self.track(track).ok_or(EditError::NoSuchTrack(track))?;
        if let Some(parent) = parent {
            match self.track(parent) {
                None => return Err(EditError::NoSuchTrack(parent)),
                Some(group) if group.track_type != TrackType::Group => {
                    return Err(EditError::NotGroup(parent));
                }
                Some(_) => {}
            }
            if self.reaches(parent, track) {
                return Err(EditError::Cycle(track));
            }
        }
        if let Some(target) = self.track_mut(track) {
            target.parent = parent;
        }
        Ok(())
    }

    /// Adds a scene, which must have a new id.
    pub fn add_scene(&mut self, scene: Scene) -> Result<(), EditError> {
        if self.scenes.iter().any(|existing| existing.id == scene.id) {
            return Err(EditError::IdInUse(scene.id.0));
        }
        self.scenes.push(scene);
        Ok(())
    }

    /// Removes a scene.
    pub fn remove_scene(&mut self, id: SceneId) -> Result<(), EditError> {
        let row = self.scene_row(id).ok_or(EditError::NoSuchScene(id))?;
        self.scenes.remove(row);
        Ok(())
    }

    /// Adds a device to the end of a track's device chain. The device id must be new in the Set.
    pub fn add_device(&mut self, track: TrackId, device: Device) -> Result<(), EditError> {
        if self
            .all_tracks()
            .any(|other| other.device_chain.iter().any(|known| known.id == device.id))
        {
            return Err(EditError::IdInUse(device.id.0));
        }
        self.track_mut(track)
            .ok_or(EditError::NoSuchTrack(track))?
            .device_chain
            .push(device);
        Ok(())
    }

    /// Removes a device from a track, with the automation that targets it.
    pub fn remove_device(&mut self, track: TrackId, device: DeviceId) -> Result<(), EditError> {
        let chain = &mut self
            .track_mut(track)
            .ok_or(EditError::NoSuchTrack(track))?
            .device_chain;
        let index = chain
            .iter()
            .position(|known| known.id == device)
            .ok_or(EditError::NoSuchDevice { track, device })?;
        chain.remove(index);
        for other in self.all_tracks_mut() {
            other.automation_lanes.retain(|lane| {
                !matches!(
                    lane.target,
                    AutomationTarget::DeviceParam { device_id, .. } if device_id == device
                )
            });
        }
        Ok(())
    }

    /// Puts a clip in a clip slot of a track. Group tracks hold no clips, and the clip id must be
    /// new in the Set.
    pub fn set_clip(&mut self, track: TrackId, slot: usize, clip: Clip) -> Result<(), EditError> {
        let kind = self
            .track(track)
            .ok_or(EditError::NoSuchTrack(track))?
            .track_type;
        if kind == TrackType::Group {
            return Err(EditError::GroupHoldsClips);
        }
        if self.clip_id_in_use(clip.id) {
            return Err(EditError::IdInUse(clip.id.0));
        }
        let target = self.track_mut(track).ok_or(EditError::NoSuchTrack(track))?;
        let clip_slot = target
            .clip_slots
            .get_mut(slot)
            .ok_or(EditError::NoSuchSlot { track, slot })?;
        clip_slot.clip = Some(clip);
        Ok(())
    }

    /// Empties a clip slot.
    pub fn clear_clip(&mut self, track: TrackId, slot: usize) -> Result<(), EditError> {
        let target = self.track_mut(track).ok_or(EditError::NoSuchTrack(track))?;
        let clip_slot = target
            .clip_slots
            .get_mut(slot)
            .ok_or(EditError::NoSuchSlot { track, slot })?;
        clip_slot.clip = None;
        Ok(())
    }

    /// Sets a track's routing. Any track it routes to or from must be another track in the Set.
    pub fn set_route(&mut self, track: TrackId, routing: Routing) -> Result<(), EditError> {
        self.track(track).ok_or(EditError::NoSuchTrack(track))?;
        let output = match routing.audio_to {
            AudioOutput::Track(target) => Some(target),
            AudioOutput::Main => None,
        };
        let input = match routing.audio_from {
            AudioInput::Track { track: source, .. } => Some(source),
            _ => None,
        };
        for target in output.into_iter().chain(input) {
            if target == track || self.track(target).is_none() {
                return Err(EditError::NoSuchRouteTarget(target));
            }
        }
        if let Some(target) = self.track_mut(track) {
            target.routing = routing;
        }
        Ok(())
    }

    fn clip_id_in_use(&self, id: ClipId) -> bool {
        self.all_tracks().any(|track| {
            track
                .clip_slots
                .iter()
                .filter_map(|slot| slot.clip.as_ref())
                .chain(
                    track
                        .arrangement_clips
                        .iter()
                        .map(|arranged| &arranged.clip),
                )
                .any(|clip| clip.id == id)
        })
    }
}
