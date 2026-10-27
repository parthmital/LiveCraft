# M38: Remote control, control surfaces, synchronisation

Purpose: scope, decisions, and acceptance criteria for external control and sync. Read when: expanding or reviewing M38.
Phase: 6. Depends on: M37. Manual: 3.20 MIDI and Key Remote (60), 34 MIDI and Key Remote Control (865 to 873), 35 Using Push 1 (874 to 913), 36 Using Push 2 (914 to 972), 37 Synchronizing with Link, Tempo Follower, and MIDI (973 to 981), 42.24 Key/MIDI Map Mode and the Computer MIDI Keyboard (1033). Parity rows: 3.20, 34.x, 35.x, 36.x, 37.x.

## Goal

MIDI Map and Key Map modes, the computer MIDI keyboard, control surface support with takeover modes, MIDI clock in and out, Tempo Follower, and (if the owner approves) Link and Link Audio behave like Live; Push 2 is supported as a control surface.

## Decisions

- New crate `livecraft-control` (depends on time, model; `LAYERS` updated).
- MIDI and Key mappings are model data saved in `.als` where Live stores them (structure from oracle sets).
- Control surface scripts are Rhai scripts (D24) with an API for parameter banks, transport, clip launch, and feedback; generic scripts for common controllers ship with LiveCraft; Live's Python remote scripts are not executed (proprietary and outside scope).
- Push 2: MIDI and display protocol as published by Ableton's open Push 2 interface documentation (the expansion confirms licence and availability; if not available under permissive terms, Push support is Out of Scope). Push 1 likewise.
- Sync: MIDI clock out and in with song position pointer (37.3); Tempo Follower from audio input (37.2) using M23 tempo detection; Link and Link Audio only with feature `link` (D14, D-OWN-1).

## Task Outline

1. Create `livecraft-control`; Key Map mode and computer MIDI keyboard (34, 42.24).
2. MIDI Map mode with takeover modes (34.1.3) and the Mapping Browser (34.2).
3. Control surface script runtime and two generic scripts; settings tab (34.1.1, 34.1.2).
4. MIDI clock out and in, sync settings (37.3).
5. Tempo Follower (37.2).
6. Link and Link Audio behind `link` (HUMAN decision gate: stop if D-OWN-1 is not answered).
7. Push 2 support (36) if licensing allows; else record Out of Scope.
8. Milestone verification.

## Acceptance Criteria

- MIDI clock out jitter under 1 ms measured with a loopback test; clock in follows tempo changes within one beat.
- Mappings round-trip through `.als`.
- Parity rows Done or Out of Scope with recorded reasons.
