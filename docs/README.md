# LiveCraft Documentation Suite and Ableton Live 12 Reference

LiveCraft is an open-source, native Rust implementation of Ableton Live 12. This directory contains the exhaustive reference documentation and single source of truth for the entire project.

## Authoritative Reference Suite

1. [Ableton Live 12 Complete Specification](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/docs/ABLETON_12_SPECIFICATION.md)
   - Chapter-by-chapter mapping of all 43 chapters of the manual to LiveCraft architecture.
   - Core concepts: Session View, Arrangement View, Browser taxonomy, Audio Warping, Routing, Mixing, and MPE.

2. [Live 12 Complete Devices Reference](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/docs/DEVICES_REFERENCE.md)
   - Comprehensive catalog of all 42 Audio Effects (Amp, Auto Filter, Auto Pan-Tremolo, Roar, Saturator, and more).
   - All 8 MIDI Effects (Arpeggiator, Chord, Scale, Velocity, and more).
   - All 13 Instruments (Operator, Simpler, Drum Rack, Meld, Wavetable, Drift, and more).
   - Parameter lists, DSP topologies, and implementation status across the 16 studio-grade devices.

3. [MIDI Tools, Scale Systems and Microtonal Tuning](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/docs/MIDI_TOOLS_AND_TUNING.md)
   - Live 12 MIDI Transformations (Quantize, Transpose, Chop, Strum, Humanize, Ornament, Span, Invert, Reverse, Time Warp).
   - Generative Tools (Stacks, Euclidean rhythm generator, Rhythm, Seed).
   - Scale engine (12 modes, pentatonics, blues, Fold to Scale).
   - Microtonal Scala (`.scl`) tuning engine integrated with synthesizers and samplers.

4. [Complete Keyboard Shortcuts Matrix](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/docs/KEYBOARD_SHORTCUTS.md)
   - Comprehensive shortcut reference from Chapter 42 across Windows and macOS.
   - View navigation, editing, transport, session matrix, track operations, and computer MIDI keyboard.

5. [LiveCraft Architecture and Engineering Manual](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/docs/ARCHITECTURE.md)
   - Crate boundaries: `livecraft-core`, `livecraft-dsp`, `livecraft-gui`, `livecraft-cli`.
   - Lock-free audio engine, DAG topological graph, dynamic processor chain, voice scheduling, and offline bouncer.
   - Conformance to Chapters 38, 39, and 40 (lock-free safety, neutral operations, bit-transparency).

6. [LiveCraft UI/UX Design System Specification](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/DESIGN.md)
   - Ableton Live 12 color tokens, layout proportions, typography, and widget specifications.
   - Piecewise tapered volume fader and stereo peak meter mathematical alignment.
   - Dual theme support (Dark Neutral and Light) with runtime F8 toggle.

## Ableton Live 12 Reference Manual PDF

The primary source of truth for all LiveCraft specifications, formulas, and UI behaviors is:

- File: `docs/ableton-live-12-reference-manual.pdf` (108 MB, 1039 pages, 1069 sections)
- Version: Ableton Live 12 (Windows and macOS)

### Querying the Manual via CLI

Use the repository query script in `.agent-local/scripts/query_manual.py` to search or read the manual directly:

```bash
# Filter Table of Contents
python .agent-local/scripts/query_manual.py --filter-toc "Auto Pan"

# Read specific page or range
python .agent-local/scripts/query_manual.py --page 562
python .agent-local/scripts/query_manual.py --range 562-566

# Search keyword across all 1039 pages
python .agent-local/scripts/query_manual.py --search "Euclidean"

# Extract full section by title
python .agent-local/scripts/query_manual.py --section "Auto Pan-Tremolo"
```
