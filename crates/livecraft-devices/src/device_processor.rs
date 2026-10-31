//! The block-based device contract (M05 item 2, TARGET-ARCHITECTURE "Engine Decisions").
//!
//! A device processes whole blocks. It reads and writes `AudioIo` in place, takes note events at
//! sample offsets, and identifies its parameters by `ParamId` (a position in its kind's table in
//! `livecraft_model::device`). No method looks up a name, allocates, locks, or logs, because the
//! engine calls these methods on the audio thread.

use livecraft_model::param::ParamId;

/// Transport and timing at the first frame of the block being processed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProcessContext {
    pub sample_rate: f32,
    /// Frames in this block.
    pub frames: usize,
    pub bpm: f32,
    /// Song position in beats at the first frame of the block.
    pub playhead_beats: f64,
    /// Beats per frame at the current tempo, or 0 when the transport is stopped.
    pub beats_per_frame: f64,
}

/// The stereo audio of one block. Effects change it in place, and sources add their output to it.
pub struct AudioIo<'a> {
    pub left: &'a mut [f32],
    pub right: &'a mut [f32],
}

impl AudioIo<'_> {
    /// The number of frames both channels hold.
    pub fn frames(&self) -> usize {
        self.left.len().min(self.right.len())
    }

    /// Replaces each frame with `f(left, right)`, which is how a per-sample effect runs on a block.
    pub fn for_each_frame(&mut self, mut f: impl FnMut(f32, f32) -> (f32, f32)) {
        for (left, right) in self.left.iter_mut().zip(self.right.iter_mut()) {
            (*left, *right) = f(*left, *right);
        }
    }
}

/// A note event at a sample offset inside the block. `pitch` follows Live's naming (60 is C3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NoteEvent {
    On {
        offset: u32,
        pitch: u8,
        velocity: u8,
        freq_hz: f32,
    },
    Off {
        offset: u32,
        pitch: u8,
    },
}

impl NoteEvent {
    /// Frames from the start of the block at which the event happens.
    pub fn offset(&self) -> u32 {
        match *self {
            NoteEvent::On { offset, .. } | NoteEvent::Off { offset, .. } => offset,
        }
    }

    /// The same event at another offset. The engine uses it to move an event between blocks.
    pub fn at(self, offset: u32) -> Self {
        match self {
            NoteEvent::On {
                pitch,
                velocity,
                freq_hz,
                ..
            } => NoteEvent::On {
                offset,
                pitch,
                velocity,
                freq_hz,
            },
            NoteEvent::Off { pitch, .. } => NoteEvent::Off { offset, pitch },
        }
    }
}

/// A source that plays notes and makes one stereo frame at a time. The instruments implement it,
/// and `render_notes` runs it over a block.
pub trait NoteSource {
    fn apply_note(&mut self, event: &NoteEvent);
    fn next_frame(&mut self) -> (f32, f32);
}

/// Renders a block from a note source: each event takes effect on the frame at its offset, and the
/// frame's output is added to `io`, which holds the input of the source.
pub fn render_notes(source: &mut impl NoteSource, io: &mut AudioIo<'_>, events: &EventList) {
    let mut pending = events.iter().peekable();
    for frame in 0..io.frames() {
        while let Some(event) = pending.next_if(|event| event.offset() as usize <= frame) {
            source.apply_note(event);
        }
        let (left, right) = source.next_frame();
        io.left[frame] += left;
        io.right[frame] += right;
    }
}

/// A fixed-capacity list of note events. It is allocated once, off the audio thread, and `push`
/// refuses events beyond its capacity instead of growing.
#[derive(Debug, Clone, Default)]
pub struct EventList {
    events: Vec<NoteEvent>,
}

impl EventList {
    /// An empty list that can hold `capacity` events without growing.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            events: Vec::with_capacity(capacity),
        }
    }

    /// Removes every event, keeping the capacity.
    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Adds an event. Returns false, and keeps the list unchanged, when it is full.
    pub fn push(&mut self, event: NoteEvent) -> bool {
        if self.events.len() < self.events.capacity() {
            self.events.push(event);
            true
        } else {
            false
        }
    }

    /// Sorts the events by offset, with note-offs before note-ons at the same offset so a note that
    /// ends where another of the same pitch starts never cuts the new one. Other events that share an
    /// offset keep their order. It sorts in place with an insertion sort, so it never allocates.
    pub fn sort_by_offset(&mut self) {
        let key = |event: &NoteEvent| (event.offset(), matches!(event, NoteEvent::On { .. }));
        for index in 1..self.events.len() {
            let mut position = index;
            while position > 0 && key(&self.events[position - 1]) > key(&self.events[position]) {
                self.events.swap(position - 1, position);
                position -= 1;
            }
        }
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &NoteEvent> {
        self.events.iter()
    }
}

/// Events a device emits for the blocks after it. Same storage and rules as `EventList`.
pub type EventSink = EventList;

/// A device on a track or a return: an effect or an instrument, processing one block at a time.
pub trait DeviceProcessor: Send {
    /// Processes one block of `io.frames()` frames. `events` are the note events of this block,
    /// sorted by offset; effects ignore them.
    fn process(
        &mut self,
        ctx: &ProcessContext,
        io: &mut AudioIo<'_>,
        events: &EventList,
        out_events: &mut EventSink,
    );

    /// Sets a parameter. The new value applies from the next call to `process`. The engine splits
    /// blocks at every parameter event, so it always passes `offset` 0; devices in this crate do
    /// not use the offset (see the M05 review).
    fn set_param(&mut self, id: ParamId, value: f32, offset: u32);

    /// Frames of delay the device adds to its output. The engine compensates it (PDC, manual 23.3).
    fn latency_samples(&self) -> u32 {
        0
    }

    /// Clears the internal state, such as delay lines and filter memory.
    fn reset(&mut self);

    /// Frames the device keeps sounding after its input stops, or `None` when it stops at once.
    fn tail_samples(&self) -> Option<u64> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effects::{EqThreeEffect, UtilityEffect};
    use crate::test_support::{bits, run, signal};

    #[test]
    fn a_list_refuses_events_beyond_its_capacity_without_growing() {
        let mut events = EventList::with_capacity(2);
        let off = NoteEvent::Off {
            offset: 0,
            pitch: 60,
        };
        assert!(events.push(off));
        assert!(events.push(off));
        assert!(!events.push(off));
        assert_eq!(events.len(), 2);
        events.clear();
        assert!(events.is_empty());
        assert!(events.push(off));
    }

    #[test]
    fn sorting_puts_note_offs_before_note_ons_at_the_same_offset() {
        let on = |offset, pitch| NoteEvent::On {
            offset,
            pitch,
            velocity: 100,
            freq_hz: 261.6,
        };
        let off = |offset, pitch| NoteEvent::Off { offset, pitch };
        let mut events = EventList::with_capacity(4);
        for event in [on(4, 60), off(4, 60), on(2, 62), off(4, 64)] {
            assert!(events.push(event));
        }
        events.sort_by_offset();
        let sorted: Vec<NoteEvent> = events.iter().copied().collect();
        assert_eq!(sorted, vec![on(2, 62), off(4, 60), off(4, 64), on(4, 60)]);
    }

    #[test]
    fn utility_output_does_not_depend_on_the_block_size() {
        let input = signal(1024, 7);
        let mut whole = UtilityEffect::new();
        whole.set_param(ParamId(0), -6.0, 0);
        whole.set_param(ParamId(1), 0.3, 0);
        whole.set_param(ParamId(2), 0.5, 0);
        let mut single = UtilityEffect::new();
        single.set_param(ParamId(0), -6.0, 0);
        single.set_param(ParamId(1), 0.3, 0);
        single.set_param(ParamId(2), 0.5, 0);
        let (wl, wr) = run(&mut whole, &input, 512);
        let (sl, sr) = run(&mut single, &input, 1);
        assert_eq!(bits(&wl), bits(&sl));
        assert_eq!(bits(&wr), bits(&sr));
    }

    #[test]
    fn eq_three_output_does_not_depend_on_the_block_size() {
        let input = signal(1024, 11);
        let set = |eq: &mut EqThreeEffect| {
            eq.set_param(ParamId(0), 6.0, 0);
            eq.set_param(ParamId(1), 3.0, 0);
            eq.set_param(ParamId(2), -4.0, 0);
        };
        let mut whole = EqThreeEffect::new(44100.0);
        set(&mut whole);
        let mut blocks = EqThreeEffect::new(44100.0);
        set(&mut blocks);
        let (wl, wr) = run(&mut whole, &input, 512);
        let (bl, br) = run(&mut blocks, &input, 7);
        assert_eq!(bits(&wl), bits(&bl));
        assert_eq!(bits(&wr), bits(&br));
    }

    #[test]
    fn utility_mute_silences_the_block_and_unmute_restores_it() {
        let input = signal(256, 3);
        let mut utility = UtilityEffect::new();
        utility.set_param(ParamId(3), 1.0, 0);
        let (muted, _) = run(&mut utility, &input, 256);
        assert!(muted.iter().all(|&s| s == 0.0));
        utility.set_param(ParamId(3), 0.0, 0);
        let (open, _) = run(&mut utility, &input, 256);
        assert!(open.iter().any(|&s| s != 0.0));
    }

    #[test]
    fn a_parameter_id_outside_the_table_is_ignored() {
        let input = signal(256, 5);
        let mut plain = UtilityEffect::new();
        let mut unknown = UtilityEffect::new();
        unknown.set_param(ParamId(99), -60.0, 0);
        let (pl, _) = run(&mut plain, &input, 256);
        let (ul, _) = run(&mut unknown, &input, 256);
        assert_eq!(bits(&pl), bits(&ul));
    }
}
