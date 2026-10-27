# M26: Audio effects: EQ, filter, analysis, utility

Purpose: scope, decisions, and acceptance criteria for these devices. Read when: expanding or reviewing M26.
Phase: 5. Depends on: M25. Manual: chapter 29 sections below. Parity rows: 29.2, 29.7, 29.15, 29.16, 29.18, 29.38, 29.39, 29.40.

## Goal

Each device below matches Live's parameters, panel, and sound, following the per-device task pattern in `M25-device-framework-and-racks.md#decisions`.

## Devices And Approach

| Device                | Pages      | Approach (the manual section is the specification; these fix the technique)                                                                                                                                                              |
| --------------------- | ---------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Auto Filter           | 554 to 561 | Replaces the prototype. Filter types, slopes, and circuit models listed on these pages; zero-delay-feedback state-variable and ladder topologies (Zavalishin's TPT method) per circuit; LFO, envelope follower, sidechain as documented. |
| Channel EQ            | 578 to 579 | Fixed bands and adaptive Q as described; biquads from the RBJ cookbook with gain-dependent Q fitted to oracle renders.                                                                                                                   |
| EQ Eight              | 606 to 607 | Replaces the prototype. 8 bands, filter types and 12 or 48 dB slopes, Stereo, L/R, M/S modes, oversampling ("Hi Quality"), audition, and analyzer from the manual; RBJ biquads cascaded for 48 dB.                                       |
| EQ Three              | 608        | Replaces the prototype. 24 or 48 dB crossovers as documented (Linkwitz-Riley), kill switches, frequency controls.                                                                                                                        |
| External Audio Effect | 610 to 611 | Routes to hardware outputs and back through M06 I/O, with latency compensation and gain controls as documented.                                                                                                                          |
| Spectrum              | 677        | FFT analyzer (`realfft`), block size, channel, refresh, average, scale options as documented; display matches captures.                                                                                                                  |
| Tuner                 | 678 to 683 | Pitch detection with YIN, reference pitch, histogram and classic views as documented.                                                                                                                                                    |
| Utility               | 684 to 685 | Replaces the prototype. Gain range, width, mono, bass mono, balance, channel modes, phase invert, DC as documented; bit-exact at defaults.                                                                                               |

## Task Outline

Per device, the six-step pattern from M25 (spec, DSP, panel, `.als` mapping, HUMAN oracle renders, oracle comparison), smallest devices first: Utility, EQ Three, Channel EQ, EQ Eight, Auto Filter, Spectrum, Tuner, External Audio Effect. Delete each prototype file when its replacement passes. Last task: milestone verification.

## Acceptance Criteria

- Each device: spec test, panel `ui_diff.py` pass in both themes, `.als` round trip, oracle thresholds from `QUALITY-GATES.md#audio` (EQs and Utility at static settings use the linear threshold).
- Parity rows listed above Done with evidence.
