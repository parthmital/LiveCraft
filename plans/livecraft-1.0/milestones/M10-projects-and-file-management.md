# M10: Live Projects and file management

Purpose: scope, decisions, and acceptance criteria for projects, files, presets, and clip files. Read when: expanding or reviewing M10.
Phase: 2. Depends on: M09. Manual: 5.2 MIDI Files (133), 5.3 Live Clips (134), 5.5 Live Projects (142 to 146), 5.6 Locating Missing Files (147 to 149), 5.7 Collecting External Files (150 to 154), 5.8 Finding Unused Files (155 to 158), 5.9 Packing Projects into Packs (159 to 162), 5.10 FAQs (163), 3.10 MIDI Clips and MIDI Files (47). Parity rows: 5.x, 3.10. Beyond: B11.

## Goal

Live Projects, sample references, missing-file search, Collect All and Save, unused-file finding, Live Clips (`.alc`), device presets (`.adv`, `.adg`), MIDI file import and export, and Packs behave as the manual describes, plus a one-pass project health report.

## Decisions

- Live Project = folder with `Ableton Project Info` subfolder; LiveCraft writes the same layout and recognises Live's (manual 5.5). Files LiveCraft adds to that folder use only what the manual documents; Live's private files inside `Ableton Project Info` are preserved, never generated.
- Sample references keep both absolute and relative paths as `.als` does; resolution order and missing-file search follow manual 5.6.
- Path policy (finding LC-006): referenced files are only ever opened read-only; Collect All and Save copies, never moves; writing outside the project folder requires an explicit user action.
- `.alc`, `.adv`, `.adg` are gzip XML subsets handled by the M09 tree machinery.
- MIDI files: in-house SMF reader and writer (format 0 and 1, tempo and time signature meta events, running status), property-tested round trips; no new dependency.
- Packs (`.alp`): import only (extract into the User Library with path traversal checks); creating Packs is listed as a gap unless the format is fully documented in the manual.
- Health report (B11): `livecraft health <set.als>` and a dialog listing missing files, unused files, missing plug-ins, sample-rate mismatches, with JSON output.

## Task Outline

1. Project folder detection and creation; Save As into a new project.
2. Sample reference resolution (absolute, relative, search folders) and missing-file detection.
3. Manual 5.6 missing-file search UI flow (functional dialog; styling in later UI milestones).
4. Collect All and Save (manual 5.7 options), with tests in a temp project.
5. Finding unused files (manual 5.8).
6. `.alc` Live Clips save and load.
7. Device presets `.adv` and rack presets `.adg` load and save.
8. SMF reader and writer with property tests; drag MIDI file in and Export MIDI Clip.
9. Pack import with zip-slip protection tests.
10. Health report CLI and dialog (B11).
11. Milestone verification and docs (SECURITY LC-006 status updated).

## Acceptance Criteria

- Manual-quote tests for each documented behaviour in 5.5 to 5.8.
- Zip slip test: a crafted Pack entry `../../evil` is rejected.
- HUMAN: a project collected by LiveCraft opens in Live with no missing files.
