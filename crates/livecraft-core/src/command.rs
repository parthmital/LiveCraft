use crate::model::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    SetTempo(f32),
    SetPlayhead(f64),
    TogglePlay,
    Stop,
    AddTrack {
        name: String,
        track_type: TrackType,
    },
    DeleteTrack {
        track_index: usize,
    },
    SetVolume {
        track_index: usize,
        volume_db: f32,
    },
    SetPan {
        track_index: usize,
        pan: f32,
    },
    SetMute {
        track_index: usize,
        muted: bool,
    },
    SetSolo {
        track_index: usize,
        soloed: bool,
    },
    SetArm {
        track_index: usize,
        armed: bool,
    },
    SetSendLevel {
        track_index: usize,
        send_index: usize,
        level: f32,
    },
    AddMidiNote {
        track_index: usize,
        slot_index: usize,
        note: MidiNote,
    },
    DeleteMidiNote {
        track_index: usize,
        slot_index: usize,
        note_index: usize,
    },
    SetDeviceParam {
        track_index: usize,
        device_index: usize,
        param: String,
        value: f32,
    },
    ToggleDevice {
        track_index: usize,
        device_index: usize,
    },
    TriggerClip {
        track_index: usize,
        slot_index: usize,
    },
    StopClip {
        track_index: usize,
    },
    TriggerScene {
        scene_index: usize,
    },
    StopAllClips,
}

#[derive(Debug)]
pub struct CommandHistory {
    undo_stack: Vec<Box<dyn ReversibleAction>>,
    redo_stack: Vec<Box<dyn ReversibleAction>>,
    pub max_history: usize,
}

pub trait ReversibleAction: std::fmt::Debug + Send + Sync {
    fn execute(&mut self, project: &mut Project);
    fn undo(&mut self, project: &mut Project);
}

impl Default for CommandHistory {
    fn default() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_history: 100,
        }
    }
}

impl CommandHistory {
    pub fn with_capacity(max_history: usize) -> Self {
        Self {
            max_history,
            ..Default::default()
        }
    }

    pub fn execute(&mut self, mut action: Box<dyn ReversibleAction>, project: &mut Project) {
        action.execute(project);
        self.undo_stack.push(action);
        self.redo_stack.clear();
        if self.undo_stack.len() > self.max_history {
            self.undo_stack.remove(0);
        }
    }

    pub fn undo(&mut self, project: &mut Project) -> bool {
        if let Some(mut action) = self.undo_stack.pop() {
            action.undo(project);
            self.redo_stack.push(action);
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self, project: &mut Project) -> bool {
        if let Some(mut action) = self.redo_stack.pop() {
            action.execute(project);
            self.undo_stack.push(action);
            true
        } else {
            false
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }
}

// Action implementations

#[derive(Debug)]
pub struct SetTempoAction {
    pub old_tempo: f32,
    pub new_tempo: f32,
}

impl ReversibleAction for SetTempoAction {
    fn execute(&mut self, project: &mut Project) {
        self.old_tempo = project.transport.bpm;
        project.transport.bpm = self.new_tempo;
    }

    fn undo(&mut self, project: &mut Project) {
        project.transport.bpm = self.old_tempo;
    }
}

#[derive(Debug)]
pub struct SetTrackVolumeAction {
    pub track_index: usize,
    pub old_vol: f32,
    pub new_vol: f32,
}

impl ReversibleAction for SetTrackVolumeAction {
    fn execute(&mut self, project: &mut Project) {
        if let Some(trk) = project.tracks.get_mut(self.track_index) {
            self.old_vol = trk.volume_db;
            trk.volume_db = self.new_vol;
        }
    }

    fn undo(&mut self, project: &mut Project) {
        if let Some(trk) = project.tracks.get_mut(self.track_index) {
            trk.volume_db = self.old_vol;
        }
    }
}

#[derive(Debug)]
pub struct AddTrackAction {
    pub track: Track,
}

impl ReversibleAction for AddTrackAction {
    fn execute(&mut self, project: &mut Project) {
        project.tracks.push(self.track.clone());
    }

    fn undo(&mut self, project: &mut Project) {
        project.tracks.pop();
    }
}
