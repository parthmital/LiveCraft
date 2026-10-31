//! The open Live Set, with its undo history and its audio engine (M04 item 9).
//!
//! Front ends read the Set through `project` and change it only through `execute`,
//! `transaction`, `undo`, `redo`, and `go_to`. After each change the session pushes the new values
//! to the audio engine, so no front end talks to the engine about the Set.

use crate::command::{Command, CommandError, Operation};
use crate::commands::clip::CreateMidiClip;
use crate::commands::note::AddNote;
use crate::history::{HistoryEntry, Transaction, UndoHistory};
use livecraft_engine::engine::AudioEngine;
use livecraft_formats::als::{AlsError, AlsParser};
use livecraft_model::ids::{IdAllocator, SceneId, TrackId};
use livecraft_model::model::{MidiNote, Project};
use std::path::Path;
use std::time::Instant;

pub use livecraft_engine::graph::TrackMeter;

/// Reads a `.als` Live Set from disk (manual 5.4.1).
pub fn load_set(path: &Path) -> Result<Project, AlsError> {
    AlsParser::load_project(path)
}

/// The Set shown when no file is opened: a four-beat chord clip on the first track, built with
/// commands. The clip is created in the first slot and gets nine notes.
pub fn demo_set() -> Result<Project, CommandError> {
    let mut project = Project::new_default("LiveCraft Session");
    let mut ids = project.id_allocator();
    let track = TrackId(1);
    CreateMidiClip::new(track, 0, "Intro Chords", 4.0).apply(&mut project, &mut ids)?;
    let clip = project
        .track(track)
        .and_then(|track| track.clip_slots.first())
        .and_then(|slot| slot.clip.as_ref())
        .map(|clip| clip.id)
        .ok_or(CommandError::SlotEmpty { track, slot: 0 })?;
    let chords: [(u8, f64, f64); 9] = [
        (60, 0.0, 1.0),
        (64, 0.0, 1.0),
        (67, 0.0, 1.0),
        (62, 1.0, 1.0),
        (65, 1.0, 1.0),
        (69, 1.0, 1.0),
        (64, 2.0, 2.0),
        (67, 2.0, 2.0),
        (71, 2.0, 2.0),
    ];
    for (pitch, start, length) in chords {
        let velocity = if start < 2.0 { 100 } else { 105 };
        AddNote::new(clip, MidiNote::new(pitch, start, length, velocity))
            .apply(&mut project, &mut ids)?;
    }
    Ok(project)
}

/// What the engine reports, read without waiting for the audio thread.
#[derive(Debug, Clone, Default)]
pub struct EngineStatus {
    pub playhead_beats: f64,
    pub track_meters: Vec<TrackMeter>,
    pub master_meter: TrackMeter,
    /// The clip slot each track is playing, by track position.
    pub active_slots: Vec<Option<usize>>,
    pub sample_rate: Option<f32>,
}

/// The mixer values last sent to the engine for one track, so only changed values are sent again.
#[derive(Debug, Clone, PartialEq)]
struct SentMix {
    id: TrackId,
    volume_db: f32,
    pan: f32,
    muted: bool,
    soloed: bool,
    sends: Vec<f32>,
}

impl SentMix {
    fn of(track: &livecraft_model::model::Track) -> Self {
        Self {
            id: track.id,
            volume_db: track.volume_db,
            pan: track.pan,
            muted: track.is_muted,
            soloed: track.is_soloed,
            sends: track.sends.clone(),
        }
    }
}

pub struct Session {
    project: Project,
    history: UndoHistory,
    ids: IdAllocator,
    engine: Option<AudioEngine>,
    dirty: bool,
    sent: Vec<SentMix>,
}

impl Session {
    /// Opens a Set. The engine, when there is one, was built from this Set.
    pub fn new(project: Project, engine: Option<AudioEngine>) -> Self {
        let ids = project.id_allocator();
        let sent = project.tracks.iter().map(SentMix::of).collect();
        Self {
            project,
            history: UndoHistory::default(),
            ids,
            engine,
            dirty: false,
            sent,
        }
    }

    /// Opens a Set with the default audio engine, when an audio device is available.
    pub fn open(project: Project) -> Self {
        let engine = AudioEngine::new(&project).ok();
        Self::new(project, engine)
    }

    /// Saves the Set to a `.als` file (manual 5.4.1) and records it as saved.
    pub fn save(&mut self, path: &Path) -> Result<(), AlsError> {
        AlsParser::save_project_to_als(&self.project, path)?;
        self.dirty = false;
        Ok(())
    }

    /// The Set as it is now. Front ends read it from here.
    pub fn project(&self) -> &Project {
        &self.project
    }

    /// The undo history, for the Undo History view and for the undo and redo state.
    pub fn history(&self) -> &UndoHistory {
        &self.history
    }

    /// The Undo History, newest first (manual 5.4.2).
    pub fn entries(&self) -> Vec<HistoryEntry> {
        self.history.entries()
    }

    /// Applies a command, records it for undo, and updates the engine.
    pub fn execute<O: Operation>(&mut self, operation: O) -> Result<(), CommandError> {
        let command: Box<dyn Command> = Box::new(operation);
        self.history
            .execute(command, &mut self.project, &mut self.ids, Instant::now())?;
        self.changed();
        Ok(())
    }

    /// Applies the commands a view asked for. One command is one undo step, which can merge into
    /// the previous step as a gesture. Several commands from one frame are one labelled step.
    pub fn apply(&mut self, commands: Vec<Box<dyn Command>>) -> Result<(), CommandError> {
        let mut commands = commands.into_iter();
        let Some(first) = commands.next() else {
            return Ok(());
        };
        let Some(second) = commands.next() else {
            self.history
                .execute(first, &mut self.project, &mut self.ids, Instant::now())?;
            self.changed();
            return Ok(());
        };
        let label = first.label();
        let rest = std::iter::once(first)
            .chain(std::iter::once(second))
            .chain(commands);
        self.transaction(label, |tx| {
            for command in rest {
                tx.run_command(command)?;
            }
            Ok(())
        })
    }

    /// Runs commands as one labelled undo step (see `UndoHistory::transaction`).
    pub fn transaction<F>(&mut self, label: &str, f: F) -> Result<(), CommandError>
    where
        F: FnOnce(&mut Transaction<'_>) -> Result<(), CommandError>,
    {
        self.history
            .transaction(label, &mut self.project, &mut self.ids, Instant::now(), f)?;
        self.changed();
        Ok(())
    }

    /// Undoes the newest step. Returns false when there is nothing to undo.
    pub fn undo(&mut self) -> bool {
        let undone = self.history.undo(&mut self.project);
        if undone {
            self.changed();
        }
        undone
    }

    /// Redoes the oldest undone step. Returns false when there is nothing to redo.
    pub fn redo(&mut self) -> Result<bool, CommandError> {
        let redone = self.history.redo(&mut self.project, &mut self.ids)?;
        if redone {
            self.changed();
        }
        Ok(redone)
    }

    /// Makes the Set the state at an entry of the Undo History (see `UndoHistory::go_to`).
    pub fn go_to(&mut self, index: usize) -> Result<bool, CommandError> {
        let moved = self
            .history
            .go_to(index, &mut self.project, &mut self.ids)?;
        if moved {
            self.changed();
        }
        Ok(moved)
    }

    /// True when the Set has changed since it was opened or last saved.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Records that the Set was saved.
    pub fn mark_saved(&mut self) {
        self.dirty = false;
    }

    fn changed(&mut self) {
        self.dirty = true;
        self.sync_engine();
    }

    /// Sends the Set to the engine: its structure, and every mixer value that changed.
    fn sync_engine(&mut self) {
        let Some(engine) = &self.engine else {
            return;
        };
        engine.sync_project(&self.project);
        let previous = std::mem::take(&mut self.sent);
        let current: Vec<SentMix> = self.project.tracks.iter().map(SentMix::of).collect();
        for (index, now) in current.iter().enumerate() {
            let before = previous.get(index).filter(|sent| sent.id == now.id);
            let Some(before) = before else {
                continue;
            };
            if (now.volume_db - before.volume_db).abs() > 0.01 {
                engine.set_track_volume(now.id, now.volume_db);
            }
            if (now.pan - before.pan).abs() > 0.01 {
                engine.set_track_pan(now.id, now.pan);
            }
            if now.muted != before.muted {
                engine.set_track_mute(now.id, now.muted);
            }
            if now.soloed != before.soloed {
                engine.set_track_solo(now.id, now.soloed);
            }
            for (send, level) in now.sends.iter().enumerate() {
                let sent = before.sends.get(send).copied().unwrap_or(0.0);
                if (level - sent).abs() > 0.01 {
                    engine.set_track_send(now.id, send, *level);
                }
            }
        }
        engine.set_master_volume(self.project.main_track.volume_db);
        engine.set_master_pan(self.project.main_track.pan);
        self.sent = current;
    }

    /// The engine's playhead, meters, and sample rate, or the Set's own playhead when no engine
    /// is running or the audio thread holds the graph.
    pub fn status(&self) -> EngineStatus {
        let fallback = EngineStatus {
            playhead_beats: self.project.transport.playhead_beats,
            ..EngineStatus::default()
        };
        let Some(engine) = &self.engine else {
            return fallback;
        };
        let sample_rate = Some(engine.sample_rate);
        match engine.graph.try_lock() {
            Ok(graph) => EngineStatus {
                playhead_beats: graph.playhead_beats,
                track_meters: graph.track_states.iter().map(|s| s.meter).collect(),
                master_meter: graph.master_meter,
                active_slots: graph
                    .track_states
                    .iter()
                    .map(|s| s.active_clip_slot)
                    .collect(),
                sample_rate,
            },
            Err(_) => EngineStatus {
                sample_rate,
                ..fallback
            },
        }
    }

    /// Starts playback, or stops it, and records the state in the Set's transport.
    pub fn set_playing(&mut self, playing: bool) {
        self.project.transport.is_playing = playing;
        if let Some(engine) = &self.engine {
            if playing {
                engine.play();
            } else {
                engine.stop();
            }
        }
    }

    /// Starts or stops recording. Recording is transport state, so it is not an undo step.
    pub fn set_recording(&mut self, recording: bool) {
        self.project.transport.is_recording = recording;
    }

    /// Moves the playhead, in beats, in the Set and in the engine.
    pub fn set_playhead(&mut self, beats: f64) {
        self.project.transport.playhead_beats = beats;
        if let Some(engine) = &self.engine {
            engine.set_playhead(beats);
        }
    }

    pub fn trigger_clip(&mut self, track: TrackId, slot: usize) {
        if let Some(engine) = &self.engine {
            engine.trigger_clip(track, slot);
            engine.play();
        }
    }

    pub fn stop_clip(&mut self, track: TrackId) {
        if let Some(engine) = &self.engine {
            engine.stop_clip(track);
        }
    }

    pub fn trigger_scene(&mut self, scene: SceneId) {
        if let Some(engine) = &self.engine {
            engine.trigger_scene(scene);
            engine.play();
        }
    }

    /// Stops every clip and playback, and returns the playhead to the start.
    pub fn stop_all(&mut self) {
        self.set_playing(false);
        self.set_playhead(0.0);
    }
}
