# M34: Instruments: physical modelling and external

Purpose: scope, decisions, and acceptance criteria for these instruments. Read when: expanding or reviewing M34.
Phase: 5. Depends on: M33. Manual: 31.2 Collision (713 to 721), 31.5 Electric (736 to 740), 31.6 External Instrument (741 to 742), 31.12 Tension (802 to 813). Parity rows: 31.2, 31.5, 31.6, 31.12.

## Goal

Collision, Electric, Tension, and External Instrument match Live's parameters, panels, and sound.

## Decisions

- Collision: mallet and noise exciters into two resonators with the documented resonator types, modal synthesis with mode tables fitted to oracle renders (shared modal core with Corpus from M30).
- Electric: mallet, fork (tine and tone bar), damper, pickup models as documented; modal fork plus pickup nonlinearity fitted to oracle renders.
- Tension: string model with excitators (bow, hammer, plectrum), damper, termination, pickup, body as documented; digital waveguide implementation.
- External Instrument: MIDI out and audio return through M06 with latency compensation, as documented.

## Task Outline

1. Shared modal resonator core (used by Collision and later reused by Corpus if not already).
2. Collision (two tasks), Electric (two tasks), Tension (three tasks), External Instrument (one task).
3. Each device: panel, `.als` mapping, HUMAN oracle renders, oracle tests (pattern from M25).
4. Milestone verification.

## Acceptance Criteria

- Each instrument meets nonlinear oracle thresholds for its listed fixture patches.
- Parity rows listed above Done with evidence.
