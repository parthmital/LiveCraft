//! Play, Continue, and Stop through the session (M07 item 1, manual 6.3). The factory builds a silent
//! driver, so no audio device is needed.

use livecraft_engine::driver::{AudioDriver, DriverConfig, DriverError, NullDriver};
use livecraft_model::model::Project;
use livecraft_session::{DriverFactory, Session};
use std::thread::sleep;
use std::time::{Duration, Instant};

fn session() -> Session {
    let factory: DriverFactory = Box::new(
        |_: &DriverConfig| -> Result<Box<dyn AudioDriver>, DriverError> {
            Ok(Box::new(NullDriver::new(44_100, 256, 2)))
        },
    );
    Session::with_driver_factory(Project::new_default("Transport"), factory, None)
}

fn wait_until(what: &str, mut condition: impl FnMut() -> bool) {
    let started = Instant::now();
    while !condition() {
        assert!(started.elapsed() < Duration::from_secs(2), "{what}");
        sleep(Duration::from_millis(2));
    }
}

#[test]
fn play_starts_at_the_insert_marker_and_continue_where_it_stopped() {
    let mut session = session();
    session.set_playhead(8.0);
    session.play();
    wait_until("playback moves past the insert marker", || {
        session.status().playhead_beats > 8.1
    });
    session.stop();
    let stopped = session.project().transport.playhead_beats;
    sleep(Duration::from_millis(20));
    let stopped_at = session.status().playhead_beats;
    assert!(stopped_at > 8.1, "{stopped} {stopped_at}");

    session.continue_play();
    wait_until("playback continues from the stop position", || {
        session.status().playhead_beats > stopped_at + 0.05
    });
    session.stop();
    session.play();
    wait_until("Play starts from the insert marker again", || {
        let at = session.status().playhead_beats;
        (8.0..stopped_at).contains(&at)
    });
}

#[test]
fn stopping_twice_returns_the_insert_marker_to_the_start() {
    let mut session = session();
    session.set_playhead(16.0);
    session.play();
    sleep(Duration::from_millis(20));
    session.stop();
    session.stop();
    assert_eq!(session.project().transport.playhead_beats, 0.0);
    session.play();
    wait_until("Play starts from the start", || {
        let at = session.status().playhead_beats;
        at > 0.0 && at < 4.0
    });
}
