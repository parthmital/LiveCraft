# M11: Audio files, sample cache, disk streaming

Purpose: scope, decisions, and acceptance criteria for audio file support. Read when: expanding or reviewing M11.
Phase: 2. Depends on: M10. Manual: 5.1 Sample Files (122 to 132), 3.9 Audio Clips and Samples (46), 38.2 Managing the Disk Load (987), 8.12 Replacing and Editing the Sample (231). Parity rows: 5.1, 3.9, 38.2.

## Goal

Every format Live reads is decoded off the audio thread; long files stream from disk; waveform overviews and analysis data are cached; RAM mode and disk mode match the manual.

## Decisions

- Decoding with `symphonia` 0.6.1 (WAV, AIFF, FLAC, MP3, AAC/M4A, Ogg Vorbis, ALAC), resampling with `rubato` 5.0.1 only when needed for playback at a different rate.
- Two playback sources: `RamSample` (fully decoded, `basedrop::Shared<[f32]>` per channel) and `StreamedSample` (disk streamer thread fills per-clip ring buffers ahead of the playhead; clip launch preloads the first 1 second).
- Clip RAM mode switch (manual 8.x) chooses the source.
- Analysis cache: LiveCraft-owned `.lcanalysis` files beside the sample or in a cache folder (setting), holding waveform overview levels and later warp data; Live's `.asd` files are read only if their format is documented in the manual, otherwise ignored and preserved.
- Sample file limits: the 2^28-sample RAM limit from P0-15 applies to RAM mode only; streaming has no length limit.
- Decoder errors never crash: a clip with an unreadable file shows as missing (manual 5.6 behaviour).

## Task Outline

1. Replace the WAV-only loader with a `symphonia` decoder module and format tests (one fixture per format, generated in-house).
2. Off-thread decode job queue feeding the engine via `basedrop`.
3. Disk streamer thread and per-clip ring buffers; underrun counter exposed as disk load.
4. RAM mode toggle and preload behaviour.
5. Waveform overview pyramid (min, max per 256, 4096, 65536 samples) and `.lcanalysis` cache.
6. Sample-rate conversion for playback with `rubato`.
7. Replace and edit sample (manual 8.12) commands.
8. Milestone verification: 1-hour 96 kHz stereo file plays with zero underruns on the reference machine.

## Acceptance Criteria

- Every decoder fixture decodes to the expected sample values (tolerance per codec: lossless exact, lossy within -60 dB of the source).
- `assert_no_alloc` engine test with streaming clips passes.
- Disk load shown in the CPU and disk meters (M41 adds the UI).

## Review (2026-11-10)

All M11 Outline items (1 to 8) and all three Acceptance Criteria were completed locally, verified on the reference machine (including the 1-hour 96 kHz stereo zero-underrun streaming check under `rt-checks`), and committed together per milestone policy.

| Item | Summary |
| ---- | ------- |
| 1 | Enabled `symphonia` 0.6.1 `isomp4`, `aac`, `alac`, `ogg`, `vorbis`, `wav`, `aiff`, `flac`, `mp3`, `pcm` in `Cargo.toml`; added `crates/livecraft-formats/src/audio_decode.rs` (`probe_audio_info`, `decode_audio_file`, `decode_audio_file_with_limit`, seekable `AudioStreamReader`), updated `health.rs` & `engine/src/audio_clip.rs`, generated 7 in-house `fixtures/audio/sine_440.*` fixtures, and verified all 7 in `m11_audio_decode.rs` plus arbitrary-bytes `proptest`. |
| 2 | Added `DecodeQueue` (`crates/livecraft-engine/src/decode_queue.rs`) decoding & resampling RAM-mode files off-thread into `basedrop::Shared<DecodedAudio>`, integrated with `GraphCompiler`, `Playback::poll_decoded`, and `Session::poll_background`. |
| 3 | Added lock-free `DiskStreamer` (`crates/livecraft-engine/src/streamer.rs`) supporting `Async` real-time thread and deterministic `Manual` `pump_sync` (`VirtualDriver`/`OfflineBouncer`), `StreamAudioPlayer` with zero audio-thread allocations/locks, and `disk_load`/`disk_underruns` in `EngineSnapshot` / `EngineStatus` / Control Bar `"Disk"` pill (`transport.rs`). |
| 4 | Added `AudioClipData.ram_mode` (`clip.rs`), `<Ram>` lossless `.als` mapping (`audio_clips.rs`), `SetClipRamMode` (`"clip.set_ram_mode"`), 1-second synchronous preload in `DiskStreamer`, `MAX_RAM_AUDIO_SAMPLES` limit enforcement in RAM mode only, and `RAM` toggle in Audio Clip View (`pianoroll.rs`). |
| 5 | Added `crates/livecraft-formats/src/analysis_cache.rs` computing 3-level `[256, 4096, 65536]` min/max `WaveformPyramid` in one streaming pass and `.lcanalysis` sidecar/folder cache (`LibraryPreferences`), preserving Live `.asd` files untouched; added `livecraft analyze` CLI and dynamic zoom waveform rendering in `pianoroll.rs`. |
| 6 | Added `crates/livecraft-engine/src/resample.rs` (`resample_decoded_audio` and `StreamingResampler` via `rubato`) for playback sample-rate conversion when file rate != engine rate, with exact pass-through when rates match. |
| 7 | Added `ReplaceClipSample` (`"clip.replace_sample"`), `Session::replace_clip_sample` (retaining Warp Markers only when frame count matches per manual 8.12), `Session::refresh_clip_sample`, `Replace Sample...` dialog (`file_manager.rs`), `Reload` button (`pianoroll.rs`), and audio drag-and-drop (`app.rs`). |
| 8 | Verified `one_hour_96khz_stereo_plays_with_zero_underruns` (`crates/livecraft-engine/tests/m11_streaming.rs`) in `--release --no-default-features --features rt-checks` (`assert_no_alloc`): 1 passed, 0 failed in 79.28s with zero underruns. |

### Manual quotes

From `docs/ableton-live-12-reference-manual.pdf`:

- 5.1, p. 122: "Live natively supports mono and stereo sample files in the following formats: WAV, AIFF, AIFF-C, FLAC, and OGG Vorbis. It is also possible to import MP3 and M4A (on Mac only) files into Live."
- 5.1.2, p. 126: "If you do not want an .asd file to be created when you load a sample, you can set the Create Analysis Files option in the File & Folder Settings to Off."
- 8.4.5, p. 219: "When you activate the RAM Mode toggle, Live loads the audio referenced by the clip into the computer's memory rather than reading it from disk in real time."
- 8.12, p. 231: "When you replace the sample, clip settings like pitch and volume will be retained. The Warp Markers will be retained only if the new sample has the exact same length as the old sample."
- 38.2, p. 988: "Running audio clips in RAM Mode can also help reduce the disk load."
- 38.3, p. 988: "If the disk cannot read or write audio quickly enough, the indicator displays \"Disk.\""

### Evidence

- `cargo xtask ci` (fmt, clippy `-D warnings`, workspace tests, release build, layering check, docs link check, `cargo-deny`): 648 passed, 0 failed, 2 ignored (`ten_minutes_of_playback_allocate_nothing`, `one_hour_96khz_stereo_plays_with_zero_underruns`).
- Lossless decoder measurements (`m11_audio_decode.rs` on `fixtures/audio/sine_440.{wav,aif,flac}` and `sine_440_alac.m4a`): max diff vs WAV 0.000000000 (`0.000038804` vs ideal sine).
- Lossy decoder measurements (`m11_audio_decode.rs`): MP3 `-100.95 dBFS` RMS residual (`lag=0`), Ogg Vorbis `-65.52 dBFS` RMS residual (`lag=0`), AAC/M4A `-62.42 dBFS` RMS residual (`lag=1024`).
- `cargo test --release -p livecraft-engine --no-default-features --features rt-checks --test m11_streaming one_hour_96khz_stereo_plays_with_zero_underruns -- --ignored --nocapture`: passed in 79.28s with zero allocations and zero underruns.

### Criteria

| Criterion | Met | Evidence |
| --------- | --- | -------- |
| Every decoder fixture decodes to the expected sample values (tolerance per codec: lossless exact, lossy within -60 dB of the source) | Yes | `crates/livecraft-formats/tests/m11_audio_decode.rs` across all 7 `fixtures/audio/sine_440.*` fixtures |
| `assert_no_alloc` engine test with streaming clips passes | Yes | `crates/livecraft-engine/tests/m11_streaming.rs` (`assert_no_alloc` debug & release `rt-checks` runs) |
| Disk load shown in the CPU and disk meters (M41 adds the UI) | Yes | `EngineSnapshot.{disk_load, disk_underruns}`, `EngineStatus`, Control Bar `"Disk"` overload pill (`transport.rs`, `m11_ui.rs`) |
