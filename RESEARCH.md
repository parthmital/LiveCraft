# Research

Purpose: dated evidence behind technology and design decisions. Read when: choosing or revisiting a library, service, model, dataset, or approach.

Earlier undated research (cpal, hound, egui, flate2, quick-xml, clap) is superseded by the sections below. Versions were read from crates.io with `cargo search` and `cargo info` on 2026-10-27.

## Target Version (2026-10-27)

**Brief:** which Live release defines parity.
**Findings:** Latest stable is Live 12.4.7 (2026-10-22); 12.4 shipped 2026-05-05 with Link Audio, Erosion and Chorus-Ensemble and Delay updates, Stem Separation on time selections, and Learn View replacing Help View. The local manual (`docs/ableton-live-12-reference-manual.pdf`, 1039 pages) documents Learn View and Link Audio, so it is the 12.4 edition. The owner's machine has Live 12 Suite 12.4 installed (`C:\ProgramData\Ableton\Live 12 Suite`).
**Decision:** Parity target is Live 12.4 Suite as documented by the local manual. Point releases after 12.4.7 are tracked in the plan's parity matrix as they appear.
**Sources:** [Live 12 release notes](https://www.ableton.com/en/release-notes/live-12/), [Live 12.4 is out now](https://www.ableton.com/en/blog/live-12-4-is-out-now/).

## Reference Projects (2026-10-27)

**Brief:** learn from the Storytold "Craft" apps that inspire this project.
**Findings:** `storytold/filmcraft` is a clean-room Rust reimplementation of Premiere Pro, MIT OR Apache-2.0, with a layered workspace (foundations, engine with command registry, session, undo, jobs; `ui-egui` front end; CLI; MCP automation; `testkit` and `golden` crates), a `cargo xtask ci` that enforces layering, `AGENTS.md` with clean-room and asset rules, golden-image tests, oracle tests against ffmpeg, and a `docs/target-app-parity.md` tracker. Every front end dispatches the same engine commands (650+). Meadowlark, a Rust DAW, paused after its author found the scope too large for one person, which argues for strict sequencing and reuse.
**Decision:** Borrow FilmCraft's structure: layered crates with an enforced dependency check, one command registry shared by GUI, CLI, and a control server, golden tests, and a parity tracker. Build the LiveCraft specifics.
**Sources:** [storytold/filmcraft](https://github.com/storytold/filmcraft), [Meadowlark](https://github.com/fwcd/meadowlark), [Why I'm taking a break from Meadowlark](https://billydm.github.io/blog/why-im-taking-a-break-from-meadowlark).

## GUI Toolkit (2026-10-27)

**Brief:** pixel-faithful recreation of Live's custom-drawn UI, native, testable headlessly.
**Findings:** `egui` 0.37.0 is current (repo pins 0.29). Breaking changes since 0.29 include `Rounding` renamed to `CornerRadius` with integer fields (0.31), `StrokeKind` required for rect strokes (0.31), `Panel` replacing `SidePanel` and `TopBottomPanel` (0.34 to 0.35), deprecated items removed (0.35), `ViewportCommand::Screenshot` taking a callback and `default_fonts` no longer default (0.37). `egui_kittest` 0.37.0 (MIT OR Apache-2.0, MSRV 1.95) drives egui through AccessKit and renders snapshot images with the `wgpu` feature. Vizia and Iced are viable but would mean a rewrite; Yarrow (Meadowlark's author) is not mature.
**Trial (2026-10-27, throwaway worktree):** the 0.29 to 0.37 port needed about 230 mechanical renames plus a new `App::ui` entry point and callback screenshots; afterwards clippy was clean and all tests passed. With wgpu on the reference Windows machine the Vulkan backend crashed (segmentation fault after loader warnings) while DX12 and GL rendered correctly. eframe's `accesskit` feature pulled in `zvariant`, `zbus_names`, `zvariant_derive`, flagged unmaintained (RUSTSEC-2026-0370 to 0372).
**Decision:** Adapt. Keep `egui` and `eframe`, upgrade to 0.37.0, switch the renderer from glow to wgpu with DX12 as the Windows default, keep `accesskit` off until M40, and use `egui_kittest` for interaction and snapshot tests. Runner-up: Vizia.
**Sources:** [egui CHANGELOG](https://github.com/emilk/egui/blob/main/CHANGELOG.md), [egui_kittest docs](https://docs.rs/egui_kittest).

## Audio I/O (2026-10-27)

**Brief:** low-latency output and input on Windows, macOS, Linux.
**Findings:** `cpal` 0.18.2 is current (repo pins 0.15). Its `asio` feature needs the Steinberg ASIO SDK, which is GPLv3 or proprietary since October 2025. WASAPI, CoreAudio, ALSA, and JACK need no extra SDK.
**Decision:** Adopt `cpal` 0.18. ASIO only behind an opt-in Cargo feature `asio`, off by default, because enabling it makes distributed binaries GPLv3. Owner decided on 2026-10-28 that release builds enable it (DECISIONS D31).
**Sources:** [cpal docs](https://docs.rs/cpal), [VST 3 and ASIO licensing press release](https://sonicstate.com/news/2025/10/30/vst-3-now-available-under-mit-license).

## Real-Time Concurrency (2026-10-27)

**Brief:** audio thread must never lock, allocate, or free.
**Findings:** `rtrb` 0.4.1 (MIT OR Apache-2.0) is a wait-free SPSC ring buffer. `basedrop` 0.1.3 (MIT/Apache-2.0) defers frees to a collector thread. `triple_buffer` 9.0.0 (MPL-2.0) publishes latest-value snapshots. `assert_no_alloc` 1.1.2 (BSD-1-Clause, last release 2021, small and stable) aborts on allocation inside a guarded scope in debug builds. Common pattern: build graphs off-thread, send them over `rtrb`, return the old one for freeing elsewhere.
**Decision:** Adopt all four: `rtrb` for commands and events, `basedrop` for shared immutable data such as samples and graph snapshots, `triple_buffer` for meters and transport position, `assert_no_alloc` in debug and test builds around the callback. Runner-up: hand-written lock-free queues (rejected, more risk).
**Sources:** [rtrb](https://docs.rs/rtrb), [basedrop write-up](https://glowcoil.com/posts/basedrop), [assert_no_alloc](https://docs.rs/assert_no_alloc).

## Live Set Format (2026-10-27)

**Brief:** open and save `.als` without losing anything Live wrote.
**Findings:** A `.als` is gzip-compressed XML. Live 12.4's default template has 484 distinct element names and about 180 KB of XML, far more than any model can map. Open-source readers exist (`ableton` npm, `pyableton`, `daw-schemas` JSON schemas inferred from sample sets, `bravoh-daw` in Rust) but none preserve unknown content. Header of a 12.4 template: `MajorVersion="5" MinorVersion="12.0_12117" SchemaChangeCount="10"`.
**Decision:** Build a lossless approach: parse into a generic XML tree that keeps every node, attribute, and order; map the model onto that tree; write by patching the tree. Borrow element knowledge from `daw-schemas`. Upgrade `quick-xml` to 0.42.0.
**Sources:** [daw-schemas](https://pypi.org/project/daw-schemas/), [bravoh-daw](https://docs.rs/crate/bravoh-daw/0.1.0), [ableton npm](https://npmjs.com/package/ableton).

## Audio Files, Resampling, Time-Stretch (2026-10-27)

**Brief:** decode common formats, convert sample rates, and warp like Live's six warp modes.
**Findings:** `symphonia` 0.6.1 (MPL-2.0, MSRV 1.85) decodes WAV, AIFF, FLAC, MP3, AAC, ALAC, Vorbis, and more. `rubato` 5.0.1 (MIT OR Apache-2.0) does real-time-safe sinc and FFT resampling. `signalsmith-stretch` 0.1.3 (MIT) wraps Signalsmith Stretch, a polyphonic phase-vocoder time-stretch and pitch-shift. `realfft` 3.5.1 and `rustfft` 6.4.1 cover FFT needs. `hound` 3.5.1 stays for WAV writing.
**Decision:** Adopt `symphonia` for decoding (MPL-2.0 is file-level copyleft, compatible with distributing an MIT or Apache binary when its files are unmodified), `rubato` for resampling, `signalsmith-stretch` as the basis for Complex and Complex Pro. Build Beats, Tones, Texture, and Re-Pitch from the manual's descriptions.
**Sources:** [symphonia](https://docs.rs/symphonia), [rubato](https://docs.rs/rubato), [signalsmith-stretch](https://docs.rs/signalsmith-stretch).

## Plug-In Hosting (2026-10-27)

**Brief:** host third-party instruments and effects as Live does (VST2, VST3, AU) and better.
**Findings:** `clack-host` 0.2.0 (MIT OR Apache-2.0, MSRV 1.85) is the only functional safe Rust CLAP host library. The VST3 SDK is MIT since version 3.8 (2025-10-29); `vst3` 0.3.0 (MIT OR Apache-2.0) provides Rust bindings. The VST2 SDK is no longer licensable.
**Decision:** Adopt `clack-host` for CLAP and `vst3` bindings for VST3, both in an out-of-process host so a crashing plug-in cannot take down the audio engine (Live hosts in-process). AU on macOS later. No VST2.
**Sources:** [clack](https://github.com/prokopyl/clack), [vst3-rs](https://github.com/coupler-rs/vst3-rs), [VST 3 license](https://steinbergmedia.github.io/vst3_dev_portal/pages/VST+3+Licensing/VST3+License).

## MIDI (2026-10-27)

**Findings:** `midir` 0.11.1 (MIT) provides MIDI I/O on WinMM, CoreMIDI, ALSA, and optional JACK and WinRT.
**Decision:** Adopt `midir`.
**Sources:** [midir](https://docs.rs/midir).

## Sync and Ableton Link (2026-10-27)

**Brief:** Live syncs via Link, Link Audio, Tempo Follower, and MIDI clock.
**Findings:** Ableton Link is GPL-2.0-or-later, or available under a proprietary licence from Ableton (link-devs@ableton.com). `rusty_link` 0.4.9 wraps it and is GPL-2.0-or-later.
**Decision:** MIDI clock and Tempo Follower are built in. Link and Link Audio only behind an opt-in Cargo feature `link`, off by default, because enabling it makes distributed binaries GPL. Owner decided on 2026-10-28 that release builds enable it (DECISIONS D31).
**Sources:** [rusty_link](https://docs.rs/crate/rusty_link/latest).

## Stem Separation (2026-10-27)

**Findings:** `stem-splitter-core` 1.2.0 (MIT OR Apache-2.0) runs HT-Demucs through ONNX Runtime (`ort` 2.0.0-rc.15) and downloads a roughly 200 MB model on first use. The HT-Demucs ONNX conversion is Apache-2.0.
**Decision:** Adapt: use `stem-splitter-core` behind a Cargo feature `stems`, with the model downloaded only after explicit user consent and verified by SHA-256, never at build time.
**Sources:** [stem-splitter-core](https://docs.rs/crate/stem-splitter-core/1.2.0), [htdemucs-ort](https://huggingface.co/gridshiftstudio/htdemucs-ort/blob/main/README.md).

## Real-Time Safe Rust Patterns For The Audio Graph (2026-10-27)

**Findings:** Firewheel 0.12.1 (BillyDM) is a no-mutex Rust audio graph engine aimed at games; its design (graph compiled off-thread, nodes processed in blocks, parameters as events) matches what a DAW needs, but its scope and node API target games.
**Decision:** Borrow the design, build LiveCraft's graph, because Live's routing, PDC, and per-track threading needs are specific.
**Sources:** [Firewheel](https://github.com/BillyDM/firewheel).

## Property Testing (2026-10-31)

**Brief:** property tests with 10,000 cases for musical-time round trips (M02) in a workspace without a property-testing crate.
**Findings:** `proptest` 1.12.0 is the newest release (created 2026-10-10, after 1.11.0 on 2026-03-24), MIT OR Apache-2.0, MSRV 1.88, about 229 million crates.io downloads. It shrinks failing inputs and records failing seeds in `proptest-regressions/`. `quickcheck` 1.1.0 (created 2026-02-10, Unlicense OR MIT, MSRV 1.85, about 77 million downloads) is the runner-up: its `Arbitrary` generators are per type, so bounded ranges such as 20 to 999 BPM need hand-written wrappers, while proptest strategies express ranges directly.
**Decision:** Adopt `proptest` 1.12.0 as a dev-dependency of `livecraft-time` only. Runner-up: `quickcheck` 1.1.0.
**Sources:** [proptest on crates.io](https://crates.io/crates/proptest), [proptest docs](https://docs.rs/proptest), [quickcheck on crates.io](https://crates.io/crates/quickcheck).

## Command Registry And Undo (2026-10-31)

**Tier:** Quick. One bounded choice in a familiar domain. The design is already fixed by `plans/livecraft-1.0/milestones/M04-session-and-commands.md` and by the FilmCraft decision above.

**Findings:** `undo` 0.52.0 (MIT OR Apache-2.0, created 2016-12-12, last updated 2025-03-08, 189684 downloads) is a generic undo and redo stack. It has no labelled groups, no revert to an arbitrary entry, and no time-gap merge. `typetag` 0.2.23 (MIT OR Apache-2.0, last updated 2026-07-18, about 56 million downloads) serialises trait objects through link-time registration. The registry needs neither. Command names map to constructors that call `serde_json::from_value`, and `serde_json` is already a workspace dependency.

**Decision:** Adopt `serde_json` for JSON arguments. Build the undo history, transactions, and the command registry in `livecraft-session`. Runner-up: `undo` 0.52.0, rejected because the labelled transactions, the Undo History entries, and the 500 ms gesture merge would still have to be written on top of it.

**Sources:** [undo on crates.io](https://crates.io/crates/undo) and its JSON API `https://crates.io/api/v1/crates/undo`; [typetag on crates.io](https://crates.io/crates/typetag) and its JSON API `https://crates.io/api/v1/crates/typetag`. Both read on 2026-10-31.
