# Task P0-17: Full security audit and SECURITY.md update

Depends on: P0-16. Size: 1 to 3 files changed (`SECURITY.md`, possibly `CHECKLIST.md`), plus local scan reports. Milestone: P0.

## Goal

Every security checklist category has a recorded status in `SECURITY.md`, backed by scans of the code, the dependencies, the CI workflows, and the whole Git history.

## Before You Start

1. `git pull --ff-only`, then `git status --short`: empty.
2. In `plans/livecraft-1.0/CHECKLIST.md`, P0-16 is `[x]` or `[~]` and P0-17 is `[ ]`.
3. Run `cargo xtask ci`. Expected: `xtask: all checks passed`, 53 tests.

## Context

- Tools (all repo-local): `cargo-deny` 0.20.2 in `./.tools/bin` (Task P0-12); `zizmor` 1.31.1 in `.venv` (from `plans/livecraft-1.0/tools/requirements.txt`); gitleaks 8.30.1 downloaded in step 1 into `./.tools/`.
- Facts checked on 2026-10-27, to confirm, not assume: no `std::process::Command` in `crates/` (only in `xtask`); no `unsafe` (forbidden by lint); the app opens no network sockets; `.als` sample references (`AudioClipData::file_path`) can point to any local path and are read as audio (Live behaves the same; path policy is milestone M10); quick-xml does not resolve external entities (no XXE).
- Severity scale and CWE tags as in the existing Findings table. Never write secrets or exploit steps for unfixed findings into any committed file.

## Steps

1. Get gitleaks (Windows; on Linux use the `linux_x64.tar.gz` asset, on macOS `darwin_arm64.tar.gz`):

   ```bash
   mkdir -p ./.tools/downloads ./.tools/bin
   gh release download v8.30.1 --repo gitleaks/gitleaks --pattern "gitleaks_8.30.1_windows_x64.zip" --pattern "gitleaks_8.30.1_checksums.txt" --dir ./.tools/downloads --clobber
   cd ./.tools/downloads && grep windows_x64.zip gitleaks_8.30.1_checksums.txt | sha256sum -c - && cd ../..
   unzip -o ./.tools/downloads/gitleaks_8.30.1_windows_x64.zip gitleaks.exe -d ./.tools/bin
   ```

   Expected: `gitleaks_8.30.1_windows_x64.zip: OK`, then `./.tools/bin/gitleaks.exe version` prints `8.30.1`. If `gh` is not authenticated, download the two files from `https://github.com/gitleaks/gitleaks/releases/tag/v8.30.1` in a browser into `./.tools/downloads/` and continue with the checksum line.

2. Scan the whole history: `./.tools/bin/gitleaks git . --redact --report-path .agent-local/output/p0-17/gitleaks.json`. Expected: `no leaks found`. If leaks are found: do not print or commit them; append an `OWNER-QUEUE.md` row `Review N potential secrets found by gitleaks (report in .agent-local/output/p0-17/gitleaks.json) and rotate any real ones` (Needed by line P0-17, Status Open), add a SECURITY.md finding row with status Open, mark P0-17 `[~]` at the end, and continue (rotation and history rewriting need the owner; never rewrite history yourself).

3. Dependencies: `./.tools/bin/cargo-deny check`. Expected: `advisories ok, bans ok, licenses ok, sources ok`.

4. Workflows: `./.venv/Scripts/zizmor --offline .github/workflows/`. Expected: `No findings to report. Good job!`.

5. Code review by checklist. For each row, run the command and record the result:

   | Category                        | Check command                                                                                                                            | Expected now                                                             |
   | ------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------ |
   | OS command injection            | `grep -rn "process::Command\|Command::new" crates`                                                                                       | no output                                                                |
   | Unsafe code                     | `grep -rn "unsafe" crates xtask`                                                                                                         | no output                                                                |
   | Network access                  | `grep -rn "TcpStream\|UdpSocket\|reqwest\|ureq\|hyper" crates`                                                                           | no output                                                                |
   | Panics on untrusted input       | `grep -n "unwrap()\|expect(" crates/livecraft-core/src/als.rs crates/livecraft-core/src/theme.rs crates/livecraft-dsp/src/audio_clip.rs` | no output outside test code                                              |
   | Decompression bomb              | `cargo test -p livecraft-core test_als_decompression_rejects_oversized_output`                                                           | passes                                                                   |
   | Oversized audio                 | `cargo test -p livecraft-dsp --test wav_limits`                                                                                          | passes                                                                   |
   | Non-finite output               | `cargo test -p livecraft-dsp test_sanitize_output_sample`                                                                                | passes                                                                   |
   | Path traversal via sample paths | read `crates/livecraft-dsp/src/audio_clip.rs` `render_clip_samples`                                                                      | any absolute path is opened read-only; record as Low, accepted until M10 |

   For any check that does not match "Expected now": if it is a panic on untrusted input (`unwrap`/`expect` in parsing code), add a finding row with status Open and Milestone M09 (the parser rewrite); if it is anything else of High or Critical severity, log a blocker and stop.

6. Rewrite the `## Controls` section of `SECURITY.md` to keep its existing bullets and add a checklist table:

   ```markdown
   | Category                      | Status         | Evidence                                                                   |
   | ----------------------------- | -------------- | -------------------------------------------------------------------------- |
   | Secrets (code and history)    | Clean          | gitleaks 8.30.1 full-history scan, <date>                                  |
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
   ```

   Replace `<date>` with today's date and adjust any row whose check in step 5 gave a different result.

7. Add the finding row `| LC-006 | Low | CWE-22 | Accepted | Sample file paths in .als (audio_clip.rs) | Read-only; path policy and missing-file handling in M10 |` to the Findings table, change the line `Preliminary review on 2026-10-27, not a full audit. The full audit is Phase 0 of the plan.` to `Full audit on <date> (Task P0-17): tools gitleaks 8.30.1, cargo-deny 0.20.2, zizmor 1.31.1, manual checklist.`, update `Last verified:` to today, and remove the bullet `- No dependency audit has run yet. Run Phase 0 of the plan before any public release.` from Residual Risks.

## Do Not

- Change files other than: `SECURITY.md`, `plans/livecraft-1.0/CHECKLIST.md`.
- Commit `./.tools/` or anything under `.agent-local/`. Print or commit any secret.

## Verify

| Command                                               | Expected result                      |
| ----------------------------------------------------- | ------------------------------------ |
| `./.tools/bin/gitleaks git . --redact`                | `no leaks found`                     |
| `./.tools/bin/cargo-deny check`                       | all four checks ok                   |
| `./.venv/Scripts/zizmor --offline .github/workflows/` | no findings                          |
| `grep -c "^                                           | LC-00" SECURITY.md`                  | `6` |
| `cargo xtask ci`                                      | `xtask: all checks passed`, 53 tests |

## Commit And Push

1. Tick P0-17 in `plans/livecraft-1.0/CHECKLIST.md`.
2. `git add SECURITY.md plans/livecraft-1.0/CHECKLIST.md`
3. Commit:

   ```text
   Record the Phase 0 security audit

   - Scan history with gitleaks, dependencies with cargo-deny, workflows with zizmor
   - Record a status for every checklist category in SECURITY.md
   - Add finding LC-006 (sample paths) as accepted until M10
   ```

4. `git push origin main`, then `git status --short --branch`: clean, not ahead. Continue with the next line.

## If Something Fails

- A tool cannot be installed: record the category as Unverified in the table with the reason, and continue.
- Any High or Critical finding: do not commit; append under Blockers in `CHECKLIST.md`: `P0-17, <date>: <category> High finding, details in .agent-local/output/p0-17/`. Commit only `CHECKLIST.md`, push, end the run.
- `git push` rejected: `git pull --rebase`, rerun Verify, push. If the rebase conflicts: `git rebase --abort`, then the Unblock Procedure in `plans/livecraft-1.0/START-HERE.md`.
