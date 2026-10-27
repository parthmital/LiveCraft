# M32: Instruments: sample based

Purpose: scope, decisions, and acceptance criteria for sample-based instruments and Drum Rack pads. Read when: expanding or reviewing M32.
Phase: 5. Depends on: M31. Manual: 31.4 Drum Sampler (730 to 735), 31.7 Impulse (743 to 744), 31.10 Sampler (773 to 790), 31.11 Simpler (791 to 801), 25.6 Drum Racks (501 to 503). Parity rows: 31.4, 31.7, 31.10, 31.11, 25.6.

## Goal

Simpler, Sampler, Drum Sampler, Impulse, and Drum Rack pads match Live's parameters, panels, editors (zones, slicing, warping inside Simpler), and sound.

## Decisions

- Shared sample-playback core in `livecraft-devices/src/instruments/sample_core/`: voice allocation, interpolation (Hermite default, sinc for HiQ), loop modes with crossfade, envelopes, filters (shared with Auto Filter's ZDF filters), LFOs, and warped playback via `livecraft-warp`.
- Simpler replaces the prototype: Classic, One-Shot, Slice modes with every documented parameter, warp inside Simpler, slicing by transient, beat, region, manual.
- Sampler: multisample zones (key, velocity, sample select), modulation matrix, filter and global sections, MIDI and modulation tabs as documented; zone editor UI.
- Drum Sampler: playback effects list from page 730 onward, as documented.
- Drum Rack pads (25.6): pad grid, choke groups, sends and returns inside the rack, input and output notes, pad copy and swap, built on M25 racks; replaces the prototype `drumrack.rs`.
- Impulse: eight slots with stretch, saturation, filter, as documented.

## Task Outline

1. Sample core (voices, interpolation, loop crossfade) with tests.
2. Simpler Classic and One-Shot; Simpler Slice mode; Simpler warping (three tasks).
3. Drum Rack pads, choke, rack returns (two tasks).
4. Impulse.
5. Drum Sampler.
6. Sampler zones and zone editor; Sampler modulation and filter sections (three tasks).
7. Each device: panel, `.als` mapping, HUMAN oracle renders, oracle tests (pattern from M25).
8. Milestone verification; delete prototype files.

## Acceptance Criteria

- Each instrument meets nonlinear oracle thresholds on a fixture with its own CC0 sample set.
- Parity rows listed above Done with evidence.
