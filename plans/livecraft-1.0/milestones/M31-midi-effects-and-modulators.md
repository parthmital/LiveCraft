# M31: MIDI effects and modulators

Purpose: scope, decisions, and acceptance criteria for MIDI effects and the chapter 33 device set rebuilt natively. Read when: expanding or reviewing M31.
Phase: 5. Depends on: M30. Manual: 30 Live MIDI Effect Reference (691 to 703), 33 Max for Live Devices (830 to 864). Parity rows: 30.1 to 30.8, 33.1.1 to 33.3.6.

## Goal

The eight MIDI effects and every chapter 33 device (DS drum synths, Align Delay, Envelope Follower, LFO, Shaper, Envelope MIDI, Expression Control, MIDI Monitor, MPE Control, Note Echo, Shaper MIDI) exist natively with Live's parameters, panels, and behaviour, without Max (decision D04).

## Decisions

- MIDI effects implement `DeviceProcessor` on event lists only (no audio), sample-accurate, seeded randomness for Random and Arpeggiator random styles.
- Modulators (LFO, Envelope Follower, Shaper, Expression Control, Envelope MIDI, Shaper MIDI) map to any parameter like Live's modulation devices: a mapping targets `(DeviceId, ParamId)` and modulates around the current value with depth; mappings save into `.als` exactly as Live stores Max for Live mappings only if the owner's oracle sets show the structure; otherwise LiveCraft stores them in its own namespaced element preserved by M09 (decision recorded in `reference/DECISIONS.md`).
- DS drum synths are small instruments built with the M33 oscillator and envelope primitives.

## Devices

| Group                          | Devices and pages                                                                                                                               |
| ------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| MIDI effects                   | Arpeggiator 691 to 693, CC Control 694, Chord 695 to 696, Note Length 697, Pitch 698, Random 699 to 700, Scale 701, Velocity 702 to 703         |
| DS instruments                 | DS Clang 831, DS Clap 832, DS Cymbal 833, DS FM 834, DS HH 835, DS Kick 836, DS Snare 837, DS Tom 838                                           |
| Audio modulators and utilities | Align Delay 839, Envelope Follower 840 to 842, LFO 843 to 845, Shaper 846 to 847                                                                |
| MIDI devices                   | Envelope MIDI 848 to 850, Expression Control 851 to 852, MIDI Monitor 853 to 854, MPE Control 855 to 860, Note Echo 861, Shaper MIDI 862 to 864 |

## Task Outline

Per device, the six-step pattern from M25; MIDI devices use event-list oracle comparisons (the owner records Live's MIDI output into a clip and saves it as `.alc`). First task: the modulation mapping system and its UI (map button, depth, multiple targets). Last task: milestone verification.

## Acceptance Criteria

- MIDI effects produce identical event lists to the oracle for deterministic settings; random settings match statistics and are reproducible by seed.
- DS instruments meet nonlinear oracle thresholds.
- Parity rows listed above Done with evidence.
