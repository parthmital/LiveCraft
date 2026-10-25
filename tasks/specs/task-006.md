# Task Specification: task-006

## 1. Metadata

- **ID:** `task-006`
- **Phase:** Phase 2: Core Data Model & Authentic Ableton 12 Compatibility
- **Title:** Authentic Ableton Live 12 XML Schema Mapping (.als Parser)
- **Dependencies:** `task-005`
- **Manual Grounding:** Ableton Live 12 Manual Chapter 5 (Managing Files and Sets)

## 2. Context & Problem Statement

In `crates/livecraft-core/src/als.rs:143-195`, the parser looks for `<Device Type="..." IsEnabled="..." Name="...">` and `<Parameter Name="..." Value="...">`. This is an invented XML structure! Real Ableton Live 10, 11, and 12 sets use authentic device tag names: `<Operator>`, `<OriginalSimpler>`, `<Eq8>`, `<Compressor2>`, `<StereoGain>`, `<Reverb>`, `<Delay>`, `<Saturator>`, `<DrumGroupDevice>`, `<FilterEQ3>`, `<AutoFilter>`, `<Chorus>`, `<AutoPan>`, etc. Because of this, real Ableton sets cannot be parsed, and real device chains are lost.

## 3. Scope of Work

1. Update `AlsParser::parse_xml` to recognize real Ableton Live device tags inside `<DeviceChain><Devices>`:
   - `<Operator>` -> `DeviceType::Operator`
   - `<OriginalSimpler>` -> `DeviceType::Simpler`
   - `<Eq8>` -> `DeviceType::EqEight`
   - `<Compressor2>` -> `DeviceType::Compressor`
   - `<StereoGain>` -> `DeviceType::Utility`
   - `<Reverb>` -> `DeviceType::Reverb`
   - `<Delay>` -> `DeviceType::Delay`
   - `<Saturator>` -> `DeviceType::Saturator`
   - `<DrumGroupDevice>` -> `DeviceType::DrumRack`
   - `<AutoFilter>` -> `DeviceType::AutoFilter`
   - `<FilterEQ3>` -> `DeviceType::EqThree`
   - `<Chorus>` -> `DeviceType::Chorus`
   - `<AutoPan>` -> `DeviceType::AutoPan`
   - `<Gate>` -> `DeviceType::Gate`
   - `<Redux>` -> `DeviceType::Redux`
   - `<Limiter>` -> `DeviceType::Limiter`
2. Extract real Ableton parameter structures: `<On><Manual Value="true" /></On>`, `<Volume><Manual Value="..." /></Volume>`, `<Pan><Manual Value="..." /></Pan>`.
3. Support backward compatibility with LiveCraft's internal tags while prioritizing official Ableton Live tags.

## 4. Files to Inspect

- `crates/livecraft-core/src/als.rs` (lines 70-240)
- `crates/livecraft-core/src/model.rs` (DeviceType enum)

## 5. Files to Modify

- `crates/livecraft-core/src/als.rs`

## 6. What NOT to Touch

- Do NOT touch `livecraft-dsp/` or `livecraft-gui/`.
- Do NOT alter `AlsParser::save_project_to_als` (export schema is covered in `task-007`).

## 7. Step-by-Step Implementation Instructions

1. In `crates/livecraft-core/src/als.rs`:
   - Expand `tag_name` matching in `Event::Start` to include:
     ```rust
     "Operator" | "OriginalSimpler" | "Eq8" | "Compressor2" | "StereoGain" |
     "Reverb" | "Delay" | "Saturator" | "DrumGroupDevice" | "AutoFilter" |
     "Chorus" | "AutoPan" | "Gate" | "FilterEQ3" | "Redux2" | "Limiter" => {
         let dtype = map_ableton_tag_to_device_type(&tag_name);
         // Initialize device with its ID attribute and default parameters
     }
     ```
   - In `<On><Manual Value="true"/>`: parse enabled status.
   - In `<Mixer>` tags: parse `<Volume><Manual Value="..."/>` and `<Pan><Manual Value="..."/>`.
   - In `<ClipSlotList>`: extract `<MidiClip>` and `<AudioClip>` with their `<CurrentProperties>` and `<KeyTracks>`.
2. Write a comprehensive unit test that feeds a snippet of real Ableton XML with `<Operator>`, `<Eq8>`, and `<Compressor2>` and asserts that all 3 devices are parsed with their correct types and parameters.

## 8. Verification Command

```bash
cargo test -p livecraft-core --lib tests::test_als_xml_export_and_import
```

## 9. Acceptance Criteria

- [ ] Real Ableton device tags (`<Operator>`, `<Eq8>`, etc.) are recognized and parsed into valid `Device` structs.
- [ ] Track volumes and pan values from real Ableton XML are preserved accurately.
- [ ] Unit tests pass with 0 warnings.

## 10. Failure Handling Protocol

If quick-xml throws parsing errors:

- Ensure tags like `<On>` and `<Manual>` are tracked with hierarchical parent stack matching so inner `Value` attributes map to the correct property.
