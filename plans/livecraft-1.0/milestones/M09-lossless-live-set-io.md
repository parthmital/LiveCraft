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
- Oracle corpus: the owner saves a set of `.als` files from Live 12.4 covering features as they land (list in the expansion), stored in `.agent-local/reference/sets/` (never committed, they may contain the owner's content). Committed fixtures are LiveCraft-authored only.
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
