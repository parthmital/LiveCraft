# M19: MIDI Tools: transformations and generators

Purpose: scope, decisions, and acceptance criteria for Live 12's MIDI Tools. Read when: expanding or reviewing M19.
Phase: 4. Depends on: M18. Manual: 8.6 Transform and Generate Panels (225), 11 MIDI Tools (293 to 329): transformations 11.2.1 Arpeggiate, 11.2.2 Chop, 11.2.3 Connect, 11.2.4 Glissando, 11.2.5 LFO, 11.2.6 Ornament, 11.2.7 Quantize, 11.2.8 Recombine, 11.2.9 Span, 11.2.10 Strum, 11.2.11 Time Warp, 11.2.12 Velocity Shaper; generators 11.3.1 Rhythm, 11.3.2 Seed, 11.3.3 Shape, 11.3.4 Stacks, 11.3.5 Euclidean. Parity rows: 8.6, 11.1 to 11.3.

## Goal

All 12 transformations and 5 generators exist with Live's parameters, ranges, defaults, preview and apply behaviour, scale awareness, and seeded randomness; today's ad-hoc tools in `midi_tools_legacy.rs` are replaced.

## Decisions

- Crate location: `livecraft-model/src/midi_tools/` (pure functions on note lists, no UI or engine), one module per tool: `fn apply(notes: &[Note], selection: TimeRange, params: &XParams, ctx: &ToolContext) -> Vec<Note>`; `ToolContext` carries scale, grid, and a seeded PRNG.
- Parameters, ranges, and defaults are transcribed from each manual section into a `ParamSpec` table per tool, with the manual page in a comment.
- Algorithms: where the manual describes behaviour but not the algorithm (for example Seed, Shape, Recombine), implementation writes the algorithm from the manual's description and verifies it against HUMAN oracle examples: the owner applies the tool in Live to fixture clips with fixed settings and exports the result as `.alc` into `.agent-local/reference/midi-tools/`; tests compare structure (note count, pitch set, timing grid) where randomness is involved, and exact notes where it is not.
- Euclidean uses the Bjorklund algorithm with Live's rotation and voice semantics from 11.3.5.
- Panels UI (8.6) uses the M13 widget kit.

## Task Outline

1. `ToolContext`, PRNG, and the tool trait; panel framework (8.6, 11.1).
2. Quantize, Span, Strum, Time Warp (deterministic tools first, one or two per task).
3. Chop, Connect, Glissando, Ornament.
4. Arpeggiate (all styles from 11.2.1), LFO, Velocity Shaper, Recombine.
5. Generators: Rhythm, Euclidean, Stacks, Shape, Seed.
6. HUMAN: owner produces oracle `.alc` files for each tool with the settings listed in the task.
7. Oracle comparison tests per tool.
8. Delete `midi_tools_legacy.rs`; milestone verification.

## Acceptance Criteria

- Every tool's parameter table matches the manual (test asserts ranges and defaults).
- Deterministic tools match oracle results exactly; random tools match oracle structure and are reproducible by seed.
- Parity rows 8.6 and 11.x Done with evidence.
