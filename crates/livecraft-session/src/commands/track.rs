//! Track edits (M04 items 3 and 4). Item 3 ports the M01 create and volume actions. Item 4 adds
//! the rest of the track commands.

use crate::command::{CommandError, Operation};
use crate::commands::finite;
use crate::registry::CommandRegistry;
use livecraft_model::edit::EditError;
use livecraft_model::ids::{IdAllocator, TrackId};
use livecraft_model::model::{Project, Track, TrackType};
use serde::{Deserialize, Serialize};

/// Clip slots in a new track, as the GUI has always created them.
const NEW_TRACK_SLOTS: usize = 8;

pub fn register(registry: &mut CommandRegistry) {
    registry.register::<CreateAudioTrack>();
    registry.register::<CreateMidiTrack>();
    registry.register::<SetTrackVolume>();
}

/// Adds a track to the end of the track list. The id comes from `ids` the first time, and a redo
/// reuses it, so later commands that name the track still find it.
fn create(
    project: &mut Project,
    ids: &mut IdAllocator,
    created: &mut Option<TrackId>,
    name: &str,
    track_type: TrackType,
) -> Result<(), CommandError> {
    let id = *created.get_or_insert_with(|| ids.track());
    project.add_track(Track::new(id, name, track_type, NEW_TRACK_SLOTS))?;
    Ok(())
}

fn remove_created(project: &mut Project, created: Option<TrackId>) {
    if let Some(id) = created {
        project.tracks.retain(|track| track.id != id);
    }
}

/// Adds an audio track.
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateAudioTrack {
    pub name: String,
    #[serde(skip)]
    created: Option<TrackId>,
}

impl CreateAudioTrack {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            created: None,
        }
    }
}

impl Operation for CreateAudioTrack {
    const NAME: &'static str = "track.create_audio";
    const LABEL: &'static str = "Create Audio Track";

    fn perform(
        &mut self,
        project: &mut Project,
        ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        create(
            project,
            ids,
            &mut self.created,
            &self.name,
            TrackType::Audio,
        )
    }

    fn revert(&mut self, project: &mut Project) {
        remove_created(project, self.created);
    }
}

/// Adds a MIDI track.
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateMidiTrack {
    pub name: String,
    #[serde(skip)]
    created: Option<TrackId>,
}

impl CreateMidiTrack {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            created: None,
        }
    }
}

impl Operation for CreateMidiTrack {
    const NAME: &'static str = "track.create_midi";
    const LABEL: &'static str = "Create MIDI Track";

    fn perform(
        &mut self,
        project: &mut Project,
        ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        create(project, ids, &mut self.created, &self.name, TrackType::Midi)
    }

    fn revert(&mut self, project: &mut Project) {
        remove_created(project, self.created);
    }
}

/// Sets a track's volume in dB. A fader drag merges into one undo step.
#[derive(Debug, Serialize, Deserialize)]
pub struct SetTrackVolume {
    pub track: TrackId,
    pub volume_db: f32,
    #[serde(skip)]
    previous: Option<f32>,
}

impl SetTrackVolume {
    pub fn new(track: TrackId, volume_db: f32) -> Self {
        Self {
            track,
            volume_db,
            previous: None,
        }
    }
}

impl Operation for SetTrackVolume {
    const NAME: &'static str = "track.set_volume";
    const LABEL: &'static str = "Set Track Volume";

    fn perform(
        &mut self,
        project: &mut Project,
        _ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        finite("track volume", f64::from(self.volume_db))?;
        let track = project
            .track_mut(self.track)
            .ok_or(EditError::NoSuchTrack(self.track))?;
        self.previous = Some(track.volume_db);
        track.volume_db = self.volume_db;
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        if let (Some(previous), Some(track)) = (self.previous.take(), project.track_mut(self.track))
        {
            track.volume_db = previous;
        }
    }

    fn absorb(&mut self, next: &Self) -> bool {
        if self.track == next.track {
            self.volume_db = next.volume_db;
            true
        } else {
            false
        }
    }
}
