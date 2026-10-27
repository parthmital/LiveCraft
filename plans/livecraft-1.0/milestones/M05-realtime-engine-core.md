# M05: Real-time engine core

Purpose: scope, decisions, and acceptance criteria for the lock-free, allocation-free engine. Read when: expanding or reviewing M05.
Phase: 1. Depends on: M04. Manual: 38 Computer Audio Resources (982 to 988), 39 Audio Fact Sheet (989 to 996), 40 MIDI Fact Sheet (997 to 1002), 23.3 Device Delay Compensation (477 to 478). Parity rows: 23.3, 39.x, 40.x.

## Goal

The audio callback never locks, allocates, frees, logs, or does I/O; the engine runs a compiled graph swapped in without glitches; devices process blocks with sample-accurate events; neutral operations are bit-exact; plug-in-style latency is compensated.

## Decisions

- Architecture exactly as `TARGET-ARCHITECTURE.md#engine-decisions`: `GraphCompiler` (session side) builds `CompiledGraph`; `rtrb` 0.4.1 queues carry `EngineCommand` (graph swap, parameter events, transport, clip launch) to the audio thread and `EngineEvent` back; `basedrop` 0.1.3 `Shared` holds immutable data (sample buffers, note lists) read by the audio thread and freed by the collector thread; `triple_buffer` 9.0.0 publishes `EngineSnapshot { transport, track_meters, clip_states, cpu_load }`.
- `DeviceProcessor` trait from `TARGET-ARCHITECTURE.md` replaces `DspProcessor`; every existing device is ported (block processing, `ParamId` parameters, `latency_samples`).
- Clip playback reads note lists and audio through `basedrop::Shared`, never clones per block; audio file decoding happens off-thread (session or job pool) before a clip becomes playable (disk streaming for long files is M11).
- Maximum block: the driver's buffer, split internally into sub-blocks of at most 512 frames so scratch buffers are preallocated at compile time.
- PDC: compiler computes per-path latency and inserts delay lines so all tracks align at the Main track, as manual 23.3 describes; latency changes trigger a recompile.
- Neutral operations (manual 39.2): unity gain, centre pan, empty chain, disabled device, and sends at -inf must be bit-exact; tests prove each.
- Real-time checks: `assert_no_alloc` 1.1.2 as a dev-dependency and behind a `rt-checks` Cargo feature enabled in debug builds of the app; engine tests wrap every `process` call in `assert_no_alloc`.

## Interfaces

```rust
pub struct Engine { /* owned by the audio thread */ }
pub struct EngineHandle { /* session side: producers and consumers */ }
impl EngineHandle {
    pub fn submit_graph(&mut self, graph: CompiledGraph);
    pub fn send(&mut self, cmd: EngineCommand) -> Result<(), QueueFull>;
    pub fn snapshot(&mut self) -> &EngineSnapshot;
    pub fn drain_events(&mut self, out: &mut Vec<EngineEvent>);
}
impl Engine {
    pub fn process(&mut self, out: &mut [f32], channels: usize); // the only call from the driver callback
}
```

## Task Outline

1. Add `rtrb`, `basedrop`, `triple_buffer`, `assert_no_alloc` (exact versions from DECISIONS D16) and a test harness `process_checked(engine, frames)` that wraps processing in `assert_no_alloc`.
2. Define `DeviceProcessor`, `ProcessContext`, `AudioIo`, `EventList`; port Utility and EQ Three first with tests.
3. Port the remaining effects one or two per task, each with its existing tests passing.
4. Port Operator, Simpler, Drum Rack to block processing with note events at sample offsets.
5. `CompiledGraph` and `GraphCompiler` (tracks, sends, returns, groups, Main), with topological order and preallocated buffers.
6. Engine command and event queues; graph swap with transfer list; collector thread.
7. Clip players reading `basedrop::Shared` data; off-thread audio decoding before playback.
8. Parameter events and smoothing per `ParamSpec`.
9. Snapshot publishing (meters, transport, clip states, CPU load as time spent per buffer period).
10. PDC delay insertion and tests with an artificial latency device.
11. Neutral operation tests (manual 39.2 list).
12. Replace `AudioEngine` and `Arc<Mutex<AudioGraph>>`; session drives the engine through `EngineHandle`; GUI reads `EngineSnapshot`.
13. Delete the old `AudioGraph`, `DspProcessor`, `AudioParamMsg`; milestone verification and docs (ARCHITECTURE known violations rows for the engine removed).

## Acceptance Criteria

- Engine tests run 10 minutes of simulated playback of a 16-track set with device changes every 100 ms under `assert_no_alloc` with zero violations.
- `grep -rn "Mutex\|lock()\|try_lock" crates/livecraft-engine/src` finds nothing on the audio path.
- Neutral operation tests pass bit-exactly; two offline renders are byte-identical; M01 characterisation tests pass or their changed expectations are justified in the review (block processing may change last-bit rounding; difference must be below -120 dBFS).
- PDC test: a 128-sample latency device on one of two identical tracks leaves the Main output sample-aligned.
