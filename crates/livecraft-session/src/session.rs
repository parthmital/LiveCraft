//! The open Live Set, with its undo history and its audio engine (M04 item 9).
//!
//! Front ends read the Set through `project` and change it only through `execute`,
//! `transaction`, `undo`, `redo`, and `go_to`. After each change the session pushes the new values
//! to the audio engine, so no front end talks to the engine about the Set.

use crate::audio_setup::{Carried, cpal_driver, load_preferences, midi_ports, open_engine};
use crate::command::{Command, CommandError, Operation};
use crate::commands::clip::CreateMidiClip;
use crate::commands::note::AddNote;
use crate::engine_sync::{push_changes, send_command};
use crate::history::{HistoryEntry, Transaction, UndoHistory};
use crate::preferences::{AudioPreferences, MidiPreferences, Preferences, PreferencesError};
use crate::project_ops::build_project_context;
use crate::transport::TransportControl;
use livecraft_engine::driver::{
    AudioDriver, DeviceList, DriverConfig, DriverError, DriverStatus, Latency, NullDriver,
    list_devices,
};
use livecraft_engine::executor::EngineCommand;
use livecraft_engine::midi_io::{MidiPorts, list_midi_ports};
use livecraft_engine::output::Playback;
use livecraft_formats::als::{AlsDocument, AlsError, AlsParser};
use livecraft_formats::project_dir::{MissingFile, ProjectContext};
use livecraft_model::ids::{IdAllocator, SceneId, TrackId};
use livecraft_model::model::{MidiNote, Project};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub use crate::project_ops::{ExportClipOutcome, SessionFormatError};

/// How long the session waits between attempts to reopen a lost audio output (M06 item 8).
pub const AUDIO_RETRY: Duration = Duration::from_secs(2);

pub use livecraft_engine::snapshot::TrackMeter;

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

/// Builds the audio driver for a configuration. `Session::open` builds a `CpalDriver`, and tests
/// build a `NullDriver`.
pub type DriverFactory = Box<dyn Fn(&DriverConfig) -> Result<Box<dyn AudioDriver>, DriverError>>;

/// Why the audio output of a Set could not be opened or changed.
#[derive(Debug, thiserror::Error)]
pub enum SessionAudioError {
    /// The driver for the settings could not be built.
    #[error("the audio driver could not be opened: {0}")]
    Driver(#[from] DriverError),
    /// The driver was built, but the engine could not start on it.
    #[error("the audio output could not start: {0:#}")]
    Output(anyhow::Error),
    /// The new output is in use, but the preferences file could not be written.
    #[error("the new audio settings are in use, but the preferences were not saved: {0}")]
    Save(#[from] PreferencesError),
}

pub struct Session {
    pub(crate) project: Project,
    history: UndoHistory,
    ids: IdAllocator,
    pub(crate) engine: Option<Playback>,
    pub(crate) dirty: bool,
    /// The Set as the engine last received it, so each change sends only what differs.
    sent: Project,
    /// The preferences the Set was opened with, and the audio/library settings changed since.
    pub(crate) preferences: Preferences,
    /// Builds the driver for each audio output the session opens.
    driver_factory: DriverFactory,
    /// Where the preferences are saved after a change, or `None` when there is no file.
    pub(crate) prefs_path: Option<PathBuf>,
    /// The insert marker and the stop position that Play, Continue, and Stop use (manual 6.3).
    transport: TransportControl,
    /// Why the audio output is not the one the preferences name, for the status bar.
    audio_error: Option<String>,
    /// When to try the preferred output again, while the Set plays on the silent driver.
    audio_retry: Option<Instant>,
    /// The state handed to the running output when it started. It is carried over again when the
    /// output is replaced before its engine has published a snapshot.
    carried: Option<Carried>,
    /// Lossless `.als` DOM kept alongside the Set so saves patch only modified nodes.
    pub(crate) als_doc: Option<AlsDocument>,
    /// Path of the open or last-saved `.als` file, if any.
    pub(crate) set_path: Option<PathBuf>,
    /// Project directory, library roots, and set folder used to resolve sample references.
    pub(crate) project_context: ProjectContext,
    /// Missing file references detected against `project_context`.
    pub(crate) missing_files: Vec<MissingFile>,
}

impl Session {
    /// Opens a Set. The engine, when there is one, was started from this Set.
    pub fn new(project: Project, engine: Option<Playback>) -> Self {
        Self::from_parts(
            project,
            engine,
            Box::new(cpal_driver),
            Preferences::default(),
            None,
            None,
            None,
        )
    }

    /// Opens a Set from a parsed lossless [`AlsDocument`]. The engine, when there is one, was
    /// started from this Set.
    pub fn from_als_document(doc: AlsDocument, engine: Option<Playback>) -> Self {
        let project = doc.project().clone();
        Self::from_parts(
            project,
            engine,
            Box::new(cpal_driver),
            Preferences::default(),
            None,
            Some(doc),
            None,
        )
    }

    /// Opens a `.als` file losslessly without starting an audio output (headless).
    pub fn open_als(path: &Path) -> Result<Self, AlsError> {
        let doc = AlsDocument::open(path)?;
        let project = doc.project().clone();
        Ok(Self::from_parts(
            project,
            None,
            Box::new(cpal_driver),
            Preferences::default(),
            None,
            Some(doc),
            Some(path.to_path_buf()),
        ))
    }

    /// Opens a `.als` file losslessly with the audio output and MIDI inputs that the preferences
    /// file names (manual 5.4.1, 41.2, 2.3), or the default output when that one cannot be opened.
    pub fn open_file(path: &Path) -> Result<Self, AlsError> {
        Self::open_als_with_driver(path, Box::new(cpal_driver), Preferences::default_path())
    }

    /// Opens a `.als` file losslessly with the audio output that `factory` builds for the
    /// preferences at `prefs_path`.
    pub fn open_als_with_driver(
        path: &Path,
        factory: DriverFactory,
        prefs_path: Option<PathBuf>,
    ) -> Result<Self, AlsError> {
        let doc = AlsDocument::open(path)?;
        let project = doc.project().clone();
        Ok(Self::with_driver_and_doc(
            project,
            factory,
            prefs_path,
            Some(doc),
            Some(path.to_path_buf()),
        ))
    }

    /// Opens a Set from a parsed lossless [`AlsDocument`] with the audio output that `factory`
    /// builds for the preferences at `prefs_path`.
    pub fn from_als_with_driver(
        doc: AlsDocument,
        factory: DriverFactory,
        prefs_path: Option<PathBuf>,
    ) -> Self {
        let project = doc.project().clone();
        Self::with_driver_and_doc(project, factory, prefs_path, Some(doc), None)
    }

    /// Attaches or replaces the lossless [`AlsDocument`] used when saving this Set.
    pub fn attach_als_document(&mut self, doc: AlsDocument) {
        self.als_doc = Some(doc);
        self.refresh_missing_files();
    }

    /// Returns the lossless [`AlsDocument`] attached to this Set, if any.
    pub fn als_document(&self) -> Option<&AlsDocument> {
        self.als_doc.as_ref()
    }

    fn from_parts(
        project: Project,
        engine: Option<Playback>,
        driver_factory: DriverFactory,
        preferences: Preferences,
        prefs_path: Option<PathBuf>,
        als_doc: Option<AlsDocument>,
        set_path: Option<PathBuf>,
    ) -> Self {
        let ids = project.id_allocator();
        let sent = project.clone();
        let project_context = build_project_context(set_path.as_deref(), &preferences.library);
        let mut session = Self {
            project,
            history: UndoHistory::default(),
            ids,
            engine,
            dirty: false,
            sent,
            preferences,
            driver_factory,
            prefs_path,
            audio_error: None,
            audio_retry: None,
            carried: None,
            transport: TransportControl::default(),
            als_doc,
            set_path,
            project_context,
            missing_files: Vec::new(),
        };
        session.refresh_missing_files();
        session
    }

    /// Opens a Set with the audio output and the MIDI inputs that the preferences file names (manual
    /// 41.2 and 2.3), or the default output when that one cannot be opened.
    pub fn open(project: Project) -> Self {
        Self::with_driver_factory(project, Box::new(cpal_driver), Preferences::default_path())
    }

    /// Opens a Set with the audio output that `factory` builds for the preferences at `prefs_path`, and
    /// the MIDI inputs those preferences name. A missing or unreadable file gives the defaults. When the
    /// named output cannot be opened, the default output is used instead. Changes made by
    /// `set_audio_preferences` are saved to `prefs_path`, when there is one.
    pub fn with_driver_factory(
        project: Project,
        factory: DriverFactory,
        prefs_path: Option<PathBuf>,
    ) -> Self {
        Self::with_driver_and_doc(project, factory, prefs_path, None, None)
    }

    fn with_driver_and_doc(
        project: Project,
        factory: DriverFactory,
        prefs_path: Option<PathBuf>,
        als_doc: Option<AlsDocument>,
        set_path: Option<PathBuf>,
    ) -> Self {
        let preferences = prefs_path
            .as_deref()
            .map_or_else(Preferences::default, load_preferences);
        let project_context = build_project_context(set_path.as_deref(), &preferences.library);
        let engine = open_engine(&project, &factory, &preferences, &project_context);
        Self::from_parts(
            project,
            engine,
            factory,
            preferences,
            prefs_path,
            als_doc,
            set_path,
        )
    }

    /// The MIDI input and output ports this machine offers, for the settings (manual 2.3).
    pub fn midi_ports() -> MidiPorts {
        list_midi_ports()
    }

    /// The input and output latency of the running output, and its sample rate, for the settings
    /// (manual 2.3). None when the Set plays without audio.
    pub fn latency(&self) -> Option<(Latency, u32)> {
        self.engine
            .as_ref()
            .map(|engine| (engine.latency(), engine.sample_rate as u32))
    }

    /// Changes which MIDI ports feed the engine and receive notes (M06 item 9, manual 2.3). The output
    /// is reopened with the same audio settings so the ports change, and playback carries on. The new
    /// settings are saved when the session has a preferences file.
    pub fn set_midi_preferences(&mut self, midi: MidiPreferences) -> Result<(), SessionAudioError> {
        self.preferences.midi = midi;
        let driver = (self.driver_factory)(&DriverConfig::from(&self.preferences.audio))?;
        if let Err(error) = self.switch_output(driver) {
            self.fall_back(error.to_string(), Instant::now());
            return Err(error);
        }
        if let Some(path) = &self.prefs_path {
            self.preferences.save(path)?;
        }
        Ok(())
    }

    /// The audio hosts and devices this machine offers, for the settings and `livecraft devices`
    /// (M06 item 3, manual 41.2).
    pub fn audio_devices() -> DeviceList {
        list_devices()
    }

    /// The preferences the Set was opened with, and the audio settings changed since.
    pub fn preferences(&self) -> &Preferences {
        &self.preferences
    }

    /// Changes the audio output to `audio` (M06 item 4). The engine is rebuilt at the new sample rate
    /// and buffer size, and playback carries on: the playhead, whether the transport plays, and the
    /// clip each track has launched. When the new output cannot be opened, the current output keeps
    /// running and nothing changes. The new settings are saved when the session has a preferences
    /// file. An error from saving means the new output is in use but the file was not written.
    pub fn set_audio_preferences(
        &mut self,
        audio: AudioPreferences,
    ) -> Result<(), SessionAudioError> {
        let driver = (self.driver_factory)(&DriverConfig::from(&audio))?;
        if let Err(error) = self.switch_output(driver) {
            // The old output is closed by now, so the Set plays on silently and retries.
            self.fall_back(error.to_string(), Instant::now());
            return Err(error);
        }
        self.audio_error = None;
        self.audio_retry = None;
        self.preferences.audio = audio;
        if let Some(path) = &self.prefs_path {
            self.preferences.save(path)?;
        }
        Ok(())
    }

    /// Why the audio output is not the one the preferences name, or None when it is. The status bar
    /// shows it.
    pub fn audio_error(&self) -> Option<&str> {
        self.audio_error.as_deref()
    }

    /// Checks the audio output (M06 item 8). The app calls it every frame. When the device is lost, the
    /// Set carries on on a silent driver at the same rate, so the transport keeps running, and the error
    /// is reported. From then on, the preferred output is tried again every `AUDIO_RETRY`.
    pub fn poll_audio(&mut self) {
        self.poll_audio_at(Instant::now());
    }

    /// As `poll_audio`, at the time `now`. Tests pass the time, so they need not wait for a retry.
    pub fn poll_audio_at(&mut self, now: Instant) {
        if let Some(engine) = self.engine.as_ref()
            && let DriverStatus::Lost(message) = engine.driver_status()
        {
            log::warn!("The audio output was lost, so the Set plays on silently: {message}");
            self.fall_back(message, now);
            return;
        }
        if self.audio_retry.is_none_or(|at| now < at) {
            return;
        }
        let config = DriverConfig::from(&self.preferences.audio);
        let result = (self.driver_factory)(&config)
            .map_err(SessionAudioError::from)
            .and_then(|driver| self.switch_output(driver));
        match result {
            Ok(()) => {
                log::info!("The audio output is back");
                self.audio_error = None;
                self.audio_retry = None;
            }
            Err(SessionAudioError::Driver(error)) => {
                // The device is still missing, and the silent driver kept running.
                log::debug!("The audio output is still unavailable: {error}");
                self.audio_retry = Some(now + AUDIO_RETRY);
            }
            Err(error) => {
                // The silent driver was closed to start the device, so start it again.
                self.fall_back(error.to_string(), now);
            }
        }
    }

    /// Plays the Set on `driver` from where the current output is: the playhead, the transport, and
    /// the launched clips carry over. The current output, with its MIDI ports, is closed first, since a
    /// MIDI port may not open twice.
    fn switch_output(&mut self, driver: Box<dyn AudioDriver>) -> Result<(), SessionAudioError> {
        let pending = self.carried.take();
        let carried = match self.engine.as_mut() {
            Some(previous) => Carried::read(previous, &self.project, pending),
            None => pending,
        };
        self.engine = None;
        let ports = midi_ports(&self.preferences.midi);
        let mut next = Playback::open_with_driver_in_context(
            &self.project,
            driver,
            &ports,
            self.project_context.clone(),
        )
        .map_err(SessionAudioError::Output)?;
        if let Some(carried) = &carried {
            carried.apply(&mut next);
        }
        self.carried = carried;
        self.engine = Some(next);
        Ok(())
    }

    /// Moves the Set onto a silent driver at the current rate and buffer size, records `message`, and
    /// schedules a retry of the preferred output.
    fn fall_back(&mut self, message: String, now: Instant) {
        // Without a running output, the preferred rate and buffer size, or 44.1 kHz and 512 frames.
        let audio = &self.preferences.audio;
        let preferred = (
            audio.sample_rate.unwrap_or(44_100),
            audio.buffer_size.unwrap_or(512),
            2,
        );
        let (rate, frames, channels) = self.engine.as_ref().map_or(preferred, |engine| {
            (
                engine.sample_rate as u32,
                engine.buffer_frames().unwrap_or(preferred.1),
                engine.channels().unwrap_or(preferred.2),
            )
        });
        if let Err(error) = self.switch_output(Box::new(NullDriver::new(rate, frames, channels))) {
            log::error!("The silent audio driver could not start: {error}");
        }
        self.audio_error = Some(message);
        self.audio_retry = Some(now + AUDIO_RETRY);
    }

    /// Saves the Set to a `.als` file (manual 5.4.1) and records it as saved.
    ///
    /// When the Set holds an [`AlsDocument`], only modified portions of its XML tree are patched so
    /// unmodeled XML elements and attributes survive untouched. Otherwise a new document is built
    /// from the current Set and retained for subsequent saves.
    pub fn save(&mut self, path: &Path) -> Result<(), AlsError> {
        if let Some(ref mut doc) = self.als_doc {
            doc.apply_project(&self.project)?;
            doc.save_als(path)?;
        } else {
            let doc = AlsDocument::from_project(&self.project)?;
            doc.save_als(path)?;
            self.als_doc = Some(doc);
        }
        self.dirty = false;
        self.set_path = Some(path.to_path_buf());
        self.update_project_context();
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
        self.refresh_missing_files();
    }

    /// Sends the change since the last sync to the engine (see `engine_sync`).
    fn sync_engine(&mut self) {
        if let Some(engine) = self.engine.as_mut() {
            push_changes(engine, &self.sent, &self.project);
        }
        self.sent = self.project.clone();
    }

    fn send(&mut self, command: EngineCommand) {
        if let Some(engine) = self.engine.as_mut() {
            send_command(engine, command);
        }
    }

    /// The engine's playhead, meters, and clip states, read from its latest snapshot. Without an
    /// engine, the Set's own playhead and no levels.
    pub fn status(&mut self) -> EngineStatus {
        let Some(engine) = self.engine.as_mut() else {
            return EngineStatus {
                playhead_beats: self.project.transport.playhead_beats,
                ..EngineStatus::default()
            };
        };
        let snapshot = engine.handle.snapshot();
        let tracks = self.project.tracks.len();
        let nodes = &snapshot.nodes[..snapshot.node_count.min(tracks)];
        EngineStatus {
            playhead_beats: snapshot.transport.playhead_beats,
            track_meters: nodes.iter().map(|node| node.meter).collect(),
            master_meter: snapshot.output,
            active_slots: nodes.iter().map(|node| node.clip.active_slot).collect(),
            sample_rate: Some(engine.sample_rate),
        }
    }

    /// Starts playback, or stops it, and records the state in the Set's transport. Stopping also
    /// stops the clips, as the Live transport does.
    /// Plays from the insert marker, or stops (manual 6.3). The space bar does this.
    pub fn set_playing(&mut self, playing: bool) {
        if playing {
            self.play();
        } else {
            self.stop();
        }
    }

    /// The Play button: starts playback from the insert marker (manual 6.3, page 169).
    pub fn play(&mut self) {
        let start = self.transport.play();
        self.start_at(start);
    }

    /// Shift and the space bar: continues playback from where it last stopped (manual 6.3, page 169).
    pub fn continue_play(&mut self) {
        let start = self.transport.continue_play();
        self.start_at(start);
    }

    fn start_at(&mut self, start: f64) {
        self.project.transport.is_playing = true;
        self.project.transport.playhead_beats = start;
        self.send(EngineCommand::SetPlayhead(start));
        self.send(EngineCommand::SetPlaying(true));
    }

    /// The Stop button. Stopping stops the Session clips too. A second press while stopped returns the
    /// insert marker to the start, as a double-click does (manual 6.3, page 169).
    pub fn stop(&mut self) {
        let position = self.status().playhead_beats;
        let home = self.transport.stop(position);
        self.project.transport.is_playing = false;
        self.send(EngineCommand::SetPlaying(false));
        if home {
            self.project.transport.playhead_beats = 0.0;
            self.send(EngineCommand::SetPlayhead(0.0));
        }
    }

    /// The Arrangement Record button (manual 19.3.1). Recording is transport state, so it is not an
    /// undo step. With a count-in set, the next start counts in first (manual 19.6).
    pub fn set_recording(&mut self, recording: bool) {
        self.project.transport.is_recording = recording;
        self.send(EngineCommand::SetRecording(recording));
    }

    /// Moves the insert marker and the playhead, in beats, in the Set and in the engine.
    pub fn set_playhead(&mut self, beats: f64) {
        self.transport.set_insert_marker(beats);
        self.project.transport.playhead_beats = beats;
        self.send(EngineCommand::SetPlayhead(beats));
    }

    /// Jumps to a song position at the next global quantization step while playing, as a locator does
    /// (manual 6.4, page 171), and moves the insert marker there.
    pub fn jump_to(&mut self, beats: f64) {
        self.transport.set_insert_marker(beats);
        self.project.transport.playhead_beats = beats;
        self.send(EngineCommand::JumpTo(beats));
    }

    /// Presses a clip's launch button at full velocity. The transport starts if it is stopped.
    pub fn trigger_clip(&mut self, track: TrackId, slot: usize) {
        self.press_clip(track, slot, 127);
    }

    /// Presses a clip's launch button with a MIDI velocity (manual 16.2 and 16.5).
    pub fn press_clip(&mut self, track: TrackId, slot: usize, velocity: u8) {
        self.transport.playing = true;
        self.project.transport.is_playing = true;
        self.send(EngineCommand::PressClip {
            track,
            slot,
            velocity,
        });
    }

    /// Releases a clip's launch button, which stops a Gate clip and ends a Repeat (manual 16.2).
    pub fn release_clip(&mut self, track: TrackId, slot: usize) {
        self.send(EngineCommand::ReleaseClip { track, slot });
    }

    /// Jumps within a track's playing clip by the global quantization period (manual 16.6).
    pub fn nudge_clip(&mut self, track: TrackId, forward: bool) {
        self.send(EngineCommand::NudgeClip { track, forward });
    }

    /// A track's Clip Stop button, at the next global quantization step.
    pub fn stop_clip(&mut self, track: TrackId) {
        self.send(EngineCommand::StopClip { track });
    }

    /// Launches a scene. The transport starts if it is stopped.
    pub fn trigger_scene(&mut self, scene: SceneId) {
        self.transport.playing = true;
        self.project.transport.is_playing = true;
        self.send(EngineCommand::TriggerScene { scene });
    }

    /// The Stop All Clips button: stops every Session clip at the next global quantization step. The
    /// transport keeps running.
    pub fn stop_all(&mut self) {
        self.send(EngineCommand::StopAll);
    }

    /// The Back to Arrangement button (manual 3.7, page 42).
    pub fn back_to_arrangement(&mut self) {
        self.send(EngineCommand::BackToArrangement);
    }
}
