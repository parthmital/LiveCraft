# Task P0-06: Remove trademark branding and use Live's note names

Depends on: P0-05. Size: 5 files, about 50 changed lines. Milestone: P0.

## Goal

The window title and status bar say LiveCraft (not Ableton), the status bar shows the real engine sample rate, and every note name follows Live's convention (MIDI 60 = C3) through one shared function.

## Before You Start

1. `git pull --ff-only`, then `git status --short`: empty.
2. In `plans/livecraft-1.0/CHECKLIST.md`, P0-05 is ticked and P0-06 is not.
3. Run `cargo test --workspace`. Expected: 44 tests pass.

## Context

- Decision D02: no Ableton branding. Decision D25: MIDI 60 is C3 (Live's convention, manual chapter 10).
- `crates/livecraft-gui/src/lib.rs`: `.with_title("LiveCraft - Ableton Live Workstation (Rust)")`.
- `crates/livecraft-gui/src/app.rs` status bar draws the fixed text `"Ableton Live 12 Compatible Engine | 44.1 kHz 24-bit"` even when the engine runs at 48 kHz. `self.engine` is `Option<AudioEngine>` and `AudioEngine` has `pub sample_rate: f32`.
- `crates/livecraft-gui/src/pianoroll.rs` computes octaves as `(pitch / 12) as i32 - 1` twice (C4 for 60). `NOTE_NAMES` is defined in `crates/livecraft-core/src/model.rs` line 697 and re-exported by `livecraft_core`.
- CLI help in `crates/livecraft-cli/src/main.rs` says `60 = C4` and the GUI subcommand says "Launch the native Ableton Live 12 graphical desktop interface".

## Steps

1. Test first. Append this at the very end of `crates/livecraft-core/src/tests.rs`. That file is the body of the `tests` module (Task P0-05 created it), so it has no enclosing braces and anything appended is inside the module:

   ```rust

   #[test]
   fn test_note_name_follows_live_convention() {
       assert_eq!(note_name(60), "C3");
       assert_eq!(note_name(69), "A3");
       assert_eq!(note_name(0), "C-2");
       assert_eq!(note_name(127), "G8");
       assert_eq!(note_name(61), "C#3");
   }
   ```

2. In `crates/livecraft-core/src/model.rs`, find:

   ```rust
   pub const NOTE_NAMES: &[&str] = &[
       "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
   ];
   ```

   Replace it with:

   ```rust
   pub const NOTE_NAMES: &[&str] = &[
       "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
   ];

   /// Note name in Live's convention, where MIDI note 60 is C3 and note 0 is C-2.
   pub fn note_name(pitch: u8) -> String {
       let octave = i32::from(pitch / 12) - 2;
       format!("{}{}", NOTE_NAMES[usize::from(pitch % 12)], octave)
   }
   ```

3. In `crates/livecraft-gui/src/pianoroll.rs`, find:

   ```rust
                           let oct = (pitch / 12) as i32 - 1;
                           let note_name = NOTE_NAMES[(pitch % 12) as usize];
   ```

   Replace it with:

   ```rust
                           let label = livecraft_core::model::note_name(pitch);
   ```

   Then, a few lines below in the same block, find:

   ```rust
                                   format!("{}{}", note_name, oct),
   ```

   Replace it with:

   ```rust
                                   label,
   ```

4. In the same file, find:

   ```rust
                               let oct = (note.pitch / 12) as i32 - 1;
                               let name = NOTE_NAMES[(note.pitch % 12) as usize];
   ```

   Replace it with:

   ```rust
                               let label = livecraft_core::model::note_name(note.pitch);
   ```

   Then find (a few lines below):

   ```rust
                                   format!("{}{}", name, oct),
   ```

   Replace it with:

   ```rust
                                   label,
   ```

5. In `crates/livecraft-gui/src/lib.rs`, find `.with_title("LiveCraft - Ableton Live Workstation (Rust)")` and replace it with `.with_title("LiveCraft")`.

6. In `crates/livecraft-gui/src/app.rs`, find:

   ```rust
                           ui.label(
                               egui::RichText::new(
                                   "Ableton Live 12 Compatible Engine | 44.1 kHz 24-bit",
                               )
   ```

   Replace it with:

   ```rust
                           let engine_text = match &self.engine {
                               Some(eng) => format!("{:.1} kHz", eng.sample_rate / 1000.0),
                               None => "No audio device".to_string(),
                           };
                           ui.label(
                               egui::RichText::new(engine_text)
   ```

7. In `crates/livecraft-cli/src/main.rs`:
   - find `/// MIDI note number (60 = C4, 69 = A4 440Hz)` and replace with `/// MIDI note number (60 = C3, 69 = A3 = 440 Hz)`;
   - find `/// Launch the native Ableton Live 12 graphical desktop interface` and replace with `/// Launch the desktop interface`;
   - find `mdata.notes.push(MidiNote::new(60, 0.0, 1.0, 100)); // C4` and replace with `mdata.notes.push(MidiNote::new(60, 0.0, 1.0, 100)); // C3`.

8. Run `cargo build --workspace`. If the compiler reports `NOTE_NAMES` as an unused import in `pianoroll.rs`, keep it only if it is still used elsewhere in that file (line 94 and 99 use it, so it stays).

9. In `README.md`, find:

   ```text
   - The CLI help says MIDI note 60 is C4. Live names it C3, and LiveCraft will follow Live.
   ```

   Replace it with nothing (delete the line). In `ARCHITECTURE.md`, delete the row that starts with `| Window title and status bar use the Ableton trademark as branding`.

## Do Not

- Change files other than: `crates/livecraft-core/src/model.rs`, `crates/livecraft-core/src/tests.rs`, `crates/livecraft-gui/src/pianoroll.rs`, `crates/livecraft-gui/src/lib.rs`, `crates/livecraft-gui/src/app.rs`, `crates/livecraft-cli/src/main.rs`, `README.md`, `ARCHITECTURE.md`, `plans/livecraft-1.0/CHECKLIST.md`.
- Do not change the CLI `about` string ("Ableton Live compatible" is a permitted compatibility statement).

## Verify

| Command                                                                                              | Expected result                                                                                               |
| ---------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| `cargo test -p livecraft-core test_note_name_follows_live_convention`                                | 1 passed                                                                                                      |
| `grep -rn "Ableton Live Workstation\|Compatible Engine" crates`                                      | no output                                                                                                     |
| `cargo fmt --all -- --check`                                                                         | exit 0                                                                                                        |
| `cargo clippy --workspace --all-targets -- -D warnings`                                              | exit 0                                                                                                        |
| `cargo test --workspace`                                                                             | 45 tests pass                                                                                                 |
| `cargo build --release --workspace`                                                                  | exit 0                                                                                                        |
| `cargo run --bin livecraft -- gui --screenshot .agent-local/output/p0-06/session.png --view session` | prints `Screenshot successfully saved`; opening the PNG shows `kHz` at the bottom right and no "Ableton" text |

## Commit And Push

1. Tick P0-06 in `plans/livecraft-1.0/CHECKLIST.md`.
2. `git add crates/livecraft-core/src/model.rs crates/livecraft-core/src/tests.rs crates/livecraft-gui/src/pianoroll.rs crates/livecraft-gui/src/lib.rs crates/livecraft-gui/src/app.rs crates/livecraft-cli/src/main.rs README.md ARCHITECTURE.md plans/livecraft-1.0/CHECKLIST.md`
3. Commit:

   ```text
   Remove Ableton branding and use Live note names

   - Add livecraft_core::model::note_name with MIDI 60 as C3 and a test
   - Use it for both piano roll labels instead of two C4-based formulas
   - Title the window LiveCraft and show the engine sample rate in the status bar
   - Fix CLI help to C3 naming and drop the trademark from the gui subcommand help
   ```

4. `git push origin main`, then `git status --short --branch`: clean, not ahead.

## If Something Fails

- A Verify command fails: read the whole error, fix it within the listed files, rerun every Verify command. At most 3 attempts.
- Still failing: do not commit code. Append under Blockers in `CHECKLIST.md`: `P0-06, <date>: <what failed>`, the command, and the last 20 lines of output. Commit only `CHECKLIST.md`, push, stop.
- `git push` rejected: `git pull --rebase`, rerun Verify, push. If the rebase conflicts: `git rebase --abort`, log a blocker, stop.
