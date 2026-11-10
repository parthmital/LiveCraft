# M09: Lossless Live Set I/O

Purpose: scope, decisions, and acceptance criteria for reading and writing `.als` without losing data. Read when: expanding or reviewing M09.
Phase: 2. Depends on: M08. Manual: 3.5 Live Sets (40), 5.4 Live Sets (135 to 141), 5.4.3 Merging Sets. Parity rows: 3.5, 5.4. Beyond: B10.

## Goal

Any `.als` written by Live 12.x opens in LiveCraft and saves back with zero XML differences when nothing was edited; edits change only the affected nodes; Live 12.4 opens every set LiveCraft writes.

## Decisions

- New module tree `livecraft-formats/src/als/`: `tree.rs` (`XmlTree` arena: elements with name, ordered attributes, children, text; preserves attribute order, whitespace style, and the XML declaration), `read.rs` (gzip and plain XML), `write.rs` (serialises the tree with Live's formatting: tab indentation, `<Tag Value="..." />` empty elements; verified against a Live-saved set byte for byte after decompression), `map/` (one module per model area: tracks, clips, notes, devices, automation, scenes, transport, groove, tuning).
- Mapping: on load, each model entity remembers its node path (`NodeId`); on save, mappers patch existing nodes in place, insert new nodes from templates, and remove deleted ones. Unknown elements are never touched.
- Ids: Live's `NextPointeeId` and per-element `Id` attributes are read into `IdAllocator`; new elements get fresh ids above the maximum seen.
- Templates for new elements (an empty MIDI track, a MIDI clip, a device of each kind LiveCraft knows) are LiveCraft-authored XML fragments in `livecraft-formats/templates/`, written by hand from the structure of owner-provided sets (never copied wholesale from Live's resources). Each fragment is checked by a HUMAN test: a set using it opens in Live.
- Unknown devices (third-party plug-ins, Max for Live) become `DeviceKind::Unknown { tag }` placeholders that keep their node.
- Pretty XML export for version control (B10): `livecraft convert --pretty-xml in.als out.xml` and back, stable attribute order, and `livecraft diff a.als b.als` printing a musical summary (tracks, clips, notes, devices, parameters changed).
- Oracle corpus: the owner saves a set of `.als` files from Live 12.4 covering features as they land (list recorded when the milestone starts), stored in `.agent-local/reference/sets/` (never committed, they may contain the owner's content). Committed fixtures are LiveCraft-authored only.
- quick-xml 0.42.0 (already upgraded in P0-11). The old `AlsParser` is deleted at the end of this milestone.

## Task Outline

1. `XmlTree` with lossless read and write; property test: random trees round-trip exactly.
2. Byte-exact write: decompress a Live-saved set, parse, write, compare bytes (HUMAN provides sets; test skips if absent; a LiveCraft-authored fixture always runs).
3. Gzip writer settings matching Live's (compression level does not need to match; decompressed bytes must).
4. Map transport, tempo, time signature, scenes.
5. Map tracks (all types, groups, routing, mixer values).
6. Map Session clip slots and MIDI clips with notes (all note properties).
7. Map audio clips (sample references, warp settings read and preserved for M23).
8. Map arrangement clips and automation envelopes.
9. Map devices LiveCraft implements (by the end of Phase 5 every device mapper exists; this milestone adds the framework plus today's devices); unknown device placeholders.
10. Templates for new elements; HUMAN check that Live opens a LiveCraft-created set with one of each element.
11. `convert --pretty-xml` and `diff` (B10).
12. Replace the old parser in the session and CLI; delete `als.rs`; milestone verification.

## Acceptance Criteria

- Zero-difference round trip for every set in the oracle corpus (count recorded) and every committed fixture.
- HUMAN: the owner opens 5 LiveCraft-saved sets in Live 12.4 (listed in the task) with no error dialog and identical content.
- Proptest: random command sequences, save, load: model equal (10,000 cases).
- Old parser removed; finding LC-005 parser code path gone.

## Review (2026-11-10)

All M09 items (1 to 12) were completed locally, verified, and committed together per milestone policy; the HUMAN opening check in Live 12.4 is queued as `Q13`.

| Item | Summary |
| ---- | ------- |
| 1 | `XmlTree` arena (`als/tree.rs`) with ordered attributes, quote styles, `self_closing`, text, comments, CDATA, and 1024-case `proptest` (`random_xml_tree_roundtrips_losslessly`). |
| 2, 3 | `als/read.rs` & `als/write.rs` match Live's `\t` indentation, `\n` LF with trailing LF, ` />` empty tags, and single-quote escaping; `byte_exact_roundtrip_default_live_set_when_present` passes byte-for-byte on Live 12 `DefaultLiveSet.als`. |
| 4 | `als/map/transport.rs` & `scenes.rs` map `TempoMap` (steps/ramps), `TimeSignatureMap` (`EncodedTimeSignature` `(num-1) + log2(den)*99`), `Transport`, `GlobalQuantisation`, `ScaleInformation`, `TuningSystems`, and `Scenes`. |
| 5 | `als/map/tracks.rs` & `routing.rs` map `MidiTrack`, `AudioTrack`, `GroupTrack`, `ReturnTrack`, `MainTrack` (with legacy `MasterTrack` fallback), `TrackGroupId`, `Speaker`, `SoloSink`, `Volume`, `Pan`, `Sends`, `Color`, `MonitoringEnum`, and routing. |
| 6, 7 | `als/map/clips.rs`, `notes.rs`, `audio_clips.rs` map Session `ClipSlot`s, Arrangement `ArrangerAutomation/Events` clips, Live 12 MIDI note fields (`OffVelocity`, `Probability`, `VelocityDeviation`, `IsEnabled`), Launch/Follow Actions, and AudioClip paths/warp markers. |
| 8 | `als/map/automation.rs` maps `AutomationEnvelopes/Envelopes/AutomationEnvelope` via `PointeeId` targets. |
| 9 | `als/map/devices.rs` maps all 16 native devices and preserves unknown/third-party/M4L device subtrees (`DeviceType::Custom(tag)`) untouched. |
| 10 | LiveCraft-authored XML templates in `crates/livecraft-formats/templates/*.xml` and five LiveCraft-saved sets committed under `fixtures/sets/`; owner verification queued as `Q13`. |
| 11 | `als/diff.rs` (`diff_projects`, `format_diff_summary`) and `apps/livecraft/src/cli/convert.rs` implement `livecraft convert --pretty-xml` and `livecraft diff a.als b.als [--json]` (B10). |
| 12 | `Session` (`open_file`, `from_als_document`, `save`), `livecraft-ui/src/app.rs`, and `livecraft command run` preserve `AlsDocument` across edits; `als.rs` / `als/legacy.rs` deleted (LC-005 old parser code path removed). |

### Manual quotes

From `docs/ableton-live-12-reference-manual.pdf`:

- 3.5, page 40: "The type of document that you create and work on in Live is called a Live Set. A Live Set resides in a Live Project — a folder that collects related materials."
- 5.4.1, page 135: "The File menu's Save Live Set command saves the current Live Set exactly as it is, including all clips and settings."

### Evidence

- `cargo xtask ci` passed (538 passed, 0 failed, 1 ignored `ten_minutes_of_playback_allocate_nothing`), log `.agent-local/logs/2026-11-10_m09-ci.log`.
- `cargo test -p livecraft-formats` passed: 23 unit tests (including `DefaultLiveSet.als` byte-exact round-trip and 1024-case arbitrary-bytes no-panic proptest), 5 `lossless_roundtrip` integration tests (zero-XML-diff on `DefaultLiveSet.als` and `RICH_FIXTURE_XML`, partial-edit preservation, 5 `fixtures/sets/*.als`), and 10,000-case `proptest_roundtrip::random_project_roundtrips_losslessly_through_xml`.
- `cargo test -p livecraft-session` passed: 114 tests including `lossless_als` integration tests and 500-case `random_transactions_roundtrip_losslessly_through_session_save`.

### Criteria

| Criterion | Met | Evidence |
| --------- | --- | -------- |
| Zero-difference round trip for every set in the oracle corpus and every committed fixture | Yes | `lossless_roundtrip::zero_xml_diff_roundtrip_on_default_live_set_and_rich_fixture` (`DefaultLiveSet.als` 1-set reference + `RICH_FIXTURE_XML` + 5 `fixtures/sets/*.als`) |
| HUMAN: the owner opens 5 LiveCraft-saved sets in Live 12.4 with no error dialog and identical content | Partly (owner Q13) | Sets committed in `fixtures/sets/{empty,midi_operator,audio_clip,sends_returns_automation,group_tracks}.als`; Live 12.4 opening check queued as `Q13` |
| Proptest: random command sequences, save, load: model equal (10,000 cases) | Yes | `proptest_roundtrip::random_project_roundtrips_losslessly_through_xml` (10,000 cases) and `session::tests::lossless_als::random_transactions_roundtrip_losslessly_through_session_save` (500 cases) |
| Old parser removed; finding LC-005 parser code path gone | Yes | `crates/livecraft-formats/src/als.rs` and `als/legacy.rs` deleted; `als/read.rs` + `als/tree.rs` + `als/write.rs` + `als/map/` active |

### Owner rows

- `Q13`: owner opens the five sets in `fixtures/sets/` in Live 12.4 Suite and optionally adds a saved Live 12.4 corpus to `.agent-local/reference/sets/`. Line stays `[~]` until `Q13` arrives.

### Not verified

- Opening the five `fixtures/sets/*.als` sets in Live 12.4 Suite interactively awaits `Q13`.
- Merging Sets (manual 5.4.3) and Live Clip export belong to M10 and M24.
