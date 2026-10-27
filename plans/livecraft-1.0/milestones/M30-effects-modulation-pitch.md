# M30: Audio effects: modulation, pitch, resonance, vocoder

Purpose: scope, decisions, and acceptance criteria for these devices. Read when: expanding or reviewing M30.
Phase: 5. Depends on: M29. Manual: chapter 29 sections below. Parity rows: 29.3, 29.4, 29.8, 29.10, 29.29, 29.31, 29.35, 29.36, 29.42.

## Goal

Each device below matches Live's parameters, panel, and sound, following the per-device task pattern in `M25-device-framework-and-racks.md#decisions`.

## Devices And Approach

| Device             | Pages      | Approach                                                                                                                                                           |
| ------------------ | ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Auto Pan-Tremolo   | 562 to 565 | Replaces the prototype. LFO shapes, phase, offset, spin, sync, panning and tremolo behaviour exactly as documented.                                                |
| Chorus-Ensemble    | 580 to 583 | Replaces the prototype. Classic, Ensemble, Vibrato modes with Live 12.4's extended delay time and structure control (confirm in the manual or a capture).          |
| Phaser-Flanger     | 641 to 643 | Phaser, Flanger, Doubler modes, notches, LFO and envelope, safe bass, as documented.                                                                               |
| Shifter            | 663 to 667 | Pitch, frequency, and ring modes, delay, LFO, envelope, as documented; pitch mode via `signalsmith-stretch`.                                                       |
| Auto Shift         | 566 to 573 | Real-time pitch detection and correction to scale, formant, vibrato, MIDI sidechain, as documented; detection with YIN from M26 Tuner.                             |
| Resonators         | 646        | Five resonators with pitch, decay, colour, as documented.                                                                                                          |
| Corpus             | 589 to 594 | Physical resonator models (beam, marimba, string, membrane, plate, pipe, tube) as documented, modal synthesis with mode tables fitted to oracle impulse responses. |
| Spectral Resonator | 668 to 672 | STFT-domain resonator with modes, MIDI sidechain, unison, as documented.                                                                                           |
| Vocoder            | 687 to 690 | Band-pass filter bank vocoder with carrier modes, bands, range, formant, as documented.                                                                            |

## Task Outline

Per device, the six-step pattern from M25. Order: Auto Pan-Tremolo, Chorus-Ensemble, Phaser-Flanger, Resonators, Vocoder, Shifter, Auto Shift, Corpus, Spectral Resonator. Modulated devices are compared with LFO phase aligned by resetting on transport start in both apps. Last task: milestone verification.

## Acceptance Criteria

- Each device meets the nonlinear oracle thresholds; pitch devices meet the time-stretch thresholds.
- Parity rows listed above Done with evidence.
