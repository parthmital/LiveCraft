# M41: Performance: multicore, CPU and disk load, overload

Purpose: scope, decisions, and acceptance criteria for performance. Read when: expanding or reviewing M41.
Phase: 6. Depends on: M40. Manual: 38 Computer Audio Resources and Strategies (982 to 988: CPU load meter, multichannel audio, tracks and devices, track freeze, disk load, overload indicator), 18.9 Performance Impact Track Indicators (410). Parity rows: 38.1 to 38.3, 18.9.

## Goal

All budgets in `QUALITY-GATES.md#performance` are met on the reference machine; CPU and disk meters, the overload indicator, and per-track performance impact indicators behave like Live.

## Decisions

- Multicore: a fixed pool of real-time worker threads (count = physical cores minus one, setting in 2.3.3 if Live exposes it) processes independent graph branches in topological waves; workers spin briefly then park on a futex-style wait; no allocation; tracks with dependencies (sidechains, groups) are scheduled after their inputs.
- Thread priorities: audio and workers request real-time priority (MMCSS "Pro Audio" on Windows, `thread_policy` on macOS, SCHED_FIFO via RTKit on Linux when available).
- Session worker thread split from the UI thread (see `TARGET-ARCHITECTURE.md#threads`) if profiling shows UI frame time over budget.
- Measurement: `criterion` benchmarks for device processing and the 64-track benchmark set; CPU load = processing time divided by buffer period, averaged as Live describes in 38.1.1.

## Task Outline

1. Benchmark set (64 tracks) fixture and `criterion` benchmarks; baseline report.
2. Worker pool and wave scheduler with `assert_no_alloc` tests.
3. Real-time thread priorities per OS.
4. CPU meter and overload indicator (38.1.1, 38.3); disk load meter (38.2).
5. Performance impact indicators per track (18.9).
6. Profiling and optimisation tasks for any budget not met (one per hotspot, listed after the baseline).
7. Session worker split if needed.
8. Milestone verification against every budget.

## Acceptance Criteria

- Every budget in `QUALITY-GATES.md#performance` met, recorded with numbers in the review.
- 10-minute playback of the benchmark set with zero dropouts.
