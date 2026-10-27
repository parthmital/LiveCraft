# Security

Purpose: how this project is protected and what risk remains. Read when: touching file parsing, sample loading, plug-in hosting, networking, dependencies, or CI.
Last verified: 2026-10-27.

## Reporting

- Report vulnerabilities privately through GitHub's "Report a vulnerability" (Security tab of `parthmital/LiveCraft`). Do not open a public issue.
- Include the affected version or commit, a minimal reproducing file if the issue is in file parsing, and the impact.

## Threat Model

- Assets: the user's machine and files, their projects and samples, and audio output (sudden full-scale noise can damage hearing and speakers).
- Entry points: `.als`, `.json`, `.ask`, `.scl`, and WAV files opened by the user or received from others; CLI arguments; in future, plug-in binaries, MIDI devices, network sync (Link), and a local control server.
- Trust boundaries: every file is untrusted input. Plug-ins (future) are untrusted native code.
- Main threats: denial of service through decompression bombs or huge allocations, path traversal through sample references, crashes from malformed input, malicious plug-ins, dependency supply chain, and loud output from NaN or unbounded DSP.

## Controls

- `unsafe_code = "forbid"` for every crate through `[workspace.lints]` in `Cargo.toml`.
- CI (`.github/workflows/ci.yml`) runs every check on Windows, Ubuntu, and macOS with read-only token permissions, SHA-pinned actions, and `persist-credentials: false`; a separate job runs `cargo-deny` (advisories, licences, bans, sources).
- Dependabot (`.github/dependabot.yml`) proposes weekly Cargo and GitHub Actions updates.
- `cargo-deny` (`deny.toml`) checks advisories, licences, bans, and sources in `cargo xtask ci`.
- Input size limits for `.als` (512 MiB XML) and WAV (2^28 samples), and a non-finite output guard on the driver and render paths.

| Category                      | Status         | Evidence                                                                   |
| ----------------------------- | -------------- | -------------------------------------------------------------------------- |
| Secrets (code and history)    | Clean          | gitleaks 8.30.1 full-history scan, 2026-10-27                              |
| Authentication, authorisation | Not applicable | No accounts, roles, or remote access                                       |
| Injection                     | Clean          | No process spawning in product crates                                      |
| Web client                    | Not applicable | No web UI                                                                  |
| Parsing (XXE, bombs)          | Fixed          | LC-001, LC-005; quick-xml does not resolve entities                        |
| Files and paths               | Accepted risk  | LC-006, sample paths read-only; policy in M10                              |
| API                           | Not applicable | No API until M42                                                           |
| Logic and concurrency         | Open           | Audio thread locking and allocation, ARCHITECTURE.md#known-violations, M05 |
| Crypto                        | Not applicable | No cryptography                                                            |
| Errors and privacy            | Clean          | No personal data collected                                                 |
| Supply chain                  | Clean          | cargo-deny, Cargo.lock, pinned toolchain, Dependabot                       |
| CI/CD                         | Clean          | zizmor 1.31.1, SHA-pinned actions, read-only token                         |
| Containers and infrastructure | Not applicable | None                                                                       |
| DoS                           | Fixed          | LC-001, LC-002, LC-005                                                     |
| LLM and agents                | Not applicable | Until M42                                                                  |

## Findings

Full audit on 2026-10-27 (Task P0-17): tools gitleaks 8.30.1, cargo-deny 0.20.2, zizmor 1.31.1, manual checklist.

| ID     | Severity | CWE      | Status   | Location                                               | Fix                                                              |
| ------ | -------- | -------- | -------- | ------------------------------------------------------ | ---------------------------------------------------------------- |
| LC-001 | Medium   | CWE-409  | Fixed    | `crates/livecraft-core/src/als.rs` `decompress_to_xml` | 512 MiB cap in AlsParser::decompress_reader, test test_als_decompression_rejects_oversized_output (Task P0-14) |
| LC-002 | Low      | CWE-400  | Fixed    | `AudioSampleCache::load_file_with_limit` in `crates/livecraft-dsp/src/audio_clip.rs` | 2^28-sample cap before decoding; tests in crates/livecraft-dsp/tests/wav_limits.rs (Task P0-15) |
| LC-003 | Low      | CWE-1104 | Fixed    | Workspace dependencies                                 | cargo-deny in cargo xtask ci (Task P0-12)                        |
| LC-004 | Low      | CWE-20   | Fixed    | `sanitize_output_sample` in `crates/livecraft-dsp/src/graph.rs` | Non-finite samples become 0.0 in the f32 driver path and in renders; test test_sanitize_output_sample (Task P0-16) |
| LC-005 | Medium   | CWE-407  | Fixed    | `quick-xml` 0.36.2 (RUSTSEC-2026-0194, RUSTSEC-2026-0195) | Upgraded to 0.42.0 (Task P0-11) |
| LC-006 | Low      | CWE-22   | Accepted | Sample file paths in .als (audio_clip.rs)              | Read-only; path policy and missing-file handling in M10          |

## Residual Risks And Required Actions

- Ableton Link support, if added, is GPL-2.0-or-later or needs a commercial licence from Ableton; see `RESEARCH.md#sync-and-ableton-link-2026-10-27`.
