# M36: Audio to MIDI, stem separation, bounce, freeze, consolidate

Purpose: scope, decisions, and acceptance criteria for audio conversion and rendering workflows inside the set. Read when: expanding or reviewing M36.
Phase: 6. Depends on: M35. Manual: 13 Converting Audio to MIDI (340 to 344), 20 Bounce to Audio (425 to 430), 22 Stem Separation (437 to 444), 38.1.4 Track Freeze (985 to 986). Parity rows: 13.1 to 13.5, 20.1 to 20.3, 22.1 to 22.2, 38.1 (freeze part). Beyond: B12.

## Goal

Slice to New MIDI Track, Convert Harmony, Melody, and Drums to MIDI, Bounce to Audio (tracks, groups, paste bounced audio), Stem Separation (including Live 12.4's time-selection and remove-stems options), and Freeze and Flatten work like Live.

## Decisions

- Bounce, freeze, and flatten use the M08 offline render path on a sub-graph; frozen tracks play back rendered files and keep devices editable after unfreeze, as the manual documents.
- Audio to MIDI: Drums via onset detection plus spectral band classification (kick, snare, hi-hat), Melody via YIN pitch tracking with note segmentation, Harmony via constant-Q chroma and multi-pitch estimation; each compared with Live's output on HUMAN oracle clips (note-level F-measure at least 0.8 against Live's result).
- Stem Separation: `stem-splitter-core` 1.2.0 behind feature `stems`, enabled in release builds (D-OWN-5, D31); model downloaded only after consent, SHA-256 verified (`sha2` 0.11.1), stored in the platform data directory; separation runs as a job with progress as Live 12.4 shows it for the whole operation.

## Task Outline

1. Bounce to Audio for tracks and groups, paste bounced audio (20.1 to 20.3).
2. Freeze and Flatten (38.1.4).
3. Slice to New MIDI Track (13.1) with slicing presets as documented.
4. Convert Drums (13.4); Convert Melody (13.3); Convert Harmony (13.2), one task each, plus HUMAN oracle clips.
5. Stem separation job, model download consent flow, and UI (22.1, 22.2), behind `stems`.
6. Time-selection separation and remove-stems options (12.4 additions).
7. Milestone verification.

## Acceptance Criteria

- Bounce oracle: bounced audio nulls against an M08 render of the same track (bit-exact).
- Audio-to-MIDI F-measure at least 0.8 against Live's conversions on the oracle clips.
- Parity rows listed above Done with evidence.
