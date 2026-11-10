# Security

Purpose: how this project is protected and what risk remains. Read when: touching file parsing, sample loading, plug-in hosting, networking, dependencies, or CI.
Last verified: 2026-11-10.

## Reporting

- Report vulnerabilities privately through GitHub's "Report a vulnerability" (Security tab of `parthmital/LiveCraft`). Do not open a public issue.
- Include the affected version or commit, a minimal reproducing file if the issue is in file parsing, and the impact.

## Threat Model

- Assets: the user's machine and files, their projects and samples, and audio output (sudden full-scale noise can damage hearing and speakers).
- Entry points: `.als`, `.alc`, `.adv`, `.adg`, `.mid`, `.alp`, `.json`, `.ask`, `.scl`, and audio files opened by the user or received from others; CLI arguments; in future, plug-in binaries, MIDI devices, network sync (Link), and a local control server.
- Trust boundaries: every file is untrusted input. Plug-ins (future) are untrusted native code.
- Main threats: denial of service through decompression bombs or huge allocations, path traversal (zip-slip or `RelativePath` escape) through sample references or Packs, crashes from malformed input, malicious plug-ins, dependency supply chain, and loud output from NaN or unbounded DSP.

## Controls

- `unsafe_code = "forbid"` for every crate through `[workspace.lints]` in `Cargo.toml`.
- CI (`.github/workflows/ci.yml`) runs every check on Windows, Ubuntu, and macOS with read-only token permissions, SHA-pinned actions, and `persist-credentials: false`; a separate job runs `cargo-deny` (advisories, licences, bans, sources).
- Dependabot (`.github/dependabot.yml`) proposes weekly Cargo and GitHub Actions updates.
- `cargo-deny` (`deny.toml`) checks advisories, licences, bans, and sources in `cargo xtask ci`.
- Input size and traversal limits: 512 MiB XML cap and 4096 depth cap for `.als`/`.alc`/`.adv`/`.adg`, 2^28 samples (`MAX_RAM_AUDIO_SAMPLES`) for RAM audio loads (streaming clips read bounded chunks off-thread without unbounded RAM allocation), 32 MiB cap for `.mid` SMF files, 512 MiB per-entry / 2 GiB total caps plus zip-slip/symlink rejection for `.alp` Packs, read-only sample access with project-root containment checks (`delete_unused_file`, `save_set_into_project`, `walk_files`, `resolve_candidate`), arbitrary-bytes `proptest` fuzzing on parsers and `decode_audio_file`/`probe_audio_info`, and a non-finite output guard on driver and render paths.

| Category                      | Status         | Evidence                                                                     |
| ----------------------------- | -------------- | ---------------------------------------------------------------------------- |
| Secrets (code and history)    | Clean          | gitleaks 8.30.1 full-history scan, 2026-10-27                                |
| Authentication, authorisation | Not applicable | No accounts, roles, or remote access                                         |
| Injection                     | Clean          | No process spawning in product crates                                        |
| Web client                    | Not applicable | No web UI                                                                    |
| Parsing (XXE, bombs)          | Fixed          | LC-001, LC-005; `als/read.rs` 512 MiB cap, depth cap 4096, SMF 32 MiB cap, `.alp` 512 MiB/2 GiB caps, arbitrary-bytes proptests across `.als`, `.alc`, `.adv`, `.adg`, `.mid`, `.alp`, and audio probes |
| Files and paths               | Mitigated      | LC-006: read-only sample reads, copy-never-move Collect All and Save, `save_set_into_project` containment, `resolve_candidate` escape checks, `walk_files` symlink skip, `.alp` zip-slip/symlink rejection (`m10_projects.rs`, `m10_formats.rs`) |
| API                           | Not applicable | No API until M42                                                             |
| Logic and concurrency         | Fixed          | M05: the audio thread takes no locks and allocates nothing (rt-checks tests) |
| Crypto                        | Not applicable | No cryptography                                                              |
| Errors and privacy            | Clean          | No personal data collected                                                   |
| Supply chain                  | Clean          | cargo-deny, Cargo.lock, pinned toolchain, Dependabot                         |
| CI/CD                         | Clean          | zizmor 1.31.1, SHA-pinned actions, read-only token                           |
| Containers and infrastructure | Not applicable | None                                                                         |
| DoS                           | Fixed          | LC-001, LC-002, LC-005, `.alp` decompression-bomb guard                      |
| LLM and agents                | Not applicable | Until M42                                                                    |

## Findings

Full audit on 2026-10-27 (Task P0-17): tools gitleaks 8.30.1, cargo-deny 0.20.2, zizmor 1.31.1, manual checklist. Updated for M10 on 2026-11-10.

| ID     | Severity | CWE      | Status    | Location                                                                          | Fix                                                                                                                                                         |
| ------ | -------- | -------- | --------- | --------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| LC-001 | Medium   | CWE-409  | Fixed     | `crates/livecraft-formats/src/als/read.rs` `decompress_als_or_xml`                | 512 MiB cap in `decompress_reader` + XML depth cap 4096, tested in `read.rs` and `tests.rs` (P0-14, M09)                                                    |
| LC-002 | Low      | CWE-400  | Fixed     | `load_file_with_limit` in `crates/livecraft-engine/src/audio_clip.rs`             | 2^28-sample cap before decoding; tests in crates/livecraft-engine/tests/wav_limits.rs (Task P0-15)                                                          |
| LC-003 | Low      | CWE-1104 | Fixed     | Workspace dependencies                                                            | cargo-deny in cargo xtask ci (Task P0-12)                                                                                                                   |
| LC-004 | Low      | CWE-20   | Fixed     | `sanitize_output_sample` in `crates/livecraft-engine/src/executor.rs`             | Non-finite samples become 0.0 in the f32 driver path and in renders; test sanitize_replaces_non_finite_samples_and_keeps_finite_ones_bit_exact (Task P0-16) |
| LC-005 | Medium   | CWE-407  | Fixed     | `quick-xml` 0.42.0 (`crates/livecraft-formats/src/als/`)                          | Upgraded to 0.42.0 (P0-11); legacy `als.rs` parser code path deleted in M09                                                                                 |
| LC-006 | Low      | CWE-22   | Mitigated | `project_dir.rs`, `health.rs`, `live_clip.rs`, `pack.rs`, `project_ops.rs`        | Read-only sample loading, copy-never-move Collect All and Save, `save_set_into_project` containment, `resolve_candidate` traversal blocks, `walk_files` symlink skip, `.alp` zip-slip/symlink rejection, and `delete_unused_file` confinement (M10 tests in `m10_projects.rs`, `m10_formats.rs`, `m10_session.rs`) |

## Residual Risks And Required Actions

- Ableton Link support is GPL-2.0-or-later code shipped in release builds under the project's GPL-3.0-or-later licence (DECISIONS D31); see `RESEARCH.md#sync-and-ableton-link-2026-10-27`.
