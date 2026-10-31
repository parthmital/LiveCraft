//! Track edits (M04 items 3 and 4): creating, deleting, naming, coloring, and ordering tracks, and
//! their mixer settings (volume, pan, sends, arm, solo, mute, and monitoring).

use crate::command::{CommandError, Operation};
use crate::commands::finite;
use crate::registry::CommandRegistry;
use livecraft_model::edit::EditError;
use livecraft_model::ids::{IdAllocator, TrackId};
use livecraft_model::model::{MonitoringMode, Project, Track, TrackType};
use livecraft_model::routing::Routing;
use serde::{Deserialize, Serialize};

/// Clip slots in a new audio or MIDI track, as the GUI has always created them.
const NEW_TRACK_SLOTS: usize = 8;

pub fn register(registry: &mut CommandRegistry) {
    registry.register::<CreateAudioTrack>();
    registry.register::<CreateMidiTrack>();
    registry.register::<CreateReturnTrack>();
    registry.register::<CreateGroupTrack>();
    registry.register::<DeleteTrack>();
    registry.register::<RenameTrack>();
    registry.register::<SetTrackColor>();
    registry.register::<ReorderTrack>();
    registry.register::<SetTrackArmed>();
    registry.register::<SetTrackSoloed>();
    registry.register::<SetTrackMuted>();
    registry.register::<SetTrackMonitoring>();
    registry.register::<SetTrackVolume>();
    registry.register::<SetTrackPan>();
    registry.register::<SetTrackSend>();
}

/// A setting that is one plain value on a track: its operation stores the value it replaces.
macro_rules! track_setting {
    ($(#[$doc:meta])* $ty:ident, $name:literal, $label:literal, $field:ident: $value:ty) => {
        $(#[$doc])*
        #[derive(Debug, Serialize, Deserialize)]
        pub struct $ty {
            pub track: TrackId,
            pub $field: $value,
            #[serde(skip)]
            previous: Option<$value>,
        }

        impl $ty {
            pub fn new(track: TrackId, $field: $value) -> Self {
                Self {
                    track,
                    $field,
                    previous: None,
                }
            }
        }

        impl Operation for $ty {
            const NAME: &'static str = $name;
            const LABEL: &'static str = $label;

            fn perform(
                &mut self,
                project: &mut Project,
                _ids: &mut IdAllocator,
            ) -> Result<(), CommandError> {
                let track = project
                    .track_mut(self.track)
                    .ok_or(EditError::NoSuchTrack(self.track))?;
                self.previous = Some(std::mem::replace(&mut track.$field, self.$field.clone()));
                Ok(())
            }

            fn revert(&mut self, project: &mut Project) {
                if let (Some(previous), Some(track)) =
                    (self.previous.take(), project.track_mut(self.track))
                {
                    track.$field = previous;
                }
            }
        }
    };
}

track_setting!(
    /// Renames a track.
    RenameTrack, "track.rename", "Rename Track", name: String
);
track_setting!(
    /// Arms or disarms a track for recording.
    SetTrackArmed, "track.set_armed", "Set Track Armed", is_armed: bool
);
track_setting!(
    /// Solos or unsolos a track.
    SetTrackSoloed, "track.set_soloed", "Set Track Soloed", is_soloed: bool
);
track_setting!(
    /// Mutes or unmutes a track.
    SetTrackMuted, "track.set_muted", "Set Track Muted", is_muted: bool
);
track_setting!(
    /// Sets how a track monitors its input.
    SetTrackMonitoring, "track.set_monitoring", "Set Track Monitoring", monitoring: MonitoringMode
);

/// Gives a new track the id `ids` allocates the first time, so a redo reuses the same id.
fn new_id(ids: &mut IdAllocator, created: &mut Option<TrackId>) -> TrackId {
    *created.get_or_insert_with(|| ids.track())
}

/// Removes a track that a create operation added. Undo runs newest first, so the track is still in
/// the Set when this runs.
fn remove_created(project: &mut Project, created: Option<TrackId>) {
    if let Some(id) = created {
        let _ = project.remove_track(id);
    }
}

/// The track list that holds `track`, and whether it is the return list.
fn list_of(project: &mut Project, track: TrackId) -> Result<(&mut Vec<Track>, bool), EditError> {
    if project.return_tracks.iter().any(|known| known.id == track) {
        Ok((&mut project.return_tracks, true))
    } else if project.tracks.iter().any(|known| known.id == track) {
        Ok((&mut project.tracks, false))
    } else {
        Err(EditError::NoSuchTrack(track))
    }
}

/// Adds an audio track to the end of the track list.
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
        let id = new_id(ids, &mut self.created);
        project.add_track(Track::new(
            id,
            &self.name,
            TrackType::Audio,
            NEW_TRACK_SLOTS,
        ))?;
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        remove_created(project, self.created);
    }
}

/// Adds a MIDI track to the end of the track list.
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
        let id = new_id(ids, &mut self.created);
        project.add_track(Track::new(id, &self.name, TrackType::Midi, NEW_TRACK_SLOTS))?;
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        remove_created(project, self.created);
    }
}

/// Adds a group track. A group holds no clips, so it gets no clip slots.
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateGroupTrack {
    pub name: String,
    #[serde(skip)]
    created: Option<TrackId>,
}

impl CreateGroupTrack {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            created: None,
        }
    }
}

impl Operation for CreateGroupTrack {
    const NAME: &'static str = "track.create_group";
    const LABEL: &'static str = "Create Group Track";

    fn perform(
        &mut self,
        project: &mut Project,
        ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        let id = new_id(ids, &mut self.created);
        project.add_track(Track::new(
            id,
            &self.name,
            TrackType::Group,
            NEW_TRACK_SLOTS,
        ))?;
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        remove_created(project, self.created);
    }
}

/// Adds a return track after the existing ones. It gets a send on every track.
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateReturnTrack {
    pub name: String,
    #[serde(skip)]
    created: Option<TrackId>,
}

impl CreateReturnTrack {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            created: None,
        }
    }
}

impl Operation for CreateReturnTrack {
    const NAME: &'static str = "track.create_return";
    const LABEL: &'static str = "Create Return Track";

    fn perform(
        &mut self,
        project: &mut Project,
        ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        let id = new_id(ids, &mut self.created);
        // Return tracks play no clips (manual 18.4, page 402), so they get no clip slots.
        project.add_return_track(Track::new(id, &self.name, TrackType::Return, 0))?;
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        remove_created(project, self.created);
    }
}

/// How removing a track changed another track: its group parent, its routing, and its sends.
#[derive(Debug)]
struct Links {
    track: TrackId,
    parent: Option<TrackId>,
    routing: Routing,
    sends: Vec<f32>,
}

/// A removed track, where it was, and the changes its removal made to the other tracks.
#[derive(Debug)]
struct Removed {
    track: Track,
    index: usize,
    is_return: bool,
    links: Vec<Links>,
}

/// Removes a track. Its group children move up to its group parent, and routes to it fall back,
/// as `Project::remove_track` does. Undo puts the track back where it was, and restores the other
/// tracks' parents, routing, and sends.
#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteTrack {
    pub track: TrackId,
    #[serde(skip)]
    removed: Option<Removed>,
}

impl DeleteTrack {
    pub fn new(track: TrackId) -> Self {
        Self {
            track,
            removed: None,
        }
    }
}

impl Operation for DeleteTrack {
    const NAME: &'static str = "track.delete";
    const LABEL: &'static str = "Delete Track";

    fn perform(
        &mut self,
        project: &mut Project,
        _ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        if project.main_track.id == self.track {
            return Err(EditError::MainTrackFixed.into());
        }
        let track = project
            .track(self.track)
            .ok_or(EditError::NoSuchTrack(self.track))?
            .clone();
        let (index, is_return) = match project
            .return_tracks
            .iter()
            .position(|known| known.id == self.track)
        {
            Some(index) => (index, true),
            None => {
                let index = project
                    .tracks
                    .iter()
                    .position(|known| known.id == self.track)
                    .ok_or(EditError::NoSuchTrack(self.track))?;
                (index, false)
            }
        };
        let links = project
            .all_tracks()
            .filter(|other| other.id != self.track)
            .map(|other| Links {
                track: other.id,
                parent: other.parent,
                routing: other.routing,
                sends: other.sends.clone(),
            })
            .collect();
        project.remove_track(self.track)?;
        self.removed = Some(Removed {
            track,
            index,
            is_return,
            links,
        });
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        let Some(Removed {
            track,
            index,
            is_return,
            links,
        }) = self.removed.take()
        else {
            return;
        };
        if is_return {
            let index = index.min(project.return_tracks.len());
            project.return_tracks.insert(index, track);
        } else {
            let index = index.min(project.tracks.len());
            project.tracks.insert(index, track);
        }
        for link in links {
            if let Some(other) = project.track_mut(link.track) {
                other.parent = link.parent;
                other.routing = link.routing;
                other.sends = link.sends;
            }
        }
    }
}

/// True for a color in the form `#RRGGBB`, which is how the model stores colors.
fn is_hex_color(color: &str) -> bool {
    color
        .strip_prefix('#')
        .is_some_and(|digits| digits.len() == 6 && digits.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Sets a track's color, as `#RRGGBB`.
#[derive(Debug, Serialize, Deserialize)]
pub struct SetTrackColor {
    pub track: TrackId,
    pub color: String,
    #[serde(skip)]
    previous: Option<String>,
}

impl SetTrackColor {
    pub fn new(track: TrackId, color: impl Into<String>) -> Self {
        Self {
            track,
            color: color.into(),
            previous: None,
        }
    }
}

impl Operation for SetTrackColor {
    const NAME: &'static str = "track.set_color";
    const LABEL: &'static str = "Set Track Color";

    fn perform(
        &mut self,
        project: &mut Project,
        _ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        if !is_hex_color(&self.color) {
            return Err(CommandError::BadColor {
                value: self.color.clone(),
            });
        }
        let track = project
            .track_mut(self.track)
            .ok_or(EditError::NoSuchTrack(self.track))?;
        self.previous = Some(std::mem::replace(&mut track.color, self.color.clone()));
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        if let (Some(previous), Some(track)) = (self.previous.take(), project.track_mut(self.track))
        {
            track.color = previous;
        }
    }
}

/// Moves a track to a new position in its own list (the track list or the return list). The Main
/// track does not move.
#[derive(Debug, Serialize, Deserialize)]
pub struct ReorderTrack {
    pub track: TrackId,
    pub index: usize,
    #[serde(skip)]
    moved_from: Option<(usize, bool)>,
}

impl ReorderTrack {
    pub fn new(track: TrackId, index: usize) -> Self {
        Self {
            track,
            index,
            moved_from: None,
        }
    }
}

impl Operation for ReorderTrack {
    const NAME: &'static str = "track.reorder";
    const LABEL: &'static str = "Move Track";

    fn perform(
        &mut self,
        project: &mut Project,
        _ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        if project.main_track.id == self.track {
            return Err(EditError::MainTrackFixed.into());
        }
        let (list, is_return) = list_of(project, self.track)?;
        if self.index >= list.len() {
            return Err(CommandError::OutOfRange {
                what: "track position",
                min: 0.0,
                max: list.len().saturating_sub(1) as f64,
                value: self.index as f64,
            });
        }
        let from = list
            .iter()
            .position(|known| known.id == self.track)
            .ok_or(EditError::NoSuchTrack(self.track))?;
        let track = list.remove(from);
        list.insert(self.index, track);
        self.moved_from = Some((from, is_return));
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        let Some((from, is_return)) = self.moved_from.take() else {
            return;
        };
        let list = if is_return {
            &mut project.return_tracks
        } else {
            &mut project.tracks
        };
        if let Some(current) = list.iter().position(|known| known.id == self.track) {
            let track = list.remove(current);
            list.insert(from.min(list.len()), track);
        }
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

/// Sets a track's pan, from -1.0 (left) to 1.0 (right). A knob drag merges into one undo step.
#[derive(Debug, Serialize, Deserialize)]
pub struct SetTrackPan {
    pub track: TrackId,
    pub pan: f32,
    #[serde(skip)]
    previous: Option<f32>,
}

impl SetTrackPan {
    pub fn new(track: TrackId, pan: f32) -> Self {
        Self {
            track,
            pan,
            previous: None,
        }
    }
}

impl Operation for SetTrackPan {
    const NAME: &'static str = "track.set_pan";
    const LABEL: &'static str = "Set Track Pan";

    fn perform(
        &mut self,
        project: &mut Project,
        _ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        // The range check also rejects NaN, because no range contains it.
        let pan = f64::from(self.pan);
        if !(-1.0..=1.0).contains(&pan) {
            return Err(CommandError::OutOfRange {
                what: "track pan",
                min: -1.0,
                max: 1.0,
                value: pan,
            });
        }
        let track = project
            .track_mut(self.track)
            .ok_or(EditError::NoSuchTrack(self.track))?;
        self.previous = Some(track.pan);
        track.pan = self.pan;
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        if let (Some(previous), Some(track)) = (self.previous.take(), project.track_mut(self.track))
        {
            track.pan = previous;
        }
    }

    fn absorb(&mut self, next: &Self) -> bool {
        if self.track == next.track {
            self.pan = next.pan;
            true
        } else {
            false
        }
    }
}

/// Sets a track's send to one return track, from 0.0 to 1.0. The return is named by id, and its
/// send is found by its position among the return tracks. A slider drag merges into one undo step.
#[derive(Debug, Serialize, Deserialize)]
pub struct SetTrackSend {
    pub track: TrackId,
    #[serde(rename = "return")]
    pub return_track: TrackId,
    pub level: f32,
    #[serde(skip)]
    previous: Option<f32>,
}

impl SetTrackSend {
    pub fn new(track: TrackId, return_track: TrackId, level: f32) -> Self {
        Self {
            track,
            return_track,
            level,
            previous: None,
        }
    }
}

/// The position of a return track among the return tracks, which is the index of its send.
fn send_index(project: &Project, return_track: TrackId) -> Result<usize, EditError> {
    project
        .return_tracks
        .iter()
        .position(|known| known.id == return_track)
        .ok_or(EditError::NoSuchTrack(return_track))
}

impl Operation for SetTrackSend {
    const NAME: &'static str = "track.set_send";
    const LABEL: &'static str = "Set Track Send";

    fn perform(
        &mut self,
        project: &mut Project,
        _ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        // The range check also rejects NaN, because no range contains it.
        let level = f64::from(self.level);
        if !(0.0..=1.0).contains(&level) {
            return Err(CommandError::OutOfRange {
                what: "track send",
                min: 0.0,
                max: 1.0,
                value: level,
            });
        }
        let index = send_index(project, self.return_track)?;
        let returns = project.return_tracks.len();
        let track = project
            .track_mut(self.track)
            .ok_or(EditError::NoSuchTrack(self.track))?;
        let sends = track.sends.len();
        let slot = track
            .sends
            .get_mut(index)
            .ok_or(EditError::SendCount { sends, returns })?;
        self.previous = Some(std::mem::replace(slot, self.level));
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        let Ok(index) = send_index(project, self.return_track) else {
            return;
        };
        if let (Some(previous), Some(track)) = (self.previous.take(), project.track_mut(self.track))
            && let Some(slot) = track.sends.get_mut(index)
        {
            *slot = previous;
        }
    }

    fn absorb(&mut self, next: &Self) -> bool {
        if self.track == next.track && self.return_track == next.return_track {
            self.level = next.level;
            true
        } else {
            false
        }
    }
}
