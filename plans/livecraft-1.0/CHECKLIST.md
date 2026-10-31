# Checklist: LiveCraft 1.0

States: `[ ]` to do, `[x]` done, `[~]` done except owner input queued in `OWNER-QUEUE.md`. Change the box in the same commit that completes the line. Work top to bottom. Line types: Task (follow its task file), Milestone (implement the milestone file's Outline in order until every Acceptance criterion holds, one commit per Outline item), Owner review (finish `[~]` lines whose `OWNER-QUEUE.md` rows are `Arrived`). Owner decisions use their defaults from `reference/DECISIONS.md#owner-decisions`.

## Phase 0: Production foundation

- [x] P0-01 Pin the toolchain and share package metadata
- [x] P0-02 Configure logging without set_var and replace eprintln in libraries
- [x] P0-03 Remove unused dependencies
- [x] P0-04 Add workspace lints
- [x] P0-05 Move the large inline test modules out of lib.rs
- [x] P0-06 Remove trademark branding and use Live's note names
- [x] P0-07 Add cargo xtask ci, the layering check, and cargo dev
- [x] P0-08 Migrate the workspace to Rust edition 2024
- [x] P0-09 Upgrade cpal to 0.18 and thiserror to 2
- [x] P0-10 Upgrade egui and eframe to 0.37 with wgpu
- [x] P0-11 Upgrade quick-xml to 0.42 to fix two advisories
- [x] P0-12 Add cargo-deny to cargo xtask ci
- [x] P0-13 Rewrite CI with pinned actions and three platforms
- [x] P0-14 Cap .als decompression size
- [x] P0-15 One WAV loader with a size limit
- [x] P0-16 Never send NaN or infinity to the driver or a render
- [x] P0-17 Full security audit and SECURITY.md update
- [x] P0-18 Verify Phase 0 and bring the docs up to date

## Phase 1: Core architecture

- [x] M01 tasks written: Workspace restructure into target crates ([milestone](milestones/M01-workspace-restructure.md))
- [x] M01-01 Characterisation tests for render and inspection
- [x] M01-02 Create livecraft-testkit crate
- [x] M01-03 Create livecraft-model crate
- [x] M01-04 Create livecraft-formats crate
- [x] M01-05 Remove livecraft-core crate
- [x] M01-06 Create livecraft-devices crate
- [x] M01-07 Create livecraft-engine crate and trim livecraft-dsp
- [x] M01-08 Rename livecraft-gui to livecraft-ui ([task](tasks/M01-08-rename-gui-to-ui.md))
- [x] M01-09 Move CLI to apps/livecraft ([task](tasks/M01-09-move-cli-to-apps-livecraft.md))
- [ ] M01-10 Verify M01 workspace restructure and update docs ([task](tasks/M01-10-milestone-verification.md))
- [ ] M02 Musical time foundation ([milestone](milestones/M02-musical-time.md))
- [ ] M03 Document model v2 ([milestone](milestones/M03-document-model-v2.md))
- [ ] M04 Session, command registry, undo ([milestone](milestones/M04-session-and-commands.md))
- [ ] M05 Real-time engine core ([milestone](milestones/M05-realtime-engine-core.md))
- [ ] M06 Audio and MIDI I/O and device settings ([milestone](milestones/M06-audio-midi-io.md))
- [ ] M07 Transport, tempo, metronome, launch scheduler ([milestone](milestones/M07-transport-and-launch.md))
- [ ] M08 Offline render and export ([milestone](milestones/M08-offline-render-export.md))

## Phase 2: Files and content

- [ ] M09 Lossless Live Set I/O ([milestone](milestones/M09-lossless-live-set-io.md))
- [ ] M10 Live Projects and file management ([milestone](milestones/M10-projects-and-file-management.md))
- [ ] M11 Audio files, sample cache, disk streaming ([milestone](milestones/M11-audio-files-and-streaming.md))
- [ ] Owner review after Phase 2

## Phase 3: UI foundation

- [ ] M12 UI reference capture and fidelity tooling ([milestone](milestones/M12-ui-reference-capture.md))
- [ ] M13 Design system ([milestone](milestones/M13-design-system.md))
- [ ] M14 Main window shell, menus, Control Bar, Info View, shortcuts ([milestone](milestones/M14-main-window-shell.md))
- [ ] Owner review after Phase 3

## Phase 4: Core workflows

- [ ] M15 Session View ([milestone](milestones/M15-session-view.md))
- [ ] M16 Arrangement View ([milestone](milestones/M16-arrangement-view.md))
- [ ] M17 Clip View and audio clip editing ([milestone](milestones/M17-clip-view.md))
- [ ] M18 MIDI Note Editor and MPE ([milestone](milestones/M18-midi-note-editor-and-mpe.md))
- [ ] M19 MIDI Tools ([milestone](milestones/M19-midi-tools.md))
- [ ] M20 Mixer, groups, returns, routing, crossfader, cue ([milestone](milestones/M20-mixer-and-routing.md))
- [ ] M21 Recording, capture, comping ([milestone](milestones/M21-recording-and-comping.md))
- [ ] M22 Automation and clip envelopes ([milestone](milestones/M22-automation-and-clip-envelopes.md))
- [ ] M23 Warping and tempo ([milestone](milestones/M23-warping-and-tempo.md))
- [ ] M24 Browser, library, Sound Similarity, preview, hot-swap ([milestone](milestones/M24-browser-and-library.md))
- [ ] Owner review after Phase 4

## Phase 5: Devices

- [ ] M25 Device framework, Device View, racks, macros ([milestone](milestones/M25-device-framework-and-racks.md))
- [ ] M26 Audio effects: EQ, filter, analysis, utility ([milestone](milestones/M26-effects-eq-filter-utility.md))
- [ ] M27 Audio effects: dynamics ([milestone](milestones/M27-effects-dynamics.md))
- [ ] M28 Audio effects: delay, reverb, looping, spectral time ([milestone](milestones/M28-effects-delay-reverb.md))
- [ ] M29 Audio effects: distortion and character ([milestone](milestones/M29-effects-distortion.md))
- [ ] M30 Audio effects: modulation, pitch, resonance, vocoder ([milestone](milestones/M30-effects-modulation-pitch.md))
- [ ] M31 MIDI effects and modulators ([milestone](milestones/M31-midi-effects-and-modulators.md))
- [ ] M32 Instruments: sample based ([milestone](milestones/M32-instruments-sample-based.md))
- [ ] M33 Instruments: synthesizers ([milestone](milestones/M33-instruments-synthesizers.md))
- [ ] M34 Instruments: physical modelling and external ([milestone](milestones/M34-instruments-physical-modelling.md))
- [ ] Owner review after Phase 5

## Phase 6: Advanced features

- [ ] M35 Grooves, tuning systems, scale awareness ([milestone](milestones/M35-grooves-tuning-scales.md))
- [ ] M36 Audio to MIDI, stem separation, bounce, freeze ([milestone](milestones/M36-audio-to-midi-stems-bounce.md))
- [ ] M37 Plug-in hosting with sandboxing ([milestone](milestones/M37-plugin-hosting.md))
- [ ] M38 Remote control, control surfaces, synchronisation ([milestone](milestones/M38-remote-control-and-sync.md))
- [ ] M39 Video ([milestone](milestones/M39-video.md))
- [ ] M40 Settings, Learn View, accessibility, keyboard navigation ([milestone](milestones/M40-settings-learn-accessibility.md))
- [ ] M41 Performance ([milestone](milestones/M41-performance.md))
- [ ] Owner review after Phase 6

## Phase 7: Beyond Live

- [ ] M42 Control server, MCP, and scripting ([milestone](milestones/M42-control-server-and-scripting.md))
- [ ] M43 Reliability ([milestone](milestones/M43-reliability.md))
- [ ] M44 Mixing and mastering extras ([milestone](milestones/M44-mixing-mastering-extras.md))
- [ ] M45 Platforms, packaging, updates ([milestone](milestones/M45-platforms-packaging.md))
- [ ] Owner review after Phase 7

## Phase 8: Release

- [ ] M46 Release 1.0 ([milestone](milestones/M46-release-1.0.md))
- [ ] Final owner review: finish every `[~]` line whose owner input has arrived, including the M46 release steps

## Blockers

<!-- Append "<line ID>, <date>: <root cause>", the failing command, and the last 20 lines of its output when a line cannot be finished. -->
