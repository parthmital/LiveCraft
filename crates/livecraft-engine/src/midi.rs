//! MIDI timing (M06 items 6 and 7, manual chapter 40, MIDI Fact Sheet, page 997).
//!
//! Input: the MIDI input threads stamp each message with the host clock. The audio callback reads the
//! clock when its buffer starts, and maps each stamp to a frame of that buffer with `sample_offset`. A
//! message that arrived during the previous buffer period plays at the same position in this buffer,
//! so the delay is one buffer and does not jitter.
//!
//! Output: the audio callback stamps each note with its frame. `MidiOutScheduler` maps the frame to the
//! host time at which its sample is heard, from the callback's anchor (a frame and the host time of
//! that frame) and the driver's output delay, and holds the note until then.

use livecraft_devices::NoteEvent;
use std::collections::VecDeque;
use std::time::Instant;

/// A MIDI message as an input port received it, stamped with the host clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MidiMessage {
    /// The host time in microseconds at which the message arrived (see `HostClock`).
    pub time_us: u64,
    /// The index of the input port in the list the system offers (`midi_io::list_midi_ports`).
    pub port: u8,
    /// How many of `bytes` the message uses: one to three.
    pub len: u8,
    pub bytes: [u8; 3],
}

impl MidiMessage {
    /// A message of `bytes`, or None when it is empty or longer than three bytes. The engine uses only
    /// channel and real-time messages, which are at most three bytes, so a system exclusive message is
    /// left out.
    pub fn new(time_us: u64, port: u8, bytes: &[u8]) -> Option<Self> {
        if bytes.is_empty() || bytes.len() > 3 {
            return None;
        }
        let mut stored = [0; 3];
        stored[..bytes.len()].copy_from_slice(bytes);
        Some(Self {
            time_us,
            port,
            len: bytes.len() as u8,
            bytes: stored,
        })
    }
}

/// The clock that MIDI messages and audio buffers are stamped with. The MIDI input threads and the
/// engine read the same clock, so their times compare directly.
#[derive(Debug, Clone, Copy)]
pub struct HostClock {
    origin: Instant,
}

impl HostClock {
    /// A clock that reads zero now.
    pub fn start() -> Self {
        Self {
            origin: Instant::now(),
        }
    }

    /// The microseconds since the clock started. It never runs backwards.
    pub fn now_us(&self) -> u64 {
        u64::try_from(self.origin.elapsed().as_micros()).unwrap_or(u64::MAX)
    }
}

/// The frame of a buffer at which a message plays.
///
/// `block_start_us` is the host time at which the buffer starts and `period_us` the buffer's length in
/// microseconds. A message stamped during the previous buffer period, from `block_start_us - period_us`
/// up to `block_start_us`, plays at the same position in this buffer. An earlier message plays on the
/// first frame, and a later one on the last frame. The result is always below `frames`, and 0 when
/// `frames` is 0.
pub fn sample_offset(
    event_us: u64,
    block_start_us: u64,
    period_us: u64,
    frames: usize,
    sample_rate: f32,
) -> usize {
    if frames == 0 {
        return 0;
    }
    let position = (event_us as f64 + period_us as f64 - block_start_us as f64)
        * f64::from(sample_rate)
        / 1_000_000.0;
    position.floor().clamp(0.0, (frames - 1) as f64) as usize
}

/// The note event a message makes, at `offset` frames into the buffer. A note on with velocity 0 is a
/// note off, as MIDI defines it. `frequency` gives the frequency of each pitch in hertz (see
/// `CompiledGraph::note_frequencies`). The channel is not checked: every channel reaches the track.
/// Other messages, and a note message shorter than three bytes, make no note.
pub fn note_event(message: &MidiMessage, offset: u32, frequency: &[f32; 128]) -> Option<NoteEvent> {
    if message.len < 3 {
        return None;
    }
    let [status, data1, data2] = message.bytes;
    let pitch = data1 & 0x7F;
    let velocity = data2 & 0x7F;
    match status & 0xF0 {
        0x90 if velocity > 0 => Some(NoteEvent::On {
            offset,
            pitch,
            velocity,
            freq_hz: frequency[usize::from(pitch)],
        }),
        0x80 | 0x90 => Some(NoteEvent::Off { offset, pitch }),
        _ => None,
    }
}

/// The period of a buffer of `frames` frames at `sample_rate`, in whole microseconds.
pub fn period_us(frames: usize, sample_rate: f32) -> u64 {
    (frames as f64 * 1_000_000.0 / f64::from(sample_rate)).round() as u64
}

/// A MIDI message the engine sends to an output port, stamped with the frame at which it is sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MidiOutEvent {
    /// The engine frame at which the note plays. The sender waits until that frame is heard.
    pub frame: u64,
    /// The index of the output port in the list the system offers (`midi_io::list_midi_ports`).
    pub port: u8,
    /// How many of `bytes` the message uses. A note uses three.
    pub len: u8,
    pub bytes: [u8; 3],
}

/// The MIDI message of a note event, for the frame `frame` and output port `port`. A note on is `0x90`
/// with its velocity, and a note off is `0x80` with velocity 0. The channel, from 0 to 15, is the low
/// nibble of the status byte.
pub fn note_out_event(frame: u64, port: u8, channel: u8, event: &NoteEvent) -> MidiOutEvent {
    let channel = channel & 0x0F;
    let bytes = match *event {
        NoteEvent::On {
            pitch, velocity, ..
        } => [0x90 | channel, pitch & 0x7F, velocity & 0x7F],
        NoteEvent::Off { pitch, .. } => [0x80 | channel, pitch & 0x7F, 0],
    };
    MidiOutEvent {
        frame,
        port,
        len: 3,
        bytes,
    }
}

/// The host time in microseconds at which the sample of `frame` is heard. `anchor` is a frame and the host
/// time of that frame, and `latency_us` is the driver's output delay. A frame before the anchor gives a time
/// before it.
fn send_time_us(frame: u64, anchor: (u64, u64), sample_rate: f64, latency_us: u64) -> f64 {
    let (anchor_frame, anchor_us) = anchor;
    let frames = frame as f64 - anchor_frame as f64;
    anchor_us as f64 + frames * 1_000_000.0 / sample_rate + latency_us as f64
}

/// Holds MIDI output events until their send time, and gives them out in frame order (manual 40.3). The
/// engine pushes events in order within a block, but two tracks can add events of the same block in either
/// order, so `push` keeps the queue sorted. The scheduler reads no clock: its callers pass the time.
#[derive(Debug, Default)]
pub struct MidiOutScheduler {
    queue: VecDeque<MidiOutEvent>,
}

impl MidiOutScheduler {
    /// Adds an event after the queued events of the same or an earlier frame, so events of one frame keep
    /// the order they were pushed in.
    pub fn push(&mut self, event: MidiOutEvent) {
        let index = self
            .queue
            .partition_point(|queued| queued.frame <= event.frame);
        self.queue.insert(index, event);
    }

    /// Removes and returns, in frame order, the events whose send time is at or before `now_us`. `anchor` is
    /// a frame and the host time of that frame, `sample_rate` is in hertz, and `latency_us` is the output
    /// delay of the driver.
    pub fn due(
        &mut self,
        now_us: u64,
        anchor: (u64, u64),
        sample_rate: f64,
        latency_us: u64,
    ) -> impl Iterator<Item = MidiOutEvent> + '_ {
        std::iter::from_fn(move || {
            let front = self.queue.front()?;
            if send_time_us(front.frame, anchor, sample_rate, latency_us) > now_us as f64 {
                return None;
            }
            self.queue.pop_front()
        })
    }

    /// The host time in microseconds of the first queued event to send, or None when none is queued. The
    /// time is rounded up, so a thread that sleeps until it does not wake before the event is due.
    pub fn next_send_us(
        &self,
        anchor: (u64, u64),
        sample_rate: f64,
        latency_us: u64,
    ) -> Option<u64> {
        let front = self.queue.front()?;
        let time = send_time_us(front.frame, anchor, sample_rate, latency_us).ceil();
        Some(time.max(0.0) as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiled::MAX_BLOCK;

    const RATE: f32 = 44_100.0;
    /// The period of a buffer of MAX_BLOCK frames at 44.1 kHz, in microseconds.
    const PERIOD: u64 = 11_610;
    const START: u64 = 1_000_000;

    /// A frequency table where each pitch's frequency is its number, so tests can read it back.
    fn table() -> [f32; 128] {
        std::array::from_fn(|pitch| pitch as f32)
    }

    fn lcg(state: &mut u64) -> u64 {
        *state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        *state >> 33
    }

    #[test]
    fn the_host_clock_never_runs_backwards() {
        let clock = HostClock::start();
        let first = clock.now_us();
        let second = clock.now_us();
        assert!(second >= first);
    }

    #[test]
    fn a_message_keeps_only_the_bytes_it_has() {
        let note = MidiMessage::new(7, 2, &[0x90, 60, 100]).expect("a note on");
        assert_eq!((note.len, note.bytes), (3, [0x90, 60, 100]));
        let clock = MidiMessage::new(7, 2, &[0xF8]).expect("a clock tick");
        assert_eq!((clock.len, clock.bytes), (1, [0xF8, 0, 0]));
    }

    #[test]
    fn an_empty_or_system_exclusive_message_is_not_kept() {
        assert_eq!(MidiMessage::new(0, 0, &[]), None);
        assert_eq!(MidiMessage::new(0, 0, &[0xF0, 1, 2, 3]), None);
    }

    #[test]
    fn an_event_from_the_previous_period_lands_at_its_position_in_this_buffer() {
        // 10 ms into the previous period is 441 frames into this buffer at 44.1 kHz.
        let event = START - PERIOD + 10_000;
        assert_eq!(sample_offset(event, START, PERIOD, MAX_BLOCK, RATE), 441);
    }

    #[test]
    fn an_event_from_before_the_previous_period_lands_on_the_first_frame() {
        assert_eq!(
            sample_offset(START - 2 * PERIOD, START, PERIOD, MAX_BLOCK, RATE),
            0
        );
    }

    #[test]
    fn an_event_stamped_after_the_block_start_lands_on_the_last_frame() {
        assert_eq!(
            sample_offset(START + 1_000, START, PERIOD, MAX_BLOCK, RATE),
            MAX_BLOCK - 1
        );
    }

    #[test]
    fn a_buffer_of_no_frames_has_no_offset() {
        assert_eq!(sample_offset(START, START, PERIOD, 0, RATE), 0);
    }

    #[test]
    fn a_thousand_random_timestamps_land_within_one_sample_of_their_offset() {
        let mut state = 0x5EED_u64;
        for _ in 0..1000 {
            let event = START - PERIOD + lcg(&mut state) % PERIOD;
            let exact = (event as f64 + PERIOD as f64 - START as f64) * f64::from(RATE) / 1e6;
            let offset = sample_offset(event, START, PERIOD, MAX_BLOCK, RATE);
            assert!(
                (offset as f64 - exact).abs() < 1.0,
                "the event at {event} us landed on frame {offset}, but it is at {exact}"
            );
        }
    }

    #[test]
    fn a_note_on_carries_its_pitch_frequency_and_velocity() {
        let message = MidiMessage::new(0, 0, &[0x90, 60, 100]).expect("a note on");
        assert_eq!(
            note_event(&message, 5, &table()),
            Some(NoteEvent::On {
                offset: 5,
                pitch: 60,
                velocity: 100,
                freq_hz: 60.0,
            })
        );
    }

    #[test]
    fn a_note_on_with_velocity_zero_is_a_note_off() {
        let message = MidiMessage::new(0, 0, &[0x90, 60, 0]).expect("a note on");
        assert_eq!(
            note_event(&message, 5, &table()),
            Some(NoteEvent::Off {
                offset: 5,
                pitch: 60
            })
        );
    }

    #[test]
    fn a_note_off_is_a_note_off_on_any_channel() {
        let message = MidiMessage::new(0, 0, &[0x85, 64, 40]).expect("a note off");
        assert_eq!(
            note_event(&message, 0, &table()),
            Some(NoteEvent::Off {
                offset: 0,
                pitch: 64
            })
        );
    }

    #[test]
    fn other_messages_make_no_note() {
        let controller = MidiMessage::new(0, 0, &[0xB0, 7, 100]).expect("a controller");
        let bend = MidiMessage::new(0, 0, &[0xE0, 0, 64]).expect("a pitch bend");
        let tick = MidiMessage::new(0, 0, &[0xF8]).expect("a clock tick");
        for message in [controller, bend, tick] {
            assert_eq!(note_event(&message, 0, &table()), None);
        }
    }

    fn on(pitch: u8) -> NoteEvent {
        NoteEvent::On {
            offset: 0,
            pitch,
            velocity: 100,
            freq_hz: 0.0,
        }
    }

    fn off(pitch: u8) -> NoteEvent {
        NoteEvent::Off { offset: 0, pitch }
    }

    fn event_at(frame: u64) -> MidiOutEvent {
        note_out_event(frame, 0, 0, &on(60))
    }

    #[test]
    fn a_note_on_is_sent_on_its_channel() {
        let sent = note_out_event(10, 2, 3, &on(60));
        assert_eq!(
            sent,
            MidiOutEvent {
                frame: 10,
                port: 2,
                len: 3,
                bytes: [0x93, 60, 100],
            }
        );
    }

    #[test]
    fn a_note_off_is_sent_on_its_channel_with_velocity_zero() {
        assert_eq!(note_out_event(0, 0, 9, &off(64)).bytes, [0x89, 64, 0]);
    }

    #[test]
    fn the_channel_is_kept_to_the_low_nibble_of_the_status_byte() {
        assert_eq!(note_out_event(0, 0, 17, &on(60)).bytes[0], 0x91);
    }

    #[test]
    fn a_scheduled_event_is_sent_when_its_frame_is_heard() {
        // Frame 0 is heard at 1000 us. Frame 441 at 44.1 kHz is 10 ms later.
        let mut scheduler = MidiOutScheduler::default();
        scheduler.push(event_at(441));
        let anchor = (0, 1_000);
        assert_eq!(scheduler.next_send_us(anchor, 44100.0, 0), Some(11_000));
        assert_eq!(scheduler.due(10_999, anchor, 44100.0, 0).count(), 0);
        let sent: Vec<_> = scheduler.due(11_000, anchor, 44100.0, 0).collect();
        assert_eq!(sent, vec![event_at(441)]);
        assert_eq!(scheduler.next_send_us(anchor, 44100.0, 0), None);
    }

    #[test]
    fn events_are_sent_in_frame_order_even_when_pushed_out_of_order() {
        let mut scheduler = MidiOutScheduler::default();
        for frame in [900, 100, 500] {
            scheduler.push(event_at(frame));
        }
        let frames: Vec<u64> = scheduler
            .due(u64::MAX, (0, 0), 44100.0, 0)
            .map(|event| event.frame)
            .collect();
        assert_eq!(frames, vec![100, 500, 900]);
    }

    #[test]
    fn the_output_delay_moves_each_send_later() {
        // Frame 441 is heard at 10 ms, and a 5 ms output delay sends it at 15 ms.
        let mut scheduler = MidiOutScheduler::default();
        scheduler.push(event_at(441));
        assert_eq!(scheduler.due(14_999, (0, 0), 44100.0, 5_000).count(), 0);
        assert_eq!(scheduler.due(15_000, (0, 0), 44100.0, 5_000).count(), 1);
    }

    #[test]
    fn events_of_one_frame_keep_the_order_they_were_pushed_in() {
        let mut scheduler = MidiOutScheduler::default();
        scheduler.push(note_out_event(50, 0, 0, &off(60)));
        scheduler.push(note_out_event(50, 0, 0, &on(60)));
        let kinds: Vec<u8> = scheduler
            .due(u64::MAX, (0, 0), 44100.0, 0)
            .map(|event| event.bytes[0])
            .collect();
        assert_eq!(kinds, vec![0x80, 0x90]);
    }

    #[test]
    fn a_frame_before_the_anchor_is_due_before_the_anchor_time() {
        // Frame 0 is 1000 frames before the anchor, which is 22.68 ms before the anchor's 50 ms.
        let mut scheduler = MidiOutScheduler::default();
        scheduler.push(event_at(0));
        let anchor = (1_000, 50_000);
        assert_eq!(scheduler.due(27_324, anchor, 44100.0, 0).count(), 0);
        assert_eq!(scheduler.due(27_325, anchor, 44100.0, 0).count(), 1);
    }

    #[test]
    fn the_next_send_time_is_rounded_up_to_a_whole_microsecond() {
        // One frame at 44.1 kHz is 22.7 us, which rounds up to 23 us.
        let mut scheduler = MidiOutScheduler::default();
        scheduler.push(event_at(1));
        assert_eq!(scheduler.next_send_us((0, 0), 44100.0, 0), Some(23));
    }
}
