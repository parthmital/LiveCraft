# M14: Main window shell, menus, Control Bar, Info View, shortcuts

Purpose: scope, decisions, and acceptance criteria for the application frame. Read when: expanding or reviewing M14.
Phase: 3. Depends on: M13. Manual: 3.1 The Control Bar (35), 3.2 The Status Bar (36), 3.12 Clip and Device View (50), 2.2 Learning About Live (Learn View placement), 41.4 Navigating in Live (1006 to 1010), 42 Live Keyboard Shortcuts (1014 to 1036). Parity rows: 3.1, 3.2, 3.12, 42.x.

## Goal

The window frame, menu bar, Control Bar (all nine sections), Status Bar and Info View, view switching, panel show and hide, Overview, splitters, keyboard focus model, second window, full screen, and the complete shortcut registry behave and look like Live.

## Decisions

- Menu bar: Live's menus and items for Windows (File, Edit, Create, View, Options, Help on 12.4; confirm the exact list from captures). Items not yet implemented are shown disabled with the milestone in their tooltip in debug builds only.
- Control Bar sections in Live's order (manual 3.1 lists nine sections: browser options, tempo and metronome group, scale, follow and position, transport, loop, overdub and automation and capture, tools such as draw and keyboard and key or MIDI map, CPU and disk and MIDI indicators, plus view selectors; quote the manual list exactly in the expansion).
- Shortcut registry: generated table `crates/livecraft-ui/src/shortcuts/table.rs` produced by `tools/extract_shortcuts.py` from manual chapter 42 (Windows and Mac columns); each entry maps to a command name; unimplemented commands are listed in a test report, not silently dropped. This task replaces `docs/keyboard-shortcuts.md` with a generated complete version.
- Keyboard focus model per manual 41.4 and 42.2 (Tab moves focus between views when the setting is enabled; arrow navigation in lists).
- Info View: hovering any widget shows its title and description; texts are written by LiveCraft from the manual's descriptions (paraphrased, not copied verbatim beyond short labels).
- Second window and full screen via eframe viewports.

## Task Outline

1. Window frame and layout regions with splitters and saved sizes.
2. Menu bar with every menu and item; command bindings for implemented items.
3. Control Bar sections 1 to 3; 4 to 6; 7 to 9 (three tasks), each against captures.
4. Status Bar and Info View with hover help text registry.
5. View switching (Session, Arrangement, Clip, Device, Mixer sections shown or hidden, Overview).
6. `tools/extract_shortcuts.py` and the generated shortcut table; regenerate `docs/keyboard-shortcuts.md`.
7. Shortcut dispatch with focus scopes; tests for every implemented shortcut through `egui_kittest`.
8. Keyboard focus model and navigation.
9. Second window and full screen.
10. Milestone verification: shell states from `capture-list.md` pass `ui_diff.py`.

## Acceptance Criteria

- Shell, menu, Control Bar, and Info View states pass `UI-FIDELITY.md` thresholds in both themes.
- Every chapter 42 shortcut is either bound and tested or listed in the generated "unbound" report with its owning milestone.
- `docs/keyboard-shortcuts.md` covers all sections 42.1 to 42.29.
