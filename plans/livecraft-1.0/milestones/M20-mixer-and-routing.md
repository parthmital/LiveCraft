# M20: Mixer, groups, returns, routing, crossfader, cue

Purpose: scope, decisions, and acceptance criteria for mixing and signal routing. Read when: expanding or reviewing M20.
Phase: 4. Depends on: M19. Manual: 3.14 The Mixer (53 to 55), 3.16 Routing (56), 17 Routing and I/O (372 to 393: 17.1 monitoring, 17.4 resampling, 17.5 internal routings and routing points), 18 Mixing (394 to 411: 18.1 the Live mixer and additional features, 18.2 audio and MIDI tracks, 18.3 group tracks, 18.4 return tracks and the Main track, 18.5 crossfader, 18.6 soloing and cueing, 18.7 track delays, 18.8 keep monitoring latency, 18.9 performance impact indicators). Parity rows: 3.14, 3.16, 17.1, 17.4, 17.5, 18.1 to 18.9.

## Goal

Live's mixer sections, routing choosers and every internal routing point, group and return behaviour, crossfader with curves, solo modes and cue output, track delays, and monitoring latency options work and look identical.

## Decisions

- Routing is model data (M03 `routing.rs`); the graph compiler (M05) builds edges from it, including pre-FX, post-FX, post-mixer tap points (17.5.1), sidechain taps, and resampling (17.4). Cycles are rejected at the command level with Live's behaviour (the chooser does not offer cyclic options).
- Pan law and fader taper: measured with oracle renders (HUMAN) of a sine through the fader at listed positions and pan values; implemented as lookup tables with interpolation where the measured curve is not a closed form.
- Crossfader curves (18.5) measured the same way.
- Solo modes: exclusive and non-exclusive, solo-in-place and cue (18.6), with Cue output routing.
- Track delay (18.7) in ms or samples, implemented as engine delay lines integrated with PDC.
- Mixer UI sections show and hide per the View menu (Track options, In/Out, Sends, Returns, Mixer, Track Delay, Crossfader, Performance Impact).

## Task Outline

1. Routing choosers (audio and MIDI from, to, monitor) with all options from 17.x.
2. Internal routing points and sidechain taps in the compiler.
3. Group tracks (summing, folding, nesting) and return tracks with pre and post sends.
4. HUMAN: oracle renders for fader, pan, crossfader curves; implement tables; oracle tests.
5. Solo, cue, and Cue output; preview volume.
6. Crossfader assignment and curves.
7. Track delays with PDC.
8. Resampling input.
9. Keep monitoring latency (18.8) and performance impact indicators (18.9; values from M41 when available, stub meter until then).
10. Mixer UI sections and meters (peak and RMS per Live) against captures.
11. Milestone verification.

## Acceptance Criteria

- Oracle: fader and pan renders null below -90 dBFS against Live at the measured positions.
- Mixer states pass `ui_diff.py`; manual-quote tests for routing rules.
- Parity rows listed above Done with evidence.
