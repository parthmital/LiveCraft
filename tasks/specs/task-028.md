# Task Specification: task-028

## 1. Metadata

- **ID:** `task-028`
- **Phase:** Phase 5: Headless Workflows, Testing, CI & Documentation
- **Title:** Authoritative Documentation Suite & Manual Parity Audit
- **Dependencies:** `task-027`
- **Manual Grounding:** Ableton Live 12 Complete System Specification & LiveCraft Master Roadmap

## 2. Context & Problem Statement

The final task finalizes and synchronizes all documentation in the repository (`README.md`, `ROADMAP.md`, `docs/ARCHITECTURE.md`, `docs/ABLETON_12_SPECIFICATION.md`) with the actual, verified codebase. Stale documentation misleads users and contributors. All implemented features, architecture changes, CLI commands, and keyboard shortcuts must be verified and documented.

## 3. Scope of Work

1. Update `ROADMAP.md`:
   - Mark all completed tasks and milestones across Phases 0, 1, 2, 3, 4, and 5.
   - Record audited metrics (test count, bounce speed factor, code coverage).
2. Update `README.md`:
   - Verify all CLI examples, GUI shortcut lists, device lists, and installation instructions.
3. Update `docs/ARCHITECTURE.md`:
   - Document the lock-free parameter bus, modular `DspProcessor` chain, and Stacked View layout.
4. Perform final manual parity audit confirming full alignment with `docs/ableton-live-12-reference-manual.pdf`.

## 4. Files to Inspect

- `README.md`
- `ROADMAP.md`
- `docs/ARCHITECTURE.md`

## 5. Files to Modify

- `README.md`
- `ROADMAP.md`
- `docs/ARCHITECTURE.md`

## 6. What NOT to Touch

- Do NOT touch code files.

## 7. Step-by-Step Implementation Instructions

1. In `ROADMAP.md`:
   - Update checklist items to reflecting passing features.
   - Update the Gap Analysis and Audit Verdict table.
2. In `README.md`:
   - Ensure all CLI subcommands (`info`, `inspect`, `new`, `render`, `stems`, `play`, `synth`, `convert`, `gui`) have up-to-date syntax examples.
3. In `docs/ARCHITECTURE.md`:
   - Ensure diagrams and crate descriptions reflect the verified modular architecture.
4. Run full workspace test suite.

## 8. Verification Command

```bash
cargo test --workspace
```

## 9. Acceptance Criteria

- [ ] Documentation accurately reflects all verified codebase capabilities.
- [ ] No stale or broken links exist.
- [ ] `cargo test --workspace` passes with 100% success.

## 10. Failure Handling Protocol

If documentation contains contradictory claims:

- Verify against the running binary and codebase before documenting. Never invent unverified behavior.
