//! Audio driver opening, MIDI port mapping, and playback state carryover helpers for `Session`.

use crate::engine_sync::send_command;
use crate::preferences::{MidiPreferences, Preferences};
use crate::session::{DriverFactory, SessionAudioError};
use livecraft_engine::driver::{AudioDriver, CpalDriver, DriverConfig, DriverError};
use livecraft_engine::executor::EngineCommand;
use livecraft_engine::midi_io::{MidiPorts, list_midi_ports};
use livecraft_engine::output::Playback;
use livecraft_model::ids::TrackId;
use livecraft_model::model::Project;
use std::path::Path;

/// The preferences at `path`, or the defaults when the file cannot be read.
pub(crate) fn load_preferences(path: &Path) -> Preferences {
    Preferences::load(path).unwrap_or_else(|error| {
        log::warn!("The preferences could not be read, so the defaults are used: {error}");
        Preferences::default()
    })
}

/// The driver `Session::open` uses: a `CpalDriver` for the configuration.
pub(crate) fn cpal_driver(config: &DriverConfig) -> Result<Box<dyn AudioDriver>, DriverError> {
    Ok(Box::new(CpalDriver::open(config)?))
}

/// Starts `project` on the output that `factory` builds for `config`, with the MIDI inputs that
/// `midi` names feeding it.
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
pub(crate) fn midi_ports(midi: &MidiPreferences) -> MidiPorts {
    MidiPorts {
        inputs: midi.input_ports_to_open(&list_midi_ports().inputs),
        outputs: midi.output_ports_to_open(),
    }
}

/// Opens the output and the MIDI inputs that `preferences` name. When that output cannot be opened,
/// the default output is tried. The result is `None` when neither opens, and the Set then plays
/// without audio.
pub(crate) fn open_engine(
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
pub(crate) struct Carried {
    playhead: f64,
    playing: bool,
    launched: Vec<(TrackId, usize)>,
}

impl Carried {
    /// Reads them from the latest snapshot of `playback`, so they are as the audio thread last
    /// published them. Before the engine has published, they are `pending`, the state it was
    /// started with.
    pub(crate) fn read(
        playback: &mut Playback,
        project: &Project,
        pending: Option<Carried>,
    ) -> Option<Self> {
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

    pub(crate) fn apply(&self, next: &mut Playback) {
        for &(track, slot) in &self.launched {
            send_command(next, EngineCommand::TriggerClip { track, slot });
        }
        send_command(next, EngineCommand::SetPlayhead(self.playhead));
        send_command(next, EngineCommand::SetPlaying(self.playing));
    }
}
