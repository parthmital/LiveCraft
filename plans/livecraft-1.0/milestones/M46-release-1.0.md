# M46: Release 1.0

Purpose: scope, decisions, and acceptance criteria for the 1.0 release. Read when: expanding or reviewing M46.
Phase: 8. Depends on: M45. Manual: all. Parity rows: all.

## Goal

Prove parity and the improvements with evidence, close every open finding, and release LiveCraft 1.0.

## Decisions

- Parity audit: every parity matrix row is Done or Out of Scope with a decision ID; no Missing, Prototype, or In Progress rows remain.
- Blind tests (run by the owner from materials the executor prepares; owner-gated rows in `OWNER-QUEUE.md`):
  - Visual: 10 experienced Live users each see 30 pairs of screenshots (LiveCraft vs Live, same state, random order) and pick Live; pass if the correct-pick rate is not significantly above chance (two-sided binomial test, p greater than 0.05).
  - Audio: ABX tests on 20 oracle renders (devices and mixes); pass if listeners cannot distinguish at p greater than 0.05.
  - Workflow: the same users complete 10 scripted tasks in both apps; pass if no task fails in LiveCraft and median time is within 20 percent of Live's.
- D-OWN-3 must be answered with legal advice recorded (queue row Q01) before the public `v1.0.0` tag; D-OWN-1, 2, 4, 5, 6 may stand at their defaults.
- Docs: `README.md`, `ARCHITECTURE.md`, `DESIGN.md`, `SECURITY.md`, `RESEARCH.md`, `CHANGELOG.md`, user documentation for every feature (paraphrased, never copied from the manual).

## Task Outline

1. Parity audit script (`cargo xtask parity`) that fails on any non-final row; fix list for remaining rows (each becomes a task).
2. Full security audit rerun (P0-17 procedure plus server, plug-in host, and scripting reviews).
3. Performance audit rerun (M41 budgets).
4. Blind test kit: screenshot pairs, ABX render sets, scripted workflow tasks, a results form, and `tools/blind_test_stats.py` (binomial and ABX analysis with unit tests). Queue the test sessions; analysis steps are Owner-gated.
5. User documentation set.
6. Release candidate: version `1.0.0-rc.1`, tag `v1.0.0-rc.1`, release workflow builds a draft (not published) release with every package and SHA256SUMS.
7. Version 1.0.0 in `[workspace.package]`, CHANGELOG, tag `v1.0.0`, release workflow run, release published: Owner-gated on Q01 and the blind test rows, finished by the Final reference pass.
8. Post-release review: lessons learned and the next plan's starting point.

## Acceptance Criteria

- `cargo xtask parity` passes; `cargo xtask ci` passes; release artifacts published.
- Blind test results meet the pass conditions above, recorded in the review.
- No open security findings above Low.
