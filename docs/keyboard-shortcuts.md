# Live 12 Keyboard Shortcuts

Purpose: Live 12 keyboard shortcuts for binding keys in LiveCraft. Read when: adding or changing a shortcut.
Last verified: 2026-10-27 (section 42.1 checked against manual page 1014).

- Source: manual chapter 42, pages 1014 to 1036. The manual is authoritative; on any conflict, follow it.
- Coverage is partial: sections 42.2, 42.6 to 42.9, 42.11, 42.13, 42.14, 42.17, 42.18, 42.21 to 42.23, and 42.25 to 42.29 are missing. The plan's shortcut registry milestone generates the complete table from the PDF and replaces this file.
- LiveCraft today implements only `Space`, `Tab`, `Shift+Tab`, `F12`, `Ctrl+T`, `Ctrl+Shift+T`, `0`, `S`, `C`, `Ctrl+Z`, `Ctrl+Y`, `Ctrl+S` (`crates/livecraft-gui/src/app.rs`), plus `F8` for a theme toggle that Live does not have.

## 1. Showing and Hiding Views (Chapter 42.1)

| Command                               | Windows                              | macOS                                    |
| :------------------------------------ | :----------------------------------- | :--------------------------------------- |
| Toggle Full Screen Mode               | `F11`                                | `Ctrl + Cmd + F`                         |
| Toggle Second Window                  | `Ctrl + Shift + W`                   | `Cmd + Shift + W`                        |
| Toggle Session / Arrangement View     | `Tab`                                | `Tab`                                    |
| Toggle Between Device / Clip View     | `Shift + Tab` or `F12`               | `Shift + Tab` or `F12`                   |
| Toggle Device and Clip View (Stacked) | `Alt + Click` view toggle            | `Option + Click` view toggle             |
| Toggle Hot-Swap Mode                  | `Q`                                  | `Q`                                      |
| Toggle Drum Rack / Last Selected Pad  | `D`                                  | `D`                                      |
| Hide / Show Info View                 | `Shift + ?`                          | `Shift + ?`                              |
| Hide / Show Video Window              | `Ctrl + Alt + V`                     | `Cmd + Option + V`                       |
| Hide / Show Browser                   | `Ctrl + Alt + B` or `Ctrl + Alt + 5` | `Cmd + Option + B` or `Cmd + Option + 5` |
| Hide / Show Overview                  | `Ctrl + Alt + O`                     | `Cmd + Option + O`                       |
| Hide / Show In/Out (Routing)          | `Ctrl + Alt + I`                     | `Cmd + Option + I`                       |
| Hide / Show Sends                     | `Ctrl + Alt + S`                     | `Cmd + Option + S`                       |
| Hide / Show Mixer in Arrangement      | `Ctrl + Alt + M`                     | `Cmd + Option + M`                       |

## 2. Working with Sets and Program (Chapter 42.3)

| Command                     | Windows            | macOS             |
| :-------------------------- | :----------------- | :---------------- |
| New Live Set                | `Ctrl + N`         | `Cmd + N`         |
| Open Live Set               | `Ctrl + O`         | `Cmd + O`         |
| Close Live Set              | `Ctrl + W`         | `Cmd + W`         |
| Save Live Set               | `Ctrl + S`         | `Cmd + S`         |
| Save Live Set As...         | `Ctrl + Shift + S` | `Cmd + Shift + S` |
| Quit Live                   | `Ctrl + Q`         | `Cmd + Q`         |
| Open Settings / Preferences | `Ctrl + ,`         | `Cmd + ,`         |
| Export Audio / Video        | `Ctrl + Shift + R` | `Cmd + Shift + R` |
| Export MIDI Clip            | `Ctrl + Shift + E` | `Cmd + Shift + E` |

## 3. Editing Commands (Chapter 42.5)

| Command          | Windows                          | macOS                          |
| :--------------- | :------------------------------- | :----------------------------- |
| Undo             | `Ctrl + Z`                       | `Cmd + Z`                      |
| Redo             | `Ctrl + Y` or `Ctrl + Shift + Z` | `Cmd + Y` or `Cmd + Shift + Z` |
| Cut              | `Ctrl + X`                       | `Cmd + X`                      |
| Copy             | `Ctrl + C`                       | `Cmd + C`                      |
| Paste            | `Ctrl + V`                       | `Cmd + V`                      |
| Duplicate        | `Ctrl + D`                       | `Cmd + D`                      |
| Delete           | `Delete` or `Backspace`          | `Delete`                       |
| Select All       | `Ctrl + A`                       | `Cmd + A`                      |
| Rename Selection | `Ctrl + R`                       | `Cmd + R`                      |

## 4. Working with Devices and Plug-Ins (Chapter 42.4)

| Command                           | Windows                  | macOS                    |
| :-------------------------------- | :----------------------- | :----------------------- |
| Group Devices into Rack           | `Ctrl + G`               | `Cmd + G`                |
| Ungroup Rack                      | `Ctrl + Shift + G`       | `Cmd + Shift + G`        |
| Toggle Device On/Off              | Click Activator or `0`   | Click Activator or `0`   |
| Reset Parameter to Default        | `Delete` or Double-click | `Delete` or Double-click |
| Load Selected Device from Browser | `Enter`                  | `Enter`                  |
| Hot-Swap Selected Device          | `Q`                      | `Q`                      |
| Hide / Show Plug-In Windows       | `Ctrl + Alt + P`         | `Cmd + Option + P`       |

## 5. Commands for Tracks (Chapter 42.19)

| Command                       | Windows                  | macOS                      |
| :---------------------------- | :----------------------- | :------------------------- |
| Insert Audio Track            | `Ctrl + T`               | `Cmd + T`                  |
| Insert MIDI Track             | `Ctrl + Shift + T`       | `Cmd + Shift + T`          |
| Insert Return Track           | `Ctrl + Alt + T`         | `Cmd + Option + T`         |
| Rename Selected Track         | `Ctrl + R`               | `Cmd + R`                  |
| Next Track While Renaming     | `Tab`                    | `Tab`                      |
| Previous Track While Renaming | `Shift + Tab`            | `Shift + Tab`              |
| Group Selected Tracks         | `Ctrl + G`               | `Cmd + G`                  |
| Ungroup Selected Tracks       | `Ctrl + Shift + G`       | `Cmd + Shift + G`          |
| Arm Selected Tracks           | `C`                      | `C`                        |
| Solo Selected Tracks          | `S`                      | `S`                        |
| Deactivate / Mute Track       | `0`                      | `0`                        |
| Freeze / Unfreeze Tracks      | `Ctrl + Alt + Shift + F` | `Cmd + Option + Shift + F` |

## 6. Transport Controls (Chapter 42.20)

| Command                       | Windows                                | macOS                   |
| :---------------------------- | :------------------------------------- | :---------------------- |
| Play from Selection / Stop    | `Space`                                | `Space`                 |
| Continue Play from Stop Point | `Shift + Space`                        | `Shift + Space`         |
| Play Arrangement from Start   | `Shift + Space` (with cursor at 1.1.1) | `Shift + Space`         |
| Record (Arrangement)          | `F9`                                   | `F9`                    |
| Record (Session)              | `Ctrl + Shift + F9`                    | `Cmd + Shift + F9`      |
| Tap Tempo                     | Click Tap or press assigned key        | Click Tap               |
| Toggle Metronome              | Click Metronome icon                   | Click Metronome icon    |
| Nudge Backward / Forward      | Left / Right Arrow keys on transport   | Left / Right Arrow keys |

## 7. Session View Commands (Chapter 42.15)

| Command                        | Windows                 | macOS                   |
| :----------------------------- | :---------------------- | :---------------------- |
| Launch Selected Clip / Slot    | `Enter`                 | `Enter`                 |
| Select Neighboring Clip / Slot | Arrow keys              | Arrow keys              |
| Stop Clips in Selected Track   | `Ctrl + Enter`          | `Cmd + Enter`           |
| Add / Remove Stop Button       | `Ctrl + E`              | `Cmd + E`               |
| Insert Empty MIDI Clip         | `Ctrl + Shift + M`      | `Cmd + Shift + M`       |
| Insert Scene                   | `Ctrl + I`              | `Cmd + I`               |
| Insert Captured Scene          | `Ctrl + Shift + I`      | `Cmd + Shift + I`       |
| Move Between Scenes            | Up / Down Arrow keys    | Up / Down Arrow keys    |
| Move 8 Scenes at Once          | `Page Up` / `Page Down` | `Page Up` / `Page Down` |

## 8. Arrangement View Commands (Chapter 42.16)

| Command                          | Windows                 | macOS                  |
| :------------------------------- | :---------------------- | :--------------------- |
| Split Clip at Selection          | `Ctrl + E`              | `Cmd + E`              |
| Consolidate Selection into Clip  | `Ctrl + J`              | `Cmd + J`              |
| Crop Selected Clips              | `Ctrl + Shift + J`      | `Cmd + Shift + J`      |
| Loop Selection                   | `Ctrl + L`              | `Cmd + L`              |
| Insert Silence                   | `Ctrl + I`              | `Cmd + I`              |
| Cut Time across All Tracks       | `Ctrl + Shift + X`      | `Cmd + Shift + X`      |
| Delete Time across All Tracks    | `Ctrl + Shift + Delete` | `Cmd + Shift + Delete` |
| Duplicate Time across All Tracks | `Ctrl + Shift + D`      | `Cmd + Shift + D`      |
| Toggle Automation Mode           | `A`                     | `A`                    |

## 9. Clip View & MIDI Note Editor (Chapters 42.10 & 42.12)

| Command                           | Windows                       | macOS                        |
| :-------------------------------- | :---------------------------- | :--------------------------- |
| Toggle Fold to Scale / Fold View  | `F`                           | `F`                          |
| Quantize Notes                    | `Ctrl + U`                    | `Cmd + U`                    |
| Quantize Settings...              | `Ctrl + Shift + U`            | `Cmd + Shift + U`            |
| Transpose Note +1 / -1 Semitone   | Up / Down Arrow keys          | Up / Down Arrow keys         |
| Transpose Note +1 / -1 Octave     | `Shift + Up / Down Arrow`     | `Shift + Up / Down Arrow`    |
| Shorten / Lengthen Selected Notes | `Shift + Left / Right Arrow`  | `Shift + Left / Right Arrow` |
| Move Selected Notes Horizontally  | Left / Right Arrow keys       | Left / Right Arrow keys      |
| Change Note Velocity              | Click velocity handle or drag | Click velocity handle        |
| Zoom to Selection                 | `Z`                           | `Z`                          |
| Zoom Back / Fit All               | `X`                           | `X`                          |
| Fit Content to Width              | `W`                           | `W`                          |
| Fit Content to Height             | `H`                           | `H`                          |

## 10. Computer MIDI Keyboard (Chapter 42.24)

| Command                             | Key                                    |
| :---------------------------------- | :------------------------------------- |
| Toggle Computer MIDI Keyboard       | `M`                                    |
| White Keys (C, D, E, F, G, A, B, C) | `A`, `S`, `D`, `F`, `G`, `H`, `J`, `K` |
| Black Keys (C#, D#, F#, G#, A#)     | `W`, `E`, `T`, `Y`, `U`                |
| Transpose Octave Down / Up          | `Z` / `X`                              |
| Adjust Velocity Down / Up (-/+ 20)  | `C` / `V`                              |
