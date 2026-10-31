//! Undo and redo for applied commands (M04 items 1 and 8).
//!
//! The history holds steps, newest last. A step is one command, or later a transaction of several
//! (item 8). Continuous gestures merge into the step they continue, so one drag is one undo step.

use crate::command::{Command, CommandError, Operation};
use livecraft_model::ids::IdAllocator;
use livecraft_model::model::Project;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// A command continues the previous gesture when it comes within this gap of it (M04 decision).
pub const MERGE_GAP: Duration = Duration::from_millis(500);

/// The default number of steps kept for undo (M04 decision: 1000).
pub const DEFAULT_LIMIT: usize = 1000;

/// The label of the first entry of every Undo History: the opening of the Set. It cannot be undone
/// (manual 5.4.2).
pub const OPENING_LABEL: &str = "Open Set";

struct Step {
    label: String,
    commands: Vec<Box<dyn Command>>,
    /// When the step last changed, which a gesture compares against `MERGE_GAP`.
    last: Instant,
}

/// One line of the Undo History, shown newest first. An undone line is greyed out (manual 5.4.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    pub label: String,
    pub undone: bool,
}

pub struct UndoHistory {
    done: VecDeque<Step>,
    /// Undone steps. The last one is the next to redo.
    undone: Vec<Step>,
    limit: usize,
}

impl Default for UndoHistory {
    fn default() -> Self {
        Self::with_limit(DEFAULT_LIMIT)
    }
}

impl UndoHistory {
    /// Creates a history that keeps at most `limit` steps for undo. The oldest step is dropped
    /// first.
    pub fn with_limit(limit: usize) -> Self {
        Self {
            done: VecDeque::new(),
            undone: Vec::new(),
            limit,
        }
    }

    /// Applies a command and records it. A command that fails leaves the Set and the history as
    /// they were. A command that continues the previous gesture merges into its step.
    pub fn execute(
        &mut self,
        mut command: Box<dyn Command>,
        project: &mut Project,
        ids: &mut IdAllocator,
        at: Instant,
    ) -> Result<(), CommandError> {
        command.apply(project, ids)?;
        self.undone.clear();
        if let Some(step) = self.done.back_mut()
            && step.commands.len() == 1
            && at.saturating_duration_since(step.last) <= MERGE_GAP
            && step.commands[0].merge(command.as_ref())
        {
            step.last = at;
            return Ok(());
        }
        let label = command.label().to_string();
        self.push(Step {
            label,
            commands: vec![command],
            last: at,
        });
        Ok(())
    }

    /// Runs `f`, and records the commands it applies as one step labelled `label`, which undo
    /// reverts together. If `f` fails, the commands it applied are undone and nothing is recorded.
    pub fn transaction<F>(
        &mut self,
        label: &str,
        project: &mut Project,
        ids: &mut IdAllocator,
        at: Instant,
        f: F,
    ) -> Result<(), CommandError>
    where
        F: FnOnce(&mut Transaction<'_>) -> Result<(), CommandError>,
    {
        let mut transaction = Transaction {
            project,
            ids,
            commands: Vec::new(),
        };
        let outcome = f(&mut transaction);
        let Transaction {
            project,
            mut commands,
            ..
        } = transaction;
        if let Err(error) = outcome {
            for command in commands.iter_mut().rev() {
                command.undo(project);
            }
            return Err(error);
        }
        if commands.is_empty() {
            return Ok(());
        }
        self.undone.clear();
        self.push(Step {
            label: label.to_string(),
            commands,
            last: at,
        });
        Ok(())
    }

    fn push(&mut self, step: Step) {
        self.done.push_back(step);
        if self.done.len() > self.limit {
            self.done.pop_front();
        }
    }

    /// The Undo History, newest first: the undone steps, greyed out, then the active steps, then
    /// the opening of the Set (manual 5.4.2). `go_to` takes an index into this list.
    pub fn entries(&self) -> Vec<HistoryEntry> {
        let undone = self.undone.iter().map(|step| HistoryEntry {
            label: step.label.clone(),
            undone: true,
        });
        let active = self.done.iter().rev().map(|step| HistoryEntry {
            label: step.label.clone(),
            undone: false,
        });
        let opening = std::iter::once(HistoryEntry {
            label: OPENING_LABEL.to_string(),
            undone: false,
        });
        undone.chain(active).chain(opening).collect()
    }

    /// Makes the Set the state after the entry at `index` in `entries`, as selecting an entry does
    /// in Live: the steps newer than it are undone, and the undone steps up to it are redone
    /// (manual 5.4.2). Returns false when `index` is not an entry.
    pub fn go_to(
        &mut self,
        index: usize,
        project: &mut Project,
        ids: &mut IdAllocator,
    ) -> Result<bool, CommandError> {
        let steps = self.done.len() + self.undone.len();
        if index > steps {
            return Ok(false);
        }
        let wanted = steps - index;
        while self.done.len() > wanted {
            self.undo(project);
        }
        while self.done.len() < wanted && self.redo(project, ids)? {}
        Ok(true)
    }

    /// Undoes the newest step. Returns false when there is nothing to undo.
    pub fn undo(&mut self, project: &mut Project) -> bool {
        let Some(mut step) = self.done.pop_back() else {
            return false;
        };
        for command in step.commands.iter_mut().rev() {
            command.undo(project);
        }
        self.undone.push(step);
        true
    }

    /// Redoes the oldest undone step. Returns `Ok(false)` when there is nothing to redo. A step
    /// whose commands fail to apply is rolled back and stays undone.
    pub fn redo(
        &mut self,
        project: &mut Project,
        ids: &mut IdAllocator,
    ) -> Result<bool, CommandError> {
        let Some(mut step) = self.undone.pop() else {
            return Ok(false);
        };
        for index in 0..step.commands.len() {
            if let Err(error) = step.commands[index].apply(project, ids) {
                for command in step.commands[..index].iter_mut().rev() {
                    command.undo(project);
                }
                self.undone.push(step);
                return Err(error);
            }
        }
        self.done.push_back(step);
        Ok(true)
    }

    pub fn can_undo(&self) -> bool {
        !self.done.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.undone.is_empty()
    }
}

/// The commands a transaction has applied so far. `UndoHistory::transaction` passes one to its
/// closure, and everything applied through it becomes one undo step.
pub struct Transaction<'a> {
    project: &'a mut Project,
    ids: &'a mut IdAllocator,
    commands: Vec<Box<dyn Command>>,
}

impl Transaction<'_> {
    /// Applies an operation as part of the transaction.
    pub fn run<O: Operation>(&mut self, operation: O) -> Result<(), CommandError> {
        let mut command: Box<dyn Command> = Box::new(operation);
        command.apply(self.project, self.ids)?;
        self.commands.push(command);
        Ok(())
    }

    /// The Set as the transaction has left it so far.
    pub fn project(&self) -> &Project {
        self.project
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use livecraft_model::edit::EditError;
    use livecraft_model::ids::TrackId;
    use serde::{Deserialize, Serialize};

    /// A test-only edit that sets one track's volume and remembers the value it replaced.
    #[derive(Debug, Serialize, Deserialize)]
    struct SetVolume {
        track: TrackId,
        volume_db: f32,
        #[serde(skip)]
        previous: Option<f32>,
    }

    impl SetVolume {
        fn new(track: u64, volume_db: f32) -> Box<Self> {
            Box::new(Self {
                track: TrackId(track),
                volume_db,
                previous: None,
            })
        }
    }

    impl Operation for SetVolume {
        const NAME: &'static str = "test.set_volume";
        const LABEL: &'static str = "Set Volume";

        fn perform(
            &mut self,
            project: &mut Project,
            _ids: &mut IdAllocator,
        ) -> Result<(), CommandError> {
            let track = project
                .track_mut(self.track)
                .ok_or(EditError::NoSuchTrack(self.track))?;
            self.previous = Some(track.volume_db);
            track.volume_db = self.volume_db;
            Ok(())
        }

        fn revert(&mut self, project: &mut Project) {
            if let (Some(previous), Some(track)) = (self.previous, project.track_mut(self.track)) {
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

    fn volume(project: &Project, track: u64) -> f32 {
        project
            .track(TrackId(track))
            .map_or(f32::NAN, |t| t.volume_db)
    }

    #[test]
    fn apply_undo_and_redo_move_the_set_through_its_steps() {
        let mut project = Project::new_default("History");
        let mut ids = IdAllocator::new();
        let mut history = UndoHistory::default();
        let t0 = Instant::now();
        history
            .execute(SetVolume::new(1, -6.0), &mut project, &mut ids, t0)
            .expect("apply");
        assert_eq!(volume(&project, 1), -6.0);
        assert!(history.undo(&mut project));
        assert_eq!(volume(&project, 1), 0.0);
        assert!(history.can_redo());
        assert!(history.redo(&mut project, &mut ids).expect("redo"));
        assert_eq!(volume(&project, 1), -6.0);
        assert!(!history.can_redo());
    }

    #[test]
    fn a_new_command_after_undo_clears_the_redo_stack() {
        let mut project = Project::new_default("Redo");
        let mut ids = IdAllocator::new();
        let mut history = UndoHistory::default();
        let t0 = Instant::now();
        history
            .execute(SetVolume::new(1, -6.0), &mut project, &mut ids, t0)
            .expect("apply");
        history.undo(&mut project);
        history
            .execute(SetVolume::new(2, -3.0), &mut project, &mut ids, t0)
            .expect("apply");
        assert!(!history.can_redo());
    }

    #[test]
    fn a_failed_command_changes_nothing_and_is_not_recorded() {
        let mut project = Project::new_default("Failed");
        let mut ids = IdAllocator::new();
        let mut history = UndoHistory::default();
        let result = history.execute(
            SetVolume::new(99, -6.0),
            &mut project,
            &mut ids,
            Instant::now(),
        );
        assert_eq!(
            result,
            Err(CommandError::Edit(EditError::NoSuchTrack(TrackId(99))))
        );
        assert!(!history.can_undo());
    }

    #[test]
    fn the_history_keeps_only_the_newest_steps_within_its_limit() {
        let mut project = Project::new_default("Limit");
        let mut ids = IdAllocator::new();
        let mut history = UndoHistory::with_limit(3);
        let t0 = Instant::now();
        for step in 1..=5 {
            // Different tracks, so no step merges into another.
            let track = if step % 2 == 0 { 1 } else { 2 };
            history
                .execute(
                    SetVolume::new(track, step as f32),
                    &mut project,
                    &mut ids,
                    t0 + Duration::from_secs(step),
                )
                .expect("apply");
        }
        let mut undone = 0;
        while history.undo(&mut project) {
            undone += 1;
        }
        assert_eq!(undone, 3);
    }

    #[test]
    fn a_gesture_within_the_merge_gap_is_one_step() {
        let mut project = Project::new_default("Merge");
        let mut ids = IdAllocator::new();
        let mut history = UndoHistory::default();
        let t0 = Instant::now();
        for (offset_ms, volume) in [(0, -1.0), (200, -2.0), (400, -3.0)] {
            history
                .execute(
                    SetVolume::new(1, volume),
                    &mut project,
                    &mut ids,
                    t0 + Duration::from_millis(offset_ms),
                )
                .expect("apply");
        }
        assert_eq!(volume(&project, 1), -3.0);
        assert!(history.undo(&mut project));
        assert!(!history.can_undo(), "the whole drag is one step");
        assert_eq!(
            volume(&project, 1),
            0.0,
            "undo returns to the value before the drag"
        );
    }

    #[test]
    fn a_gap_longer_than_the_merge_gap_starts_a_new_step() {
        let mut project = Project::new_default("Gap");
        let mut ids = IdAllocator::new();
        let mut history = UndoHistory::default();
        let t0 = Instant::now();
        history
            .execute(SetVolume::new(1, -1.0), &mut project, &mut ids, t0)
            .expect("apply");
        history
            .execute(
                SetVolume::new(1, -2.0),
                &mut project,
                &mut ids,
                t0 + MERGE_GAP + Duration::from_millis(1),
            )
            .expect("apply");
        assert!(history.undo(&mut project));
        assert_eq!(volume(&project, 1), -1.0);
        assert!(history.undo(&mut project));
        assert_eq!(volume(&project, 1), 0.0);
    }

    #[test]
    fn a_different_target_does_not_merge() {
        let mut project = Project::new_default("Targets");
        let mut ids = IdAllocator::new();
        let mut history = UndoHistory::default();
        let t0 = Instant::now();
        history
            .execute(SetVolume::new(1, -1.0), &mut project, &mut ids, t0)
            .expect("apply");
        history
            .execute(SetVolume::new(2, -2.0), &mut project, &mut ids, t0)
            .expect("apply");
        assert!(history.undo(&mut project));
        assert_eq!(volume(&project, 2), 0.0);
        assert_eq!(volume(&project, 1), -1.0);
    }
}
