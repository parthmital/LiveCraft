# M29: Audio effects: distortion and character

Purpose: scope, decisions, and acceptance criteria for these devices. Read when: expanding or reviewing M29.
Phase: 5. Depends on: M28. Manual: chapter 29 sections below. Parity rows: 29.1, 29.6, 29.13, 29.17, 29.27, 29.28, 29.30, 29.33, 29.34, 29.41.

## Goal

Each device below matches Live's parameters, panel, and sound, following the per-device task pattern in `M25-device-framework-and-racks.md#decisions`.

## Devices And Approach

| Device           | Pages      | Approach                                                                                                                                                                                                                                     |
| ---------------- | ---------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Saturator        | 660 to 662 | Replaces the prototype. Every documented curve type, waveshaper parameters, colour EQ, soft clip, oversampling ("Hi Quality"); static transfer curves measured from oracle sine sweeps at several drives.                                    |
| Redux            | 644 to 645 | Replaces the prototype. Downsampling with jitter and filter, bit reduction with shape and DC shift, as documented.                                                                                                                           |
| Overdrive        | 637        | Pre-filter, drive, tone, dynamics as documented; waveshaper fitted to oracle.                                                                                                                                                                |
| Pedal            | 638 to 640 | Overdrive, Distortion, Fuzz types, EQ, sub, as documented.                                                                                                                                                                                   |
| Dynamic Tube     | 601        | Tube types A, B, C, bias with envelope, tone, as documented.                                                                                                                                                                                 |
| Amp              | 552 to 553 | Amp models listed on page 552 with gain, EQ, presence, volume, dual mono; modelled as waveshaper plus tone stack per model, fitted to oracle sweeps.                                                                                         |
| Cabinet          | 576 to 577 | Speaker and microphone models as documented; LiveCraft measures responses from oracle renders and ships its own fitted filters (no IR files copied).                                                                                         |
| Erosion          | 609        | Noise and sine modulation modes, with Live 12.4's continuous sine-to-noise and mono-to-stereo blends and spectrum display (confirm in the manual or a capture).                                                                              |
| Vinyl Distortion | 686        | Tracing model, pinch, crackle as documented.                                                                                                                                                                                                 |
| Roar             | 651 to 659 | Multi-stage saturator: stage routings, shaper types, filters, feedback, modulation matrix with LFOs, envelope, noise, as documented; the largest device in this milestone, split into routing, shapers, filters, modulation, feedback tasks. |

## Task Outline

Per device, the six-step pattern from M25. Order: Saturator, Redux, Overdrive, Pedal, Dynamic Tube, Vinyl Distortion, Erosion, Amp, Cabinet, Roar. All nonlinear devices run at the documented oversampling factor with half-band FIR filters from `livecraft-dsp`. Last task: milestone verification.

## Acceptance Criteria

- Each device meets the nonlinear oracle thresholds; static transfer curves within 0.5 dB at measured points.
- Parity rows listed above Done with evidence.
