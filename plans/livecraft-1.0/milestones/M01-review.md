# Milestone M01 Review: Workspace Restructure

Date: 2026-10-31. Commit: 26b544c (HEAD when verified). This review is committed on top.

| Acceptance criterion                       | Met | Evidence                                                                                                  |
| ------------------------------------------ | --- | --------------------------------------------------------------------------------------------------------- |
| `cargo xtask ci` passes                    | Yes | `xtask: all checks passed`, 55 tests, exit 0 (`cargo xtask ci` on the M01-10 tree)                        |
| Characterisation tests pass unchanged      | Yes | `cargo test -p livecraft --test characterisation`: 2 passed. Test source blob unchanged since 70076ac     |
| Target packages match target architecture  | Yes | `cargo metadata --no-deps` lists the nine packages under Package check below                              |
| No legacy crate names in Rust source files | Yes | `git grep -E` for `livecraft_core`, `livecraft_gui`, and `livecraft_cli` in `*.rs` files: no output       |
| ARCHITECTURE.md module map matches layout  | Yes | Module Map paths, Dependency Rules, Where New Code Goes, and Growth Signals match the `LAYERS` table      |

Package check (`cargo metadata --no-deps`, sorted): livecraft, livecraft-devices, livecraft-dsp, livecraft-engine, livecraft-formats, livecraft-model, livecraft-testkit, livecraft-ui, xtask.

Test counts (`cargo test --workspace`): livecraft 6 (characterisation 2, cli_tests 4), livecraft-model 9, livecraft-formats 6, livecraft-dsp 1, livecraft-devices 17, livecraft-engine 10 (unit 8, wav_limits 2), livecraft-ui 2, xtask 4, livecraft-testkit 0. Total 55.

Notes:

- The M01-10 task file's Verify grep wrote the pipes as `\|`. In extended regex that matches a literal pipe, so the command printed nothing and could not fail. The corrected pattern found three test temp-directory names (`livecraft_cli_test_1` to `_3`), which were renamed to `livecraft_app_test_1` to `_3`. See the Revision at the end of the task file.
- `cargo xtask ci` ran cargo-deny with a passing result and two warnings: `MIT-0` and `MPL-2.0` are allowed in `deny.toml` but no crate uses them. Removing them is an owner policy choice, so it is not made here.
- The known violations in ARCHITECTURE.md are Phase 1 work. They are not M01 acceptance criteria.
