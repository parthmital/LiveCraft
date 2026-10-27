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
