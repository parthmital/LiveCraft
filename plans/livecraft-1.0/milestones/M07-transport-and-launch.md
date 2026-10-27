# M07: Transport, tempo, metronome, launch scheduler

Purpose: scope, decisions, and acceptance criteria for timing and clip launching in the engine. Read when: expanding or reviewing M07.
Phase: 1. Depends on: M06. Manual: 3.1 The Control Bar (35), 6.3 Transport and Playback (168 to 169), 6.4 Launching the Arrangement with Locators (170 to 171), 6.6 The Arrangement Loop (174), 16.1 to 16.6 Launching Clips (360 to 365), 16.7 Follow Actions (366 to 371), 19.6 Recording with Count-in (420). Parity rows: 16.1, 16.4 (engine side).

## Goal

The engine implements Live's transport, tempo and time-signature changes, arrangement loop and punch, metronome with its settings, and the complete clip launch model (launch modes, quantization, legato, velocity, nudge, follow actions) sample-accurately; the UI parts come in M14 and M15.

## Decisions

- Transport state machine: Stopped, Playing, Recording armed, with "Continue Play" from stop position and Live's stop-twice-returns-to-start behaviour (confirm wording on page 168).
- Launch scheduler: per track, at most one playing clip and one queued clip (Session), with quantized launch times computed by `livecraft-time::quantize_launch`; scenes launch every clip in the row plus scene tempo and signature.
- Launch modes Trigger, Gate, Toggle, Repeat; Legato mode; launch velocity amount; clip nudge (manual 16.2 to 16.6). Each mode is a pure state machine with table-driven tests from manual sentences.
- Follow Actions (manual 16.7): actions, chance A and B, Follow Action time, linked and unlinked, global enable; random choices use a seeded PRNG (`rand_pcg` style algorithm implemented in-house, 64-bit PCG, so renders stay deterministic with a stored seed).
- Metronome: click sound synthesised in-house (no samples copied from Live); options from the manual: count-in, rhythm, sound, volume, enabled only while recording; exact option list quoted during expansion.
- Arrangement playback vs Session clips: a track plays its Session clip until "Back to Arrangement" (manual 6.x and 7.x); implement the override flag per track.

## Task Outline

1. Transport state machine and Control Bar commands; tests.
2. Tempo and time-signature changes from scenes and automation feeding `TempoMap`.
3. Arrangement loop, punch in and out, locators jumping with launch quantization.
4. Launch quantization scheduling with sample-accurate start inside a block.
5. Launch modes and legato (one task each pair, with manual-quote tests).
6. Launch velocity and clip nudge.
7. Follow Actions core (actions, chance, time) with seeded PRNG.
8. Follow Actions linked and unlinked, jump to clip, global toggle.
9. Back to Arrangement override per track.
10. Metronome and count-in.
11. Milestone verification and docs.

## Acceptance Criteria

- Every launch mode, quantization value, and follow action has a test quoting its manual sentence.
- A clip launched at quantization 1 Bar starts at the exact sample of the next bar line for 10 random tempos (sample-accurate).
- Follow action renders are deterministic for a fixed seed.
