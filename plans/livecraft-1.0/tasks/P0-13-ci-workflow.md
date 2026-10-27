# Task P0-13: Rewrite CI with pinned actions, least privilege, and three platforms

Depends on: P0-12. Size: 5 files, about 110 lines (most copied verbatim). Milestone: P0.

## Goal

CI runs `cargo xtask ci --no-deny` on Windows, Ubuntu, and macOS, runs `cargo-deny` in its own job, uploads the Windows executable, uses SHA-pinned actions with read-only permissions, and Dependabot proposes weekly updates for Cargo and GitHub Actions.

## Before You Start

1. `git pull --ff-only`, then `git status --short`: empty.
2. In `plans/livecraft-1.0/CHECKLIST.md`, P0-12 is ticked and P0-13 is not.
3. Run `cargo xtask ci`. Expected: `xtask: all checks passed`.

## Context

- Today `.github/workflows/ci.yml` uses tag-pinned actions (`actions/checkout@v4`, `dtolnay/rust-toolchain@stable`, `Swatinem/rust-cache@v2`, `actions/upload-artifact@v4`), default token permissions, no macOS, and no dependency audit.
- The new workflow is ready in `plans/livecraft-1.0/files/P0-13/ci.yml`. Action SHAs were read with `gh api` on 2026-10-27: checkout v7.0.1 `3d3c42e5...`, rust-cache v2.9.3 `d1527c8f...`, cargo-deny-action v2.1.1 `3c634983...`, upload-artifact v7.0.2 `cf430e03...`.
- The pinned toolchain comes from `rust-toolchain.toml` (Task P0-01); `rustup toolchain install` with no arguments installs it.
- `cargo xtask ci --no-deny` exists after Task P0-12; the separate `deny` job runs `cargo-deny` with `deny.toml`.
- `persist-credentials: false` keeps the token out of `.git/config` (zizmor rule "artipacked").

## Steps

1. Replace `.github/workflows/ci.yml` with the prepared file:

   ```bash
   cp plans/livecraft-1.0/files/P0-13/ci.yml .github/workflows/ci.yml
   ```

2. Create `.github/dependabot.yml` with exactly:

   ```yaml
   version: 2
   updates:
     - package-ecosystem: cargo
       directory: /
       schedule:
         interval: weekly
       open-pull-requests-limit: 5
     - package-ecosystem: github-actions
       directory: /
       schedule:
         interval: weekly
   ```

3. In `README.md`, find:

   ```text
   `cargo xtask ci` runs all of these plus a crate layering check and a Markdown link check; run it before every commit. `cargo dev` launches the GUI. The CI workflow ([.github/workflows/ci.yml](.github/workflows/ci.yml)) runs the same checks. The individual commands are:
   ```

   Replace it with:

   ```text
   `cargo xtask ci` runs all of these plus a crate layering check, a Markdown link check, and `cargo-deny`; run it before every commit. `cargo dev` launches the GUI. CI ([.github/workflows/ci.yml](.github/workflows/ci.yml)) runs `cargo xtask ci --no-deny` on Windows, Ubuntu, and macOS, `cargo-deny` in a separate job, and uploads the Windows executable as an artifact. Dependabot proposes weekly dependency and action updates. The individual commands are:
   ```

4. In `SECURITY.md`, find:

   ```text
   - CI runs `cargo clippy -D warnings` and tests on Windows and Ubuntu.
   ```

   Replace it with:

   ```text
   - CI (`.github/workflows/ci.yml`) runs every check on Windows, Ubuntu, and macOS with read-only token permissions, SHA-pinned actions, and `persist-credentials: false`; a separate job runs `cargo-deny` (advisories, licences, bans, sources).
   - Dependabot (`.github/dependabot.yml`) proposes weekly Cargo and GitHub Actions updates.
   ```

5. Validate the YAML and audit the workflow (`pyyaml` and `zizmor` are in `plans/livecraft-1.0/tools/requirements.txt`; rerun `./.venv/Scripts/python -m pip install -r plans/livecraft-1.0/tools/requirements.txt` first if either import fails):

   ```bash
   ./.venv/Scripts/python -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml')); yaml.safe_load(open('.github/dependabot.yml')); print('yaml ok')"
   ./.venv/Scripts/zizmor --offline .github/workflows/ci.yml
   ```

   Expected: `yaml ok`, and zizmor prints `No findings to report. Good job!` (validated on 2026-10-27 with zizmor 1.31.1; if it reports findings, fix each in `ci.yml` as its message says and rerun; at most 3 attempts).

## Do Not

- Change files other than: `.github/workflows/ci.yml`, `.github/dependabot.yml`, `README.md`, `SECURITY.md`, `plans/livecraft-1.0/CHECKLIST.md`.
- Do not replace SHAs with tags.

## Verify

| Command                                               | Expected result                                                                             |
| ----------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| `cargo xtask ci`                                      | `xtask: all checks passed`                                                                  |
| `grep -c "@[0-9a-f]\{40\}" .github/workflows/ci.yml`  | `7` (every `uses:` line is SHA-pinned)                                                      |
| `grep -n "permissions:" -A1 .github/workflows/ci.yml` | shows `contents: read`                                                                      |
| After pushing: `gh run list --workflow CI --limit 1`  | the run for this commit; then `gh run watch` until it completes with `success` for all jobs |

## Commit And Push

1. Tick P0-13 in `plans/livecraft-1.0/CHECKLIST.md`.
2. `git add .github/workflows/ci.yml .github/dependabot.yml README.md SECURITY.md plans/livecraft-1.0/CHECKLIST.md`
3. Commit:

   ```text
   Rewrite CI with pinned actions and three platforms

   - Run cargo xtask ci on Windows, Ubuntu, and macOS
   - Add a cargo-deny job and keep the Windows executable artifact
   - Pin actions by SHA, set read-only permissions, disable persisted credentials
   - Add Dependabot for Cargo and GitHub Actions
   ```

4. `git push origin main`, then `git status --short --branch`: clean, not ahead. Then run the last Verify row.

## If Something Fails

- The CI run fails on one OS only: read the job log with `gh run view --log-failed`. If the fix is in the workflow, fix it (at most 3 attempts, one commit each with subject `Fix CI: <what>`). If the failure is in Rust code on macOS or Linux only, log a blocker with the log excerpt; do not change Rust code in this task.
- `gh` is not authenticated: skip the last Verify row, write `CI run not verified: gh not authenticated` in the commit body, and continue.
- Still failing: append under Blockers in `CHECKLIST.md`: `P0-13, <date>: <what failed>` and the last 20 lines of the failing log. Commit only `CHECKLIST.md`, push, stop.
