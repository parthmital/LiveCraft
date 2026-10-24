# LiveCraft Documentation & Reference

## Ableton Live 12 Reference Manual

The single source of truth for all Ableton Live 12 specifications, features, DSP instruments, audio effects, MIDI tools, scale systems, and user interface behaviors is the official Ableton Live 12 Reference Manual:

- **Path:** `docs/ableton-live-12-reference-manual.pdf` (108 MB, 1039 pages, 1069 sections)
- **Version:** Ableton Live 12 (Windows & macOS)
- **Status:** Definitive source of truth for all LiveCraft engineering and agent tasks.

> **Note:** The PDF file is ignored by Git (`.gitignore`) to keep the repository lightweight. It resides locally in `docs/` for all developers and AI agents.

### Agent Access & CLI Query Tool

Because the PDF exceeds standard 100 MB single-read limits, a dedicated query script is provided in `.agent-local/scripts/query_manual.py` for instant inspection:

- **Filter Table of Contents:**
  ```bash
  python .agent-local/scripts/query_manual.py --filter-toc "Operator"
  ```
- **Read Specific Page or Range:**
  ```bash
  python .agent-local/scripts/query_manual.py --page 755
  python .agent-local/scripts/query_manual.py --range 755-760
  ```
- **Search Keyword:**
  ```bash
  python .agent-local/scripts/query_manual.py --search "Saturator"
  ```
- **Extract Section by Title:**
  ```bash
  python .agent-local/scripts/query_manual.py --section "Operator"
  ```
