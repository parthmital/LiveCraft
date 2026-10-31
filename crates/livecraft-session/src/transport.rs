//! Play, Continue, and Stop (M07 item 1; manual 6.3, pages 168 and 169).
//!
//! "The flashing blue insert marker on a track determines where playback starts." The state machine
//! keeps the insert marker and the position where playback last stopped, and says where each press
//! starts the song.

/// The transport state that Play and Stop act on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransportControl {
    /// Where Play starts. "By default, the marker is at the start of the Arrangement."
    pub insert_marker: f64,
    /// Where playback last stopped, for Continue.
    pub stopped_at: f64,
    pub playing: bool,
}

impl Default for TransportControl {
    fn default() -> Self {
        Self {
            insert_marker: 0.0,
            stopped_at: 0.0,
            playing: false,
        }
    }
}

impl TransportControl {
    /// "You can click the Play button to start playback". Returns where playback starts: the insert
    /// marker.
    pub fn play(&mut self) -> f64 {
        self.playing = true;
        self.insert_marker
    }

    /// "To continue playback from where it last stopped, rather than from the insert marker, hold Shift
    /// while pressing the space bar." Returns where playback starts.
    pub fn continue_play(&mut self) -> f64 {
        self.playing = true;
        self.stopped_at
    }

    /// "to stop playback, click the Stop button." `position` is the song position when it is pressed.
    /// A press while stopped is the second click of a double-click: "To return the insert marker to the
    /// starting play position, you can double-click the Stop button". Returns true when the insert
    /// marker went back to the start.
    pub fn stop(&mut self, position: f64) -> bool {
        if self.playing {
            self.playing = false;
            self.stopped_at = position;
            false
        } else {
            self.home();
            true
        }
    }

    /// "Alternatively, you can use the space bar to toggle playback on or off." Returns where playback
    /// starts, or None when it stopped.
    pub fn toggle(&mut self, position: f64) -> Option<f64> {
        if self.playing {
            self.stop(position);
            None
        } else {
            Some(self.play())
        }
    }

    /// The Home key: the insert marker goes back to the start of the Arrangement.
    pub fn home(&mut self) {
        self.insert_marker = 0.0;
    }

    /// "You can click anywhere within a track to move the insert marker and set a new play position."
    pub fn set_insert_marker(&mut self, beats: f64) {
        self.insert_marker = beats;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn play_starts_from_the_insert_marker_and_stop_stops() {
        // "The flashing blue insert marker on a track determines where playback starts. By default, the
        // marker is at the start of the Arrangement." (manual 6.3, page 169)
        let mut transport = TransportControl::default();
        assert_eq!(transport.play(), 0.0);
        assert!(!transport.stop(12.0));
        transport.set_insert_marker(8.0);
        assert_eq!(transport.play(), 8.0);
        assert!(transport.playing);
    }

    #[test]
    fn continue_starts_where_playback_last_stopped() {
        // "To continue playback from where it last stopped, rather than from the insert marker, hold
        // Shift while pressing the space bar." (page 169)
        let mut transport = TransportControl::default();
        transport.set_insert_marker(4.0);
        transport.play();
        transport.stop(10.5);
        assert_eq!(transport.continue_play(), 10.5);
        transport.stop(11.0);
        assert_eq!(transport.play(), 4.0);
    }

    #[test]
    fn a_double_click_on_stop_returns_the_insert_marker_to_the_start() {
        // "To return the insert marker to the starting play position, you can double-click the Stop
        // button or press the Home key" (page 169)
        let mut transport = TransportControl::default();
        transport.set_insert_marker(16.0);
        transport.play();
        assert!(!transport.stop(20.0));
        assert_eq!(transport.insert_marker, 16.0);
        assert!(transport.stop(20.0));
        assert_eq!(transport.insert_marker, 0.0);
        transport.set_insert_marker(4.0);
        transport.home();
        assert_eq!(transport.insert_marker, 0.0);
    }

    #[test]
    fn the_space_bar_toggles_playback() {
        // "Alternatively, you can use the space bar to toggle playback on or off." (page 168)
        let mut transport = TransportControl::default();
        assert_eq!(transport.toggle(0.0), Some(0.0));
        assert_eq!(transport.toggle(3.0), None);
        assert!(!transport.playing);
        assert_eq!(transport.stopped_at, 3.0);
    }
}
