//! Opening, changing, losing, and recovering the audio output of a Set (M06 items 3, 4, and 8). The
//! factories build `NullDriver`s, so these tests need no audio device.

use livecraft_engine::driver::{
    AudioDriver, DriverConfig, DriverError, DriverStatus, Latency, NullDriver,
};
use livecraft_engine::executor::Engine;
use livecraft_model::ids::TrackId;
use livecraft_model::model::Project;
use livecraft_session::{
    AUDIO_RETRY, AudioPreferences, DriverFactory, Preferences, Session, SessionAudioError, demo_set,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread::sleep;
use std::time::{Duration, Instant};

fn demo() -> Project {
    demo_set().expect("the demo Set builds")
}

/// A silent driver for `config`. A missing sample rate is 44.1 kHz, and a missing buffer size is
/// 512 frames.
fn null_driver(config: &DriverConfig) -> Box<dyn AudioDriver> {
    Box::new(NullDriver::new(
        config.sample_rate.unwrap_or(44_100),
        config.buffer_frames.unwrap_or(512),
        2,
    ))
}

/// A factory that builds a silent driver for every configuration.
fn null_factory() -> DriverFactory {
    Box::new(
        |config: &DriverConfig| -> Result<Box<dyn AudioDriver>, DriverError> {
            Ok(null_driver(config))
        },
    )
}

/// A factory that refuses `refused` as a sample rate, and builds a silent driver for the others.
fn factory_refusing(refused: u32) -> DriverFactory {
    Box::new(
        move |config: &DriverConfig| -> Result<Box<dyn AudioDriver>, DriverError> {
            if config.sample_rate == Some(refused) {
                return Err(DriverError::UnsupportedSampleRate {
                    device: "Test device".to_string(),
                    rate: refused,
                });
            }
            Ok(null_driver(config))
        },
    )
}

/// A preferences file path in a folder of this test's own, emptied first. The file is not written.
fn prefs_path(test: &str) -> PathBuf {
    let folder = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("audio-settings")
        .join(test);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).expect("create the test folder");
    folder.join("Preferences.toml")
}

/// A preferences file in this test's folder, holding `text`.
fn prefs_file(test: &str, text: &str) -> PathBuf {
    let path = prefs_path(test);
    std::fs::write(&path, text).expect("write the preferences");
    path
}

fn audio(sample_rate: u32, buffer_frames: u32) -> AudioPreferences {
    AudioPreferences {
        sample_rate: Some(sample_rate),
        buffer_size: Some(buffer_frames),
        ..AudioPreferences::default()
    }
}

/// Polls `condition` until it holds, and fails with `what` when that takes longer than `limit`.
fn wait_for(limit: Duration, what: &str, mut condition: impl FnMut() -> bool) {
    let started = Instant::now();
    while !condition() {
        assert!(
            started.elapsed() < limit,
            "{what} did not happen within {limit:?}"
        );
        sleep(Duration::from_millis(2));
    }
}

#[test]
fn a_missing_preferences_file_opens_the_set_with_the_defaults() {
    let session = Session::with_driver_factory(demo(), null_factory(), Some(prefs_path("missing")));
    assert_eq!(session.preferences(), &Preferences::default());
}

#[test]
fn the_preferences_file_is_read_when_the_set_opens() {
    let path = prefs_file("read", "[audio]\nbuffer_size = 128\n");
    let session = Session::with_driver_factory(demo(), null_factory(), Some(path));
    assert_eq!(session.preferences().audio.buffer_size, Some(128));
}

#[test]
fn an_invalid_preferences_file_opens_the_set_with_the_defaults() {
    let path = prefs_file("invalid", "[audio\nbuffer_size = ");
    let session = Session::with_driver_factory(demo(), null_factory(), Some(path));
    assert_eq!(session.preferences(), &Preferences::default());
}

#[test]
fn a_preferred_output_that_cannot_open_falls_back_to_the_default_output() {
    let path = prefs_file("fallback", "[audio]\nsample_rate = 12345\n");
    let mut session = Session::with_driver_factory(demo(), factory_refusing(12_345), Some(path));
    assert_eq!(session.preferences().audio.sample_rate, Some(12_345));
    assert_eq!(session.status().sample_rate, Some(44_100.0));
}

#[test]
fn a_playing_set_resumes_within_500_ms_of_a_buffer_size_and_sample_rate_change() {
    let mut session = Session::with_driver_factory(demo(), null_factory(), None);
    session.set_playing(true);
    sleep(Duration::from_millis(100));
    let before = session.status().playhead_beats;
    assert!(before > 0.0, "the transport should move before the change");

    session
        .set_audio_preferences(audio(48_000, 128))
        .expect("the output changes while playing");
    wait_for(
        Duration::from_millis(500),
        "the playhead moves past its position",
        || session.status().playhead_beats > before,
    );
    assert_eq!(session.status().sample_rate, Some(48_000.0));

    let later = session.status().playhead_beats;
    sleep(Duration::from_millis(100));
    assert!(
        session.status().playhead_beats > later,
        "the new output keeps the transport moving"
    );
}

#[test]
fn the_new_output_keeps_the_launched_clip() {
    let mut session = Session::with_driver_factory(demo(), null_factory(), None);
    session.trigger_clip(TrackId(1), 0);
    session.set_playing(true);
    wait_for(Duration::from_millis(500), "the clip launches", || {
        session.status().active_slots.first() == Some(&Some(0))
    });

    session
        .set_audio_preferences(audio(48_000, 256))
        .expect("the output changes while playing");
    wait_for(
        Duration::from_millis(500),
        "the clip is launched on the new output",
        || session.status().active_slots.first() == Some(&Some(0)),
    );
}

#[test]
fn a_stopped_transport_keeps_its_playhead_after_the_change() {
    let mut session = Session::with_driver_factory(demo(), null_factory(), None);
    session.set_playhead(8.0);
    // Let the current output publish the new playhead before it is carried over.
    sleep(Duration::from_millis(50));
    session
        .set_audio_preferences(audio(48_000, 128))
        .expect("the output changes while stopped");
    wait_for(
        Duration::from_millis(500),
        "the playhead is carried over",
        || session.status().playhead_beats == 8.0,
    );
    sleep(Duration::from_millis(100));
    assert_eq!(session.status().playhead_beats, 8.0);
}

#[test]
fn a_failed_change_keeps_the_current_output_and_its_settings() {
    let mut session = Session::with_driver_factory(demo(), factory_refusing(12_345), None);
    session.set_playing(true);
    wait_for(Duration::from_millis(500), "the transport starts", || {
        session.status().playhead_beats > 0.0
    });
    let before = session.status().playhead_beats;

    let result = session.set_audio_preferences(audio(12_345, 512));
    assert!(matches!(
        result,
        Err(SessionAudioError::Driver(
            DriverError::UnsupportedSampleRate { rate: 12_345, .. }
        ))
    ));
    assert_eq!(session.preferences().audio, AudioPreferences::default());
    assert_eq!(session.status().sample_rate, Some(44_100.0));
    wait_for(
        Duration::from_millis(500),
        "the current output keeps playing",
        || session.status().playhead_beats > before,
    );
}

#[test]
fn changed_audio_settings_are_saved_to_the_preferences_file() {
    let path = prefs_path("saved");
    let mut session = Session::with_driver_factory(demo(), null_factory(), Some(path.clone()));
    session
        .set_audio_preferences(audio(48_000, 128))
        .expect("the output changes");
    assert_eq!(session.preferences().audio, audio(48_000, 128));

    let saved = Preferences::load(&path).expect("the preferences file is readable");
    assert_eq!(saved.audio, audio(48_000, 128));
}

#[test]
fn a_midi_input_that_is_not_connected_does_not_stop_the_set_from_playing() {
    let path = prefs_file(
        "midi-missing",
        "[[midi.inputs]]\nname = \"No Such MIDI Input Port\"\ntrack = true\n",
    );
    let mut session = Session::with_driver_factory(demo(), null_factory(), Some(path));
    assert_eq!(session.preferences().midi.inputs.len(), 1);
    assert_eq!(session.status().sample_rate, Some(44_100.0));
    session.set_playing(true);
    let before = session.status().playhead_beats;
    wait_for(
        Duration::from_millis(500),
        "the playhead moves with a missing MIDI input",
        || session.status().playhead_beats > before,
    );
}

/// A device that plays silently until it is unplugged, and then reports itself lost.
struct TestDevice {
    inner: NullDriver,
    unplugged: Arc<AtomicBool>,
}

impl AudioDriver for TestDevice {
    fn name(&self) -> &str {
        "Test device"
    }
    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }
    fn buffer_frames(&self) -> u32 {
        self.inner.buffer_frames()
    }
    fn channels(&self) -> usize {
        self.inner.channels()
    }
    fn latency(&self) -> Latency {
        self.inner.latency()
    }
    fn start(&mut self, engine: Engine) -> Result<(), DriverError> {
        self.inner.start(engine)
    }
    fn status(&self) -> DriverStatus {
        if self.unplugged.load(Ordering::Relaxed) {
            DriverStatus::Lost("the test device was unplugged".to_string())
        } else {
            self.inner.status()
        }
    }
}

/// The test device's plug, and how many times the factory opened it.
#[derive(Clone, Default)]
struct Plug {
    unplugged: Arc<AtomicBool>,
    opened: Arc<AtomicUsize>,
}

impl Plug {
    fn set_unplugged(&self, unplugged: bool) {
        self.unplugged.store(unplugged, Ordering::Relaxed);
    }

    /// A factory that opens the test device while it is plugged in, and finds no device otherwise.
    fn factory(&self) -> DriverFactory {
        let plug = self.clone();
        Box::new(
            move |config: &DriverConfig| -> Result<Box<dyn AudioDriver>, DriverError> {
                if plug.unplugged.load(Ordering::Relaxed) {
                    return Err(DriverError::NoOutputDevice);
                }
                plug.opened.fetch_add(1, Ordering::Relaxed);
                Ok(Box::new(TestDevice {
                    inner: NullDriver::new(config.sample_rate.unwrap_or(48_000), 256, 2),
                    unplugged: Arc::clone(&plug.unplugged),
                }))
            },
        )
    }
}

/// Fails unless the playhead moves within 500 ms of `since`.
fn assert_playhead_moves(session: &mut Session, what: &str) {
    let since = session.status().playhead_beats;
    wait_for(Duration::from_millis(500), what, || {
        session.status().playhead_beats > since
    });
}

#[test]
fn an_unplugged_device_leaves_the_set_playing_silently_and_reports_the_error() {
    let plug = Plug::default();
    let mut session = Session::with_driver_factory(demo(), plug.factory(), None);
    session.set_playing(true);
    assert_playhead_moves(&mut session, "the transport moves on the device");
    assert_eq!(session.audio_error(), None);

    plug.set_unplugged(true);
    let now = Instant::now();
    session.poll_audio_at(now);
    assert_eq!(session.audio_error(), Some("the test device was unplugged"));
    assert_playhead_moves(&mut session, "the transport moves on the silent driver");
    assert_eq!(session.status().sample_rate, Some(48_000.0));

    // Before the retry time, nothing is tried. At it, the device is still missing.
    session.poll_audio_at(now + AUDIO_RETRY / 2);
    session.poll_audio_at(now + AUDIO_RETRY);
    assert_eq!(plug.opened.load(Ordering::Relaxed), 1);
    assert!(session.audio_error().is_some());
    assert_playhead_moves(
        &mut session,
        "the transport still moves while the device is missing",
    );
}

#[test]
fn a_device_plugged_back_in_is_used_again_at_the_next_retry() {
    let plug = Plug::default();
    let mut session = Session::with_driver_factory(demo(), plug.factory(), None);
    session.trigger_clip(TrackId(1), 0);
    session.set_playing(true);
    wait_for(Duration::from_millis(500), "the clip launches", || {
        session.status().active_slots.first() == Some(&Some(0))
    });

    plug.set_unplugged(true);
    let now = Instant::now();
    session.poll_audio_at(now);
    assert!(session.audio_error().is_some());

    plug.set_unplugged(false);
    session.poll_audio_at(now + AUDIO_RETRY);
    assert_eq!(session.audio_error(), None);
    assert_eq!(plug.opened.load(Ordering::Relaxed), 2);
    assert_playhead_moves(&mut session, "the transport moves on the device again");
    wait_for(
        Duration::from_millis(500),
        "the clip is still launched",
        || session.status().active_slots.first() == Some(&Some(0)),
    );
    // A healthy device is not reopened.
    session.poll_audio_at(now + AUDIO_RETRY * 3);
    assert_eq!(plug.opened.load(Ordering::Relaxed), 2);
}
