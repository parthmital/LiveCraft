//! The open Live Set, with its undo history and its audio engine (M04 item 9).
//!
//! Front ends read the Set through `project` and change it only through `execute`,
//! `transaction`, `undo`, `redo`, and `go_to`. After each change the session pushes the new values
//! to the audio engine, so no front end talks to the engine about the Set.

use crate::command::{Command, CommandError, Operation};
use crate::commands::clip::CreateMidiClip;
use crate::commands::note::AddNote;
use crate::engine_sync::{push_changes, send_command};
use crate::history::{HistoryEntry, Transaction, UndoHistory};
use crate::preferences::{AudioPreferences, MidiPreferences, Preferences, PreferencesError};
use livecraft_engine::driver::{
    AudioDriver, CpalDriver, DeviceList, DriverConfig, DriverError, DriverStatus, Latency,
    NullDriver, list_devices,
};
use livecraft_engine::executor::EngineCommand;
use livecraft_engine::midi_io::{MidiPorts, list_midi_ports};
use livecraft_engine::output::Playback;
use livecraft_formats::als::{AlsError, AlsParser};
use livecraft_model::ids::{IdAllocator, SceneId, TrackId};
use livecraft_model::model::{MidiNote, Project};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

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
    project: Project,
    history: UndoHistory,
    ids: IdAllocator,
    engine: Option<Playback>,
    dirty: bool,
    /// The Set as the engine last received it, so each change sends only what differs.
    sent: Project,
    /// The preferences the Set was opened with, and the audio settings changed since.
    preferences: Preferences,
    /// Builds the driver for each audio output the session opens.
    driver_factory: DriverFactory,
    /// Where the preferences are saved after an audio change, or `None` when there is no file.
    prefs_path: Option<PathBuf>,
    /// Why the audio output is not the one the preferences name, for the status bar.
    audio_error: Option<String>,
    /// When to try the preferred output again, while the Set plays on the silent driver.
    audio_retry: Option<Instant>,
    /// The state handed to the running output when it started. It is carried over again when the
    /// output is replaced before its engine has published a snapshot.
    carried: Option<Carried>,
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
        )
    }

    fn from_parts(
        project: Project,
        engine: Option<Playback>,
        driver_factory: DriverFactory,
        preferences: Preferences,
        prefs_path: Option<PathBuf>,
    ) -> Self {
        let ids = project.id_allocator();
        let sent = project.clone();
        Self {
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
        }
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
        let preferences = prefs_path
            .as_deref()
            .map_or_else(Preferences::default, load_preferences);
        let engine = open_engine(&project, &factory, &preferences);
        Self::from_parts(project, engine, factory, preferences, prefs_path)
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
        let mut next = Playback::open_with_driver(&self.project, driver, &ports)
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
    pub fn set_playing(&mut self, playing: bool) {
        self.project.transport.is_playing = playing;
        self.send(EngineCommand::SetPlaying(playing));
        if !playing {
            self.send(EngineCommand::StopAll);
        }
    }

    /// Starts or stops recording. Recording is transport state, so it is not an undo step.
    pub fn set_recording(&mut self, recording: bool) {
        self.project.transport.is_recording = recording;
    }

    /// Moves the playhead, in beats, in the Set and in the engine.
    pub fn set_playhead(&mut self, beats: f64) {
        self.project.transport.playhead_beats = beats;
        self.send(EngineCommand::SetPlayhead(beats));
    }

    pub fn trigger_clip(&mut self, track: TrackId, slot: usize) {
        self.send(EngineCommand::TriggerClip { track, slot });
        self.send(EngineCommand::SetPlaying(true));
    }

    pub fn stop_clip(&mut self, track: TrackId) {
        self.send(EngineCommand::StopClip { track });
    }

    pub fn trigger_scene(&mut self, scene: SceneId) {
        self.send(EngineCommand::TriggerScene { scene });
        self.send(EngineCommand::SetPlaying(true));
    }

    /// Stops every clip and playback, and returns the playhead to the start.
    pub fn stop_all(&mut self) {
        self.set_playing(false);
        self.set_playhead(0.0);
    }
}

/// The preferences at `path`, or the defaults when the file cannot be read.
fn load_preferences(path: &Path) -> Preferences {
    Preferences::load(path).unwrap_or_else(|error| {
        log::warn!("The preferences could not be read, so the defaults are used: {error}");
        Preferences::default()
    })
}

/// The driver `Session::open` uses: a `CpalDriver` for the configuration.
fn cpal_driver(config: &DriverConfig) -> Result<Box<dyn AudioDriver>, DriverError> {
    Ok(Box::new(CpalDriver::open(config)?))
}

/// Starts `project` on the output that `factory` builds for `config`, with the MIDI inputs that `midi`
/// names feeding it.
fn open_output(
    project: &Project,
    factory: &DriverFactory,
    config: &DriverConfig,
    midi: &MidiPreferences,
) -> Result<Playback, SessionAudioError> {
    let driver = factory(config)?;
    Playback::open_with_driver(project, driver, &midi_ports(midi))
        .map_err(SessionAudioError::Output)
}

/// The MIDI ports that `midi` asks for, among the ports this machine offers.
fn midi_ports(midi: &MidiPreferences) -> MidiPorts {
    MidiPorts {
        inputs: midi.input_ports_to_open(&list_midi_ports().inputs),
        outputs: midi.output_ports_to_open(),
    }
}

/// Opens the output and the MIDI inputs that `preferences` name. When that output cannot be opened, the
/// default output is tried. The result is `None` when neither opens, and the Set then plays without audio.
fn open_engine(
    project: &Project,
    factory: &DriverFactory,
    preferences: &Preferences,
) -> Option<Playback> {
    open_output(
        project,
        factory,
        &DriverConfig::from(&preferences.audio),
        &preferences.midi,
    )
    .or_else(|error| {
        log::warn!(
            "The audio output in the preferences could not be opened, so the default output is used: {error}"
        );
        open_output(
            project,
            factory,
            &DriverConfig::default(),
            &preferences.midi,
        )
    })
        .map_err(|error| log::warn!("No audio output could be opened: {error}"))
        .ok()
}

/// The playhead, the transport, and the launched clips of an output, carried over to the next one.
#[derive(Debug, Clone)]
struct Carried {
    playhead: f64,
    playing: bool,
    launched: Vec<(TrackId, usize)>,
}

impl Carried {
    /// Reads them from the latest snapshot of `playback`, so they are as the audio thread last
    /// published them. Before the engine has published, they are `pending`, the state it was started
    /// with.
    fn read(playback: &mut Playback, project: &Project, pending: Option<Carried>) -> Option<Self> {
        let snapshot = playback.handle.snapshot();
        if snapshot.frames == 0 {
            return pending;
        }
        let launched = project
            .tracks
            .iter()
            .take(snapshot.node_count)
            .zip(&snapshot.nodes)
            .filter_map(|(track, node)| node.clip.active_slot.map(|slot| (track.id, slot)))
            .collect();
        Some(Self {
            playhead: snapshot.transport.playhead_beats,
            playing: snapshot.transport.playing,
            launched,
        })
    }

    fn apply(&self, next: &mut Playback) {
        for &(track, slot) in &self.launched {
            send_command(next, EngineCommand::TriggerClip { track, slot });
        }
        send_command(next, EngineCommand::SetPlayhead(self.playhead));
        send_command(next, EngineCommand::SetPlaying(self.playing));
    }
}
