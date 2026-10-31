//! The clip launch model of one track (M07 items 4 to 8; manual 16.2 to 16.7).
//!
//! Times here are beats on the launch clock, which runs on while the transport plays and does not jump
//! when the Arrangement loops or the playhead moves, so launched clips keep their sync. The executor
//! turns quantized song positions into launch clock times. Everything here is plain data and
//! arithmetic, so it runs on the audio thread without allocating.

use livecraft_model::model::{ClipLaunch, FollowAction, FollowActions, LaunchMode};

/// A small, fast, seeded random number generator: PCG32 (64-bit state, XSH RR output), written here so
/// Follow Action renders are the same for a stored seed (M07 decisions).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pcg32 {
    state: u64,
    increment: u64,
}

impl Pcg32 {
    const MULTIPLIER: u64 = 6_364_136_223_846_793_005;

    pub fn new(seed: u64) -> Self {
        let mut rng = Self {
            state: 0,
            increment: (0xda3e_39cb_94b9_5bdb_u64 << 1) | 1,
        };
        rng.next_u32();
        rng.state = rng.state.wrapping_add(seed);
        rng.next_u32();
        rng
    }

    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old
            .wrapping_mul(Self::MULTIPLIER)
            .wrapping_add(self.increment);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rotation = (old >> 59) as u32;
        xorshifted.rotate_right(rotation)
    }

    /// A number in `0.0..1.0`.
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }

    /// A number in `0..n`, or 0 when `n` is 0.
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        ((u64::from(self.next_u32()) * n as u64) >> 32) as usize
    }
}

/// The gain a launch velocity gives a clip: "If set to zero, there is no influence; at 100 percent, the
/// softest notes play the clip silently." (manual 16.5, page 364)
pub fn velocity_gain(amount: f32, velocity: u8) -> f32 {
    let amount = amount.clamp(0.0, 1.0);
    1.0 - amount + amount * f32::from(velocity.min(127)) / 127.0
}

/// Picks Follow Action A or B by their chances (manual 16.7, page 366). Both at zero picks nothing.
pub fn pick_action(follow: &FollowActions, rng: &mut Pcg32) -> FollowAction {
    let a = follow.chance_a.max(0.0);
    let b = follow.chance_b.max(0.0);
    if a + b <= 0.0 {
        return FollowAction::NoAction;
    }
    if rng.next_f32() * (a + b) < a {
        follow.action_a
    } else {
        follow.action_b
    }
}

/// What a Follow Action does to the track.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FollowTarget {
    /// Nothing happens, and the clip's Follow Actions end for this play.
    Nothing,
    Stop,
    Launch(usize),
}

/// The group of `slot`: the run of filled slots around it, as a range of rows. "A group is defined by
/// clips arranged in successive slots of the same track. Tracks can have an unlimited number of groups,
/// separated by empty slots." (manual 16.7, page 366)
pub fn group_of(filled: &[bool], slot: usize) -> std::ops::Range<usize> {
    if !filled.get(slot).copied().unwrap_or(false) {
        return slot..slot;
    }
    let mut start = slot;
    while start > 0 && filled[start - 1] {
        start -= 1;
    }
    let mut end = slot + 1;
    while end < filled.len() && filled[end] {
        end += 1;
    }
    start..end
}

/// Where a Follow Action goes from the clip at `slot`, among the slots that hold clips (manual 16.7,
/// page 367).
pub fn follow_target(
    action: FollowAction,
    slot: usize,
    filled: &[bool],
    rng: &mut Pcg32,
) -> FollowTarget {
    let group = group_of(filled, slot);
    if group.is_empty() {
        return FollowTarget::Nothing;
    }
    let (first, last) = (group.start, group.end - 1);
    match action {
        FollowAction::NoAction => FollowTarget::Nothing,
        FollowAction::Stop => FollowTarget::Stop,
        FollowAction::PlayAgain => FollowTarget::Launch(slot),
        // "triggers the previous clip (the one above the current one)". The first clip wraps to the last,
        // as Next wraps the last to the first.
        FollowAction::Previous => FollowTarget::Launch(if slot == first { last } else { slot - 1 }),
        // "If a clip with this setting is last in a group, this Follow Action triggers the first clip."
        FollowAction::Next => FollowTarget::Launch(if slot == last { first } else { slot + 1 }),
        FollowAction::First => FollowTarget::Launch(first),
        FollowAction::Last => FollowTarget::Launch(last),
        FollowAction::Any => FollowTarget::Launch(first + rng.below(group.len())),
        // "as long as the current clip is not alone in the group, no clip will play consecutively."
        FollowAction::Other => {
            if group.len() == 1 {
                return FollowTarget::Launch(slot);
            }
            let pick = first + rng.below(group.len() - 1);
            FollowTarget::Launch(if pick >= slot { pick + 1 } else { pick })
        }
        FollowAction::Jump(row) => {
            let row = row as usize;
            if filled.get(row).copied().unwrap_or(false) {
                FollowTarget::Launch(row)
            } else {
                FollowTarget::Nothing
            }
        }
    }
}

/// What a clip needs for launching: its launch settings, Follow Actions, and length.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LaunchInfo {
    pub launch: ClipLaunch,
    pub follow: FollowActions,
    pub length_beats: f64,
    pub is_looping: bool,
}

/// A clip that plays: its row, the launch clock time at which its position is 0, its velocity gain,
/// and when its Follow Action and its end come.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Running {
    pub slot: usize,
    pub origin: f64,
    pub gain: f32,
    /// When the Follow Action happens, while it can still happen this play.
    pub follow_at: Option<f64>,
    /// When a clip that does not loop ends.
    pub end_at: Option<f64>,
}

impl Running {
    /// The clip position at `now`, from its start. A looping clip wraps at its length.
    pub fn position(&self, now: f64, info: &LaunchInfo) -> f64 {
        let position = (now - self.origin).max(0.0);
        let length = info.length_beats.max(1e-9);
        if info.is_looping {
            position % length
        } else {
            position
        }
    }
}

/// A launch or stop waiting for its time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Queued {
    Launch { slot: usize, at: f64, velocity: u8 },
    Stop { at: f64 },
}

impl Queued {
    pub fn at(&self) -> f64 {
        match *self {
            Queued::Launch { at, .. } | Queued::Stop { at } => at,
        }
    }
}

/// What happened when the launcher reached a time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Fired {
    Launched(usize),
    Stopped,
    /// The Follow Action of the clip at this row is due.
    FollowDue(usize),
}

/// A launch button held in Repeat mode: the row, when it triggers next, and how often.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Repeating {
    pub slot: usize,
    pub next: f64,
    pub period: f64,
    pub velocity: u8,
}

/// The launch state of one track: at most one playing clip and one queued launch or stop (M07
/// decisions), the slot held down in Repeat mode, and whether the track left the Arrangement.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Launcher {
    pub running: Option<Running>,
    pub queued: Option<Queued>,
    /// The slot held in Repeat mode.
    pub repeating: Option<Repeating>,
    /// True once a Session clip played on the track, until Back to Arrangement (manual 3.7, page 42).
    pub left_arrangement: bool,
}

impl Launcher {
    /// The row of the clip that plays, if any.
    pub fn playing_slot(&self) -> Option<usize> {
        self.running.map(|running| running.slot)
    }

    /// The row of the queued launch, if any.
    pub fn queued_slot(&self) -> Option<usize> {
        match self.queued {
            Some(Queued::Launch { slot, .. }) => Some(slot),
            _ => None,
        }
    }

    /// A down on the launch button of `slot`, whose quantized launch time is `at` (manual 16.2).
    /// `repeat_period` is the clip's quantization period, for Repeat mode.
    pub fn press(
        &mut self,
        slot: usize,
        info: &LaunchInfo,
        velocity: u8,
        at: f64,
        repeat_period: Option<f64>,
    ) {
        let current = self.playing_slot() == Some(slot) || self.queued_slot() == Some(slot);
        match info.launch.mode {
            LaunchMode::Toggle if current => self.queued = Some(Queued::Stop { at }),
            LaunchMode::Repeat => {
                self.queue_launch(slot, at, velocity);
                self.repeating =
                    repeat_period
                        .filter(|period| *period > 0.0)
                        .map(|period| Repeating {
                            slot,
                            next: at + period,
                            period,
                            velocity,
                        });
            }
            _ => self.queue_launch(slot, at, velocity),
        }
    }

    /// An up on the launch button of `slot`, whose quantized stop time is `at` (manual 16.2).
    pub fn release(&mut self, slot: usize, info: &LaunchInfo, at: f64) {
        match info.launch.mode {
            LaunchMode::Gate
                if self.playing_slot() == Some(slot) || self.queued_slot() == Some(slot) =>
            {
                self.queued = Some(Queued::Stop { at });
            }
            LaunchMode::Repeat if self.repeating.is_some_and(|held| held.slot == slot) => {
                self.repeating = None;
            }
            _ => {}
        }
    }

    fn queue_launch(&mut self, slot: usize, at: f64, velocity: u8) {
        self.queued = Some(Queued::Launch { slot, at, velocity });
    }

    /// Queues a stop at `at`. A held Repeat ends.
    pub fn stop(&mut self, at: f64) {
        self.queued = Some(Queued::Stop { at });
        self.repeating = None;
    }

    /// Stops at once and forgets the queue, as stopping the transport does.
    pub fn halt(&mut self) {
        self.running = None;
        self.queued = None;
        self.repeating = None;
    }

    /// The first time after `now` at which something happens: a queued launch or stop, a Repeat
    /// trigger, a Follow Action, or the end of a clip that does not loop.
    pub fn next_event(&self, now: f64, follow_enabled: bool) -> Option<f64> {
        let running = self.running.as_ref();
        [
            self.queued.map(|queued| queued.at()),
            self.repeating.map(|held| held.next),
            running
                .and_then(|running| running.follow_at)
                .filter(|_| follow_enabled),
            running.and_then(|running| running.end_at),
        ]
        .into_iter()
        .flatten()
        .filter(|at| *at > now)
        .reduce(f64::min)
    }

    /// Does what is due at `now`, one thing per call, in order: a Follow Action, a queued launch or stop,
    /// a Repeat trigger, and the end of the clip. `info` gives each row's launch settings. The caller
    /// calls again until it returns None.
    pub fn fire(
        &mut self,
        now: f64,
        epsilon: f64,
        follow_enabled: bool,
        info: impl Fn(usize) -> Option<LaunchInfo>,
    ) -> Option<Fired> {
        let due = |at: f64| at <= now + epsilon;
        if let Some(running) = self.running.as_mut()
            && let Some(at) = running.follow_at
            && due(at)
        {
            running.follow_at = None;
            if follow_enabled {
                return Some(Fired::FollowDue(running.slot));
            }
        }
        if let Some(queued) = self.queued
            && due(queued.at())
        {
            self.queued = None;
            return Some(match queued {
                Queued::Launch { slot, at, velocity } => {
                    let Some(clip) = info(slot) else {
                        self.running = None;
                        return Some(Fired::Stopped);
                    };
                    self.start(slot, &clip, at, velocity);
                    Fired::Launched(slot)
                }
                Queued::Stop { .. } => {
                    self.running = None;
                    Fired::Stopped
                }
            });
        }
        if let Some(held) = self.repeating
            && due(held.next)
        {
            let clip = info(held.slot)?;
            // Each repeat starts the clip again from its start.
            self.running = None;
            self.start(held.slot, &clip, held.next, held.velocity);
            self.repeating = Some(Repeating {
                next: held.next + held.period,
                ..held
            });
            return Some(Fired::Launched(held.slot));
        }
        if let Some(running) = self.running
            && running.end_at.is_some_and(due)
        {
            self.running = None;
            return Some(Fired::Stopped);
        }
        None
    }

    /// Starts `slot` at `at`. In Legato Mode the clip "takes over the play position from whatever clip
    /// was played in that track before" (manual 16.3, page 362).
    /// Follow Action launches keep this too: "Legato Mode will sync the new play position with the old one
    /// in beat-time" (16.7.4, page 370).
    fn start(&mut self, slot: usize, clip: &LaunchInfo, at: f64, velocity: u8) {
        let mut origin = at;
        if clip.launch.legato
            && let Some(previous) = self.running
        {
            origin = previous.origin.min(at);
        }
        let length = clip.length_beats.max(1e-9);
        let follow_at = clip.follow.enabled.then(|| {
            if clip.follow.linked {
                origin + length * f64::from(clip.follow.multiplier.max(1))
            } else {
                origin + clip.follow.time_beats.max(0.0)
            }
        });
        self.running = Some(Running {
            slot,
            origin,
            gain: velocity_gain(clip.launch.velocity_amount, velocity),
            follow_at: follow_at.filter(|time| *time > at),
            end_at: (!clip.is_looping).then_some(origin + length),
        });
        self.left_arrangement = true;
    }

    /// Queues the launch a Follow Action chose, at `at`.
    pub fn follow_launch(&mut self, slot: usize, at: f64) {
        self.queue_launch(slot, at, 127);
    }

    /// Moves the playing clip's position by `beats`, forward when positive (manual 16.6, page 364).
    pub fn nudge(&mut self, beats: f64, info: &LaunchInfo) {
        if let Some(running) = self.running.as_mut() {
            running.origin -= beats;
            if let Some(end) = running.end_at.as_mut() {
                *end -= beats;
            }
            if info.follow.linked
                && let Some(at) = running.follow_at.as_mut()
            {
                *at -= beats;
            }
        }
    }
}

#[cfg(test)]
mod tests;
