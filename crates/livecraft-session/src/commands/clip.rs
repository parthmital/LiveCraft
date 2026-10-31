//! Clip and clip slot edits (M04 item 5). A clip is named by its id. A slot is named by its track
//! and its row in the Session View: a slot has no identity of its own, so creation and moves
//! name the row they fill.

use crate::command::{CommandError, Operation};
use crate::commands::finite;
use crate::registry::CommandRegistry;
use livecraft_model::clip::{Clip, ClipSlot};
use livecraft_model::edit::EditError;
use livecraft_model::ids::{ClipId, IdAllocator, LaneId, TrackId};
use livecraft_model::model::{Project, TrackType};
use serde::{Deserialize, Serialize};

pub fn register(registry: &mut CommandRegistry) {
    registry.register::<CreateMidiClip>();
    registry.register::<CreateAudioClip>();
    registry.register::<DeleteClip>();
    registry.register::<DuplicateClip>();
    registry.register::<MoveClip>();
    registry.register::<RenameClip>();
    registry.register::<SetClipLoop>();
    registry.register::<SetClipMarkers>();
}

/// The track and row of a clip in the Session View.
pub(crate) fn locate(project: &Project, clip: ClipId) -> Result<(TrackId, usize), CommandError> {
    project
        .all_tracks()
        .find_map(|track| {
            track
                .clip_slots
                .iter()
                .position(|slot| slot.clip.as_ref().is_some_and(|known| known.id == clip))
                .map(|row| (track.id, row))
        })
        .ok_or(CommandError::NoSuchClip(clip))
}

/// The clip with this id, for editing.
pub(crate) fn clip_mut(project: &mut Project, clip: ClipId) -> Result<&mut Clip, CommandError> {
    let (track, row) = locate(project, clip)?;
    project
        .track_mut(track)
        .and_then(|track| track.clip_slots.get_mut(row))
        .and_then(|slot| slot.clip.as_mut())
        .ok_or(CommandError::NoSuchClip(clip))
}

/// The slot at a track and row, which must exist.
fn slot_mut(
    project: &mut Project,
    track: TrackId,
    row: usize,
) -> Result<&mut ClipSlot, CommandError> {
    let owner = project
        .track_mut(track)
        .ok_or(EditError::NoSuchTrack(track))?;
    owner
        .clip_slots
        .get_mut(row)
        .ok_or_else(|| EditError::NoSuchSlot { track, slot: row }.into())
}

/// Puts a clip into an empty slot, which must exist and be on a track that holds clips.
fn put_into_empty(
    project: &mut Project,
    track: TrackId,
    slot: usize,
    clip: Clip,
) -> Result<(), CommandError> {
    if slot_mut(project, track, slot)?.clip.is_some() {
        return Err(CommandError::SlotOccupied { track, slot });
    }
    project.set_clip(track, slot, clip)?;
    Ok(())
}

/// Takes the clip out of a slot, leaving the slot empty.
fn take_clip(project: &mut Project, track: TrackId, slot: usize) -> Result<Clip, CommandError> {
    slot_mut(project, track, slot)?
        .clip
        .take()
        .ok_or(CommandError::SlotEmpty { track, slot })
}

/// Clears the slot a clip is in, if the clip is still in the Set. Used by undo of a create.
fn remove_created(project: &mut Project, created: Option<ClipId>) {
    if let Some(clip) = created
        && let Ok((track, slot)) = locate(project, clip)
    {
        let _ = project.clear_clip(track, slot);
    }
}

/// A start and an end, in beats, with the start at or after zero and the end after it.
pub(crate) fn check_range(what: &'static str, start: f64, end: f64) -> Result<(), CommandError> {
    finite(what, start)?;
    finite(what, end)?;
    if start < 0.0 || end <= start {
        return Err(CommandError::InvalidRange { what, start, end });
    }
    Ok(())
}

/// Creates a MIDI clip in an empty slot.
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateMidiClip {
    pub track: TrackId,
    pub slot: usize,
    pub name: String,
    pub length_beats: f64,
    #[serde(skip)]
    created: Option<ClipId>,
}

impl CreateMidiClip {
    pub fn new(track: TrackId, slot: usize, name: impl Into<String>, length_beats: f64) -> Self {
        Self {
            track,
            slot,
            name: name.into(),
            length_beats,
            created: None,
        }
    }
}

impl Operation for CreateMidiClip {
    const NAME: &'static str = "clip.create_midi";
    const LABEL: &'static str = "Create MIDI Clip";

    fn perform(
        &mut self,
        project: &mut Project,
        ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        finite("clip length", self.length_beats)?;
        if self.length_beats <= 0.0 {
            return Err(CommandError::NotPositive {
                what: "clip length",
                value: self.length_beats,
            });
        }
        let id = *self.created.get_or_insert_with(|| ids.clip());
        let clip = Clip::new_midi(id, &self.name, self.length_beats);
        put_into_empty(project, self.track, self.slot, clip)
    }

    fn revert(&mut self, project: &mut Project) {
        remove_created(project, self.created);
    }
}

/// Creates an audio clip, which plays a sample file, in an empty slot.
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateAudioClip {
    pub track: TrackId,
    pub slot: usize,
    pub name: String,
    pub file_path: String,
    pub length_beats: f64,
    #[serde(skip)]
    created: Option<ClipId>,
}

impl CreateAudioClip {
    pub fn new(
        track: TrackId,
        slot: usize,
        name: impl Into<String>,
        file_path: impl Into<String>,
        length_beats: f64,
    ) -> Self {
        Self {
            track,
            slot,
            name: name.into(),
            file_path: file_path.into(),
            length_beats,
            created: None,
        }
    }
}

impl Operation for CreateAudioClip {
    const NAME: &'static str = "clip.create_audio";
    const LABEL: &'static str = "Create Audio Clip";

    fn perform(
        &mut self,
        project: &mut Project,
        ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        finite("clip length", self.length_beats)?;
        if self.length_beats <= 0.0 {
            return Err(CommandError::NotPositive {
                what: "clip length",
                value: self.length_beats,
            });
        }
        let id = *self.created.get_or_insert_with(|| ids.clip());
        let clip = Clip::new_audio(id, &self.name, &self.file_path, self.length_beats);
        put_into_empty(project, self.track, self.slot, clip)
    }

    fn revert(&mut self, project: &mut Project) {
        remove_created(project, self.created);
    }
}

/// Empties the slot that holds a clip. Undo puts the clip back in the same slot.
#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteClip {
    pub clip: ClipId,
    #[serde(skip)]
    removed: Option<(TrackId, usize, Clip)>,
}

impl DeleteClip {
    pub fn new(clip: ClipId) -> Self {
        Self {
            clip,
            removed: None,
        }
    }
}

impl Operation for DeleteClip {
    const NAME: &'static str = "clip.delete";
    const LABEL: &'static str = "Delete Clip";

    fn perform(
        &mut self,
        project: &mut Project,
        _ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        let (track, slot) = locate(project, self.clip)?;
        let clip = take_clip(project, track, slot)?;
        self.removed = Some((track, slot, clip));
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        if let Some((track, slot, clip)) = self.removed.take() {
            let _ = project.set_clip(track, slot, clip);
        }
    }
}

/// Copies a clip into an empty slot, under a new id. The copy's automation lanes get new ids, so
/// lane ids stay unique in the Set.
#[derive(Debug, Serialize, Deserialize)]
pub struct DuplicateClip {
    pub clip: ClipId,
    pub track: TrackId,
    pub slot: usize,
    #[serde(skip)]
    created: Option<ClipId>,
    #[serde(skip)]
    lanes: Option<Vec<LaneId>>,
}

impl DuplicateClip {
    pub fn new(clip: ClipId, track: TrackId, slot: usize) -> Self {
        Self {
            clip,
            track,
            slot,
            created: None,
            lanes: None,
        }
    }
}

impl Operation for DuplicateClip {
    const NAME: &'static str = "clip.duplicate";
    const LABEL: &'static str = "Duplicate Clip";

    fn perform(
        &mut self,
        project: &mut Project,
        ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        let mut copy = clip_mut(project, self.clip)?.clone();
        let id = *self.created.get_or_insert_with(|| ids.clip());
        let lanes = self
            .lanes
            .get_or_insert_with(|| copy.automation_lanes.iter().map(|_| ids.lane()).collect());
        copy.id = id;
        for (lane, lane_id) in copy.automation_lanes.iter_mut().zip(lanes.iter()) {
            lane.id = *lane_id;
        }
        put_into_empty(project, self.track, self.slot, copy)
    }

    fn revert(&mut self, project: &mut Project) {
        remove_created(project, self.created);
    }
}

/// Moves a clip to an empty slot. The clip keeps its id.
#[derive(Debug, Serialize, Deserialize)]
pub struct MoveClip {
    pub clip: ClipId,
    pub track: TrackId,
    pub slot: usize,
    #[serde(skip)]
    origin: Option<(TrackId, usize)>,
}

impl MoveClip {
    pub fn new(clip: ClipId, track: TrackId, slot: usize) -> Self {
        Self {
            clip,
            track,
            slot,
            origin: None,
        }
    }
}

impl Operation for MoveClip {
    const NAME: &'static str = "clip.move";
    const LABEL: &'static str = "Move Clip";

    fn perform(
        &mut self,
        project: &mut Project,
        _ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        let (from_track, from_slot) = locate(project, self.clip)?;
        if project.track(self.track).map(|track| track.track_type) == Some(TrackType::Group) {
            return Err(EditError::GroupHoldsClips.into());
        }
        if slot_mut(project, self.track, self.slot)?.clip.is_some() {
            return Err(CommandError::SlotOccupied {
                track: self.track,
                slot: self.slot,
            });
        }
        let clip = take_clip(project, from_track, from_slot)?;
        put_into_empty(project, self.track, self.slot, clip)?;
        self.origin = Some((from_track, from_slot));
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        let Some((track, slot)) = self.origin.take() else {
            return;
        };
        if let Ok((current_track, current_slot)) = locate(project, self.clip)
            && let Ok(clip) = take_clip(project, current_track, current_slot)
        {
            let _ = project.set_clip(track, slot, clip);
        }
    }
}

/// Renames a clip.
#[derive(Debug, Serialize, Deserialize)]
pub struct RenameClip {
    pub clip: ClipId,
    pub name: String,
    #[serde(skip)]
    previous: Option<String>,
}

impl RenameClip {
    pub fn new(clip: ClipId, name: impl Into<String>) -> Self {
        Self {
            clip,
            name: name.into(),
            previous: None,
        }
    }
}

impl Operation for RenameClip {
    const NAME: &'static str = "clip.rename";
    const LABEL: &'static str = "Rename Clip";

    fn perform(
        &mut self,
        project: &mut Project,
        _ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        let clip = clip_mut(project, self.clip)?;
        self.previous = Some(std::mem::replace(&mut clip.name, self.name.clone()));
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        if let (Some(previous), Ok(clip)) = (self.previous.take(), clip_mut(project, self.clip)) {
            clip.name = previous;
        }
    }
}

/// The loop of a clip: its start, its end, and whether it loops, all in beats.
#[derive(Debug, Serialize, Deserialize)]
pub struct SetClipLoop {
    pub clip: ClipId,
    pub loop_start_beat: f64,
    pub loop_end_beat: f64,
    pub is_looping: bool,
    #[serde(skip)]
    previous: Option<(f64, f64, bool)>,
}

impl SetClipLoop {
    pub fn new(clip: ClipId, loop_start_beat: f64, loop_end_beat: f64, is_looping: bool) -> Self {
        Self {
            clip,
            loop_start_beat,
            loop_end_beat,
            is_looping,
            previous: None,
        }
    }
}

impl Operation for SetClipLoop {
    const NAME: &'static str = "clip.set_loop";
    const LABEL: &'static str = "Set Clip Loop";

    fn perform(
        &mut self,
        project: &mut Project,
        _ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        check_range("clip loop", self.loop_start_beat, self.loop_end_beat)?;
        let clip = clip_mut(project, self.clip)?;
        self.previous = Some((clip.loop_start_beat, clip.loop_end_beat, clip.is_looping));
        clip.loop_start_beat = self.loop_start_beat;
        clip.loop_end_beat = self.loop_end_beat;
        clip.is_looping = self.is_looping;
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        if let (Some((start, end, looping)), Ok(clip)) =
            (self.previous.take(), clip_mut(project, self.clip))
        {
            clip.loop_start_beat = start;
            clip.loop_end_beat = end;
            clip.is_looping = looping;
        }
    }
}

/// The start and end markers of a clip, in beats.
#[derive(Debug, Serialize, Deserialize)]
pub struct SetClipMarkers {
    pub clip: ClipId,
    pub start_marker_beat: f64,
    pub end_marker_beat: f64,
    #[serde(skip)]
    previous: Option<(f64, f64)>,
}

impl SetClipMarkers {
    pub fn new(clip: ClipId, start_marker_beat: f64, end_marker_beat: f64) -> Self {
        Self {
            clip,
            start_marker_beat,
            end_marker_beat,
            previous: None,
        }
    }
}

impl Operation for SetClipMarkers {
    const NAME: &'static str = "clip.set_markers";
    const LABEL: &'static str = "Set Clip Markers";

    fn perform(
        &mut self,
        project: &mut Project,
        _ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        check_range("clip markers", self.start_marker_beat, self.end_marker_beat)?;
        let clip = clip_mut(project, self.clip)?;
        self.previous = Some((clip.start_marker_beat, clip.end_marker_beat));
        clip.start_marker_beat = self.start_marker_beat;
        clip.end_marker_beat = self.end_marker_beat;
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        if let (Some((start, end)), Ok(clip)) = (self.previous.take(), clip_mut(project, self.clip))
        {
            clip.start_marker_beat = start;
            clip.end_marker_beat = end;
        }
    }
}
