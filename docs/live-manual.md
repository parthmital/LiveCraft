# Live 12 Reference Manual Guide

Purpose: where each topic is in the Live 12 reference manual and how to read it from the command line. Read when: implementing or checking any Live feature.
Last verified: 2026-10-28.

## The File

- `docs/ableton-live-12-reference-manual.pdf`: the Live 12.4 edition (documents Learn View and Link Audio), 1039 pages, 108 MB.
- Git-ignored (`.gitignore` excludes `*.pdf`). Ask the repository owner for it; never download it from unofficial sources and never commit it or page renders.
- The manual is the specification. When code and manual disagree, the manual wins unless `plans/livecraft-1.0/reference/DECISIONS.md` records a deliberate difference.

## Querying

Use the repo-local Python environment (setup in `README.md#prerequisites`). On macOS or Linux replace `./.venv/Scripts/python` with `./.venv/bin/python`.

| Need                    | Command                                                                                                               |
| ----------------------- | --------------------------------------------------------------------------------------------------------------------- |
| Chapters and sections   | `./.venv/Scripts/python plans/livecraft-1.0/tools/manual.py toc --depth 2`                                            |
| Find a section by title | `./.venv/Scripts/python plans/livecraft-1.0/tools/manual.py toc --filter "Follow Actions"`                            |
| Read a whole section    | `./.venv/Scripts/python plans/livecraft-1.0/tools/manual.py section "16.7 Follow Actions"`                            |
| Read pages              | `./.venv/Scripts/python plans/livecraft-1.0/tools/manual.py range 366 371`                                            |
| Search text             | `./.venv/Scripts/python plans/livecraft-1.0/tools/manual.py search "Legato" --max 20`                                 |
| See a page's figures    | `./.venv/Scripts/python plans/livecraft-1.0/tools/manual.py render 35 36` (PNG into `.agent-local/reference/manual/`) |

Manual screenshots are stored as thousands of image tiles, so render pages instead of extracting images. Rendered screenshots are small; pixel measurements come from captures of the installed Live (`plans/livecraft-1.0/reference/UI-FIDELITY.md`).

## Chapter Map

Page where each chapter starts. Section-level pages and the milestone that implements each section are in `plans/livecraft-1.0/reference/PARITY-MATRIX.md`.

| Ch  | Title                           | Page | Ch  | Title                                             | Page |
| --- | ------------------------------- | ---- | --- | ------------------------------------------------- | ---- |
| 1   | Welcome to Live                 | 23   | 23  | Working with Instruments and Effects              | 445  |
| 2   | First Steps                     | 24   | 24  | Using Plug-Ins                                    | 479  |
| 3   | Live Concepts                   | 35   | 25  | Instrument, Drum and Effect Racks                 | 491  |
| 4   | Working with the Browser        | 62   | 26  | Automation                                        | 511  |
| 5   | Managing Files and Sets         | 122  | 27  | Clip Envelopes                                    | 533  |
| 6   | Arrangement View                | 165  | 28  | Working with Video                                | 546  |
| 7   | Session View                    | 187  | 29  | Live Audio Effect Reference                       | 552  |
| 8   | Clip View                       | 200  | 30  | Live MIDI Effect Reference                        | 691  |
| 9   | Audio Clips, Tempo, and Warping | 234  | 31  | Live Instrument Reference                         | 704  |
| 10  | Editing MIDI                    | 252  | 32  | Max for Live                                      | 824  |
| 11  | MIDI Tools                      | 293  | 33  | Max for Live Devices                              | 830  |
| 12  | Editing MPE                     | 330  | 34  | MIDI and Key Remote Control                       | 865  |
| 13  | Converting Audio to MIDI        | 340  | 35  | Using Push 1                                      | 874  |
| 14  | Using Grooves                   | 345  | 36  | Using Push 2                                      | 914  |
| 15  | Using Tuning Systems            | 351  | 37  | Synchronizing with Link, Tempo Follower, and MIDI | 973  |
| 16  | Launching Clips                 | 360  | 38  | Computer Audio Resources and Strategies           | 982  |
| 17  | Routing and I/O                 | 372  | 39  | Audio Fact Sheet                                  | 989  |
| 18  | Mixing                          | 394  | 40  | MIDI Fact Sheet                                   | 997  |
| 19  | Recording New Clips             | 412  | 41  | Accessibility and Keyboard Navigation             | 1003 |
| 20  | Bounce to Audio                 | 425  | 42  | Live Keyboard Shortcuts                           | 1014 |
| 21  | Comping                         | 431  | 43  | Credits                                           | 1037 |
| 22  | Stem Separation                 | 437  |     |                                                   |      |

## Reading Rules

- Quote the manual sentences a task implements, with page numbers, in the task file.
- Live terms are used exactly as written in the manual (for example "Main track", not "Master track", in Live 12).
- Where the manual is silent (exact pixel sizes, drag sensitivity, DSP curves), the owner measures the installed Live; see `plans/livecraft-1.0/reference/UI-FIDELITY.md` and `QUALITY-GATES.md#audio`.
