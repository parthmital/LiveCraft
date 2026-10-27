# M39: Video

Purpose: scope, decisions, and acceptance criteria for video in the Arrangement. Read when: expanding or reviewing M39.
Phase: 6. Depends on: M38. Manual: 28 Working with Video (546 to 551). Parity rows: 28.1 to 28.4.

## Goal

Video clips import into the Arrangement, show in the video window synchronised to the timeline, can be trimmed and warped as the manual describes, and export with audio.

## Decisions

- Decoding: research when the milestone starts (Rust video decoders with permissive licences, or the platform media APIs: Media Foundation on Windows, AVFoundation on macOS, a GStreamer-free path on Linux); record the choice and licence check in RESEARCH and DECISIONS. FFmpeg-based crates are acceptable only if cargo-deny accepts the licences of the build used (LGPL is compatible with GPL-3.0-or-later under D31, so no owner decision is needed; add its licence to `deny.toml` with a reason when adopted).
- The video window is an eframe viewport; frames are decoded ahead on a worker thread and shown by presentation timestamp from the engine snapshot transport position.
- Export: video export as the manual lists for the Export dialog; if encoding is not available under acceptable licences, export is listed as a gap.

## Task Outline

1. Research and decision for decoding and encoding (RESEARCH section, DECISIONS row).
2. Video clip model and import (28.1).
3. Video window and synchronisation (28.2).
4. Matching sound to video and trimming tricks (28.3, 28.4).
5. Export with video (if decided).
6. Milestone verification.

## Acceptance Criteria

- A/V offset under one frame at 60 fps during playback of the fixture video.
- Parity rows 28.x Done or recorded gaps.
