use crate::ids::{DeviceId, LaneId, SceneId, TrackId};
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
        track: TrackId,
    },
    SetVolume {
        track: TrackId,
        volume_db: f32,
    },
    SetPan {
        track: TrackId,
        pan: f32,
    },
    SetMute {
        track: TrackId,
        muted: bool,
    },
    SetSolo {
        track: TrackId,
        soloed: bool,
    },
    SetArm {
        track: TrackId,
        armed: bool,
    },
    SetSendLevel {
        track: TrackId,
        send_index: usize,
        level: f32,
    },
    AddMidiNote {
        track: TrackId,
        slot_index: usize,
        note: MidiNote,
    },
    DeleteMidiNote {
        track: TrackId,
        slot_index: usize,
        note_index: usize,
    },
    SetDeviceParam {
        track: TrackId,
        device: DeviceId,
        param: String,
        value: f32,
    },
    ToggleDevice {
        track: TrackId,
        device: DeviceId,
    },
    TriggerClip {
        track: TrackId,
        slot_index: usize,
    },
    StopClip {
        track: TrackId,
    },
    TriggerScene {
        scene: SceneId,
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
        match self.undo_stack.pop() {
            Some(mut action) => {
                action.undo(project);
                self.redo_stack.push(action);
                true
            }
            _ => false,
        }
    }

    pub fn redo(&mut self, project: &mut Project) -> bool {
        match self.redo_stack.pop() {
            Some(mut action) => {
                action.execute(project);
                self.undo_stack.push(action);
                true
            }
            _ => false,
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
        self.old_tempo = project.transport.bpm();
        project.transport.set_bpm(self.new_tempo);
    }

    fn undo(&mut self, project: &mut Project) {
        project.transport.set_bpm(self.old_tempo);
    }
}

#[derive(Debug)]
pub struct SetTrackVolumeAction {
    pub track: TrackId,
    pub old_vol: f32,
    pub new_vol: f32,
}

impl ReversibleAction for SetTrackVolumeAction {
    fn execute(&mut self, project: &mut Project) {
        if let Some(trk) = project.track_mut(self.track) {
            self.old_vol = trk.volume_db;
            trk.volume_db = self.new_vol;
        }
    }

    fn undo(&mut self, project: &mut Project) {
        if let Some(trk) = project.track_mut(self.track) {
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

#[derive(Debug)]
pub struct AddAutomationPointAction {
    pub track: TrackId,
    pub lane: LaneId,
    pub point: AutomationPoint,
    pub inserted_index: Option<usize>,
}

impl AddAutomationPointAction {
    pub fn new(track: TrackId, lane: LaneId, point: AutomationPoint) -> Self {
        Self {
            track,
            lane,
            point,
            inserted_index: None,
        }
    }
}

impl ReversibleAction for AddAutomationPointAction {
    fn execute(&mut self, project: &mut Project) {
        if let Some(track) = project.track_mut(self.track)
            && let Some(lane) = track.lane_mut(self.lane)
        {
            let pos = lane.points.partition_point(|p| p.beat < self.point.beat);
            lane.points.insert(pos, self.point.clone());
            self.inserted_index = Some(pos);
        }
    }

    fn undo(&mut self, project: &mut Project) {
        if let Some(idx) = self.inserted_index.take()
            && let Some(track) = project.track_mut(self.track)
            && let Some(lane) = track.lane_mut(self.lane)
            && idx < lane.points.len()
        {
            lane.points.remove(idx);
        }
    }
}

#[derive(Debug)]
pub struct RemoveAutomationPointAction {
    pub track: TrackId,
    pub lane: LaneId,
    pub point_index: usize,
    pub removed_point: Option<AutomationPoint>,
}

impl RemoveAutomationPointAction {
    pub fn new(track: TrackId, lane: LaneId, point_index: usize) -> Self {
        Self {
            track,
            lane,
            point_index,
            removed_point: None,
        }
    }
}

impl ReversibleAction for RemoveAutomationPointAction {
    fn execute(&mut self, project: &mut Project) {
        if let Some(track) = project.track_mut(self.track)
            && let Some(lane) = track.lane_mut(self.lane)
            && self.point_index < lane.points.len()
        {
            let p = lane.points.remove(self.point_index);
            self.removed_point = Some(p);
        }
    }

    fn undo(&mut self, project: &mut Project) {
        if let Some(p) = self.removed_point.take()
            && let Some(track) = project.track_mut(self.track)
            && let Some(lane) = track.lane_mut(self.lane)
            && self.point_index <= lane.points.len()
        {
            lane.points.insert(self.point_index, p);
        }
    }
}

#[derive(Debug)]
pub struct MoveAutomationPointAction {
    pub track: TrackId,
    pub lane: LaneId,
    pub point_index: usize,
    pub old_beat: f64,
    pub old_value: f32,
    pub old_curve: f32,
    pub new_beat: f64,
    pub new_value: f32,
    pub new_curve: f32,
}

impl MoveAutomationPointAction {
    pub fn new(
        track: TrackId,
        lane: LaneId,
        point_index: usize,
        new_beat: f64,
        new_value: f32,
        new_curve: f32,
    ) -> Self {
        Self {
            track,
            lane,
            point_index,
            old_beat: 0.0,
            old_value: 0.0,
            old_curve: 0.0,
            new_beat,
            new_value,
            new_curve,
        }
    }
}

impl ReversibleAction for MoveAutomationPointAction {
    fn execute(&mut self, project: &mut Project) {
        if let Some(track) = project.track_mut(self.track)
            && let Some(lane) = track.lane_mut(self.lane)
            && let Some(point) = lane.points.get_mut(self.point_index)
        {
            self.old_beat = point.beat;
            self.old_value = point.value;
            self.old_curve = point.curve;
            point.beat = self.new_beat;
            point.value = self.new_value;
            point.curve = self.new_curve;
        }
    }

    fn undo(&mut self, project: &mut Project) {
        if let Some(track) = project.track_mut(self.track)
            && let Some(lane) = track.lane_mut(self.lane)
            && let Some(point) = lane.points.get_mut(self.point_index)
        {
            point.beat = self.old_beat;
            point.value = self.old_value;
            point.curve = self.old_curve;
        }
    }
}
