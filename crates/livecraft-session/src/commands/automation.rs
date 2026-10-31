//! Automation breakpoints (manual chapter 26), ported from M01's `ReversibleAction`s (M04 item 3).
//!
//! A lane keeps its points sorted by beat, which `AutomationLane::evaluate_at` relies on. Points
//! have no ids until M22, so a point is addressed by its index in the lane.

use crate::command::{CommandError, Operation};
use crate::commands::finite;
use crate::registry::CommandRegistry;
use livecraft_model::automation::{AutomationLane, AutomationPoint};
use livecraft_model::edit::EditError;
use livecraft_model::ids::{IdAllocator, LaneId, TrackId};
use livecraft_model::model::Project;
use serde::{Deserialize, Serialize};

pub fn register(registry: &mut CommandRegistry) {
    registry.register::<AddAutomationPoint>();
    registry.register::<RemoveAutomationPoint>();
    registry.register::<MoveAutomationPoint>();
}

fn lane_mut(
    project: &mut Project,
    track_id: TrackId,
    lane: LaneId,
) -> Result<&mut AutomationLane, CommandError> {
    let track = project
        .track_mut(track_id)
        .ok_or(EditError::NoSuchTrack(track_id))?;
    track.lane_mut(lane).ok_or(CommandError::NoSuchLane {
        track: track_id,
        lane,
    })
}

fn checked_point(point: &AutomationPoint) -> Result<(), CommandError> {
    finite("automation beat", point.beat)?;
    finite("automation value", f64::from(point.value))?;
    finite("automation curve", f64::from(point.curve))
}

/// Adds a breakpoint at its sorted position by beat.
#[derive(Debug, Serialize, Deserialize)]
pub struct AddAutomationPoint {
    pub track: TrackId,
    pub lane: LaneId,
    pub point: AutomationPoint,
    #[serde(skip)]
    inserted_at: Option<usize>,
}

impl AddAutomationPoint {
    pub fn new(track: TrackId, lane: LaneId, point: AutomationPoint) -> Self {
        Self {
            track,
            lane,
            point,
            inserted_at: None,
        }
    }
}

impl Operation for AddAutomationPoint {
    const NAME: &'static str = "automation.add_point";
    const LABEL: &'static str = "Add Automation Point";

    fn perform(
        &mut self,
        project: &mut Project,
        _ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        checked_point(&self.point)?;
        let lane = lane_mut(project, self.track, self.lane)?;
        let index = lane.points.partition_point(|p| p.beat < self.point.beat);
        lane.points.insert(index, self.point.clone());
        self.inserted_at = Some(index);
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        if let Some(index) = self.inserted_at.take()
            && let Ok(lane) = lane_mut(project, self.track, self.lane)
            && index < lane.points.len()
        {
            lane.points.remove(index);
        }
    }
}

/// Removes the breakpoint at `index`.
#[derive(Debug, Serialize, Deserialize)]
pub struct RemoveAutomationPoint {
    pub track: TrackId,
    pub lane: LaneId,
    pub index: usize,
    #[serde(skip)]
    removed: Option<AutomationPoint>,
}

impl RemoveAutomationPoint {
    pub fn new(track: TrackId, lane: LaneId, index: usize) -> Self {
        Self {
            track,
            lane,
            index,
            removed: None,
        }
    }
}

impl Operation for RemoveAutomationPoint {
    const NAME: &'static str = "automation.remove_point";
    const LABEL: &'static str = "Remove Automation Point";

    fn perform(
        &mut self,
        project: &mut Project,
        _ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        let (track, lane_id, index) = (self.track, self.lane, self.index);
        let lane = lane_mut(project, track, lane_id)?;
        if index >= lane.points.len() {
            return Err(CommandError::NoSuchPoint {
                track,
                lane: lane_id,
                index,
            });
        }
        self.removed = Some(lane.points.remove(index));
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        if let Some(point) = self.removed.take()
            && let Ok(lane) = lane_mut(project, self.track, self.lane)
            && self.index <= lane.points.len()
        {
            lane.points.insert(self.index, point);
        }
    }
}

/// Where a moved breakpoint was and where it sits now, with the point it replaced.
#[derive(Debug)]
struct Moved {
    from: usize,
    to: usize,
    previous: AutomationPoint,
}

/// Replaces the breakpoint at `index` with `point`, and moves it to its sorted position by beat.
#[derive(Debug, Serialize, Deserialize)]
pub struct MoveAutomationPoint {
    pub track: TrackId,
    pub lane: LaneId,
    pub index: usize,
    pub point: AutomationPoint,
    #[serde(skip)]
    moved: Option<Moved>,
}

impl MoveAutomationPoint {
    pub fn new(track: TrackId, lane: LaneId, index: usize, point: AutomationPoint) -> Self {
        Self {
            track,
            lane,
            index,
            point,
            moved: None,
        }
    }
}

impl Operation for MoveAutomationPoint {
    const NAME: &'static str = "automation.move_point";
    const LABEL: &'static str = "Move Automation Point";

    fn perform(
        &mut self,
        project: &mut Project,
        _ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        checked_point(&self.point)?;
        let (track, lane_id, from) = (self.track, self.lane, self.index);
        let lane = lane_mut(project, track, lane_id)?;
        if from >= lane.points.len() {
            return Err(CommandError::NoSuchPoint {
                track,
                lane: lane_id,
                index: from,
            });
        }
        let previous = lane.points.remove(from);
        let to = lane.points.partition_point(|p| p.beat < self.point.beat);
        lane.points.insert(to, self.point.clone());
        self.moved = Some(Moved { from, to, previous });
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        if let Some(Moved { from, to, previous }) = self.moved.take()
            && let Ok(lane) = lane_mut(project, self.track, self.lane)
            && to < lane.points.len()
        {
            lane.points.remove(to);
            lane.points.insert(from, previous);
        }
    }
}
