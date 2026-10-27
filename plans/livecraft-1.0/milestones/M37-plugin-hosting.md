# M37: Plug-in hosting (CLAP, VST3, AU) with sandboxing

Purpose: scope, decisions, and acceptance criteria for third-party plug-ins. Read when: expanding or reviewing M37.
Phase: 6. Depends on: M36. Manual: 24 Using Plug-Ins (479 to 490: plug-ins in Device View, separate windows, configure mode, sidechain parameters, VST folders, presets and banks, Audio Units), 2.3.8 Plug-Ins settings (34), 23.3 Device Delay Compensation (477). Parity rows: 24.1 to 24.4. Beyond: B02.

## Goal

VST3 and CLAP plug-ins (and AU on macOS) scan, load, show their editors, automate, save state in `.als`, compensate latency, use sidechains, and crash without taking LiveCraft down.

## Decisions

- Out-of-process host `apps/livecraft-plugin-host` (one process per plug-in by default, configurable to shared processes) talking to the engine through `livecraft-plugin-protocol`: shared-memory audio and event buffers with a lock-free handshake (two semaphores or futex-style wait on the host side only; the engine side never blocks: if the host misses the deadline, the engine outputs the last good block, counts a dropout, and keeps going).
- The plug-in protocol and host crates are the only crates allowed `unsafe` (FFI and shared memory); each `unsafe` block has a `// SAFETY:` comment; `SECURITY.md` records the exception.
- CLAP via `clack-host` 0.2.0; VST3 via `vst3` 0.3.0 bindings (VST3 SDK is MIT since 3.8); AU via the macOS AudioToolbox API in a macOS-only module. VST2 is not supported (SDK no longer licensable); the manual's VST2-specific statements are recorded as Out of Scope rows.
- Plug-in state saved in `.als` in the same elements Live uses for VST3 and AU state when the owner's oracle sets show the structure; otherwise preserved untouched (M09 rule) and LiveCraft-hosted state written in a namespaced element.
- Editors embedded via native child windows (egui viewport with raw window handle) or shown as separate windows (24.1.1).
- Crash handling: host process death disables the device, shows Live-like error text in the device title, and keeps audio running (B02).

## Task Outline

1. Protocol crate: shared memory layout, handshake, tests with a fake host.
2. Host app skeleton, process supervision, crash detection test (a test plug-in that crashes on purpose).
3. CLAP scanning, loading, processing, parameters, state.
4. VST3 scanning, loading, processing, parameters, state, presets (24.3.2).
5. Plug-in editors (embedded and separate windows); configure mode (24.1.2).
6. Sidechain inputs (24.2) and latency reporting into PDC (23.3).
7. AU on macOS (24.4).
8. Plug-in settings tab (2.3.8) and folders (24.3.1).
9. `.als` mapping for plug-in devices.
10. Milestone verification with three free plug-ins listed in the expansion (owner installs them).

## Acceptance Criteria

- Crash test: a crashing plug-in leaves playback running and the set intact.
- Oracle: a free deterministic VST3 effect at fixed settings renders identically in LiveCraft and Live (linear threshold).
- Parity rows 24.x Done or Out of Scope with reasons.
