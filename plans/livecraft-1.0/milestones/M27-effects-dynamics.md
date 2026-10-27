# M27: Audio effects: dynamics

Purpose: scope, decisions, and acceptance criteria for these devices. Read when: expanding or reviewing M27.
Phase: 5. Depends on: M26. Manual: chapter 29 sections below. Parity rows: 29.9, 29.12, 29.20, 29.21, 29.24, 29.26.

## Goal

Each device below matches Live's parameters, panel, and sound, following the per-device task pattern in `M25-device-framework-and-racks.md#decisions`.

## Devices And Approach

| Device             | Pages      | Approach                                                                                                                                                                                                                                   |
| ------------------ | ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Compressor         | 584 to 588 | Replaces the prototype. Peak, RMS, Expand modes; feed-forward and feedback; knee; lookahead options; envelope modes; sidechain with EQ; display modes, as documented. Gain computer and ballistics fitted to oracle step-response renders. |
| Drum Buss          | 599 to 600 | Drive types, crunch, boom (resonant low filter with decay), transients, compressor, damping, as documented; curves fitted to oracle renders.                                                                                               |
| Gate               | 613 to 614 | Replaces the prototype. Threshold, return (hysteresis), attack, hold, release, floor, lookahead, flip, sidechain as documented.                                                                                                            |
| Glue Compressor    | 615 to 617 | Bus compressor with the documented discrete attack, release (including auto), ratios, range, soft clip, peak clip and oversampling; ballistics fitted to oracle renders.                                                                   |
| Limiter            | 626 to 627 | Replaces the prototype. Ceiling, gain, lookahead, release with auto, stereo and L/R and M/S routing modes, True Peak mode (4x oversampled peak detection) as documented.                                                                   |
| Multiband Dynamics | 632 to 636 | Three bands with Linkwitz-Riley crossovers, upward and downward compression and expansion above and below thresholds, soft knee, peak and RMS, time scaling, as documented.                                                                |

## Task Outline

Per device, the six-step pattern from M25. Order: Gate, Limiter, Compressor, Glue Compressor, Multiband Dynamics, Drum Buss. HUMAN oracle renders include step responses (tone bursts) at listed attack and release settings so ballistics can be fitted. Last task: milestone verification.

## Acceptance Criteria

- Each device: spec test, panel `ui_diff.py`, `.als` round trip, and oracle nonlinear thresholds; gain-reduction meters match oracle step responses within 1 dB.
- Parity rows listed above Done with evidence.
