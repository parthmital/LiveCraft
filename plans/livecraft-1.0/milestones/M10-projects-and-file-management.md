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

## Review (2026-11-10)

All M10 Outline items (1 to 11) were completed locally across `livecraft-model`, `livecraft-formats`, `livecraft-engine`, `livecraft-session`, `livecraft-ui`, and `apps/livecraft`, verified, and committed together per milestone policy; the HUMAN opening check of a LiveCraft-collected project in Live 12.4 is queued as `Q14`.

| Item | Summary |
| ---- | ------- |
| 1 | `project_dir.rs` (`is_project_root`, `detect_project_root`, `create_project_layout`, `save_set_into_project`), `Session::save_into_project`, `FileManagerWindow` Save As dialog (`Ctrl+Shift+S`), and `livecraft new --project`. Preserves private files inside `Ableton Project Info` and validates containment before directory creation. |
| 2 | `AudioClipData.relative_path` (`clip.rs`, `als/map/audio_clips.rs`), `ProjectContext` + `resolve_sample_reference` (`Path` then `RelativePath`), non-clip XML `<FileRef>` repair (`replace_xml_sample_reference`), `GraphCompiler`/`Playback`/`OfflineBouncer` context resolution, and `Session::locate_missing_manual`/`locate_missing_auto` with undoable `ReplaceSampleReference`. |
| 3 | Status bar missing-file Locate warning button (`app.rs`) opening `FileManagerWindow` (`file_manager.rs`) `Missing Files` tab with manual replacement and `Automatic Search` (`Search Folder`, `Search Project`, `Search Library`, `NoCandidate` / auto-accept / `MultipleCandidates` picker). |
| 4 | `collect_all_and_save` (`CollectAllOptions`, `CollectFilesOnExport`, copy-never-move with byte-verified `copy_dedup`), `LibraryPreferences`, `Session::collect_all_and_save`, CLI `livecraft collect`, UI Collect tab, and `Ask` confirmation modal (`m10_projects.rs`, `m10_ui.rs`, `m10_cli.rs`). |
| 5 | `find_unused_files` scanning `.als`/`.alc`/`.adv`/`.adg` (skipping `Ableton Project Info`, `Backup`, and symlinks), `Session::find_unused_files`/`delete_unused_file` (confined to verified unused files inside `project_root`), CLI `livecraft unused`, and UI Unused Files tab with two-step delete. |
| 6 | `.alc` `LiveClipFile` (`live_clip.rs`) with `copy_dedup` sample import, `Session::import_live_clip`/`export_live_clip` enforcing manual 5.3 empty-track vs occupied-track device chain rule and `CollectFilesOnExport`, CLI `livecraft clip import-alc`/`export-alc`, Clip View `Save .alc`, and Browser/drag-drop support. |
| 7 | `.adv` device presets and `.adg` rack presets (`presets.rs`) with `RackKind` (`AudioEffect`, `Instrument`, `MidiEffect`) and `Device.sample_refs` preservation, `Session` undoable preset load/save, and UI Device Chain / Browser hooks. |
| 8 | In-house SMF `.mid` reader/writer (`midi_file.rs`: Format 0 and 1, running status, tempo & time-signature meta events, 32 MiB cap, proptest round-trips), `Session::import_midi_file`/`export_midi_clip`, CLI `livecraft midi import`/`export`, Piano Roll `Export MIDI`, and GUI drag-drop. |
| 9 | `.alp` Pack extraction (`pack.rs`, `zip` `=9.1.2`) with two-pass preflight validation rejecting `../../evil` zip-slip, backslashes, drive letters, encrypted entries, symlinks, and decompression bombs (512 MiB entry / 2 GiB total), `Session::install_pack`, CLI `livecraft pack install`, and UI dialog. |
| 10 | B11 one-pass `ProjectHealthReport` (`health.rs`: missing files, unused files, missing plug-ins, sample-rate mismatches across WAV/AIFF/FLAC/MP3 probes, external files), `Session::health_report`, `livecraft health <set.als> [--json] [--strict]`, and UI Health Report dialog. |
| 11 | Updated `SECURITY.md` (`LC-006` status `Mitigated` + parser limits), `RESEARCH.md` (`zip` `=9.1.2`), `DECISIONS.md` (`D33`), `ARCHITECTURE.md`, `README.md`, `PARITY-MATRIX.md`, `OWNER-QUEUE.md` (`Q14`), and `CHECKLIST.md`. |

### Manual quotes

From `docs/ableton-live-12-reference-manual.pdf`:

- 5.2, p. 133: "Importing MIDI files into Live works differently than with samples: MIDI file data is incorporated into the Live Set, and the resulting MIDI clips lose all reference to the original file."
- 5.3, p. 134: "In order to recreate a Live Clip's device chain, either drag it into a track containing no clips or devices, or drag it into the space in the Session or Arrangement View containing no tracks. Note that Live Clips that are imported into tracks already containing devices or clips will appear with their clip settings but not their devices."
- 5.5, p. 142: "When you save a Live Set under a new name or in a new folder location, Live will create a new project folder and store the Live Set there — unless you are saving the Live Set into an existing Live Project."
- 5.6, p. 147: "Clips and instrument sample slots that reference missing samples will appear marked \"Offline,\" and Live will play silence instead of the missing samples."
- 5.6.1, p. 148: "Note that Live will not care if the file you offer is really the file that was missing."
- 5.7, p. 150: "To prevent broken file references, you can collect (i.e., copy) all external files used in a Set into its Project folder."
- 5.8, p. 156: "Live considers any file in the Project which is not referenced by its Sets, clips, or presets as unused — even if the file is actively used in other Projects."
- 5.9, p. 159: "When you install a Project-based Pack, the complete folder is restored, including its corresponding Set (or all Sets if the Project originally contained multiple .als files)."

### Evidence

- `cargo xtask ci` (fmt, clippy `-D warnings`, workspace tests, release build, layering check, docs link check, `cargo-deny`): 609 passed, 0 failed, 1 ignored (`ten_minutes_of_playback_allocate_nothing`).
- `crates/livecraft-formats/tests/m10_projects.rs` (10 tests), `crates/livecraft-formats/tests/m10_formats.rs` (5 tests), `crates/livecraft-formats` unit tests (46 tests including arbitrary-bytes proptests on `.als`, `.alc`, `.adv`, `.adg`, `.mid`, `.alp`, and container probes).
- `crates/livecraft-session/tests/m10_session.rs` (7 integration tests covering undo/redo and manual 5.3/5.6/5.7.1 rules).
- `crates/livecraft-ui/tests/m10_ui.rs` (6 headless egui integration tests) and `apps/livecraft/tests/m10_cli.rs` (8 CLI binary integration tests).
- End-to-end CLI verification into `.agent-local/output/m10-q14/CollectedDemo Project/`: `livecraft new --project`, `livecraft collect`, `livecraft health --strict` (`HEALTHY`), and `livecraft render` all succeeded.

### Criteria

| Criterion | Met | Evidence |
| --------- | --- | -------- |
| Manual-quote tests for each documented behaviour in 5.5 to 5.8 | Yes | `m10_projects.rs` (`manual_5_5_*`, `manual_5_6_*`, `manual_5_7_*`, `manual_5_8_*`), `m10_session.rs`, `m10_ui.rs`, `m10_cli.rs` |
| Zip slip test: a crafted Pack entry `../../evil` is rejected | Yes | `pack_install_rejects_zip_slip_traversal_variants_with_zero_writes` (`m10_formats.rs`), `pack.rs` unit tests, `pack_install_extracts_valid_alp_and_rejects_zip_slip` (`m10_cli.rs`) |
| HUMAN: a project collected by LiveCraft opens in Live with no missing files | Partly (owner Q14) | End-to-end collected project verified with `livecraft health --strict` and `livecraft render`; Live 12.4 interactive open check queued as `Q14` |

### Owner rows

- `Q14`: owner opens the LiveCraft-collected test project in Live 12.4 Suite (including after moving the project folder) and confirms zero missing-file warnings and playback. Line stays `[~]` until `Q14` arrives.

### Not verified

- Opening a LiveCraft-collected project interactively in Live 12.4 Suite awaits `Q14`.
- Creating `.alp` Packs remains a documented gap per Decisions above.
