# M21: Recording, capture, comping

Purpose: scope, decisions, and acceptance criteria for recording audio and MIDI and for take lanes. Read when: expanding or reviewing M21.
Phase: 4. Depends on: M20. Manual: 3.17 Recording New Clips (57 to 58), 19 Recording New Clips (412 to 424: inputs, arming, recording, recording in sync, record quantization, count-in, file types, where samples go, remote control, Capture MIDI), 21 Comping (431 to 436: take lanes, inserting and managing, recording takes, inserting samples, auditioning, creating a comp, source highlights). Parity rows: 3.17, 19.1 to 19.10, 21.1 to 21.7.

## Goal

Audio and MIDI recording into Session slots and the Arrangement, overdub, punch, count-in, record quantization, Capture MIDI, and comping with take lanes behave like Live, with latency-compensated, glitch-free capture.

## Decisions

- Audio capture: the audio thread writes input into preallocated `rtrb` rings; a writer thread streams to disk as WAV (bit depth and file type from settings, manual 19.7) into the project's `Samples/Recorded` folder (manual 19.8), so recording never allocates or does I/O on the audio thread.
- Input latency compensation uses driver-reported latency plus the user's driver error compensation setting (manual 2.3.3).
- Capture MIDI (19.10): a rolling MIDI buffer of the length the manual describes, kept per armed or monitored track, always on.
- Take lanes (21): lanes are arrangement data per track; comp selection is a set of time ranges per lane; the comp lane renders from sources without copying audio.

## Task Outline

1. Arming and input selection behaviour (19.1, 19.2).
2. Audio recording pipeline (ring, writer thread, file naming) with a synthetic input test.
3. MIDI recording into Session and Arrangement, overdub, and record quantization (19.3, 19.5).
4. Recording in sync and launch quantization interactions (19.4); count-in (19.6).
5. Punch in and out and loop recording.
6. Capture MIDI (19.10).
7. Remote control for recording (19.9) via commands.
8. Take lanes model and UI (21.1, 21.2); recording takes (21.3); inserting samples (21.4).
9. Auditioning and comping (21.5 to 21.7).
10. Milestone verification (HUMAN: owner records 10 minutes of audio and MIDI on the reference machine; zero dropouts; file sample-aligned with the input loopback test).

## Acceptance Criteria

- Loopback test: recorded audio aligns with the played reference within 1 sample after compensation.
- `assert_no_alloc` passes while recording.
- Parity rows 3.17, 19.x, 21.x Done with evidence.
