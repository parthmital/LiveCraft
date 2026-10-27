# Beyond Live

Purpose: the improvements that make LiveCraft more capable than Live 12.4, each tied to a verified gap and a milestone. Read when: expanding a milestone listed below, or proposing a new improvement.
Last verified: 2026-10-27.

Rules for this list:

- An item is added only when the gap is verified (manual page, manual search with no result, or the installed Live) and recorded in the Evidence column.
- Improvements never change Live-compatible behaviour by default. Where an improvement alters a workflow, it is a setting that defaults to Live's behaviour, so parity tests keep passing.
- Every item ends in tests and a parity matrix entry under "Beyond".

| ID  | Improvement                                                                                                                                                       | Gap in Live 12.4 (evidence)                                                                                            | Milestone |
| --- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- | --------- |
| B01 | Native Linux build (x64), plus Windows and macOS                                                                                                                  | Manual lists only Windows and Mac ("for Windows and Mac", title page); no Linux mention                                | M45       |
| B02 | Plug-ins hosted out of process; a crashing plug-in is disabled and reported while audio continues                                                                 | Manual p. 487: faulty VST plug-ins "may cause the program to crash"                                                    | M37       |
| B03 | Headless CLI: render, stems, inspect, convert, patch sets, batch jobs, CI-friendly exit codes                                                                     | Manual has no command-line interface (search "command line": no result)                                                | M08, M42  |
| B04 | Local control server (JSON-RPC 2.0) and MCP adapter exposing every command, for automation and AI agents                                                          | Live's remote control is MIDI mapping and control surface scripts (chapter 34); no documented external API             | M42       |
| B05 | Scripted MIDI Tools and control scripts in Rhai, without a separate paid add-on                                                                                   | Custom MIDI Tools need Max for Live (manual 32.4)                                                                      | M42       |
| B06 | Continuous autosave with a browsable version history of the set (time-stamped, unlimited by count, pruned by disk budget) and crash recovery to the last autosave | Backup folder keeps "the last ten saved versions", only on save (manual p. 115)                                        | M43       |
| B07 | Main output safety limiter (on by default for monitoring only, never in renders) and NaN guard                                                                    | No hearing-protection limiter documented (search "safety" and "protect": no output-protection feature)                 | M43       |
| B08 | Integrated loudness (LUFS momentary, short-term, integrated) and true-peak meters on any track and in renders                                                     | Manual has no LUFS metering (search "LUFS": no result)                                                                 | M44       |
| B09 | DAWproject import and export (open interchange format used by other DAWs)                                                                                         | Manual has no DAWproject support (search: no result)                                                                   | M44       |
| B10 | Version-control friendly sets: stable, pretty-printed XML export and `livecraft diff a.als b.als` showing musical changes                                         | `.als` is gzip-compressed (manual 5.4); no diff tool                                                                   | M09, M42  |
| B11 | Project health report (missing files, unused files, missing plug-ins, sample-rate mismatches) from GUI and CLI in one pass                                        | Separate manual workflows: Locating Missing Files, Collecting External Files, Finding Unused Files (manual 5.6 to 5.8) | M10       |
| B12 | Stem Separation with no edition restriction, fully offline after one opt-in model download                                                                        | Live documents Stem Separation as a Live 12 Suite feature (12.4 release notes)                                         | M36       |
| B13 | Deterministic, bit-identical offline renders with a render report (settings, versions, hashes)                                                                    | Not documented by Live                                                                                                 | M08       |
| B14 | No account, no network access unless a feature the user enables needs it, no telemetry                                                                            | Live requires authorisation (manual 2.1)                                                                               | M45       |

## Candidates Not Yet Verified

Investigate during the named milestone; add to the table only with evidence.

- Detachable panels across monitors beyond Live's Second Window (manual 42.1 "Toggle Second Window") - M14.
- Arrangement sections that move and duplicate as named blocks (beyond locators, manual 6.4) - M16.
- Per-track spectrum and phase scope without inserting a device - M44.
