use super::*;
use livecraft_model::model::Quantization;

const EPS: f64 = 1e-9;

fn info(mode: LaunchMode) -> LaunchInfo {
    LaunchInfo {
        launch: ClipLaunch {
            mode,
            quantization: Quantization::Global,
            legato: false,
            velocity_amount: 0.0,
        },
        follow: FollowActions::default(),
        length_beats: 4.0,
        is_looping: true,
    }
}

/// Fires everything due at `now` and returns what happened.
fn fire_all(launcher: &mut Launcher, now: f64, clip: LaunchInfo) -> Vec<Fired> {
    let mut fired = Vec::new();
    while let Some(event) = launcher.fire(now, EPS, true, |_| Some(clip)) {
        fired.push(event);
    }
    fired
}

#[test]
fn trigger_down_starts_the_clip_and_up_is_ignored() {
    // "Trigger: down starts the clip; up is ignored." (manual 16.2, page 361)
    let clip = info(LaunchMode::Trigger);
    let mut launcher = Launcher::default();
    launcher.press(0, &clip, 127, 4.0, None);
    assert_eq!(fire_all(&mut launcher, 4.0, clip), vec![Fired::Launched(0)]);
    launcher.release(0, &clip, 8.0);
    assert_eq!(launcher.queued, None);
    assert_eq!(launcher.playing_slot(), Some(0));
}

#[test]
fn gate_down_starts_the_clip_and_up_stops_it() {
    // "Gate: down starts the clip; up stops the clip." (manual 16.2, page 361)
    let clip = info(LaunchMode::Gate);
    let mut launcher = Launcher::default();
    launcher.press(0, &clip, 127, 4.0, None);
    fire_all(&mut launcher, 4.0, clip);
    launcher.release(0, &clip, 8.0);
    assert_eq!(fire_all(&mut launcher, 8.0, clip), vec![Fired::Stopped]);
    assert_eq!(launcher.playing_slot(), None);
}

#[test]
fn toggle_down_starts_the_clip_up_is_ignored_and_the_next_down_stops_it() {
    // "Toggle: down starts the clip; up is ignored. The clip will stop on the next down." (16.2)
    let clip = info(LaunchMode::Toggle);
    let mut launcher = Launcher::default();
    launcher.press(0, &clip, 127, 0.0, None);
    fire_all(&mut launcher, 0.0, clip);
    launcher.release(0, &clip, 4.0);
    assert_eq!(launcher.queued, None);
    launcher.press(0, &clip, 127, 4.0, None);
    assert_eq!(fire_all(&mut launcher, 4.0, clip), vec![Fired::Stopped]);
}

#[test]
fn repeat_triggers_the_clip_at_the_quantization_rate_while_held() {
    // "Repeat: As long as the mouse switch/key is held, the clip is triggered repeatedly at the clip
    // quantization rate." (16.2)
    let clip = info(LaunchMode::Repeat);
    let mut launcher = Launcher::default();
    launcher.press(0, &clip, 127, 0.0, Some(1.0));
    fire_all(&mut launcher, 0.0, clip);
    assert_eq!(launcher.next_event(0.0, true), Some(1.0));
    assert_eq!(fire_all(&mut launcher, 1.0, clip), vec![Fired::Launched(0)]);
    assert_eq!(launcher.running.map(|r| r.origin), Some(1.0));
    launcher.release(0, &clip, 1.5);
    assert_eq!(launcher.repeating, None);
    assert_eq!(fire_all(&mut launcher, 2.0, clip), vec![]);
}

#[test]
fn a_legato_clip_takes_over_the_play_position_of_the_clip_before_it() {
    // "When a clip in Legato Mode is launched, it takes over the play position from whatever clip was
    // played in that track before." (16.3, page 362)
    let plain = info(LaunchMode::Trigger);
    let mut legato = plain;
    legato.launch.legato = true;
    let mut launcher = Launcher::default();
    launcher.press(0, &plain, 127, 0.0, None);
    fire_all(&mut launcher, 0.0, plain);
    launcher.press(1, &legato, 127, 2.5, None);
    fire_all(&mut launcher, 2.5, legato);
    let running = launcher.running.expect("the legato clip plays");
    assert_eq!(running.slot, 1);
    assert_eq!(running.position(2.5, &legato), 2.5);

    // Without Legato Mode, the clip starts from its start.
    launcher.press(0, &plain, 127, 3.0, None);
    fire_all(&mut launcher, 3.0, plain);
    assert_eq!(launcher.running.expect("plays").position(3.0, &plain), 0.0);
}

#[test]
fn velocity_amount_zero_has_no_influence_and_full_makes_the_softest_notes_silent() {
    // "If set to zero, there is no influence; at 100 percent, the softest notes play the clip silently."
    // (16.5, page 364)
    assert_eq!(velocity_gain(0.0, 1), 1.0);
    assert_eq!(velocity_gain(1.0, 0), 0.0);
    assert_eq!(velocity_gain(1.0, 127), 1.0);
    assert!((velocity_gain(0.5, 0) - 0.5).abs() < 1e-6);
}

#[test]
fn nudging_jumps_within_the_playing_clip() {
    // "To jump within a playing clip in increments the size of the global quantization period, you can
    // use the Nudge Backward/Forward buttons." (16.6, page 364)
    let clip = info(LaunchMode::Trigger);
    let mut launcher = Launcher::default();
    launcher.press(0, &clip, 127, 0.0, None);
    fire_all(&mut launcher, 0.0, clip);
    launcher.nudge(1.0, &clip);
    assert_eq!(launcher.running.expect("plays").position(0.5, &clip), 1.5);
    launcher.nudge(-1.0, &clip);
    assert_eq!(launcher.running.expect("plays").position(0.5, &clip), 0.5);
}

#[test]
fn a_clip_that_does_not_loop_stops_at_its_end() {
    let mut clip = info(LaunchMode::Trigger);
    clip.is_looping = false;
    let mut launcher = Launcher::default();
    launcher.press(0, &clip, 127, 2.0, None);
    fire_all(&mut launcher, 2.0, clip);
    assert_eq!(launcher.next_event(2.0, true), Some(6.0));
    assert_eq!(fire_all(&mut launcher, 6.0, clip), vec![Fired::Stopped]);
}

#[test]
fn a_launched_clip_leaves_the_arrangement() {
    // "The Arrangement clips in the track where the Session clip was launched will not resume playback
    // until you manually restart it using the Back to Arrangement button." (3.7, page 42)
    let clip = info(LaunchMode::Trigger);
    let mut launcher = Launcher::default();
    launcher.press(0, &clip, 127, 0.0, None);
    fire_all(&mut launcher, 0.0, clip);
    launcher.stop(1.0);
    fire_all(&mut launcher, 1.0, clip);
    assert!(launcher.left_arrangement);
}

fn follow(action_a: FollowAction, linked: bool) -> LaunchInfo {
    let mut clip = info(LaunchMode::Trigger);
    clip.follow = FollowActions {
        enabled: true,
        action_a,
        linked,
        ..FollowActions::default()
    };
    clip
}

#[test]
fn a_linked_follow_action_is_due_at_the_end_of_the_clip_times_the_multiplier() {
    // "In Linked mode, the Follow Action is triggered at the end of the clip, or after the number of
    // loops set in the Follow Action Multiplier field." (16.7, page 366)
    let mut clip = follow(FollowAction::Next, true);
    clip.follow.multiplier = 2;
    let mut launcher = Launcher::default();
    launcher.press(0, &clip, 127, 0.0, None);
    fire_all(&mut launcher, 0.0, clip);
    assert_eq!(launcher.next_event(0.0, true), Some(8.0));
    assert_eq!(
        fire_all(&mut launcher, 8.0, clip),
        vec![Fired::FollowDue(0)]
    );
}

#[test]
fn an_unlinked_follow_action_is_due_after_the_follow_action_time() {
    // "In Unlinked mode, the Follow Action is triggered after the clip has played for the duration of
    // the Follow Action Time." (16.7, page 366)
    let mut clip = follow(FollowAction::Next, false);
    clip.follow.time_beats = 3.0;
    let mut launcher = Launcher::default();
    launcher.press(0, &clip, 127, 1.0, None);
    fire_all(&mut launcher, 1.0, clip);
    assert_eq!(launcher.next_event(1.0, true), Some(4.0));
}

#[test]
fn follow_actions_disabled_globally_do_not_fire() {
    // "By disabling the Enable Follow Actions Globally button, you can edit running clips without being
    // interrupted by playback jumping to other clips." (16.7, page 368)
    let clip = follow(FollowAction::Next, true);
    let mut launcher = Launcher::default();
    launcher.press(0, &clip, 127, 0.0, None);
    fire_all(&mut launcher, 0.0, clip);
    assert_eq!(launcher.next_event(0.0, false), None);
    assert_eq!(launcher.fire(4.0, EPS, false, |_| Some(clip)), None);
}

#[test]
fn chance_a_at_full_and_b_at_zero_always_picks_a_and_zero_never_happens() {
    // "If a clip or scene has Chance A set to 100% and Chance B set to 0%, Follow Action A will occur
    // every time the clip or scene is launched." "a Chance setting of 0% means that an action will
    // never happen." (16.7, page 366)
    let follow = FollowActions {
        action_a: FollowAction::Next,
        action_b: FollowAction::Stop,
        chance_a: 1.0,
        chance_b: 0.0,
        ..FollowActions::default()
    };
    let mut rng = Pcg32::new(7);
    for _ in 0..1000 {
        assert_eq!(pick_action(&follow, &mut rng), FollowAction::Next);
    }
}

#[test]
fn chance_b_at_ninety_makes_a_occur_about_once_in_ten() {
    // "Changing Chance B to 90% in this scenario makes Follow Action A occur much less often —
    // approximately once out of every ten clip or scene launches." (16.7, page 366)
    let follow = FollowActions {
        action_a: FollowAction::Next,
        action_b: FollowAction::Stop,
        chance_a: 1.0,
        chance_b: 0.9,
        ..FollowActions::default()
    };
    let mut rng = Pcg32::new(42);
    let a = (0..10_000)
        .filter(|_| pick_action(&follow, &mut rng) == FollowAction::Next)
        .count();
    // 1.0 / 1.9 of the launches pick A when both chances are relative weights.
    let expected = 10_000.0 / 1.9;
    assert!((a as f64 - expected).abs() < 300.0, "{a}");
}

#[test]
fn the_same_seed_gives_the_same_choices() {
    let mut first = Pcg32::new(99);
    let mut second = Pcg32::new(99);
    for _ in 0..100 {
        assert_eq!(first.next_u32(), second.next_u32());
    }
    assert_ne!(Pcg32::new(1).next_u32(), Pcg32::new(2).next_u32());
}

#[test]
fn groups_are_runs_of_filled_slots_separated_by_empty_ones() {
    // "A group is defined by clips arranged in successive slots of the same track. Tracks can have an
    // unlimited number of groups, separated by empty slots." (16.7, page 366)
    let filled = [true, true, false, true, true, true];
    assert_eq!(group_of(&filled, 1), 0..2);
    assert_eq!(group_of(&filled, 4), 3..6);
    assert_eq!(group_of(&filled, 2), 2..2);
}

#[test]
fn every_follow_action_goes_where_the_manual_says() {
    let filled = [true, true, true, false, true];
    let mut rng = Pcg32::new(3);
    let mut target = |action, slot| follow_target(action, slot, &filled, &mut rng);
    // "No Action means that no Follow Action will occur."
    assert_eq!(target(FollowAction::NoAction, 1), FollowTarget::Nothing);
    // "Stop simply stops the clip after it has played for the chosen Follow Action Time."
    assert_eq!(target(FollowAction::Stop, 1), FollowTarget::Stop);
    // "Play Again restarts the clip."
    assert_eq!(target(FollowAction::PlayAgain, 1), FollowTarget::Launch(1));
    // "Previous triggers the previous clip (the one above the current one)."
    assert_eq!(target(FollowAction::Previous, 1), FollowTarget::Launch(0));
    assert_eq!(target(FollowAction::Previous, 0), FollowTarget::Launch(2));
    // "Next triggers the next clip down in the group. If a clip with this setting is last in a group,
    // this Follow Action triggers the first clip."
    assert_eq!(target(FollowAction::Next, 1), FollowTarget::Launch(2));
    assert_eq!(target(FollowAction::Next, 2), FollowTarget::Launch(0));
    // "First launches the first (top) clip in a group." "Last launches the last (bottom) clip in a group."
    assert_eq!(target(FollowAction::First, 2), FollowTarget::Launch(0));
    assert_eq!(target(FollowAction::Last, 0), FollowTarget::Launch(2));
    // "Jump lets you select a target clip slot or scene for the Follow Action to jump to."
    assert_eq!(target(FollowAction::Jump(4), 0), FollowTarget::Launch(4));
    assert_eq!(target(FollowAction::Jump(3), 0), FollowTarget::Nothing);
    // A clip alone in its group is its own Next.
    assert_eq!(target(FollowAction::Next, 4), FollowTarget::Launch(4));
}

#[test]
fn any_plays_any_clip_in_the_group_and_other_never_plays_the_same_clip_twice() {
    // "Any plays any clip in the group." "Other is similar to "Any," but as long as the current clip is
    // not alone in the group, no clip will play consecutively." (16.7, page 367)
    let filled = [true, true, true, false, true];
    let mut rng = Pcg32::new(5);
    let mut seen_any = [false; 3];
    for _ in 0..200 {
        match follow_target(FollowAction::Any, 1, &filled, &mut rng) {
            FollowTarget::Launch(slot) => {
                assert!(slot < 3);
                seen_any[slot] = true;
            }
            other => panic!("Any gave {other:?}"),
        }
        match follow_target(FollowAction::Other, 1, &filled, &mut rng) {
            FollowTarget::Launch(slot) => assert!(slot < 3 && slot != 1),
            other => panic!("Other gave {other:?}"),
        }
    }
    assert_eq!(seen_any, [true; 3]);
    assert_eq!(
        follow_target(FollowAction::Other, 4, &filled, &mut rng),
        FollowTarget::Launch(4)
    );
}
