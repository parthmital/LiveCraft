//! MIDI note edits in a MIDI clip (M04 item 5). A clip is named by its id. A note is named by its
//! index in the clip, because notes have no ids until M18.

use crate::command::{CommandError, Operation};
use crate::commands::clip::clip_mut;
use crate::commands::finite;
use crate::registry::CommandRegistry;
use livecraft_model::ids::{ClipId, IdAllocator};
use livecraft_model::model::{ClipPayload, MidiNote, Project};
use serde::{Deserialize, Serialize};

pub fn register(registry: &mut CommandRegistry) {
    registry.register::<AddNote>();
    registry.register::<RemoveNote>();
    registry.register::<EditNote>();
}

/// The notes of a MIDI clip, for editing.
fn notes_mut(project: &mut Project, clip: ClipId) -> Result<&mut Vec<MidiNote>, CommandError> {
    match &mut clip_mut(project, clip)?.payload {
        ClipPayload::Midi(data) => Ok(&mut data.notes),
        ClipPayload::Audio(_) => Err(CommandError::NotMidiClip(clip)),
    }
}

/// Checks a note's values against the ranges of Live's note properties (manual 10.5): pitch 0 to
/// 127, velocity 1 to 127, release velocity 0 to 127, and probability 0 to 1.
fn checked_note(note: &MidiNote) -> Result<(), CommandError> {
    finite("note start", note.start_beat)?;
    finite(
        "note velocity deviation",
        f64::from(note.velocity_deviation),
    )?;
    if note.duration_beats.is_nan() || note.duration_beats <= 0.0 {
        return Err(CommandError::NotPositive {
            what: "note duration",
            value: note.duration_beats,
        });
    }
    if note.pitch > 127 {
        return Err(CommandError::OutOfRange {
            what: "note pitch",
            min: 0.0,
            max: 127.0,
            value: f64::from(note.pitch),
        });
    }
    range("note velocity", f64::from(note.velocity), 1.0, 127.0)?;
    range(
        "note release velocity",
        f64::from(note.release_velocity),
        0.0,
        127.0,
    )?;
    range("note probability", f64::from(note.probability), 0.0, 1.0)
}

/// Fails unless `value` lies between `min` and `max`. NaN lies between no two numbers.
fn range(what: &'static str, value: f64, min: f64, max: f64) -> Result<(), CommandError> {
    if (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(CommandError::OutOfRange {
            what,
            min,
            max,
            value,
        })
    }
}

/// Adds a note at the end of a clip's notes.
#[derive(Debug, Serialize, Deserialize)]
pub struct AddNote {
    pub clip: ClipId,
    pub note: MidiNote,
    #[serde(skip)]
    index: Option<usize>,
}

impl AddNote {
    pub fn new(clip: ClipId, note: MidiNote) -> Self {
        Self {
            clip,
            note,
            index: None,
        }
    }
}

impl Operation for AddNote {
    const NAME: &'static str = "note.add";
    const LABEL: &'static str = "Add Note";

    fn perform(
        &mut self,
        project: &mut Project,
        _ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        checked_note(&self.note)?;
        let notes = notes_mut(project, self.clip)?;
        notes.push(self.note.clone());
        self.index = Some(notes.len() - 1);
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        if let Some(index) = self.index.take()
            && let Ok(notes) = notes_mut(project, self.clip)
            && index < notes.len()
        {
            notes.remove(index);
        }
    }
}

/// Removes the note at `index`.
#[derive(Debug, Serialize, Deserialize)]
pub struct RemoveNote {
    pub clip: ClipId,
    pub index: usize,
    #[serde(skip)]
    removed: Option<MidiNote>,
}

impl RemoveNote {
    pub fn new(clip: ClipId, index: usize) -> Self {
        Self {
            clip,
            index,
            removed: None,
        }
    }
}

impl Operation for RemoveNote {
    const NAME: &'static str = "note.remove";
    const LABEL: &'static str = "Delete Note";

    fn perform(
        &mut self,
        project: &mut Project,
        _ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        let (clip, index) = (self.clip, self.index);
        let notes = notes_mut(project, clip)?;
        if index >= notes.len() {
            return Err(CommandError::NoSuchNote { clip, index });
        }
        self.removed = Some(notes.remove(index));
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        if let Some(note) = self.removed.take()
            && let Ok(notes) = notes_mut(project, self.clip)
            && self.index <= notes.len()
        {
            notes.insert(self.index, note);
        }
    }
}

/// Replaces the note at `index` with `note`.
#[derive(Debug, Serialize, Deserialize)]
pub struct EditNote {
    pub clip: ClipId,
    pub index: usize,
    pub note: MidiNote,
    #[serde(skip)]
    previous: Option<MidiNote>,
}

impl EditNote {
    pub fn new(clip: ClipId, index: usize, note: MidiNote) -> Self {
        Self {
            clip,
            index,
            note,
            previous: None,
        }
    }
}

impl Operation for EditNote {
    const NAME: &'static str = "note.edit";
    const LABEL: &'static str = "Edit Note";

    fn perform(
        &mut self,
        project: &mut Project,
        _ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        checked_note(&self.note)?;
        let (clip, index) = (self.clip, self.index);
        let notes = notes_mut(project, clip)?;
        let slot = notes
            .get_mut(index)
            .ok_or(CommandError::NoSuchNote { clip, index })?;
        self.previous = Some(std::mem::replace(slot, self.note.clone()));
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        if let Some(previous) = self.previous.take()
            && let Ok(notes) = notes_mut(project, self.clip)
            && let Some(slot) = notes.get_mut(self.index)
        {
            *slot = previous;
        }
    }
}
