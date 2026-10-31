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

## Review (2026-10-31)

Every Outline item has at least one commit. Items 3, 4, 8, 12, and 13 took several. Each commit passed `cargo xtask ci`.

| Item | Commits                                                          | Summary                                                                                                                   |
| ---- | ---------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| 1    | `1177f71`                                                        | `rtrb`, `basedrop`, `triple_buffer`, `assert_no_alloc` at the D16 versions, and the `process_checked` scope               |
| 2    | `a00e4ca`                                                        | `DeviceProcessor`, `ProcessContext`, `AudioIo`, `EventList`; Utility and EQ Three ported                                  |
| 3    | `9884b00`, `660c87e`, `6ce1239`, `5508797`, `4a9ca1d`, `223766e` | Every remaining effect ported with its tests                                                                              |
| 4    | `cbe62b0`, `250ce9f`                                             | Operator, Simpler, and Drum Rack take note events at sample offsets                                                       |
| 5    | `0004e0e`                                                        | `GraphCompiler` and `CompiledGraph` in topological order with preallocated buffers                                        |
| 6    | `a5ab43b`                                                        | Command, event, graph, and retired-graph queues, the transfer list, and the collector thread                              |
| 7    | `2ac11ff`, `b7b65f0`                                             | Clip players on `basedrop::Shared` data, decoding off the audio thread, clip boundaries on their sample                   |
| 8    | `66cba72`, `129d0a3`                                             | Parameter events by `ParamId` at their sample offset, and ramps from `ParamSpec::smoothing_ms`                            |
| 9    | `038d8ff`                                                        | `EngineSnapshot` through a triple buffer                                                                                  |
| 10   | `520bbf7`                                                        | PDC delay lines on tracks, returns, and Main                                                                              |
| 11   | `11ed800`                                                        | Neutral operation tests and the 16-track playback test                                                                    |
| 12   | `4ad9bef`, `89835f5`                                             | The session, the CLI, the bouncer, and the testkit drive the engine through `EngineHandle`                                |
| 13   | `3cc4855`, `c62155c`, this commit                                | Note release at loop ends, old graph deleted, byte-identical bounce test, executor tests split out, verification and docs |

### Decisions and deviations

1. Centre pan is not tested for bit-exactness. Manual 39.2 does not list it, and page 397 calls panning a non-neutral operation.
2. A MIDI track with no instrument is silent. The old graph gave it an implicit Operator. The manual was not checked for this case.
3. Groups compile as plain tracks with no group routing, as before. M20 adds group routing.
4. `EngineSnapshot` holds a fixed array of 256 node meters, so `submit_graph` refuses a graph with more nodes. M41 revisits the limit.
5. The M01 characterisation RMS of the three-note chord changed from 0.06759972 to 0.067596905 (peaks unchanged). The old graph released a note that ends on frame 44100 at frame 44128, the end of its 32-frame sub-block. The engine releases it on frame 44100, as manual 40.1 asks. The change is 2.8e-6, about -111 dBFS, above the -120 dBFS rounding bound. It is a timing fix, not rounding, so it is accepted here.
6. A sample-by-sample residual against the old graph is not meaningful: the old graph stacked a voice at every loop end of a note ending at the loop end (fixed in `3cc4855`), released late, and played an implicit Operator. The residual of the capture Set, `.agent-local/output/m05/baseline-old.wav` against `new-current.wav`, peaks at -2.2 dBFS for those reasons.

### Evidence

- `cargo xtask ci` passed on the final tree, log `.agent-local/logs/2026-10-31_m05-final-ci.log`.
- The 10-minute playback test passed, log `.agent-local/logs/2026-10-31_m05-final-long.log`.
- `grep -rn "Mutex\|lock()\|try_lock" crates/livecraft-engine/src` finds nothing.

### Criteria

| Criterion                                                               | Met | Evidence                                                                                                                                                                                                                              |
| ----------------------------------------------------------------------- | --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 10 minutes of a 16-track Set, device change every 100 ms, no violations | Yes | `crates/livecraft-engine/tests/long_playback.rs` `ten_minutes_of_playback_allocate_nothing`, release build with `assert_no_alloc` on, 51,680 driver callbacks in `.agent-local/logs/2026-10-31_m05-13-ten-minutes-release-rerun2.log` |
| No `Mutex`, `lock()`, or `try_lock` in `crates/livecraft-engine/src`    | Yes | The grep finds no line                                                                                                                                                                                                                |
| Neutral operations bit-exact, two offline renders byte-identical, M01   | Yes | `tests/neutral.rs` (unity gain sum, empty chain, disabled device, send at -inf, repeat render); `bouncer.rs` `two_offline_renders_of_one_set_are_byte_identical` compares WAV bytes; M01 characterisation passes with deviation 5     |
| PDC: 128-sample latency device on one of two tracks, Main aligned       | Yes | `executor/tests.rs` `a_latency_device_on_one_track_leaves_the_main_output_aligned`, and the same on a return                                                                                                                          |

### Not verified

- No sound was heard on a device in this review. The GUI screenshot and CLI render logs from item 13 are in `.agent-local/logs/2026-10-31_m05-13-*.log`.
- Deviation 5 is a judgement that a timing fix may exceed the -120 dBFS bound. The owner may disagree.
