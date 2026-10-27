# M44: Mixing and mastering extras

Purpose: scope, decisions, and acceptance criteria for mixing and interchange improvements. Read when: expanding or reviewing M44.
Phase: 7. Depends on: M43. Manual: none (beyond Live). Beyond: B08, B09.

## Goal

Loudness and true-peak metering on any track and in renders, and DAWproject import and export.

## Decisions

- Loudness: ITU-R BS.1770-4 K-weighting and gating (momentary 400 ms, short-term 3 s, integrated), true peak with 4x oversampling; the `livecraft-dsp` metric functions from M08 become real-time meters (allocation-free) shown in a mixer section that is hidden by default (Live parity default).
- Renders can embed a loudness report in the M08 render report.
- DAWproject: the open format's published XML schema (research at expansion: confirm the current version and licence of the specification and reference files); import maps tracks, clips, notes, automation, and plug-in references to the model; export writes the same subset; unsupported elements reported.

## Task Outline

1. Loudness meter DSP validated against the EBU R128 test signals (generated in-house from the published specification).
2. Mixer loudness section UI and render report fields.
3. DAWproject research and decision rows.
4. DAWproject import.
5. DAWproject export with round-trip tests.
6. Milestone verification.

## Acceptance Criteria

- Loudness meters within 0.1 LU of the specification's expected values for each test signal.
- DAWproject round trip of a fixture keeps every mapped element.
- Beyond rows B08, B09 Done with evidence.
