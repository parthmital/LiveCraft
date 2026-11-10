//! Project-level operations: sample reference replacement and prepared clip/device insertion (M10).

use crate::command::{CommandError, Operation};
use crate::commands::finite;
use crate::registry::CommandRegistry;
use livecraft_model::clip::{ArrangementClip, Clip};
use livecraft_model::device::Device;
use livecraft_model::edit::EditError;
use livecraft_model::ids::{ClipId, DeviceId, IdAllocator, LaneId, TrackId};
use livecraft_model::model::{ClipPayload, Project, TrackType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub fn register(registry: &mut CommandRegistry) {
    registry.register::<ReplaceSampleReference>();
}

fn all_clips_mut(project: &mut Project) -> impl Iterator<Item = &mut Clip> {
    project.all_tracks_mut().flat_map(|track| {
        track
            .clip_slots
            .iter_mut()
            .filter_map(|slot| slot.clip.as_mut())
            .chain(track.arrangement_clips.iter_mut().map(|arr| &mut arr.clip))
    })
}

fn locate_slot_clip(project: &Project, clip_id: ClipId) -> Option<(TrackId, usize)> {
    project.all_tracks().find_map(|track| {
        track
            .clip_slots
            .iter()
            .position(|slot| slot.clip.as_ref().is_some_and(|c| c.id == clip_id))
            .map(|slot| (track.id, slot))
    })
}

fn clip_id_in_use(project: &Project, id: ClipId) -> bool {
    project.all_tracks().any(|track| {
        track
            .clip_slots
            .iter()
            .filter_map(|slot| slot.clip.as_ref())
            .chain(track.arrangement_clips.iter().map(|arr| &arr.clip))
            .any(|clip| clip.id == id)
    })
}

/// Replaces matching `AudioClipData.file_path` or `relative_path` values across the Set.
#[derive(Debug, Serialize, Deserialize)]
pub struct ReplaceSampleReference {
    pub old_ref: String,
    pub new_path: String,
    #[serde(default)]
    pub new_relative_path: Option<String>,
    #[serde(skip)]
    previous: Vec<(ClipId, String, Option<String>)>,
    #[serde(skip)]
    previous_device_refs: Vec<(TrackId, DeviceId, usize, String, Option<String>)>,
}

impl ReplaceSampleReference {
    pub fn new(
        old_ref: impl Into<String>,
        new_path: impl Into<String>,
        new_relative_path: Option<String>,
    ) -> Self {
        Self {
            old_ref: old_ref.into(),
            new_path: new_path.into(),
            new_relative_path,
            previous: Vec::new(),
            previous_device_refs: Vec::new(),
        }
    }
}

impl Operation for ReplaceSampleReference {
    const NAME: &'static str = "project.replace_sample_reference";
    const LABEL: &'static str = "Replace Sample Reference";

    fn perform(
        &mut self,
        project: &mut Project,
        _ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        self.previous.clear();
        self.previous_device_refs.clear();
        if self.old_ref.trim().is_empty() {
            return Ok(());
        }
        for clip in all_clips_mut(project) {
            if let ClipPayload::Audio(ref mut audio) = clip.payload
                && (audio.file_path == self.old_ref
                    || audio.relative_path.as_deref() == Some(self.old_ref.as_str()))
            {
                self.previous.push((
                    clip.id,
                    audio.file_path.clone(),
                    audio.relative_path.clone(),
                ));
                audio.file_path = self.new_path.clone();
                audio.relative_path = self.new_relative_path.clone();
            }
        }
        for track in project.all_tracks_mut() {
            for device in &mut track.device_chain {
                for (idx, sref) in device.sample_refs.iter_mut().enumerate() {
                    if sref.path == self.old_ref
                        || sref.relative_path.as_deref() == Some(self.old_ref.as_str())
                    {
                        self.previous_device_refs.push((
                            track.id,
                            device.id,
                            idx,
                            sref.path.clone(),
                            sref.relative_path.clone(),
                        ));
                        sref.path = self.new_path.clone();
                        sref.relative_path = self.new_relative_path.clone();
                    }
                }
            }
        }
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        let previous = std::mem::take(&mut self.previous);
        if !previous.is_empty() {
            let mut by_id: HashMap<ClipId, (String, Option<String>)> = previous
                .into_iter()
                .map(|(id, path, rel)| (id, (path, rel)))
                .collect();
            for clip in all_clips_mut(project) {
                if let Some((old_path, old_rel)) = by_id.remove(&clip.id)
                    && let ClipPayload::Audio(ref mut audio) = clip.payload
                {
                    audio.file_path = old_path;
                    audio.relative_path = old_rel;
                }
            }
        }
        let previous_devs = std::mem::take(&mut self.previous_device_refs);
        for (track_id, device_id, idx, old_path, old_rel) in previous_devs {
            if let Some(track) = project.track_mut(track_id)
                && let Some(device) = track.device_chain.iter_mut().find(|d| d.id == device_id)
                && let Some(sref) = device.sample_refs.get_mut(idx)
            {
                sref.path = old_path;
                sref.relative_path = old_rel;
            }
        }
    }
}

/// Inserts a fully populated [`Clip`] into a Session slot, allocating fresh IDs via [`IdAllocator`].
#[derive(Debug, Serialize, Deserialize)]
pub struct InsertPreparedClip {
    pub track: TrackId,
    pub slot: usize,
    pub clip: Clip,
    #[serde(skip)]
    created: Option<ClipId>,
    #[serde(skip)]
    lanes: Option<Vec<LaneId>>,
}

impl InsertPreparedClip {
    pub fn new(track: TrackId, slot: usize, clip: Clip) -> Self {
        Self {
            track,
            slot,
            clip,
            created: None,
            lanes: None,
        }
    }
}

impl Operation for InsertPreparedClip {
    const NAME: &'static str = "project.insert_prepared_clip";
    const LABEL: &'static str = "Insert Clip";

    fn perform(
        &mut self,
        project: &mut Project,
        ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        finite("clip length", self.clip.length_beats)?;
        if self.clip.length_beats <= 0.0 {
            return Err(CommandError::NotPositive {
                what: "clip length",
                value: self.clip.length_beats,
            });
        }
        let owner = project
            .track(self.track)
            .ok_or(EditError::NoSuchTrack(self.track))?;
        match owner.track_type {
            TrackType::Group => return Err(EditError::GroupHoldsClips.into()),
            TrackType::Return | TrackType::Main => return Err(EditError::ReturnHoldsClips.into()),
            TrackType::Audio | TrackType::Midi => {}
        }
        let slot = owner
            .clip_slots
            .get(self.slot)
            .ok_or(EditError::NoSuchSlot {
                track: self.track,
                slot: self.slot,
            })?;
        if slot.clip.is_some() {
            return Err(CommandError::SlotOccupied {
                track: self.track,
                slot: self.slot,
            });
        }

        let id = *self.created.get_or_insert_with(|| ids.clip());
        let lanes = self.lanes.get_or_insert_with(|| {
            self.clip
                .automation_lanes
                .iter()
                .map(|_| ids.lane())
                .collect()
        });
        let mut clip = self.clip.clone();
        clip.id = id;
        for (lane, lane_id) in clip.automation_lanes.iter_mut().zip(lanes.iter()) {
            lane.id = *lane_id;
        }
        project.set_clip(self.track, self.slot, clip)?;
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        if let Some(clip_id) = self.created
            && let Some((track, slot)) = locate_slot_clip(project, clip_id)
        {
            let _ = project.clear_clip(track, slot);
        }
    }
}

/// Inserts a fully populated [`ArrangementClip`] onto a track, allocating fresh IDs via [`IdAllocator`].
#[derive(Debug, Serialize, Deserialize)]
pub struct InsertPreparedArrangementClip {
    pub track: TrackId,
    pub arrangement_clip: ArrangementClip,
    #[serde(skip)]
    created: Option<ClipId>,
    #[serde(skip)]
    lanes: Option<Vec<LaneId>>,
}

impl InsertPreparedArrangementClip {
    pub fn new(track: TrackId, arrangement_clip: ArrangementClip) -> Self {
        Self {
            track,
            arrangement_clip,
            created: None,
            lanes: None,
        }
    }
}

impl Operation for InsertPreparedArrangementClip {
    const NAME: &'static str = "project.insert_prepared_arrangement_clip";
    const LABEL: &'static str = "Insert Arrangement Clip";

    fn perform(
        &mut self,
        project: &mut Project,
        ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        finite("clip start", self.arrangement_clip.start_beat)?;
        if self.arrangement_clip.start_beat < 0.0 {
            return Err(CommandError::OutOfRange {
                what: "clip start",
                min: 0.0,
                max: f64::MAX,
                value: self.arrangement_clip.start_beat,
            });
        }
        finite("clip length", self.arrangement_clip.length_beats)?;
        if self.arrangement_clip.length_beats <= 0.0 {
            return Err(CommandError::NotPositive {
                what: "clip length",
                value: self.arrangement_clip.length_beats,
            });
        }
        finite(
            "clip payload length",
            self.arrangement_clip.clip.length_beats,
        )?;
        if self.arrangement_clip.clip.length_beats <= 0.0 {
            return Err(CommandError::NotPositive {
                what: "clip payload length",
                value: self.arrangement_clip.clip.length_beats,
            });
        }

        let owner = project
            .track(self.track)
            .ok_or(EditError::NoSuchTrack(self.track))?;
        match owner.track_type {
            TrackType::Group => return Err(EditError::GroupHoldsClips.into()),
            TrackType::Return | TrackType::Main => return Err(EditError::ReturnHoldsClips.into()),
            TrackType::Audio | TrackType::Midi => {}
        }

        let id = *self.created.get_or_insert_with(|| ids.clip());
        if clip_id_in_use(project, id) {
            return Err(EditError::IdInUse(id.0).into());
        }
        let lanes = self.lanes.get_or_insert_with(|| {
            self.arrangement_clip
                .clip
                .automation_lanes
                .iter()
                .map(|_| ids.lane())
                .collect()
        });
        let mut arr = self.arrangement_clip.clone();
        arr.id = id;
        arr.clip.id = id;
        for (lane, lane_id) in arr.clip.automation_lanes.iter_mut().zip(lanes.iter()) {
            lane.id = *lane_id;
        }
        project
            .track_mut(self.track)
            .ok_or(EditError::NoSuchTrack(self.track))?
            .arrangement_clips
            .push(arr);
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        if let Some(clip_id) = self.created {
            for track in project.all_tracks_mut() {
                track
                    .arrangement_clips
                    .retain(|arr| arr.id != clip_id && arr.clip.id != clip_id);
            }
        }
    }
}

/// Inserts a fully populated [`Device`] onto a track's device chain, allocating a fresh [`DeviceId`].
#[derive(Debug, Serialize, Deserialize)]
pub struct InsertPreparedDevice {
    pub track: TrackId,
    pub device: Device,
    #[serde(skip)]
    created: Option<DeviceId>,
}

impl InsertPreparedDevice {
    pub fn new(track: TrackId, device: Device) -> Self {
        Self {
            track,
            device,
            created: None,
        }
    }
}

impl Operation for InsertPreparedDevice {
    const NAME: &'static str = "project.insert_prepared_device";
    const LABEL: &'static str = "Insert Device";

    fn perform(
        &mut self,
        project: &mut Project,
        ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        if project.track(self.track).is_none() {
            return Err(EditError::NoSuchTrack(self.track).into());
        }
        let id = *self.created.get_or_insert_with(|| ids.device());
        let mut device = self.device.clone();
        device.id = id;
        project.add_device(self.track, device)?;
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        if let Some(device_id) = self.created {
            let _ = project.remove_device(self.track, device_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::UndoHistory;
    use livecraft_model::automation::{AutomationLane, AutomationTarget};
    use livecraft_model::device::{DeviceSampleRef, DeviceType};
    use livecraft_model::model::{MidiNote, WarpMarker};
    use std::time::Instant;

    #[test]
    fn replace_sample_reference_updates_slots_and_arrangement_and_undoes_exactly() {
        let mut project = Project::new_default("ReplaceSample");
        let mut ids = project.id_allocator();
        let mut history = UndoHistory::default();

        let mut c1 = Clip::new_audio(ids.clip(), "SlotKick", "C:/missing/kick.wav", 4.0);
        if let ClipPayload::Audio(ref mut audio) = c1.payload {
            audio.relative_path = Some("Samples/Old/kick.wav".to_owned());
        }
        project.set_clip(TrackId(3), 0, c1).expect("set slot clip");

        let mut c2 = Clip::new_audio(ids.clip(), "ArrKick", "", 4.0);
        if let ClipPayload::Audio(ref mut audio) = c2.payload {
            audio.relative_path = Some("C:/missing/kick.wav".to_owned());
        }
        let arr_id = c2.id;
        project
            .track_mut(TrackId(4))
            .expect("track 4")
            .arrangement_clips
            .push(ArrangementClip::new(arr_id, c2, 8.0, 4.0));

        let cmd = ReplaceSampleReference::new(
            "C:/missing/kick.wav",
            "D:/Proj/Samples/Imported/kick.wav",
            Some("Samples/Imported/kick.wav".to_owned()),
        );
        history
            .execute(Box::new(cmd), &mut project, &mut ids, Instant::now())
            .expect("replace sample ref");

        let slot_audio = match &project.track(TrackId(3)).expect("t3").clip_slots[0]
            .clip
            .as_ref()
            .expect("c1")
            .payload
        {
            ClipPayload::Audio(a) => a,
            ClipPayload::Midi(_) => panic!("expected audio"),
        };
        assert_eq!(slot_audio.file_path, "D:/Proj/Samples/Imported/kick.wav");
        assert_eq!(
            slot_audio.relative_path.as_deref(),
            Some("Samples/Imported/kick.wav")
        );

        let arr_audio = match &project.track(TrackId(4)).expect("t4").arrangement_clips[0]
            .clip
            .payload
        {
            ClipPayload::Audio(a) => a,
            ClipPayload::Midi(_) => panic!("expected audio"),
        };
        assert_eq!(arr_audio.file_path, "D:/Proj/Samples/Imported/kick.wav");
        assert_eq!(
            arr_audio.relative_path.as_deref(),
            Some("Samples/Imported/kick.wav")
        );

        assert!(history.undo(&mut project));
        let reverted_slot = match &project.track(TrackId(3)).expect("t3").clip_slots[0]
            .clip
            .as_ref()
            .expect("c1")
            .payload
        {
            ClipPayload::Audio(a) => a,
            ClipPayload::Midi(_) => panic!("expected audio"),
        };
        assert_eq!(reverted_slot.file_path, "C:/missing/kick.wav");
        assert_eq!(
            reverted_slot.relative_path.as_deref(),
            Some("Samples/Old/kick.wav")
        );

        let reverted_arr = match &project.track(TrackId(4)).expect("t4").arrangement_clips[0]
            .clip
            .payload
        {
            ClipPayload::Audio(a) => a,
            ClipPayload::Midi(_) => panic!("expected audio"),
        };
        assert_eq!(reverted_arr.file_path, "");
        assert_eq!(
            reverted_arr.relative_path.as_deref(),
            Some("C:/missing/kick.wav")
        );

        assert!(history.redo(&mut project, &mut ids).expect("redo"));
        let redone_slot = match &project.track(TrackId(3)).expect("t3").clip_slots[0]
            .clip
            .as_ref()
            .expect("c1")
            .payload
        {
            ClipPayload::Audio(a) => a,
            ClipPayload::Midi(_) => panic!("expected audio"),
        };
        assert_eq!(redone_slot.file_path, "D:/Proj/Samples/Imported/kick.wav");
    }

    #[test]
    fn insert_prepared_clip_arrangement_and_device_allocate_ids_and_undo_redo() {
        let mut project = Project::new_default("Prepared");
        let mut ids = project.id_allocator();
        let mut history = UndoHistory::default();

        let mut clip = Clip::new_midi(ClipId(1), "ChordClip", 4.0);
        if let ClipPayload::Midi(ref mut m) = clip.payload {
            m.notes.push(MidiNote::new(60, 0.0, 1.0, 100));
        }
        clip.automation_lanes.push(AutomationLane::new(
            LaneId(1),
            AutomationTarget::TrackVolume,
        ));

        history
            .execute(
                Box::new(InsertPreparedClip::new(TrackId(1), 0, clip)),
                &mut project,
                &mut ids,
                Instant::now(),
            )
            .expect("insert slot clip");
        let slot_clip_id = project.track(TrackId(1)).expect("t1").clip_slots[0]
            .clip
            .as_ref()
            .expect("clip")
            .id;
        assert_ne!(slot_clip_id, ClipId(1));

        let mut audio_clip = Clip::new_audio(ClipId(1), "WarpAudio", "a.wav", 4.0);
        if let ClipPayload::Audio(ref mut a) = audio_clip.payload {
            a.warp_markers.push(WarpMarker {
                sample_time_sec: 0.5,
                beat_time: 1.0,
            });
        }
        let arr = ArrangementClip::new(ClipId(1), audio_clip, 4.0, 4.0);
        history
            .execute(
                Box::new(InsertPreparedArrangementClip::new(TrackId(3), arr)),
                &mut project,
                &mut ids,
                Instant::now(),
            )
            .expect("insert arr clip");
        let arr_clip_id = project.track(TrackId(3)).expect("t3").arrangement_clips[0]
            .clip
            .id;
        assert_ne!(arr_clip_id, slot_clip_id);

        let mut dev = Device::new(DeviceId(1), "OneShot", DeviceType::Simpler);
        dev.set_param("gain_db", -3.0);
        dev.sample_refs.push(DeviceSampleRef {
            path: "snare.wav".to_owned(),
            relative_path: Some("Samples/Imported/snare.wav".to_owned()),
        });
        history
            .execute(
                Box::new(InsertPreparedDevice::new(TrackId(1), dev)),
                &mut project,
                &mut ids,
                Instant::now(),
            )
            .expect("insert device");
        let inserted_dev = project
            .track(TrackId(1))
            .expect("t1")
            .device_chain
            .last()
            .expect("inserted dev");
        let dev_id = inserted_dev.id;
        assert_ne!(dev_id, DeviceId(1));
        assert_eq!(inserted_dev.sample_refs.len(), 1);
        assert!((inserted_dev.get_param("gain_db") - (-3.0)).abs() < 1e-4);

        assert!(history.undo(&mut project));
        assert_eq!(project.track(TrackId(1)).expect("t1").device_chain.len(), 1);
        assert!(history.undo(&mut project));
        assert!(
            project
                .track(TrackId(3))
                .expect("t3")
                .arrangement_clips
                .is_empty()
        );
        assert!(history.undo(&mut project));
        assert!(
            project.track(TrackId(1)).expect("t1").clip_slots[0]
                .clip
                .is_none()
        );

        assert!(history.redo(&mut project, &mut ids).expect("redo clip"));
        assert_eq!(
            project.track(TrackId(1)).expect("t1").clip_slots[0]
                .clip
                .as_ref()
                .expect("clip")
                .id,
            slot_clip_id
        );
        assert!(history.redo(&mut project, &mut ids).expect("redo arr"));
        assert_eq!(
            project.track(TrackId(3)).expect("t3").arrangement_clips[0]
                .clip
                .id,
            arr_clip_id
        );
        assert!(history.redo(&mut project, &mut ids).expect("redo dev"));
        assert_eq!(
            project
                .track(TrackId(1))
                .expect("t1")
                .device_chain
                .last()
                .expect("dev")
                .id,
            dev_id
        );
    }
}
