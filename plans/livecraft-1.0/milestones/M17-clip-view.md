# M17: Clip View and audio clip editing

Purpose: scope, decisions, and acceptance criteria for Clip View parity. Read when: expanding or reviewing M17.
Phase: 4. Depends on: M16. Manual: 8 Clip View (200 to 233: 8.1 layout, 8.2 main clip properties with clip and loop region, clip time signature, clip groove, clip scale, 8.3 extended properties, 8.4 audio utilities with warp controls, reversing, destructive editing, fades, RAM mode, high quality interpolation, gain and pitch, 8.5 pitch and time utilities, 8.7 zoom and scroll, 8.8 playing and scrubbing, 8.9 looping, 8.10 sample details, 8.11 cropping, 8.13 multiple clips, 8.14 clip defaults). Parity rows: 8.1 to 8.14 except 8.6 (M19).

## Goal

Clip View's panels, editors, and properties for audio and MIDI clips look and behave like Live's, including multi-clip property editing; warping controls appear here but the warp engine is M23.

## Decisions

- Panels are separate widgets composed per clip type; property edits are commands with merge for drags.
- Sample Editor uses M11 waveform overviews; scrubbing uses the engine's preview path (no allocation on the audio thread).
- High Quality interpolation toggle selects the engine's sinc interpolator (`rubato` based) vs the default (Hermite) per manual 8.4.6.
- Destructive sample editing (8.4.3) writes new files through the M10 project rules, never overwriting the original without the manual's documented confirmation.
- Clip scale (8.2.4) links to M35 scale awareness; until M35 the property is stored and displayed.

## Task Outline

1. Clip View layout and panel switching for audio and MIDI clips.
2. Main clip properties (name, colour, signature, groove, scale fields, start, end, loop, position, length).
3. Loop region and markers editing in the editor ruler.
4. Audio utilities (gain, pitch, reverse, fades, RAM mode, HiQ).
5. Sample Editor: waveform display, zoom, scroll, scrub.
6. Pitch and time utilities panel (8.5).
7. Cropping and replace sample.
8. Multi-clip property editing (8.13) and clip defaults (8.14).
9. Destructive sample editing.
10. Milestone verification: Clip View states pass `ui_diff.py`.

## Acceptance Criteria

- Clip View states pass thresholds; manual-quote tests for each property.
- Parity rows 8.1 to 8.5 and 8.7 to 8.14 Done with evidence.
