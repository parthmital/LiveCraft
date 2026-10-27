# M43: Reliability: autosave, version history, crash recovery, output safety

Purpose: scope, decisions, and acceptance criteria for reliability features. Read when: expanding or reviewing M43.
Phase: 7. Depends on: M42. Manual: 5.4 Live Sets (backups described on pages 114 to 115). Beyond: B06, B07.

## Goal

Work is never lost: continuous autosave, a browsable version history, recovery after a crash, Live-compatible Backup folder behaviour, and a monitoring-only safety limiter.

## Decisions

- Autosave: every 60 seconds of idle after a change (setting), written atomically (write temp file, fsync, rename) into `<project>/Backup/LiveCraft Autosave/` with timestamps; Live's own Backup behaviour (last ten saved versions, manual page 115) is kept identical.
- Version history: a list of autosaves and saves with time, size, and a musical diff summary from M09 `diff`; restore opens a copy, never overwrites.
- Crash recovery: a session lock file; on next start, if the lock is stale, offer to recover the newest autosave (like Live's recovery prompt, wording LiveCraft's own).
- Panic handling: a panic hook writes a crash report (backtrace, versions, last 200 log lines, no personal data) into the platform data directory; no upload.
- Safety limiter (B07): a brick-wall limiter on the hardware output path only, at -0.1 dBFS by default, with a setting to disable; never applied to renders; a test proves renders are unaffected.

## Task Outline

1. Atomic save and Live-identical Backup folder behaviour.
2. Autosave scheduler and storage with pruning by disk budget.
3. Version history UI and restore.
4. Lock file and crash recovery flow.
5. Panic hook and crash reports.
6. Output safety limiter on the driver path with tests.
7. Milestone verification (fault injection tests: kill during save leaves the previous file intact).

## Acceptance Criteria

- Fault injection test passes 100 out of 100 runs.
- Renders are bit-identical with the safety limiter on and off.
- Beyond rows B06, B07 Done with evidence.
