# Task P0-11: Upgrade quick-xml to 0.42 to fix two advisories

Depends on: P0-10. Size: 5 files, about 90 changed lines (mostly by a validated script). Milestone: P0.

## Goal

`.als` parsing uses quick-xml 0.42.0, removing RUSTSEC-2026-0194 and RUSTSEC-2026-0195, with unchanged parsing results.

## Before You Start

1. `git pull --ff-only`, then `git status --short`: empty.
2. In `plans/livecraft-1.0/CHECKLIST.md`, P0-10 is `[x]` or `[~]` and P0-11 is `[ ]`.
3. Run `cargo xtask ci`. Expected: `xtask: all checks passed`, 49 tests.

## Context

- cargo-deny on 2026-10-27 reported for quick-xml 0.36.2: RUSTSEC-2026-0194 (quadratic run time when checking a start tag for duplicate attribute names) and RUSTSEC-2026-0195 (unbounded namespace-declaration allocation in `NsReader`). Solution: upgrade to 0.41.0 or later. `.als` files are untrusted input (`SECURITY.md#threat-model`), so a crafted set could hang the app.
- quick-xml 0.42 exposes attribute keys, attribute values, and element names as `str` instead of bytes. `plans/livecraft-1.0/tools/quick_xml_042_migrate.py` adapts `crates/livecraft-core/src/als.rs` with six rules (`.as_bytes()` adapters and `b"Key"` literals becoming `"Key"`). Validated on 2026-10-27: clippy clean, all tests pass, including the parse of the installed Live 12.4 default template.
- Decision D15: upgrade now; M09 still replaces this parser with the lossless one.

## Steps

1. In `Cargo.toml` `[workspace.dependencies]`, replace `quick-xml = { version = "0.36", features = ["serialize"] }` with `quick-xml = { version = "0.42.0", features = ["serialize"] }`.
2. Run `./.venv/Scripts/python plans/livecraft-1.0/tools/quick_xml_042_migrate.py crates/livecraft-core/src/als.rs`. Expected counts close to: 8, 16, 1, 3, 11, 12 (edition 2024 clean-ups in Task P0-08 may merge some lines; the total should be about 51).
3. Run `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings`. Expected: clean. If an error remains of the form `expected &[u8], found &str` or `&Cow<'_, str>`, add `.as_bytes()` to the `str` value the error points at; if `can't compare str with [u8; N]`, remove the `b` prefix from that byte-string literal.
4. In `SECURITY.md`, add this row at the end of the Findings table:

   ```text
   | LC-005 | Medium   | CWE-407  | Fixed  | `quick-xml` 0.36.2 (RUSTSEC-2026-0194, RUSTSEC-2026-0195) | Upgraded to 0.42.0 (Task P0-11) |
   ```

5. In `README.md`, in the Tech Stack table, change the `quick-xml`, `flate2` row's version cell from `0.36, 1.0` to `0.42, 1.0`.

## Do Not

- Change files other than: `Cargo.toml`, `Cargo.lock`, `crates/livecraft-core/src/als.rs`, `SECURITY.md`, `README.md`, `plans/livecraft-1.0/CHECKLIST.md`.
- Do not restructure the parser.

## Verify

| Command                                                                                                                                          | Expected result                               |
| ------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------- |
| `cargo tree -i quick-xml --depth 0`                                                                                                              | `quick-xml v0.42.0` (or a later 0.42 patch)   |
| `cargo xtask ci`                                                                                                                                 | `xtask: all checks passed`, 49 tests          |
| `cargo test -p livecraft-core test_als_full_roundtrip_with_devices_and_clips`                                                                    | 1 passed                                      |
| `cargo dev -- new .agent-local/output/p0-11/demo.als --bpm 128` then `cargo dev -- inspect .agent-local/output/p0-11/demo.als --devices --clips` | both exit 0; inspect lists tracks and devices |

## Commit And Push

1. Tick P0-11 in `plans/livecraft-1.0/CHECKLIST.md`.
2. `git add Cargo.toml Cargo.lock crates/livecraft-core/src/als.rs SECURITY.md README.md plans/livecraft-1.0/CHECKLIST.md`
3. Commit:

   ```text
   Upgrade quick-xml to 0.42 to fix two advisories

   - Fix RUSTSEC-2026-0194 and RUSTSEC-2026-0195 in .als parsing
   - Adapt als.rs to str-based keys, values, and names
   - Record finding LC-005
   ```

4. `git push origin main`, then `git status --short --branch`: clean, not ahead. Continue with the next line.

## If Something Fails

- Tests fail after the change: compare the failing assertion with the parsed value; log a blocker with the test name and output (do not edit tests).
- Other failures: fix within the listed files, at most 3 attempts. Still failing after the Unblock Procedure in `plans/livecraft-1.0/START-HERE.md`: discard code changes, append under Blockers in `CHECKLIST.md`: `P0-11, <date>: <what failed>`, the command, and the last 20 lines of output. Commit only `CHECKLIST.md`, push, end the run.
- `git push` rejected: `git pull --rebase`, rerun Verify, push. If the rebase conflicts: `git rebase --abort`, then the Unblock Procedure in `plans/livecraft-1.0/START-HERE.md`.
