# M07: Transport, tempo, metronome, launch scheduler

Purpose: scope, decisions, and acceptance criteria for timing and clip launching in the engine. Read when: expanding or reviewing M07.
Phase: 1. Depends on: M06. Manual: 3.1 The Control Bar (35), 6.3 Transport and Playback (168 to 169), 6.4 Launching the Arrangement with Locators (170 to 171), 6.6 The Arrangement Loop (174), 16.1 to 16.6 Launching Clips (360 to 365), 16.7 Follow Actions (366 to 371), 19.6 Recording with Count-in (420). Parity rows: 16.1, 16.4 (engine side).

## Goal

The engine implements Live's transport, tempo and time-signature changes, arrangement loop and punch, metronome with its settings, and the complete clip launch model (launch modes, quantization, legato, velocity, nudge, follow actions) sample-accurately; the UI parts come in M14 and M15.

## Decisions

- Transport state machine: Stopped, Playing, Recording armed, with "Continue Play" from stop position and Live's stop-twice-returns-to-start behaviour (confirm wording on page 168).
- Launch scheduler: per track, at most one playing clip and one queued clip (Session), with quantized launch times computed by `livecraft-time::quantize_launch`; scenes launch every clip in the row plus scene tempo and signature.
- Launch modes Trigger, Gate, Toggle, Repeat; Legato mode; launch velocity amount; clip nudge (manual 16.2 to 16.6). Each mode is a pure state machine with table-driven tests from manual sentences.
- Follow Actions (manual 16.7): actions, chance A and B, Follow Action time, linked and unlinked, global enable; random choices use a seeded PRNG (`rand_pcg` style algorithm implemented in-house, 64-bit PCG, so renders stay deterministic with a stored seed).
- Metronome: click sound synthesised in-house (no samples copied from Live); options from the manual: count-in, rhythm, sound, volume, enabled only while recording; exact option list quoted when the milestone starts.
- Arrangement playback vs Session clips: a track plays its Session clip until "Back to Arrangement" (manual 6.x and 7.x); implement the override flag per track.

## Task Outline

1. Transport state machine and Control Bar commands; tests.
2. Tempo and time-signature changes from scenes and automation feeding `TempoMap`.
3. Arrangement loop, punch in and out, locators jumping with launch quantization.
4. Launch quantization scheduling with sample-accurate start inside a block.
5. Launch modes and legato (one task each pair, with manual-quote tests).
6. Launch velocity and clip nudge.
7. Follow Actions core (actions, chance, time) with seeded PRNG.
8. Follow Actions linked and unlinked, jump to clip, global toggle.
9. Back to Arrangement override per track.
10. Metronome and count-in.
11. Milestone verification and docs.

## Acceptance Criteria

- Every launch mode, quantization value, and follow action has a test quoting its manual sentence.
- A clip launched at quantization 1 Bar starts at the exact sample of the next bar line for 10 random tempos (sample-accurate).
- Follow action renders are deterministic for a fixed seed.

## Review (2026-11-01)

Items 1 to 10 share one commit, `e795b7e`. The launch clock, the per-track launcher, and the executor's segment splitting serve every item at once, so the items could not be split without commits that leave the engine half-converted. Item 11 is this commit.

| Items  | Commit      | Summary                                                                                                                                         |
| ------ | ----------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| 1      | `e795b7e`   | `TransportControl` (insert marker, Continue, Stop twice), engine Play and Stop, Session API and the Stop button and Shift + Space in the UI     |
| 2      | `e795b7e`   | The graph carries the tempo and signature maps; tempo changes start on their sample; scene tempo and signature apply at the scene's launch time |
| 3      | `e795b7e`   | The Arrangement Loop wraps on its end sample; punch points from the loop; quantized jumps for locators                                          |
| 4 to 6 | `e795b7e`   | Quantized launches on the launch clock, Trigger, Gate, Toggle, Repeat, Legato, velocity, nudge                                                  |
| 7, 8   | `e795b7e`   | Follow Actions with chances, linked and unlinked, the ten actions in the clip group, Jump, the global switch, the seeded PCG32                  |
| 9      | `e795b7e`   | Back to Arrangement per track and its snapshot flag                                                                                             |
| 10     | `e795b7e`   | The synthesised metronome, its settings, and the count-in                                                                                       |
| 11     | this commit | Review, owner row Q11, checklist `[~]`                                                                                                          |

### Manual quotes

From `docs/ableton-live-12-reference-manual.pdf`, read with `plans/livecraft-1.0/tools/manual.py`. Each quote is also in the test that checks it.

- 16.2, page 361: "Trigger: down starts the clip; up is ignored." "Gate: down starts the clip; up stops the clip." "Toggle: down starts the clip; up is ignored. The clip will stop on the next down." "Repeat: As long as the mouse switch/key is held, the clip is triggered repeatedly at the clip quantization rate."
- 16.3, page 362: "When a clip in Legato Mode is launched, it takes over the play position from whatever clip was played in that track before."
- 16.4, page 363: "To disable clip quantization, choose "None."" "To use the Control Bar's Global Quantization setting, choose "Global."" "Note that any setting other than "None" will quantize the clip's launch when it is triggered by Follow Actions."
- 16.5, page 364: "If set to zero, there is no influence; at 100 percent, the softest notes play the clip silently."
- 16.6, page 364: "To jump within a playing clip in increments the size of the global quantization period, you can use the Nudge Backward/Forward buttons."
- 16.7, pages 366 to 368: the group definition, the Chance A and B examples, Linked and Unlinked, the default of one bar, the ten Follow Actions, "Follow Actions circumvent global quantization but not clip quantization.", and the Enable Follow Actions Globally button.
- 16.7.4, page 370: "Legato Mode will sync the new play position with the old one in beat-time", so Follow Action launches keep Legato.
- 3.7, page 42: Session clips take precedence, and "The Arrangement clips in the track where the Session clip was launched will not resume playback until you manually restart it using the Back to Arrangement button."
- 6.3, pages 168 and 169: Play, Stop, the space bar, the insert marker, double-clicking Stop, and Shift with the space bar.
- 6.4, page 171: "Locators are quantized according to the global launch quantization value set in the Control Bar."
- 6.6, page 174: the Arrangement Loop toggle and its Loop Start and Loop Length.
- 19.3.1, page 414: "The punch-in point is identical to the Arrangement Loop's start position." and the punch-out equivalent.
- 19.4.1, page 419: the count-in, the sound, the Rhythm with "Auto" following the denominator and the fallback to Auto, and Enable Only While Recording with the punch-in rule.
- 19.6, page 420: "When set to any value other than "None," Live will not begin recording until the count-in is complete." and the count-in from negative bars to 1.1.1.

### Decisions and deviations

1. Launched clips run on a launch clock that advances with the transport and does not jump when the song position loops or jumps, so a Session clip keeps its sync through an Arrangement Loop or a locator jump. Quantization uses the song's bar lines.
2. Previous on the first clip of a group wraps to the last clip. The manual says only what Next does on the last clip; Q11 asks for Live's behaviour.
3. Repeat: releasing the button ends the repeats and the clip plays on. The manual does not say what release does; Q11 asks.
4. A launch while the transport is stopped starts the transport at the current song position and starts the clip at once.
5. A scene launch uses the Global Quantization, launches each filled slot in its row (a Toggle clip is relaunched, not stopped), and presses the Clip Stop button of empty slots that have one. Scene Follow Actions are not implemented; their UI is M15.
6. Clip Stop buttons and Stop All Clips are quantized by the Global Quantization. Stopping the transport stops every Session clip at once.
7. The metronome sounds are synthesised tones named Classic, Click, and Wood. The names and the list are provisional until Q11. The level is a Set setting, `Metronome::volume_db`, because the Preview Volume knob (page 418) is not implemented yet (M20).
8. Tempo: a tempo change by hand or by a scene overrides tempo changes in the Set until Back to Arrangement, as a manual tempo change overrides tempo automation in Live. One tempo for the whole Set changes in place without an override. Tempo ramps are followed in 32-frame steps.
9. A non-looping launched clip stops at its end. Before, every launched clip looped.
10. Locators are not stored in the Set yet; the engine's quantized `JumpTo` is what a locator press uses. The locator model and UI are M16.
11. Recording itself is M21: the count-in, the Record button state, and the punch rule for the metronome exist, but nothing is recorded.
12. The Session View still sends a single press for a click, so Gate and Repeat clips need the mouse up that M15 sends with `Session::release_clip`.
13. Fixed while doing this milestone: a graph swap dropped every launched clip and left the instruments' voices without note-offs. `ClipLane::adopt` now carries the launch state and the sounding notes over.

### Evidence

- `cargo xtask ci` passed, log `.agent-local/logs/2026-11-01_m07-ci.log`, with 448 tests.
- The 10-minute playback test passed again under the allocation check in a release build, log `.agent-local/logs/2026-11-01_m07-long.log`.
- `livecraft command list` prints 50 names.

### Criteria

| Criterion                                                                                       | Met | Evidence                                                                                                                                                                                                                                                       |
| ----------------------------------------------------------------------------------------------- | --- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Every launch mode, quantization value, and follow action has a test quoting its manual sentence | Yes | `crates/livecraft-engine/src/launch/tests.rs` (four modes, Legato, velocity, nudge, linked and unlinked, chances, groups, all ten actions); quantization: `tests/launch.rs` 1 Bar and None, and `livecraft-time` `grid.rs` tests for every menu entry from M02 |
| A clip launched at 1 Bar starts at the exact sample of the next bar line for 10 random tempos   | Yes | `tests/launch.rs` `a_clip_launched_at_one_bar_starts_on_the_sample_of_the_next_bar_line_at_ten_tempos` compares the note-on's engine frame with the first frame at or after the bar line, exactly                                                              |
| Follow action renders are deterministic for a fixed seed                                        | Yes | `tests/launch.rs` `follow_actions_play_the_same_way_for_the_same_seed` (and differ for another seed), `a_follow_action_render_is_byte_identical_for_a_stored_seed`                                                                                             |

### Owner rows

- Q11: Live's behaviour for Previous on the first clip of a group, Repeat on release, and the Metronome Settings menu entries. The line is `[~]` until Q11 arrives.

### Not verified

- No sound was heard. The metronome and launches were checked through samples, frames, and the MIDI output queue only.
- The UI does not yet show queued, Back to Arrangement, or count-in states; M14 and M15 draw them from the snapshot fields added here.
