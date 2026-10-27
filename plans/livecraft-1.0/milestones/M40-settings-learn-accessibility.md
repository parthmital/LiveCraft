# M40: Settings, Learn View, accessibility, keyboard navigation

Purpose: scope, decisions, and acceptance criteria for settings, help, and accessibility. Read when: expanding or reviewing M40.
Phase: 6. Depends on: M39. Manual: 2.2 Learning About Live (24 to 30, Learn View), 2.3 Live's Settings (31 to 34: Display and Input, Theme and Colors, Audio, Link, Tempo and MIDI, File and Folder, Library, Plug-Ins, Record Warp and Launch, Licenses and Updates), 41 Accessibility and Keyboard Navigation (1003 to 1013), 42 shortcuts remaining. Parity rows: 2.2, 2.3, 41.1 to 41.5.

## Goal

The Settings window with every tab and option, Learn View with LiveCraft's own lessons, screen reader support, and keyboard navigation of every view behave like Live.

## Decisions

- Settings options: transcribe every option from 2.3.1 to 2.3.10 into the `Preferences` schema (M06); options that depend on out-of-scope features (account, licences, Splice, cloud) are hidden and recorded Out of Scope.
- Learn View: same panel layout and lesson-tracking behaviour as 2.2 describes; lesson content written by LiveCraft (no Ableton text or videos), stored as Markdown in `crates/livecraft-ui/assets/learn/`.
- Accessibility: enable eframe's `accesskit` feature once cargo-deny passes for it (D29 revisited at expansion; if the unmaintained crates remain, record an advisory exception with reason only if they are not security vulnerabilities); every widget exposes role, name, value, and actions; navigation per 41.4 and editing envelopes per 41.5.
- Keyboard navigation settings from 41.1.

## Task Outline

1. Settings window framework and tabs; one task per two tabs.
2. Theme and Colors tab with every Live theme from M13 and the zoom setting.
3. Learn View panel, lesson tracking, and the first 10 lessons.
4. AccessKit integration (D29 revisit) with `egui_kittest` accessibility queries for each view.
5. Keyboard navigation per 41.1 to 41.5.
6. Remaining unbound shortcuts from the M14 report.
7. Milestone verification (HUMAN: owner tests the main workflows with Windows Narrator and records results).

## Acceptance Criteria

- Every setting from 2.3 is implemented or recorded Out of Scope.
- `egui_kittest` finds every interactive widget by accessible name in the main views.
- The M14 unbound-shortcut report is empty except Out of Scope commands.
