# M25: Device framework, Device View, racks, macros

Purpose: scope, decisions, and acceptance criteria for devices as a system. Read when: expanding or reviewing M25, and before any device milestone (M26 to M34).
Phase: 5. Depends on: M24. Manual: 3.11 Devices (48 to 49), 3.15 Presets and Racks (56), 23 Working with Instruments and Effects (445 to 478: Device View, using devices, presets, hot-swapping, device delay compensation from M05), 25 Instrument, Drum and Effect Racks (491 to 510: overview, creating, looking at racks, chain list and auto select, zones with key, velocity, chain select, Drum Racks, Macro Controls with map mode, randomizing, variations, mixing with racks). Parity rows: 3.11, 3.15, 23.1, 23.2, 25.1 to 25.8 (25.6 pads and choke shared with M32).

## Goal

A uniform device system: parameter specs, generic Device View chrome (title bar, activator, fold, hot-swap, presets, context menus), per-device panel layouts, and Instrument, Drum, MIDI Effect, and Audio Effect Racks with chains, zones, macros, variations, and chain mixing.

## Decisions

- Per-device folder layout from `TARGET-ARCHITECTURE.md#where-new-code-goes`. Each device declares `DeviceSpec { kind, display_name, category, params: &[ParamSpec], panel: PanelLayout }`.
- Device display names equal Live's (D03, D-OWN-3). Parameter names, units, ranges, and defaults are transcribed from the manual section of each device and from captures.
- Racks are graph sub-structures compiled by M05: chains in parallel, chain volume, pan, mute, solo, zones evaluated per note (key, velocity) and per chain-select value.
- Macros: 1 to 16 macro controls (Live 12 count to be confirmed on page 504), mappings with min and max, map mode, randomize with exclusions, variations store and recall (25.7.1 to 25.7.3).
- Device View UI chrome built once; device panels are declarative layouts of widget-kit controls positioned from captures.
- The per-device milestones that follow (M26 to M34) use one fixed task pattern per device, defined here:
  1. Spec: transcribe parameters from the manual section into `spec.rs`, with a test asserting names, ranges, defaults, and units.
  2. DSP: implement `processor.rs` from the manual's description; unit tests on synthetic signals with numeric expectations.
  3. Panel: layout against the device's default-state capture; snapshot test; `ui_diff.py` passes.
  4. `.als` mapping in `livecraft-formats/src/als/devices/<device>.rs`; round-trip test with an owner-saved set.
  5. HUMAN: oracle renders from Live of the fixture set for that device at listed settings.
  6. Oracle comparison test and parity row update.
     Small devices may combine steps 1 and 2 or 3 and 4 in one task if the size limit allows.

## Task Outline

1. `DeviceSpec`, `ParamSpec` display formatting (units, rounding) matching Live; tests.
2. Device View chrome: title bar, activator, fold, presets menu, hot-swap, context menu, drag reorder, delete.
3. Panel layout system (declarative) and the generic fallback panel.
4. Rack model and compiler support: chains, chain mixer, solo and mute.
5. Zones: key, velocity, chain select, fades (25.5).
6. Chain list UI and auto select (25.4).
7. Macros, map mode, randomize (25.7.1, 25.7.2).
8. Macro variations (25.7.3).
9. Group into rack and ungroup commands; rack presets `.adg` via M10.
10. Milestone verification: rack states pass `ui_diff.py`; oracle render of a 3-chain rack with zone fades.

## Acceptance Criteria

- Rack oracle render meets linear thresholds.
- Parity rows listed above Done with evidence; the per-device task pattern is documented in this file for M26 to M34.
