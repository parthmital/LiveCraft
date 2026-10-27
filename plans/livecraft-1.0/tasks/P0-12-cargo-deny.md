# Task P0-12: Add cargo-deny to `cargo xtask ci`

Depends on: P0-11. Size: 6 files, about 70 changed lines. Milestone: P0.

## Goal

`cargo xtask ci` also runs `cargo-deny` (advisories, licences, bans, sources) with a committed `deny.toml`, using a repo-local cargo-deny 0.20.2 in `./.tools/`; `cargo xtask ci --no-deny` skips it (CI runs deny in its own job).

## Before You Start

1. `git pull --ff-only`, then `git status --short`: empty.
2. In `plans/livecraft-1.0/CHECKLIST.md`, P0-11 is `[x]` or `[~]` and P0-12 is `[ ]`.
3. Run `cargo xtask ci`. Expected: `xtask: all checks passed`, 49 tests.

## Context

- Decision D13: licence allow-list; D14: GPL components stay behind opt-in features. `plans/livecraft-1.0/files/P0-12/deny.toml` is ready. It was run on 2026-10-27 against the dependency tree as it will be after Tasks P0-03 to P0-11: advisories ok, sources ok, licences ok (the workspace crates need the `license` field from Task P0-01), bans ok with `publish = false`.
- The single licence exception is egui's bundled font (`epaint_default_fonts`, `Ubuntu-font-1.0`), which permits redistribution; M13 removes it.
- `./.tools/` is Git-ignored (Task P0-01). Installing builds cargo-deny from source (several minutes).
- xtask today (`xtask/src/main.rs`) has `fn ci() -> Result {` which runs fmt, clippy, test, build, `layering()?;`, `docs()?;` and then prints `xtask: all checks passed`.

## Steps

1. Install cargo-deny: `cargo install cargo-deny@0.20.2 --locked --root .tools`. Expected: ends with `Installed package cargo-deny v0.20.2`. Check: `./.tools/bin/cargo-deny --version` prints `cargo-deny 0.20.2`.

2. Copy the config: `cp plans/livecraft-1.0/files/P0-12/deny.toml deny.toml`.

3. Run `./.tools/bin/cargo-deny check`. Expected: `advisories ok, bans ok, licenses ok, sources ok` (warnings about licences "not encountered" are fine). If an advisory appears that was not there on 2026-10-27, do not ignore it: run `cargo update -p <crate>`; if that does not fix it, log a blocker with the advisory ID.

4. In `xtask/src/main.rs`, find:

   ```rust
           Some("ci") => ci(),
   ```

   Replace it with:

   ```rust
           Some("ci") => ci(std::env::args().any(|arg| arg == "--no-deny")),
   ```

5. In the same file, find:

   ```rust
           _ => Err("usage: cargo xtask <ci|layering|docs>".to_string()),
   ```

   Replace it with:

   ```rust
           _ => Err("usage: cargo xtask <ci [--no-deny]|layering|docs>".to_string()),
   ```

6. In the same file, find `fn ci() -> Result {` and replace it with `fn ci(skip_deny: bool) -> Result {`.

7. In the same file, find:

   ```rust
       docs()?;
       println!("xtask: all checks passed");
   ```

   Replace it with:

   ```rust
       docs()?;
       if skip_deny {
           println!("xtask: skipping cargo-deny (--no-deny)");
       } else {
           deny()?;
       }
       println!("xtask: all checks passed");
   ```

8. In the same file, find:

   ```rust
   #[cfg(test)]
   mod tests {
   ```

   Replace it with:

   ```rust
   /// Runs `cargo-deny check` with the repo-local binary in `./.tools/bin` and `deny.toml`.
   fn deny() -> Result {
       let exe = std::env::current_dir()
           .map_err(|e| format!("no current directory: {e}"))?
           .join(".tools")
           .join("bin")
           .join(if cfg!(windows) {
               "cargo-deny.exe"
           } else {
               "cargo-deny"
           });
       if !exe.exists() {
           return Err(format!(
               "{} is missing; install it with `cargo install cargo-deny@0.20.2 --locked --root .tools`, or run `cargo xtask ci --no-deny`",
               exe.display()
           ));
       }
       println!("xtask: cargo-deny check");
       let status = Command::new(&exe)
           .arg("check")
           .status()
           .map_err(|e| format!("could not run cargo-deny: {e}"))?;
       if status.success() {
           Ok(())
       } else {
           Err("cargo-deny check failed".to_string())
       }
   }

   #[cfg(test)]
   mod tests {
   ```

9. Run `cargo fmt --all`.

10. In `AGENTS.md`, find:

    ```text
    - All checks in one command: `cargo xtask ci` (format check, clippy, tests, release build, crate layering, Markdown links). Run it before every commit.
    ```

    Replace it with:

    ```text
    - All checks in one command: `cargo xtask ci` (format check, clippy, tests, release build, crate layering, Markdown links, cargo-deny). Run it before every commit.
    - First-time setup for cargo-deny: `cargo install cargo-deny@0.20.2 --locked --root .tools` (repo-local, Git-ignored). `cargo xtask ci --no-deny` skips it.
    - Dependency licence policy: `deny.toml`; never add a licence or advisory exception without a comment giving the reason.
    ```

11. In `SECURITY.md`, find:

    ```text
    - Planned in `plans/livecraft-1.0/` Phase 0: `cargo-deny` for advisories, licences, and sources; input size limits; `unsafe_code = "forbid"`; output safety limiter.
    ```

    Replace it with:

    ```text
    - `cargo-deny` (`deny.toml`) checks advisories, licences, bans, and sources in `cargo xtask ci`.
    - Planned in `plans/livecraft-1.0/` Phase 0: input size limits and a non-finite output guard.
    ```

    And in the Findings table, change the LC-003 row's Status from `Open` to `Fixed` and its Fix cell to `cargo-deny in cargo xtask ci (Task P0-12)`.

## Do Not

- Change files other than: `deny.toml` (new), `xtask/src/main.rs`, `AGENTS.md`, `SECURITY.md`, `plans/livecraft-1.0/CHECKLIST.md`.
- Do not commit `./.tools/`. Do not add `ignore` entries to `deny.toml`.

## Verify

| Command                    | Expected result                                                                  |
| -------------------------- | -------------------------------------------------------------------------------- |
| `cargo xtask ci`           | prints `xtask: cargo-deny check`, then `xtask: all checks passed`                |
| `cargo xtask ci --no-deny` | prints `xtask: skipping cargo-deny (--no-deny)`, then `xtask: all checks passed` |
| `git status --short`       | no `.tools` entry                                                                |

## Commit And Push

1. Tick P0-12 in `plans/livecraft-1.0/CHECKLIST.md`.
2. `git add deny.toml xtask/src/main.rs AGENTS.md SECURITY.md plans/livecraft-1.0/CHECKLIST.md`
3. Commit:

   ```text
   Add cargo-deny to cargo xtask ci

   - Add deny.toml with the licence allow-list and source rules
   - Run the repo-local cargo-deny 0.20.2 from cargo xtask ci, skippable with --no-deny
   - Document setup and mark finding LC-003 fixed
   ```

4. `git push origin main`, then `git status --short --branch`: clean, not ahead. Continue with the next line.

## If Something Fails

- A licence is rejected that is not in the allow-list: if it is OSI-approved and permissive (for example `CDLA-Permissive-2.0`, `Zlib`, `BSL-1.0`), add it to the `allow` list in `deny.toml` with a comment naming the crate, and append an `OWNER-QUEUE.md` row "Approve licence <licence> for <crate>" (Status Open, Needed by line none); otherwise (copyleft or unknown) follow the Unblock Procedure in `plans/livecraft-1.0/START-HERE.md`.
- Other failures: fix within the listed files, at most 3 attempts. Still failing after the Unblock Procedure in `plans/livecraft-1.0/START-HERE.md`: append under Blockers in `CHECKLIST.md`: `P0-12, <date>: <what failed>`, the command, and the last 20 lines of output. Commit only `CHECKLIST.md`, push, end the run.
- `git push` rejected: `git pull --rebase`, rerun Verify, push. If the rebase conflicts: `git rebase --abort`, then the Unblock Procedure in `plans/livecraft-1.0/START-HERE.md`.
