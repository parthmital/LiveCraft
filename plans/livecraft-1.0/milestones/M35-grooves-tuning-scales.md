# M35: Grooves, tuning systems, scale awareness

Purpose: scope, decisions, and acceptance criteria for grooves, tunings, and scales. Read when: expanding or reviewing M35.
Phase: 6. Depends on: M34. Manual: 3.13 Scale Awareness (51 to 52), 8.2.4 Clip Scale (211), 10.6 Folding and Scales (284 to 286), 14 Using Grooves (345 to 350), 15 Using Tuning Systems (351 to 359). Parity rows: 3.13, 14.1 to 14.3, 15.1 to 15.4.

## Goal

The Groove Pool and groove application, Scala and `.ascl` tuning systems with the Tuning section and MIDI track options, and scale awareness across clips, devices, and MIDI Tools match Live.

## Decisions

- Grooves: groove files are clips whose timing and velocity define the groove (manual 14); application is non-destructive at playback with Base, Quantize, Timing, Random, Velocity, and Global Amount as documented; Commit writes notes or warp markers.
- Tuning: the existing Scala parser moves into `livecraft-formats/src/scala.rs` and gains `.ascl` (Ableton Scala extensions documented in chapter 15); the engine applies tuning per note for native instruments and via MPE pitch bend or MIDI 2.0-ready per-note pitch for plug-ins as the manual's MIDI track options describe (15.3).
- Scale awareness: the set and each clip hold a scale (root and mode list from Live's scale menu captured in M12); devices and MIDI Tools that follow the scale read it through `ToolContext` or `ProcessContext`.

## Task Outline

1. Groove model, Groove Pool UI, groove extraction from clips (14.1, 14.2).
2. Groove application in the engine and commit; tips behaviours (14.3).
3. Scala and `.ascl` parsing with tests from the manual's examples.
4. Tuning section UI, reference pitch, and MIDI track options (15.2, 15.3).
5. Scale model and Live's full scale list; clip scale and set scale linking (3.13, 8.2.4).
6. Scale-aware devices and tools (Scale MIDI effect from M31, MIDI Tools from M19, Fold to Scale from M18) audited and connected.
7. Milestone verification with oracle renders for a grooved clip and a tuned clip.

## Acceptance Criteria

- Grooved MIDI oracle: event timings within 1 sample of Live's for deterministic groove settings.
- Tuned render: pitch of each note within 0.5 cents of the oracle.
- Parity rows listed above Done with evidence.
