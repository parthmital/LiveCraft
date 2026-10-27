# M22: Automation and clip envelopes

Purpose: scope, decisions, and acceptance criteria for automation and modulation. Read when: expanding or reviewing M22.
Phase: 4. Depends on: M21. Manual: 3.18 Automation Envelopes (59), 3.19 Clip Envelopes (59), 26 Automation (511 to 532: recording in Session and Arrangement, overwriting, overriding and re-enable, viewing, creating, editing including curves and shapes, deleting, tips), 27 Clip Envelopes (533 to 545: editor, audio clip envelopes, mixer and device clip envelopes, MIDI controller envelopes, unlinking), 41.5 and 42.27 editing envelopes with the keyboard, 42.7 commands for breakpoint envelopes. Parity rows: 3.18, 3.19, 26.1 to 26.9, 27.1 to 27.5.

## Goal

Arrangement automation, Session clip automation, and clip modulation envelopes (including audio clip transposition, volume, and sample offset envelopes) record, display, edit, and play back exactly as in Live.

## Decisions

- Envelope model: breakpoints with Live's curve segments (the manual describes curved segments created with Alt-drag; curve shape measured by oracle render, HUMAN), stair-step for quantized parameters, and inserted shapes (26.6) from Live's shape list.
- Automation vs modulation: automation sets the parameter value; clip envelopes modulate relative to the current value with ranges from manual 27.3.
- Override and re-enable (26.4): per-parameter overridden flag, global Re-Enable Automation button state.
- Engine evaluates envelopes per sub-block with sample-accurate breakpoints and per-parameter smoothing only where Live smooths (measured).

## Task Outline

1. Envelope model with curves, stairs, and evaluation tests.
2. Arrangement automation lanes: show, hide, add lanes, choose parameter (26.5.2).
3. Drawing and editing breakpoints, curves, shapes, keyboard editing (26.6, 26.7, 41.5, 42.7, 42.27).
4. Recording automation in Arrangement and Session (26.1, 26.2), overwriting (26.3), override and re-enable (26.4), deleting (26.8).
5. Session automation display (26.5.1).
6. Clip envelope editor for mixer and device parameters (27.1, 27.3).
7. Audio clip envelopes: transposition, volume, sample offset (27.2).
8. MIDI controller clip envelopes and unlinking (27.4, 27.5).
9. HUMAN: oracle renders for curve shapes; tests.
10. Milestone verification.

## Acceptance Criteria

- Oracle: automated volume ramp renders null below -90 dBFS against Live; curved segments meet nonlinear thresholds.
- Parity rows listed above Done with evidence.
