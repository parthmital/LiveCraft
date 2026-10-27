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
- CI runs `cargo clippy -D warnings` and tests on Windows and Ubuntu.
- Planned in `plans/livecraft-1.0/` Phase 0: `cargo-deny` for advisories, licences, and sources; input size limits; `unsafe_code = "forbid"`; output safety limiter.

## Findings

Preliminary review on 2026-10-27, not a full audit. The full audit is Phase 0 of the plan.

| ID     | Severity | CWE      | Status | Location                                               | Fix                                                              |
| ------ | -------- | -------- | ------ | ------------------------------------------------------ | ---------------------------------------------------------------- |
| LC-001 | Medium   | CWE-409  | Open   | `crates/livecraft-core/src/als.rs` `decompress_to_xml` | Cap decompressed size and reject larger files                    |
| LC-002 | Low      | CWE-400  | Open   | WAV and sample loading in `livecraft-dsp`              | Cap file size and sample count before allocating                 |
| LC-003 | Low      | CWE-1104 | Open   | Workspace dependencies                                 | Add `cargo-deny` advisories and licence checks to CI             |
| LC-004 | Low      | CWE-20   | Open   | DSP output path                                        | Replace NaN and infinity with silence and clamp the final output |

## Residual Risks And Required Actions

- No dependency audit has run yet. Run Phase 0 of the plan before any public release.
- Ableton Link support, if added, is GPL-2.0-or-later or needs a commercial licence from Ableton; see `RESEARCH.md#sync-and-ableton-link-2026-10-27`.
