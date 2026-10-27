# M33: Instruments: synthesizers

Purpose: scope, decisions, and acceptance criteria for synthesizers. Read when: expanding or reviewing M33.
Phase: 5. Depends on: M32. Manual: 31.1 Analog (704 to 712), 31.3 Drift (722 to 729), 31.8 Meld (745 to 754), 31.9 Operator (755 to 772), 31.13 Wavetable (814 to 823). Parity rows: 31.1, 31.3, 31.8, 31.9, 31.13.

## Goal

Analog, Drift, Meld, Operator, and Wavetable match Live's parameters, panels, modulation, MPE response, and sound.

## Decisions

- Shared synth primitives in `livecraft-dsp`: band-limited oscillators (PolyBLEP for analog shapes, minBLEP where aliasing tests require), wavetable oscillator with mip-mapped tables, ZDF filters (shared with M26), ADSR and multi-stage envelopes with Live's curve shapes measured from oracle renders, LFOs, unison and voice stealing, MPE routing.
- Operator replaces the prototype: four operators, the eleven algorithms, waveforms including user-drawn partials, fixed frequency mode, envelopes with loop modes, LFO, filter with documented types, pitch envelope, global section, as documented on pages 755 to 772.
- Wavetable: LiveCraft ships only its own wavetables (generated or CC0); users load their own; oscillator effects, sub, filters, modulation matrix as documented.
- Meld: two macro oscillators with the documented engine list; each engine implemented from the manual's description and fitted to oracle renders.
- Analog and Drift: virtual analog per the manual's signal flow diagrams.

## Task Outline

1. Synth primitives with aliasing and accuracy tests.
2. Operator (oscillators and algorithms; envelopes and LFO; filter and global; panel) as four tasks.
3. Drift (two tasks), Analog (three tasks), Wavetable (three tasks), Meld (one task per engine group, about four tasks).
4. Each device: panel, `.als` mapping, HUMAN oracle renders, oracle tests (pattern from M25).
5. Milestone verification; delete the prototype Operator.

## Acceptance Criteria

- Each synthesizer meets nonlinear oracle thresholds for its listed fixture patches; aliasing below -90 dB for a 10 kHz saw at 48 kHz.
- Parity rows listed above Done with evidence.
