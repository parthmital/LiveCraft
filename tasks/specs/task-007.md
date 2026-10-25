# Task Specification: task-007

## 1. Metadata

- **ID:** `task-007`
- **Phase:** Phase 2: Core Data Model & Authentic Ableton 12 Compatibility
- **Title:** Authentic Ableton Live 12 Set Serializer (.als Exporter)
- **Dependencies:** `task-006`
- **Manual Grounding:** Ableton Live 12 Manual Chapter 5 (Managing Files and Sets)

## 2. Context & Problem Statement

In `crates/livecraft-core/src/als.rs:520-560`, `export_to_xml` serializes devices as `<Device Type="..." IsEnabled="..." Name="...">` and `<Parameter Name="..." Value="...">`. This invalid XML causes real Ableton Live 12 to fail with a corrupted file error when attempting to open a LiveCraft-exported `.als` file.

## 3. Scope of Work

1. Update `AlsParser::export_to_xml` to output official Ableton Live 12 XML elements:
   - Wrap devices in `<DeviceChain><Devices>` using authentic tags: `<Operator Id="...">`, `<Eq8 Id="...">`, `<Compressor2 Id="...">`, `<StereoGain Id="...">`, etc.
   - Output authentic parameter structures:
     ```xml
     <On>
       <Manual Value="true" />
     </On>
     ```
2. Serialize track mixer volume and pan with authentic `<Volume><Manual Value="..."/>` and `<Pan><Manual Value="..."/>` structures.
3. Serialize `<ScaleInformation>` matching Ableton Live 12.1+ format:
   ```xml
   <ScaleInformation>
     <RootNote Value="0" />
     <Name Value="Major" />
   </ScaleInformation>
   ```
4. Verify complete bidirectional round-trip: `Project` -> XML -> `Project` without data loss.

## 4. Files to Inspect

- `crates/livecraft-core/src/als.rs` (lines 455-650)

## 5. Files to Modify

- `crates/livecraft-core/src/als.rs`

## 6. What NOT to Touch

- Do NOT touch `livecraft-dsp/` or `livecraft-gui/`.

## 7. Step-by-Step Implementation Instructions

1. In `crates/livecraft-core/src/als.rs`:
   - Replace the generic `<Device>` tag serializer with a helper function `write_ableton_device`:
     ```rust
     fn write_ableton_device<W: Write>(writer: &mut Writer<W>, dev: &Device) -> Result<(), quick_xml::Error> {
         let tag_name = match dev.device_type {
             DeviceType::Operator => "Operator",
             DeviceType::EqEight => "Eq8",
             DeviceType::Compressor => "Compressor2",
             DeviceType::Utility => "StereoGain",
             DeviceType::Delay => "Delay",
             DeviceType::Reverb => "Reverb",
             DeviceType::Saturator => "Saturator",
             DeviceType::AutoFilter => "AutoFilter",
             DeviceType::DrumRack => "DrumGroupDevice",
             DeviceType::Simpler => "OriginalSimpler",
             DeviceType::Chorus => "Chorus",
             DeviceType::AutoPan => "AutoPan",
             DeviceType::Gate => "Gate",
             DeviceType::EqThree => "FilterEQ3",
             DeviceType::Redux => "Redux2",
             DeviceType::Limiter => "Limiter",
             DeviceType::Custom(ref s) => s.as_str(),
         };
         // Write start tag with Id
         // Write <On><Manual Value="..." /></On>
         // Write device specific parameters
         // Write end tag
     }
     ```
   - Serialize `<Ableton MajorVersion="5" MinorVersion="12.0_12117" Creator="Ableton Live 12.0.5" Revision="...">`.
2. Test full round-trip with `test_als_full_roundtrip_with_devices_and_clips`.

## 8. Verification Command

```bash
cargo test -p livecraft-core --lib tests::test_als_full_roundtrip_with_devices_and_clips
```

## 9. Acceptance Criteria

- [ ] Exported XML uses authentic `<Operator>`, `<Eq8>`, etc. instead of `<Device Type="...">`.
- [ ] Round-trip export and re-import preserves all tracks, devices, clips, notes, and scale configuration.
- [ ] Gzip compression matches Ableton Live `.als` archive requirements.

## 10. Failure Handling Protocol

If XML tags do not close properly:

- Ensure each `writer.write_event(Event::Start(...))` has an exact matching `Event::End(...)` with the identical tag name.
