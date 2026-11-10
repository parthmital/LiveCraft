//! Clip playback on the audio thread (M05 item 7, M07). The session side prepares every clip the graph
//! can play as shared, immutable data (`basedrop::Shared`): note lists with frequencies, and decoded
//! audio. The audio thread reads that data and schedules note events at their sample offsets, so it
//! never clones a clip, decodes a file, or allocates. A launched Session clip plays from its launch
//! on the launch clock (`launch::Launcher`); the Arrangement plays from the song position.

use crate::audio_clip::{DecodedAudio, hermite_interp};
use crate::launch::{LaunchInfo, Launcher, Running};
use crate::streamer::StreamAudioPlayer;
use basedrop::Shared;
use livecraft_devices::{EventList, NoteEvent, ProcessContext};
use livecraft_model::model::{ClipLaunch, FollowActions};
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
    Streamed {
        player: Box<StreamAudioPlayer>,
        gain: f32,
        /// True when the clip follows the Set's tempo, as a warped clip does.
        is_warped: bool,
        original_bpm: f64,
        sample_rate: u32,
        total_frames: u64,
    },
}

/// A clip ready to play: its length in beats, whether it loops, its source, and how it launches.
pub struct PlayableClip {
    pub length_beats: f64,
    pub is_looping: bool,
    pub source: PlayableSource,
    pub launch: ClipLaunch,
    pub follow: FollowActions,
}

impl PlayableClip {
    /// A clip with the default launch settings and no Follow Actions.
    pub fn new(length_beats: f64, is_looping: bool, source: PlayableSource) -> Self {
        Self {
            length_beats,
            is_looping,
            source,
            launch: ClipLaunch::default(),
            follow: FollowActions::default(),
        }
    }

    /// What the launcher needs to know about the clip.
    pub fn info(&self) -> LaunchInfo {
        LaunchInfo {
            launch: self.launch,
            follow: self.follow,
            length_beats: self.length_beats.max(1.0),
            is_looping: self.is_looping,
        }
    }
}

/// An arrangement clip, placed on the timeline.
pub struct ArrangedClip {
    pub start_beat: f64,
    pub length_beats: f64,
    pub clip: PlayableClip,
}

/// The clips of one track and the notes they have started. A track plays its launched Session clip,
/// else the Arrangement clip under the playhead unless a Session clip took the track out of the
/// Arrangement (manual 3.7, page 42). A Session clip that was not launched stays silent (see
/// `schedule`).
pub struct ClipLane {
    /// One entry per clip slot row; `None` for an empty slot.
    pub slots: Vec<Option<PlayableClip>>,
    /// Whether each row holds a clip, for Follow Action groups.
    pub filled: Vec<bool>,
    /// Whether each row's slot has a Clip Stop button, which a scene launch presses on an empty slot.
    pub stop_buttons: Vec<bool>,
    pub arranged: Vec<ArrangedClip>,
    /// What plays and what is queued (M07).
    pub launcher: Launcher,
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
        let filled = slots.iter().map(Option::is_some).collect();
        let stop_buttons = vec![true; slots.len()];
        Self {
            slots,
            filled,
            stop_buttons,
            arranged,
            launcher: Launcher::default(),
            plays_clips,
            sounding: Vec::with_capacity(capacity),
            stop_pending: false,
            playing: false,
        }
    }

    /// The row of the clip the track plays, if any.
    pub fn active_slot(&self) -> Option<usize> {
        self.launcher.playing_slot()
    }

    /// The launch settings of the clip at `slot`, if the row holds one.
    pub fn info(&self, slot: usize) -> Option<LaunchInfo> {
        self.slots
            .get(slot)
            .and_then(Option::as_ref)
            .map(PlayableClip::info)
    }

    /// Starts `slot` at once, from its start at launch clock time 0, as a launch at the song start does.
    pub fn launch(&mut self, slot: usize) {
        let gain = 1.0;
        self.launcher.running = Some(Running {
            slot,
            origin: 0.0,
            gain,
            follow_at: None,
            end_at: None,
        });
        self.launcher.left_arrangement = true;
        self.stop_pending = true;
    }

    /// Stops the clip the track is playing, at once.
    pub fn stop(&mut self) {
        self.launcher.halt();
        self.stop_pending = true;
    }

    /// Marks the notes that sound for note-offs at the start of the next segment, because the clip
    /// changed or stopped.
    pub fn changed(&mut self) {
        self.stop_pending = true;
    }

    /// Takes over the launch state and the sounding notes of the same track in the graph this one
    /// replaces, so a graph swap neither stops a launched clip nor leaves a note without its note-off.
    /// It swaps vectors and pointers only, so it does not allocate or drop on the audio thread.
    pub fn adopt(&mut self, previous: &mut ClipLane) {
        self.launcher = previous.launcher;
        std::mem::swap(&mut self.sounding, &mut previous.sounding);
        self.stop_pending = previous.stop_pending;
        for (new_slot, old_slot) in self.slots.iter_mut().zip(previous.slots.iter_mut()) {
            if let (Some(new_clip), Some(old_clip)) = (new_slot.as_mut(), old_slot.as_mut()) {
                adopt_stream_player(&mut new_clip.source, &mut old_clip.source);
            }
        }
        for (new_arr, old_arr) in self.arranged.iter_mut().zip(previous.arranged.iter_mut()) {
            if (new_arr.start_beat - old_arr.start_beat).abs() < 1e-9 {
                adopt_stream_player(&mut new_arr.clip.source, &mut old_arr.clip.source);
            }
        }
    }

    /// Lock-free disk streaming telemetry `(disk_load, total_underruns)` across this lane's
    /// streamed clips.
    pub fn stream_telemetry(&self) -> (f32, u64) {
        let mut load = 0.0f32;
        let mut underruns = 0u64;
        for clip in self
            .slots
            .iter()
            .flatten()
            .chain(self.arranged.iter().map(|a| &a.clip))
        {
            if let PlayableSource::Streamed { player, .. } = &clip.source {
                load += player.disk_load();
                underruns = underruns.saturating_add(player.underruns());
            }
        }
        (load, underruns)
    }

    /// Fills `events` with this block's note events and `left` and `right` with this block's audio,
    /// for the frames of `ctx`. The buffers must already be cleared.
    ///
    /// `clock` is the launch clock at the first frame, which places a launched clip; the Arrangement
    /// plays from `ctx.playhead_beats`.
    pub fn schedule(
        &mut self,
        ctx: &ProcessContext,
        clock: f64,
        events: &mut EventList,
        left: &mut [f32],
        right: &mut [f32],
    ) {
        for clip in self
            .slots
            .iter_mut()
            .flatten()
            .chain(self.arranged.iter_mut().map(|a| &mut a.clip))
        {
            if let PlayableSource::Streamed { player, .. } = &mut clip.source {
                player.begin_block();
            }
        }
        let beats_per_frame = ctx.beats_per_frame;
        self.playing = false;
        if self.stop_pending || !self.plays_clips || beats_per_frame == 0.0 {
            self.end_sounding(events);
        }
        if !self.plays_clips || beats_per_frame == 0.0 {
            return;
        }
        let position = ctx.playhead_beats;
        if let Some(running) = self.launcher.running {
            if let Some(clip) = self.slots.get_mut(running.slot).and_then(Option::as_mut) {
                self.playing = true;
                let info = clip.info();
                play(
                    PlayPass {
                        clip,
                        loop_length: info.is_looping.then_some(info.length_beats),
                        position: running.position(clock, &info),
                        gain: running.gain,
                    },
                    ctx,
                    events,
                    &mut self.sounding,
                    left,
                    right,
                );
                return;
            }
        } else if !self.arranged.is_empty() && !self.launcher.left_arrangement {
            let found = self.arranged.iter_mut().find(|arranged| {
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
                        clip: &mut arranged.clip,
                        loop_length,
                        position: clip_position,
                        gain: 1.0,
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
        if self.launcher.running.is_some() || self.launcher.left_arrangement || !self.plays_clips {
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

fn adopt_stream_player(new: &mut PlayableSource, old: &mut PlayableSource) {
    if let (
        PlayableSource::Streamed {
            player: new_player, ..
        },
        PlayableSource::Streamed {
            player: old_player, ..
        },
    ) = (new, old)
        && new_player.stream_key() == old_player.stream_key()
    {
        std::mem::swap(new_player, old_player);
    }
}

/// One clip for one block: where it starts and how it loops.
struct PlayPass<'a> {
    clip: &'a mut PlayableClip,
    /// The loop length in beats, when the clip wraps back to its start.
    loop_length: Option<f64>,
    /// The clip position in beats at the first frame of the block.
    position: f64,
    /// The launch velocity gain (manual 16.5).
    gain: f32,
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
        match &mut pass.clip.source {
            PlayableSource::Midi(notes) => {
                schedule_notes(notes, base, dt, start..end, pass.gain, events, sounding);
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
                    *gain * pass.gain,
                    beats_to_samples,
                    base,
                    dt,
                    &mut left[start..end],
                    &mut right[start..end],
                );
            }
            PlayableSource::Streamed {
                player,
                gain,
                is_warped,
                original_bpm,
                sample_rate,
                total_frames,
            } => {
                let beats_to_samples = if *is_warped {
                    samples_per_beat(f64::from(ctx.bpm), f64::from(*sample_rate))
                } else {
                    samples_per_beat(*original_bpm, f64::from(*sample_rate))
                };
                render_streamed(
                    player,
                    *total_frames,
                    *gain * pass.gain,
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
    segment: std::ops::Range<usize>,
    gain: f32,
    events: &mut EventList,
    sounding: &mut Vec<(usize, u8)>,
) {
    let (seg_start, seg_end) = (segment.start, segment.end);
    let seg_len = (seg_end - seg_start) as f64;
    // A sounding note whose span no longer holds the segment's first frame ended at the segment
    // boundary: at a loop wrap, or at the last frame of the previous segment or block.
    sounding.retain(|&(index, pitch)| {
        let holds = notes
            .get(index)
            .is_some_and(|note| base >= note.start && base < note.end);
        if !holds {
            let _ = events.push(NoteEvent::Off {
                offset: seg_start as u32,
                pitch,
            });
        }
        holds
    });
    for (index, note) in notes.iter().enumerate() {
        let first = ((note.start - base) / dt).ceil();
        let last = ((note.end - base) / dt).ceil();
        if first >= last {
            continue;
        }
        if first >= 0.0 && first < seg_len {
            let offset = (seg_start + first as usize) as u32;
            // A note that restarts while it still sounds (it spans the whole loop) is retriggered.
            if let Some(at) = sounding.iter().position(|&(i, _)| i == index) {
                let (_, pitch) = sounding.swap_remove(at);
                let _ = events.push(NoteEvent::Off { offset, pitch });
            }
            // A launch velocity gain of zero plays the clip silently, so its notes are not sent.
            let velocity = (f32::from(note.velocity) * gain).round().min(127.0) as u8;
            if velocity > 0
                && sounding.len() < sounding.capacity()
                && events.push(NoteEvent::On {
                    offset,
                    pitch: note.pitch,
                    velocity,
                    freq_hz: note.freq_hz,
                })
            {
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

/// Writes the audio of a disk-streamed clip over one segment using Hermite interpolation.
#[allow(clippy::too_many_arguments)]
fn render_streamed(
    player: &mut StreamAudioPlayer,
    total_frames: u64,
    gain: f32,
    beats_to_samples: f64,
    base: f64,
    dt: f64,
    left: &mut [f32],
    right: &mut [f32],
) {
    let effective_total = player.effective_total_frames(total_frames);
    if effective_total == 0 {
        return;
    }
    for (frame, (out_l, out_r)) in left.iter_mut().zip(right.iter_mut()).enumerate() {
        let position = base + frame as f64 * dt;
        let sample_position = position * beats_to_samples;
        if position < 0.0 || sample_position < 0.0 {
            continue;
        }
        let index = sample_position.floor() as u64;
        if index >= effective_total {
            continue;
        }
        let mu = (sample_position - sample_position.floor()) as f32;
        let [l, r] = player.read_interpolated(index, mu, effective_total);
        *out_l = l * gain;
        *out_r = r * gain;
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
        PlayableClip::new(
            length,
            true,
            PlayableSource::Midi(Shared::new(&collector.handle(), notes)),
        )
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
        lane.schedule(ctx, ctx.playhead_beats, &mut events, &mut left, &mut right);
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

    /// The (offset, is_on, pitch) of each event, in order.
    fn listed(events: &EventList) -> Vec<(u32, bool, u8)> {
        events
            .iter()
            .map(|event| match *event {
                NoteEvent::On { offset, pitch, .. } => (offset, true, pitch),
                NoteEvent::Off { offset, pitch } => (offset, false, pitch),
            })
            .collect()
    }

    #[test]
    fn a_note_that_ends_at_the_loop_end_is_released_at_the_wrap() {
        let collector = Collector::new();
        let mut lane = lane_with(midi_clip(&collector, 1.0, vec![note(0.5, 1.0, 60)]));
        // Two passes of a one-beat loop in one block: the note ends where the loop wraps.
        let events = schedule(&mut lane, &context(8, 0.0, 0.25));
        assert_eq!(
            listed(&events),
            vec![(2, true, 60), (4, false, 60), (6, true, 60)]
        );
        assert_eq!(lane.sounding.len(), 1);
    }

    #[test]
    fn a_note_that_ends_at_the_loop_end_of_a_block_is_released_at_the_next_block() {
        let collector = Collector::new();
        let mut lane = lane_with(midi_clip(&collector, 1.0, vec![note(0.5, 1.0, 60)]));
        let first = schedule(&mut lane, &context(4, 0.0, 0.25));
        assert_eq!(listed(&first), vec![(2, true, 60)]);
        let second = schedule(&mut lane, &context(4, 1.0, 0.25));
        assert_eq!(listed(&second), vec![(0, false, 60), (2, true, 60)]);
        assert_eq!(lane.sounding.len(), 1);
    }

    #[test]
    fn a_note_that_spans_the_whole_loop_is_retriggered_not_stacked() {
        let collector = Collector::new();
        let mut lane = lane_with(midi_clip(&collector, 1.0, vec![note(0.0, 1.0, 60)]));
        let events = schedule(&mut lane, &context(8, 0.0, 0.25));
        assert_eq!(
            listed(&events),
            vec![(0, true, 60), (4, false, 60), (4, true, 60)]
        );
        assert_eq!(lane.sounding.len(), 1);
    }

    #[test]
    fn overlapping_notes_of_one_pitch_are_two_notes_that_end_apart() {
        let collector = Collector::new();
        let notes = vec![note(0.0, 1.5, 60), note(1.0, 2.5, 60)];
        let mut lane = lane_with(midi_clip(&collector, 4.0, notes));
        let events = schedule(&mut lane, &context(12, 0.0, 0.25));
        assert_eq!(
            listed(&events),
            vec![
                (0, true, 60),
                (4, true, 60),
                (6, false, 60),
                (10, false, 60)
            ]
        );
        assert_eq!(lane.sounding.len(), 0);
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
            launch: Default::default(),
            follow: Default::default(),
        };
        // At 120 BPM and 44.1 kHz one frame is one sample, so the file covers frames 0 to 99.
        let mut lane = lane_with(clip);
        let ctx = context(512, 0.0, 1.0 / 22050.0);
        let mut left = vec![0.0f32; ctx.frames];
        let mut right = vec![0.0f32; ctx.frames];
        lane.schedule(
            &ctx,
            0.0,
            &mut EventList::with_capacity(4),
            &mut left,
            &mut right,
        );
        assert!((left[0] - 0.5).abs() < 1e-6);
        assert_eq!(left[200], 0.0);
        assert_eq!(right[200], 0.0);
    }
}
