# M24: Browser, library, Sound Similarity, preview, hot-swap

Purpose: scope, decisions, and acceptance criteria for the Browser. Read when: expanding or reviewing M24.
Phase: 4. Depends on: M23. Manual: 3.3 The Browser (37), 3.4 Sound Similarity (38 to 39), 4 Working with the Browser (62 to 121: content pane, search bar, history, filters and tags with filter groups, tags, tag editor, quick tags, collections, library, places including User Library, Current Project, user folders, navigating, previewing files, adding content). Parity rows: 3.3, 3.4, 4.1 to 4.10. Out of scope by D05: 4.7.3 Splice, 4.7.4 Ableton Cloud, 4.7.5 Push 3 transfer, Packs store downloads in 4.7.1.

## Goal

The Browser looks and behaves like Live's: sidebar, content pane, search with Live's syntax, history, filters and tags, collections, places, preview with tempo sync, drag and drop into the set, hot-swap, and Sound Similarity search, over LiveCraft's own content plus user folders.

## Decisions

- New crate `livecraft-library` (depends on model, formats, dsp). Index store: SQLite via `rusqlite` 0.41.0 (`bundled`), in the platform data directory, never in the repository.
- Indexing runs as a background job with file-system change watching (`notify` 8.2.1, CC0-1.0; if 9.0 final has been out at least 14 days at expansion time, D28 allows moving to it).
- Tags: Live's built-in tag hierarchy is described in 4.4; LiveCraft implements user tags and tag groups with the same UI. Live's own tag data from its library is read only if D-OWN-6 allows it.
- Sound Similarity (3.4): audio embedding per file computed from MFCC statistics and spectral contrast (in-house, `realfft` 3.5.1), cosine similarity search; evaluated by a HUMAN listening check on 20 queries and by agreement with Live's similar-samples results on the owner's machine (top-10 overlap reported).
- Preview: decoded through M11 off the audio thread, routed to the Cue output; tempo-synced preview uses M23 warping.
- Content: LiveCraft ships a small CC0 or self-made content set (D06) with device presets for its own devices.

## Task Outline

1. Create `livecraft-library`, schema, indexer job, and tests on a temp folder.
2. Sidebar and content pane against captures; navigation and history (4.1, 4.3, 4.8).
3. Search syntax and behaviour quoted from 4.2; tests per example.
4. Filters and tags, tag editor, quick tags (4.4).
5. Collections (4.5) and places: User Library, Current Project, user folders (4.7.6 to 4.7.8).
6. Preview (4.9) with Cue routing and tempo sync.
7. Adding content: drag and drop, double click, hot-swap (4.10, 42.1 Q).
8. Sound Similarity embeddings, search, and the similar-samples swap (3.4, 42.23).
9. File watching and incremental re-index.
10. Milestone verification.

## Acceptance Criteria

- Browser states pass `ui_diff.py`; search examples from 4.2 pass.
- Indexing 50,000 files completes in under 5 minutes on the reference machine; search returns in under 50 ms.
- Parity rows 3.3, 3.4, 4.x (except out-of-scope rows marked Out of Scope) Done with evidence.
