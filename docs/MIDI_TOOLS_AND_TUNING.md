# Ableton Live 12 MIDI Tools, Scale Systems, and Microtonal Tuning Specification

This document provides the exhaustive specification for Live 12's MIDI Transformations, Generative Tools, Scale Systems, and Microtonal Tuning Systems as defined in Chapters 11, 12, and 15 of the Ableton Live 12 Reference Manual.

## 1. MIDI Transformations (Chapter 11.2)

MIDI Transformations manipulate existing notes within a clip or time selection. LiveCraft implements these algorithms natively in `livecraft_core::MidiClipData`:

### 1.1 Quantize (p. 309)

- Snaps note start times and durations to musical grid intervals.
- Supported grid divisions: 1/4, 1/8, 1/16, 1/32, 1/64, and triplet divisions.
- Formula:
  $$\text{start}' = \text{round}\left(\frac{\text{start}}{\text{grid}}\right) \times \text{grid}$$
  $$\text{duration}' = \max\left(\text{grid}, \text{round}\left(\frac{\text{duration}}{\text{grid}}\right) \times \text{grid}\right)$$

### 1.2 Transpose (p. 256)

- Shifts note pitches by semitones or octaves with optional scale-snapping.
- When scale awareness is enabled, transposed notes snap to the closest in-scale pitch class.

### 1.3 Chop (p. 298)

- Slices selected notes into $N$ equal subdivisions (from 1 to 64 parts).
- Each sub-slice receives duration $\frac{\text{duration}}{N}$ and inherits the parent note's velocity.
- Optional timing spread and velocity ramps across slices.

### 1.4 Strum (p. 315)

- Staggers chord note start times to emulate guitar string picking.
- Directional control: Strum Up (lowest to highest pitch) or Strum Down (highest to lowest pitch).
- Offset calculation:
  $$\Delta t_i = i \times \frac{\text{spread}}{\text{chord\_size}}$$

### 1.5 Humanize (p. 310)

- Introduces micro-timing and velocity variations using deterministic pseudo-random distribution.
- Formula:
  $$\text{start}' = \max(0, \text{start} + \text{rand}_{[-1, 1]} \times \text{spread}_{\text{time}})$$
  $$\text{vel}' = \text{clamp}(1, 127, \text{vel} + \text{round}(\text{rand}_{[-1, 1]} \times \text{spread}_{\text{vel}}))$$

### 1.6 Ornament (p. 306)

- Adds grace notes or flams preceding note onsets.
- Prepends a brief note with duration of 1/64 or 1/32 beat and reduced velocity (e.g. 70% of main note velocity).

### 1.7 Span (p. 313)

- Extends the duration of each note until the onset of the following note on the timeline, eliminating silent gaps (legato articulation).
- If no subsequent note exists, notes extend to the end marker of the clip.

### 1.8 Invert & Reverse (p. 310)

- Invert: Reflects note pitches around the arithmetic average pitch of the selection.
  $$\text{pitch}' = \text{clamp}(0, 127, \bar{p} - (p - \bar{p}))$$
- Reverse: Horizontally mirrors note start times along the timeline selection.
  $$\text{start}' = t_{\text{max}} - (\text{start} + \text{duration})$$

### 1.9 Time Warp (p. 317)

- Applies non-linear acceleration or deceleration to note timing across the selection:
  $$\text{norm} = \frac{t - t_{\text{min}}}{t_{\text{max}} - t_{\text{min}}}$$
  $$\text{start}' = t_{\text{min}} + (t_{\text{max}} - t_{\text{min}}) \times \text{norm}^\gamma$$
  where $\gamma < 1$ accelerates timing (rushing) and $\gamma > 1$ decelerates timing (dragging).

### 1.10 Velocity Ramp (p. 319)

- Interpolates velocities linearly from a start velocity to an end velocity based on note start times.

## 2. Generative Tools (Chapter 11.3)

Generative Tools synthesize brand new musical content directly into empty or active clips:

### 2.1 Stacks (p. 326)

- Synthesizes diatonic chord progressions and triads on scale degrees.
- Built-in chord templates: Triads (1-3-5), 7ths (1-3-5-7), 9ths, Suspended, Inversions.
- Pitches automatically snap to the active scale defined in the clip or project.

### 2.2 Euclidean Rhythms (p. 327)

- Generates mathematically balanced rhythmic distributions using the Bjorklund algorithm.
- Distributes $K$ pulses across $N$ time steps as evenly as possible.
- Multi-voice capability: up to 4 voices with independent pulse counts, step counts, rotation offsets, and pitches.

### 2.3 Rhythm & Seed (pp. 321-323)

- Generates algorithmic rhythmic accents and melodic motifs constrained by musical density and key/scale rules.

## 3. Scale Systems & Fold to Scale (Chapter 3.13 & Chapter 10)

Ableton Live 12 features a unified scale engine shared across the Control Bar, Clip View, and devices:

### 3.1 Supported Scales and Modes

LiveCraft natively implements the complete diatonic and modern scale library:

1. **Major (Ionian):** [0, 2, 4, 5, 7, 9, 11]
2. **Minor (Aeolian):** [0, 2, 3, 5, 7, 8, 10]
3. **Harmonic Minor:** [0, 2, 3, 5, 7, 8, 11]
4. **Melodic Minor:** [0, 2, 3, 5, 7, 9, 11]
5. **Dorian:** [0, 2, 3, 5, 7, 9, 10]
6. **Phrygian:** [0, 1, 3, 5, 7, 8, 10]
7. **Lydian:** [0, 2, 4, 6, 7, 9, 11]
8. **Mixolydian:** [0, 2, 4, 5, 7, 9, 10]
9. **Locrian:** [0, 1, 3, 5, 6, 8, 10]
10. **Major Pentatonic:** [0, 2, 4, 7, 9]
11. **Minor Pentatonic:** [0, 3, 5, 7, 10]
12. **Blues:** [0, 3, 5, 6, 7, 10]

### 3.2 Fold to Scale

- When enabled in the Piano Roll, rows corresponding to out-of-scale pitches are hidden, collapsing the display into a diatonic keyboard.
- In-scale rows display amber root markers and muted lane backgrounds.

## 4. Microtonal Tuning Systems (Chapter 15)

Live 12 introduces direct support for Scala `.scl` and `.ascl` files:

### 4.1 Scala File Format (.scl)

- Text-based specification defining non-standard musical pitch systems:
  - Line 1: Optional description of the tuning.
  - Line 2: Number of notes / scale degrees (e.g. 19 for 19-EDO, 24 for quarter-tone, 5 for Slendro).
  - Subsequent lines: Pitch values in cents (e.g. `1200.0` or `100.0`) or frequency ratios (e.g. `3/2` for pure fifth, `5/4` for major third, `2/1` for octave).

### 4.2 LiveCraft Tuning Engine Architecture

- `TuningSystem` struct in `livecraft_core::model`:
  - Stores degree count, cent offsets, root reference frequency (default $440.0\text{ Hz}$ for A3 / MIDI 69), and reference pitch.
  - Converts MIDI note number to absolute Hz:
    $$f(n) = f_{\text{ref}} \times 2^{\frac{\text{cents}(n)}{1200}}$$
  - Supported across all native DSP instruments (Operator, Simpler, DrumRack).
