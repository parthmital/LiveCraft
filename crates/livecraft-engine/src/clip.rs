//! Clip playback on the audio thread (M05 item 7). The session side prepares every clip the graph
//! can play as shared, immutable data (`basedrop::Shared`): note lists with frequencies, and decoded
//! audio. The audio thread reads that data and schedules note events at their sample offsets, so it
//! never clones a clip, decodes a file, or allocates.

use crate::audio_clip::{DecodedAudio, hermite_interp};
use basedrop::Shared;
use livecraft_devices::{EventList, NoteEvent, ProcessContext};
use livecraft_time::samples_per_beat;

/// A MIDI note as the scheduler reads it. Times are beats from the clip start, and the frequency is
/// computed at compile time from the Set's tuning.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayNote {
    pub start: f64,
    pub end: f64,
    pub pitch: u8,
    pub velocity: u8,
    pub freq_hz: f32,
}

/// What a clip plays.
pub enum PlayableSource {
    Midi(Shared<Vec<PlayNote>>),
    Audio {
        data: Shared<DecodedAudio>,
        gain: f32,
        /// True when the clip follows the Set's tempo, as a warped clip does.
        is_warped: bool,
        original_bpm: f64,
    },
}

/// A clip ready to play: its length in beats, whether it loops, and its source.
pub struct PlayableClip {
    pub length_beats: f64,
    pub is_looping: bool,
    pub source: PlayableSource,
}

/// An arrangement clip, placed on the timeline.
pub struct ArrangedClip {
    pub start_beat: f64,
    pub length_beats: f64,
    pub clip: PlayableClip,
}

/// The clips of one track and the notes they have started. A track plays its launched session slot,
/// else the arrangement clip under the playhead. A session clip that was not launched stays silent
/// (see `schedule`).
pub struct ClipLane {
    /// One entry per clip slot row; `None` for an empty slot.
    pub slots: Vec<Option<PlayableClip>>,
    pub arranged: Vec<ArrangedClip>,
    /// The slot a launch made active, if any.
    pub active_slot: Option<usize>,
    /// False while the track monitors its input rather than its clips (manual 18.4 monitoring).
    pub plays_clips: bool,
    /// The notes that started and have not ended: (note index, pitch).
    sounding: Vec<(usize, u8)>,
    /// Set when the clip changes or stops, so the notes that are sounding get note-offs.
    stop_pending: bool,
    /// True when a clip played in the last block.
    pub playing: bool,
}

impl ClipLane {
    pub fn new(
        slots: Vec<Option<PlayableClip>>,
        arranged: Vec<ArrangedClip>,
        plays_clips: bool,
        capacity: usize,
    ) -> Self {
        Self {
            slots,
            arranged,
            active_slot: None,
            plays_clips,
            sounding: Vec::with_capacity(capacity),
            stop_pending: false,
            playing: false,
        }
    }

    /// Launches a slot, or stops playback when the slot is empty.
    pub fn launch(&mut self, slot: usize) {
        self.active_slot = Some(slot);
        self.stop_pending = true;
    }

    /// Stops the clip the track is playing.
    pub fn stop(&mut self) {
        self.active_slot = None;
        self.stop_pending = true;
    }

    /// Fills `events` with this block's note events and `left` and `right` with this block's audio,
    /// for the frames of `ctx`. The buffers must already be cleared.
    pub fn schedule(
        &mut self,
        ctx: &ProcessContext,
        events: &mut EventList,
        left: &mut [f32],
        right: &mut [f32],
    ) {
        let beats_per_frame = ctx.beats_per_frame;
        self.playing = false;
        if self.stop_pending || !self.plays_clips || beats_per_frame == 0.0 {
            self.end_sounding(events);
        }
        if !self.plays_clips || beats_per_frame == 0.0 {
            return;
        }
        let position = ctx.playhead_beats;
        if let Some(slot) = self.active_slot {
            let clip = self.slots.get(slot).and_then(Option::as_ref);
            if let Some(clip) = clip {
                self.playing = true;
                let length = clip.length_beats.max(1.0);
                play(
                    PlayPass {
                        clip,
                        loop_length: Some(length),
                        position: position % length,
                    },
                    ctx,
                    events,
                    &mut self.sounding,
                    left,
                    right,
                );
                return;
            }
        } else if !self.arranged.is_empty() {
            let found = self.arranged.iter().find(|arranged| {
                position >= arranged.start_beat
                    && position < arranged.start_beat + arranged.length_beats
            });
            if let Some(arranged) = found {
                self.playing = true;
                let relative = position - arranged.start_beat;
                let length = arranged.clip.length_beats.max(1.0);
                let (loop_length, clip_position) = if arranged.clip.is_looping {
                    (Some(length), relative % length)
                } else {
                    (None, relative)
                };
                play(
                    PlayPass {
                        clip: &arranged.clip,
                        loop_length,
                        position: clip_position,
                    },
                    ctx,
                    events,
                    &mut self.sounding,
                    left,
                    right,
                );
                return;
            }
        }
        self.end_sounding(events);
    }

    /// The first beat after `position` at which the arrangement clip under the playhead starts or
    /// ends, or None when no arrangement clip starts or ends after it. A launched clip loops, so it
    /// has no boundary.
    pub fn next_boundary(&self, position: f64) -> Option<f64> {
        if self.active_slot.is_some() || !self.plays_clips {
            return None;
        }
        self.arranged
            .iter()
            .flat_map(|arranged| {
                [
                    arranged.start_beat,
                    arranged.start_beat + arranged.length_beats,
                ]
            })
            .filter(|beat| *beat > position)
            .reduce(f64::min)
    }

    /// Sends a note-off for every note that is sounding, and forgets them.
    fn end_sounding(&mut self, events: &mut EventList) {
        for (_, pitch) in self.sounding.drain(..) {
            let _ = events.push(NoteEvent::Off { offset: 0, pitch });
        }
        self.stop_pending = false;
    }
}

/// One clip for one block: where it starts and how it loops.
struct PlayPass<'a> {
    clip: &'a PlayableClip,
    /// The loop length in beats, when the clip wraps back to its start.
    loop_length: Option<f64>,
    /// The clip position in beats at the first frame of the block.
    position: f64,
}

/// Plays a clip over the block, splitting it wherever the loop wraps so that every event lands on
/// its exact frame.
fn play(
    pass: PlayPass<'_>,
    ctx: &ProcessContext,
    events: &mut EventList,
    sounding: &mut Vec<(usize, u8)>,
    left: &mut [f32],
    right: &mut [f32],
) {
    let dt = ctx.beats_per_frame;
    let mut start = 0;
    let mut base = pass.position;
    while start < ctx.frames {
        let end = match pass.loop_length {
            Some(length) => {
                let to_wrap = ((length - base) / dt).ceil().max(1.0) as usize;
                (start + to_wrap).min(ctx.frames)
            }
            None => ctx.frames,
        };
        match &pass.clip.source {
            PlayableSource::Midi(notes) => {
                schedule_notes(notes, base, dt, start, end, events, sounding);
            }
            PlayableSource::Audio {
                data,
                gain,
                is_warped,
                original_bpm,
            } => {
                let beats_to_samples = if *is_warped {
                    samples_per_beat(f64::from(ctx.bpm), f64::from(data.sample_rate))
                } else {
                    samples_per_beat(*original_bpm, f64::from(data.sample_rate))
                };
                render_audio(
                    data,
                    *gain,
                    beats_to_samples,
                    base,
                    dt,
                    &mut left[start..end],
                    &mut right[start..end],
                );
            }
        }
        base += (end - start) as f64 * dt;
        if let Some(length) = pass.loop_length
            && base >= length
        {
            base -= length;
        }
        start = end;
    }
}

/// Schedules the note events of one segment of a block. The note sounds on the frames whose clip
/// position lies in `[note.start, note.end)`, where frame `k` of the segment is at `base + k * dt`.
fn schedule_notes(
    notes: &[PlayNote],
    base: f64,
    dt: f64,
    seg_start: usize,
    seg_end: usize,
    events: &mut EventList,
    sounding: &mut Vec<(usize, u8)>,
) {
    let seg_len = (seg_end - seg_start) as f64;
    for (index, note) in notes.iter().enumerate() {
        let first = ((note.start - base) / dt).ceil();
        let last = ((note.end - base) / dt).ceil();
        if first >= last {
            continue;
        }
        if first >= 0.0 && first < seg_len && sounding.len() < sounding.capacity() {
            let offset = (seg_start + first as usize) as u32;
            if events.push(NoteEvent::On {
                offset,
                pitch: note.pitch,
                velocity: note.velocity,
                freq_hz: note.freq_hz,
            }) {
                sounding.push((index, note.pitch));
            }
        }
        if last >= 0.0 && last < seg_len {
            let offset = (seg_start + last as usize) as u32;
            if let Some(at) = sounding.iter().position(|&(i, _)| i == index) {
                let (_, pitch) = sounding.swap_remove(at);
                let _ = events.push(NoteEvent::Off { offset, pitch });
            }
        }
    }
    events.sort_by_offset();
}

/// Writes the audio of a clip over one segment, with the same interpolation as the decoder path
/// did before M05 item 7. Samples past the end of the file are silent.
fn render_audio(
    data: &DecodedAudio,
    gain: f32,
    beats_to_samples: f64,
    base: f64,
    dt: f64,
    left: &mut [f32],
    right: &mut [f32],
) {
    let total = data.samples_l.len();
    if total == 0 {
        return;
    }
    let last = (total - 1) as isize;
    let clamp = |index: isize| index.clamp(0, last) as usize;
    for (frame, (out_l, out_r)) in left.iter_mut().zip(right.iter_mut()).enumerate() {
        let position = base + frame as f64 * dt;
        let sample_position = position * beats_to_samples;
        if position < 0.0 || sample_position < 0.0 {
            continue;
        }
        let index = sample_position.floor() as isize;
        if index >= total as isize {
            continue;
        }
        let mu = (sample_position - sample_position.floor()) as f32;
        let (i0, i1, i2, i3) = (
            clamp(index - 1),
            clamp(index),
            clamp(index + 1),
            clamp(index + 2),
        );
        *out_l = hermite_interp(
            data.samples_l[i0],
            data.samples_l[i1],
            data.samples_l[i2],
            data.samples_l[i3],
            mu,
        ) * gain;
        *out_r = hermite_interp(
            data.samples_r[i0],
            data.samples_r[i1],
            data.samples_r[i2],
            data.samples_r[i3],
            mu,
        ) * gain;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use basedrop::Collector;

    /// A context whose beats advance by `beats_per_frame` per frame, so frame `k` is at
    /// `playhead + k * beats_per_frame`. Quarter-beat steps are exact in binary.
    fn context(frames: usize, playhead: f64, beats_per_frame: f64) -> ProcessContext {
        ProcessContext {
            sample_rate: 44100.0,
            frames,
            bpm: 120.0,
            playhead_beats: playhead,
            beats_per_frame,
        }
    }

    fn note(start: f64, end: f64, pitch: u8) -> PlayNote {
        PlayNote {
            start,
            end,
            pitch,
            velocity: 100,
            freq_hz: 261.6,
        }
    }

    fn midi_clip(collector: &Collector, length: f64, notes: Vec<PlayNote>) -> PlayableClip {
        PlayableClip {
            length_beats: length,
            is_looping: false,
            source: PlayableSource::Midi(Shared::new(&collector.handle(), notes)),
        }
    }

    /// A lane with one launched slot holding `clip`.
    fn lane_with(clip: PlayableClip) -> ClipLane {
        let mut lane = ClipLane::new(vec![Some(clip)], Vec::new(), true, 16);
        lane.launch(0);
        lane
    }

    fn schedule(lane: &mut ClipLane, ctx: &ProcessContext) -> EventList {
        let mut events = EventList::with_capacity(32);
        let mut left = vec![0.0; ctx.frames];
        let mut right = vec![0.0; ctx.frames];
        lane.schedule(ctx, &mut events, &mut left, &mut right);
        events
    }

    #[test]
    fn a_note_starts_on_the_frame_its_position_is_reached() {
        let collector = Collector::new();
        let mut lane = lane_with(midi_clip(&collector, 4.0, vec![note(1.0, 1.5, 60)]));
        let events = schedule(&mut lane, &context(16, 0.0, 0.25));
        let on = events.iter().find_map(|event| match event {
            NoteEvent::On { offset, pitch, .. } => Some((*offset, *pitch)),
            NoteEvent::Off { .. } => None,
        });
        assert_eq!(on, Some((4, 60)));
    }

    #[test]
    fn a_note_ends_with_a_note_off_on_its_frame() {
        let collector = Collector::new();
        let mut lane = lane_with(midi_clip(&collector, 4.0, vec![note(0.0, 0.5, 60)]));
        let events = schedule(&mut lane, &context(8, 0.0, 0.25));
        let offs: Vec<u32> = events
            .iter()
            .filter_map(|event| match event {
                NoteEvent::Off { offset, .. } => Some(*offset),
                NoteEvent::On { .. } => None,
            })
            .collect();
        assert_eq!(offs, vec![2]);
    }

    #[test]
    fn a_loop_wrap_inside_a_block_places_the_next_note_on_its_frame() {
        let collector = Collector::new();
        let mut lane = lane_with(midi_clip(&collector, 4.0, vec![note(0.25, 0.5, 60)]));
        // The clip is at beat 3.5 and wraps to 0.0 at frame 2, so the note starts at frame 3.
        let events = schedule(&mut lane, &context(16, 3.5, 0.25));
        assert!(events.iter().any(|event| matches!(
            event,
            NoteEvent::On {
                offset: 3,
                pitch: 60,
                ..
            }
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            NoteEvent::Off {
                offset: 4,
                pitch: 60
            }
        )));
    }

    #[test]
    fn stopping_the_clip_ends_the_notes_that_are_sounding() {
        let collector = Collector::new();
        let mut lane = lane_with(midi_clip(&collector, 4.0, vec![note(0.0, 2.0, 60)]));
        let started = schedule(&mut lane, &context(4, 0.0, 0.25));
        assert!(
            started
                .iter()
                .any(|event| matches!(event, NoteEvent::On { .. }))
        );
        lane.stop();
        let stopped = schedule(&mut lane, &context(4, 1.0, 0.25));
        assert!(stopped.iter().any(|event| matches!(
            event,
            NoteEvent::Off {
                offset: 0,
                pitch: 60
            }
        )));
    }

    #[test]
    fn audio_plays_from_shared_data_and_is_silent_past_the_file() {
        let collector = Collector::new();
        let data = DecodedAudio {
            samples_l: vec![0.5; 100],
            samples_r: vec![0.5; 100],
            sample_rate: 44100,
            channels: 2,
        };
        let clip = PlayableClip {
            length_beats: 4.0,
            is_looping: false,
            source: PlayableSource::Audio {
                data: Shared::new(&collector.handle(), data),
                gain: 1.0,
                is_warped: false,
                original_bpm: 120.0,
            },
        };
        // At 120 BPM and 44.1 kHz one frame is one sample, so the file covers frames 0 to 99.
        let mut lane = lane_with(clip);
        let ctx = context(512, 0.0, 1.0 / 22050.0);
        let mut left = vec![0.0; ctx.frames];
        let mut right = vec![0.0; ctx.frames];
        lane.schedule(
            &ctx,
            &mut EventList::with_capacity(4),
            &mut left,
            &mut right,
        );
        assert!((left[0] - 0.5).abs() < 1e-6);
        assert_eq!(left[200], 0.0);
        assert_eq!(right[200], 0.0);
    }
}
