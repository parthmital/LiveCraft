# Checklist: LiveCraft 1.0

States: `[ ]` to do, `[x]` done, `[~]` done except owner input queued in `OWNER-QUEUE.md`. Change the box in the same commit that completes the line. Work strictly top to bottom, without stopping between lines. Line types: Task (do it), Expand (write the milestone's tasks with `prompts/EXPAND-MILESTONE.md`, insert them directly below the Expand line, commit, push, continue with the first new task), Reference pass (`tasks/REFERENCE-PASS.md`). Owner decisions use their defaults from `reference/DECISIONS.md#owner-decisions`; no line waits for them.

## Phase 0: Production foundation

- [x] P0-01 Pin the toolchain and share package metadata ([task](tasks/P0-01-pin-toolchain-and-package-metadata.md))
- [x] P0-02 Configure logging without set_var and replace eprintln in libraries ([task](tasks/P0-02-logging-without-set-var.md))
- [x] P0-03 Remove unused dependencies ([task](tasks/P0-03-remove-unused-dependencies.md))
- [x] P0-04 Add workspace lints ([task](tasks/P0-04-workspace-lints.md))
- [x] P0-05 Move the large inline test modules out of lib.rs ([task](tasks/P0-05-split-test-modules.md))
- [x] P0-06 Remove trademark branding and use Live's note names ([task](tasks/P0-06-branding-and-note-names.md))
- [x] P0-07 Add cargo xtask ci, the layering check, and cargo dev ([task](tasks/P0-07-xtask-ci-runner.md))
- [x] P0-08 Migrate the workspace to Rust edition 2024 ([task](tasks/P0-08-edition-2024.md))
- [x] P0-09 Upgrade cpal to 0.18 and thiserror to 2 ([task](tasks/P0-09-upgrade-cpal-and-thiserror.md))
- [x] P0-10 Upgrade egui and eframe to 0.37 with wgpu ([task](tasks/P0-10-upgrade-egui-and-wgpu.md))
- [x] P0-11 Upgrade quick-xml to 0.42 to fix two advisories ([task](tasks/P0-11-upgrade-quick-xml.md))
- [x] P0-12 Add cargo-deny to cargo xtask ci ([task](tasks/P0-12-cargo-deny.md))
- [x] P0-13 Rewrite CI with pinned actions and three platforms ([task](tasks/P0-13-ci-workflow.md))
- [x] P0-14 Cap .als decompression size ([task](tasks/P0-14-cap-als-decompression.md))
- [x] P0-15 One WAV loader with a size limit ([task](tasks/P0-15-wav-loader-limit-and-dedupe.md))
- [ ] P0-16 Never send NaN or infinity to the driver or a render ([task](tasks/P0-16-non-finite-output-guard.md))
- [ ] P0-17 Full security audit and SECURITY.md update ([task](tasks/P0-17-security-audit.md))
- [ ] P0-18 Verify Phase 0 and bring the docs up to date ([task](tasks/P0-18-phase-0-verification.md))

## Phase 1: Core architecture

- [ ] Expand M01 Workspace restructure into target crates ([milestone](milestones/M01-workspace-restructure.md))
- [ ] Expand M02 Musical time foundation ([milestone](milestones/M02-musical-time.md))
- [ ] Expand M03 Document model v2 ([milestone](milestones/M03-document-model-v2.md))
- [ ] Expand M04 Session, command registry, undo ([milestone](milestones/M04-session-and-commands.md))
- [ ] Expand M05 Real-time engine core ([milestone](milestones/M05-realtime-engine-core.md))
- [ ] Expand M06 Audio and MIDI I/O and device settings ([milestone](milestones/M06-audio-midi-io.md))
- [ ] Expand M07 Transport, tempo, metronome, launch scheduler ([milestone](milestones/M07-transport-and-launch.md))
- [ ] Expand M08 Offline render and export ([milestone](milestones/M08-offline-render-export.md))

## Phase 2: Files and content

- [ ] Expand M09 Lossless Live Set I/O ([milestone](milestones/M09-lossless-live-set-io.md))
- [ ] Expand M10 Live Projects and file management ([milestone](milestones/M10-projects-and-file-management.md))
- [ ] Expand M11 Audio files, sample cache, disk streaming ([milestone](milestones/M11-audio-files-and-streaming.md))
- [ ] Reference pass after Phase 2 ([procedure](tasks/REFERENCE-PASS.md))

## Phase 3: UI foundation

- [ ] Expand M12 UI reference capture and fidelity tooling ([milestone](milestones/M12-ui-reference-capture.md))
- [ ] Expand M13 Design system ([milestone](milestones/M13-design-system.md))
- [ ] Expand M14 Main window shell, menus, Control Bar, Info View, shortcuts ([milestone](milestones/M14-main-window-shell.md))
- [ ] Reference pass after Phase 3 ([procedure](tasks/REFERENCE-PASS.md))

## Phase 4: Core workflows

- [ ] Expand M15 Session View ([milestone](milestones/M15-session-view.md))
- [ ] Expand M16 Arrangement View ([milestone](milestones/M16-arrangement-view.md))
- [ ] Expand M17 Clip View and audio clip editing ([milestone](milestones/M17-clip-view.md))
- [ ] Expand M18 MIDI Note Editor and MPE ([milestone](milestones/M18-midi-note-editor-and-mpe.md))
- [ ] Expand M19 MIDI Tools ([milestone](milestones/M19-midi-tools.md))
- [ ] Expand M20 Mixer, groups, returns, routing, crossfader, cue ([milestone](milestones/M20-mixer-and-routing.md))
- [ ] Expand M21 Recording, capture, comping ([milestone](milestones/M21-recording-and-comping.md))
- [ ] Expand M22 Automation and clip envelopes ([milestone](milestones/M22-automation-and-clip-envelopes.md))
- [ ] Expand M23 Warping and tempo ([milestone](milestones/M23-warping-and-tempo.md))
- [ ] Expand M24 Browser, library, Sound Similarity, preview, hot-swap ([milestone](milestones/M24-browser-and-library.md))
- [ ] Reference pass after Phase 4 ([procedure](tasks/REFERENCE-PASS.md))

## Phase 5: Devices

- [ ] Expand M25 Device framework, Device View, racks, macros ([milestone](milestones/M25-device-framework-and-racks.md))
- [ ] Expand M26 Audio effects: EQ, filter, analysis, utility ([milestone](milestones/M26-effects-eq-filter-utility.md))
- [ ] Expand M27 Audio effects: dynamics ([milestone](milestones/M27-effects-dynamics.md))
- [ ] Expand M28 Audio effects: delay, reverb, looping, spectral time ([milestone](milestones/M28-effects-delay-reverb.md))
- [ ] Expand M29 Audio effects: distortion and character ([milestone](milestones/M29-effects-distortion.md))
- [ ] Expand M30 Audio effects: modulation, pitch, resonance, vocoder ([milestone](milestones/M30-effects-modulation-pitch.md))
- [ ] Expand M31 MIDI effects and modulators ([milestone](milestones/M31-midi-effects-and-modulators.md))
- [ ] Expand M32 Instruments: sample based ([milestone](milestones/M32-instruments-sample-based.md))
- [ ] Expand M33 Instruments: synthesizers ([milestone](milestones/M33-instruments-synthesizers.md))
- [ ] Expand M34 Instruments: physical modelling and external ([milestone](milestones/M34-instruments-physical-modelling.md))
- [ ] Reference pass after Phase 5 ([procedure](tasks/REFERENCE-PASS.md))

## Phase 6: Advanced features

- [ ] Expand M35 Grooves, tuning systems, scale awareness ([milestone](milestones/M35-grooves-tuning-scales.md))
- [ ] Expand M36 Audio to MIDI, stem separation, bounce, freeze ([milestone](milestones/M36-audio-to-midi-stems-bounce.md))
- [ ] Expand M37 Plug-in hosting with sandboxing ([milestone](milestones/M37-plugin-hosting.md))
- [ ] Expand M38 Remote control, control surfaces, synchronisation ([milestone](milestones/M38-remote-control-and-sync.md))
- [ ] Expand M39 Video ([milestone](milestones/M39-video.md))
- [ ] Expand M40 Settings, Learn View, accessibility, keyboard navigation ([milestone](milestones/M40-settings-learn-accessibility.md))
- [ ] Expand M41 Performance ([milestone](milestones/M41-performance.md))
- [ ] Reference pass after Phase 6 ([procedure](tasks/REFERENCE-PASS.md))

## Phase 7: Beyond Live

- [ ] Expand M42 Control server, MCP, and scripting ([milestone](milestones/M42-control-server-and-scripting.md))
- [ ] Expand M43 Reliability ([milestone](milestones/M43-reliability.md))
- [ ] Expand M44 Mixing and mastering extras ([milestone](milestones/M44-mixing-mastering-extras.md))
- [ ] Expand M45 Platforms, packaging, updates ([milestone](milestones/M45-platforms-packaging.md))
- [ ] Reference pass after Phase 7 ([procedure](tasks/REFERENCE-PASS.md))

## Phase 8: Release

- [ ] Expand M46 Release 1.0 ([milestone](milestones/M46-release-1.0.md))
- [ ] Final reference pass: finish every `[~]` line whose owner input has arrived, including the M46 release steps ([procedure](tasks/REFERENCE-PASS.md))

## Blockers

<!-- Executor: only after the Unblock Procedure in START-HERE.md fails, append "<line ID>, <date>: <root cause>", the failing command, and the last 20 lines of its output, commit, push, and end the run. -->
