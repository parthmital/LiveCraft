//! Opening and changing the audio output of a Set (M06 items 3 and 4). The factories build
//! `NullDriver`s, so these tests need no audio device.

use livecraft_engine::driver::{AudioDriver, DriverConfig, DriverError, NullDriver};
use livecraft_model::ids::TrackId;
use livecraft_model::model::Project;
use livecraft_session::{
    AudioPreferences, DriverFactory, Preferences, Session, SessionAudioError, demo_set,
};
use std::path::PathBuf;
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
