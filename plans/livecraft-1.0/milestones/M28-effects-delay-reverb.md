# M28: Audio effects: delay, reverb, looping, spectral time

Purpose: scope, decisions, and acceptance criteria for these devices. Read when: expanding or reviewing M28.
Phase: 5. Depends on: M27. Manual: chapter 29 sections below. Parity rows: 29.5, 29.11, 29.14, 29.19, 29.22, 29.23, 29.25, 29.32, 29.37.

## Goal

Each device below matches Live's parameters, panel, and sound, following the per-device task pattern in `M25-device-framework-and-racks.md#decisions`.

## Devices And Approach

| Device        | Pages      | Approach                                                                                                                                                                                                                                                                                |
| ------------- | ---------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Delay         | 595 to 598 | Replaces the prototype. Stereo, ping-pong, sync and time modes, Repitch, Fade, Jump transitions, filter, LFO with Live 12.4's new LFO time modes and waveforms (12.4 release notes; confirm the exact options in the manual or a capture). Fractional delay with Hermite interpolation. |
| Echo          | 602 to 605 | Tape-style delay with modulation, reverb, ducking, gate, noise and wobble, character section as documented.                                                                                                                                                                             |
| Filter Delay  | 612        | Three delay lines with band-pass filters and routing as documented.                                                                                                                                                                                                                     |
| Grain Delay   | 618        | Granular delay with spray, frequency, pitch, random pitch, feedback, as documented.                                                                                                                                                                                                     |
| Beat Repeat   | 574 to 575 | Tempo-synced capture and repeat with interval, offset, chance, gate, grid, variation, pitch decay, filter, mix modes as documented; seeded randomness.                                                                                                                                  |
| Looper        | 628 to 631 | Record, overdub, play, stop state machine with tempo control, quantization, feedback, reverse, speed, as documented.                                                                                                                                                                    |
| Reverb        | 647 to 650 | Replaces the prototype (Freeverb). Input filter, early reflections with spin, diffusion network with chorus, shape, size, decay, freeze, flat, cut, density, scale, as documented; feedback delay network (8 lines, Hadamard matrix) tuned to oracle impulse responses.                 |
| Hybrid Reverb | 619 to 625 | Convolution engine (uniformly partitioned FFT convolution with `realfft`) plus algorithmic engine with the documented modes and routing; impulse responses: LiveCraft ships only self-made or CC0 IRs; users load their own.                                                            |
| Spectral Time | 673 to 676 | STFT freezer and spectral delay with the documented parameters.                                                                                                                                                                                                                         |

## Task Outline

Per device, the six-step pattern from M25. Order: Delay, Filter Delay, Echo, Grain Delay, Beat Repeat, Looper, Reverb, Hybrid Reverb, Spectral Time. HUMAN oracle renders include impulse responses for each reverb setting listed. Last task: milestone verification.

## Acceptance Criteria

- Delays at static settings meet the linear threshold; reverbs meet the nonlinear threshold and decay time (RT60) within 5 percent of the oracle.
- Parity rows listed above Done with evidence.
