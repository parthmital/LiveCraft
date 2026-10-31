# M06: Audio and MIDI I/O and device settings

Purpose: scope, decisions, and acceptance criteria for hardware audio and MIDI. Read when: expanding or reviewing M06.
Phase: 1. Depends on: M05. Manual: 2.3 Live's Settings (31 to 34; Audio and Link, Tempo and MIDI tabs), 17.2 External Audio In/Out (375), 17.3 External MIDI In/Out (375 to 379), 41.2 Audio Setup (1005), 41.3 Connecting MIDI Devices (1005). Parity rows: 17.2, 17.3.

## Goal

Users pick audio driver type, input and output devices, sample rate, and buffer size, see input and output latency, use audio inputs, and connect MIDI inputs and outputs, with settings persisted and device loss handled without crashing.

## Decisions

- Audio: `cpal` 0.18 hosts. Windows: WASAPI (shared; exclusive when cpal exposes it, otherwise documented as a gap), plus `asio` behind feature `asio`, enabled in release builds (DECISIONS D14, D31, D-OWN-2). macOS: CoreAudio. Linux: ALSA and JACK (cpal `jack` feature on, since JACK is LGPL-licensed client library loaded dynamically; confirm with cargo-deny in the task).
- Duplex: input and output streams joined through an `rtrb` ring per channel group, with input latency compensation recorded for recording (M21).
- Settings persistence: `Preferences` stored as TOML in the platform config directory (`directories` 6.0.0) under `LiveCraft/Preferences.toml`; never in the repository.
- MIDI: `midir` 0.11.1 inputs feed timestamped events into an `rtrb` queue; the engine converts host timestamps to sample offsets (manual chapter 40 timing rules); outputs send from a dedicated thread with timestamps.
- Device loss: engine switches to a `NullDriver` that keeps the transport running silently and shows the error in the status bar, as Live does when an interface disconnects.
- Sample rates offered: whatever the device supports among 44100, 48000, 88200, 96000, 176400, 192000.

## Task Outline

1. `Preferences` type, load and save with defaults, tests with a temp directory.
2. Driver abstraction (`AudioDriver` trait) with `CpalDriver`, `NullDriver`, and `VirtualDriver` (offline, used by M08).
3. Device enumeration and selection API in the session, CLI `livecraft devices` listing.
4. Buffer size and sample-rate changes with engine reconfiguration (graph recompile at the new rate).
5. Audio input capture into the engine (input routing types from M03), latency reporting.
6. MIDI input with sample-accurate conversion; tests with synthetic timestamps.
7. MIDI output with timing; tests with a loopback mock.
8. Device loss and recovery; tests with a fake driver that errors.
9. Settings UI panels for Audio and MIDI tabs (visual parity later in M40; functional now).
10. Milestone verification and docs.

## Acceptance Criteria

- `livecraft devices` lists inputs and outputs on the reference machine (HUMAN check: the owner confirms the list matches Windows sound settings).
- Changing buffer size during playback causes no crash; audio resumes within 500 ms.
- MIDI timing test: 1000 events with random host timestamps land within 1 sample of the expected offset.
- Unplugging the audio device (simulated in tests) leaves the app running.

## Review (2026-11-01)

Every Outline item has its own commit. Item 10 is this commit. Each commit passed `cargo xtask ci`.

| Item | Commit      | Summary                                                                                                                  |
| ---- | ----------- | ------------------------------------------------------------------------------------------------------------------------ |
| 1    | `c9338dc`   | `Preferences` as TOML in `<config dir>/LiveCraft/Preferences.toml`, atomic save                                          |
| 2    | `822f2e1`   | `AudioDriver` with `CpalDriver`, `NullDriver`, `VirtualDriver`; `Playback` holds a driver                                |
| 3    | `848654b`   | `list_devices`, `Session::audio_devices`, `livecraft devices [--json]`, preferences read at open                         |
| 4    | `d18ee0b`   | `Session::set_audio_preferences` rebuilds the engine at the new rate and buffer and carries the transport and clips over |
| 5    | `2acd090`   | Input stream into an `rtrb` ring, per-track Ext. In monitoring (In, Auto armed, Off), input and output latency           |
| 6    | `3eb656a`   | `midir` inputs, one ring per port, host-clock stamps mapped to sample offsets, notes to armed or monitoring MIDI tracks  |
| 7    | `2b78683`   | `Routing::midi_to`, clip notes to an output queue with frames, `MidiOutScheduler`, the `MidiOutputs` thread              |
| 8    | `bd48ff4`   | A lost device moves the Set to a `NullDriver`, the status bar names it, and the device is retried every 2 s              |
| 9    | `bf872c8`   | Settings window (`Ctrl + ,`, `gui --settings`): Audio tab and MIDI Ports table                                           |
| 10   | this commit | JACK on Linux, the `asio` feature, owner rows Q09 and Q10, parity rows 17.2 and 17.3, docs, and this review              |

### Decisions and deviations

1. cpal 0.18.2 names: stream errors are `cpal::Error` with `ErrorKind::DeviceNotAvailable`, and device names come from `description()`. `AudioDriver` is not `Send`, because a CoreAudio stream may not leave its thread.
2. WASAPI exclusive mode is a gap: cpal 0.18.2 opens WASAPI in shared mode only.
3. ASIO: the `asio` feature exists on `livecraft-engine` and `livecraft` (it forwards `cpal/asio`), but it is off in every build here. Building it needs the Steinberg SDK and LLVM, which this machine lacks, so the D31 release setting waits for Q10.
4. JACK: cpal's `jack` feature is on for Linux. The `jack` crate loads libjack at run time, but `jack-sys` needs the JACK headers to build, so CI and the README prerequisites install `libjack-jackd2-dev`. Not built on Linux here.
5. Changing the output closes the old one before starting the new one, because WinMM refuses to open a MIDI port twice. The new driver is resolved first, so a refused setting leaves the running output alone. When the new output fails to start after the old one closed, the Set moves to the silent driver and the error is shown.
6. MIDI input timing: a message stamped during the previous buffer period plays at its position in this buffer, so input has one buffer of constant latency and no jitter. A message stamped after the buffer start plays on the last frame.
7. MIDI input feeds every armed or In-monitoring MIDI track from every channel of every open port. Per-track MIDI From and channel filters are M20.
8. MIDI output ports open only when their Track switch is on. Without a MIDI preference, every input port opens and no output port does.
9. `Routing::midi_to` channels are zero-based, as in the status byte. The `.als` writer ignores the field until M09.
10. Ext. In is one mono channel copied to both sides. Stereo pairs and the input chooser are M20.
11. Auto monitoring: an armed Auto track monitors and plays no clips. Manual page 374 says Auto monitoring is inhibited while the track plays clips. Matching it means letting armed tracks play clips, which changes M05 behaviour, so it moves to M20 (parity row 17.1). Manual page 374 also gives Off as the default for audio tracks, and `Track::new` still uses Auto for all tracks, also for M20.
12. Input opens the default input device whenever the host has one, as Live does. On Windows this lights the microphone indicator.
13. No drift correction between the input and output clocks: the input ring absorbs drift until it is full (about 370 ms at 44.1 kHz stereo), then drops frames. M21 (recording) revisits this.
14. `EngineSnapshot::frames` was added, so the session can tell a published snapshot from the default one when it carries state across two quick output changes.
15. The Settings window's selected tab label is amber on amber and hard to read. The colours come from the global style, which M13 replaces.
16. The Session View mixer now labels the Main track "Main". The Set's own track name and its JSON form wait for Q04.

### Evidence

- `cargo xtask ci` passed on the final tree, log `.agent-local/logs/2026-11-01_m06-10-final-ci.log`, with 398 tests. `cargo-deny` reports no warnings.
- `livecraft devices` on the reference machine, `.agent-local/output/m06/devices.txt`: WASAPI (default), output Speakers (Realtek(R) Audio), input Microphone Array (Realtek(R) Audio), no MIDI inputs, MIDI output Microsoft GS Wavetable Synth.
- `livecraft gui --settings --screenshot .agent-local/output/m06/settings-audio.png` showed the Settings window on the real device with measured latencies (input 10.58 ms, output 32.23 ms, overall 42.81 ms).
- `grep -rn "Mutex\|lock()\|try_lock" crates/livecraft-engine/src` finds only two test names that contain "block".

### Criteria

| Criterion                                                                        | Met                | Evidence                                                                                                                                                                  |
| -------------------------------------------------------------------------------- | ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `livecraft devices` lists inputs and outputs; the owner confirms against Windows | Partly (owner Q09) | The list is in Evidence. The HUMAN check is Q09                                                                                                                           |
| A buffer size change during playback causes no crash; audio resumes in 500 ms    | Yes                | `tests/audio.rs` `a_playing_set_resumes_within_500_ms_of_a_buffer_size_and_sample_rate_change` (512 to 128 frames and 44.1 to 48 kHz while playing, on the silent driver) |
| 1000 MIDI events with random host timestamps land within 1 sample                | Yes                | `midi.rs` `a_thousand_random_timestamps_land_within_one_sample_of_their_offset`                                                                                           |
| Unplugging the audio device (simulated) leaves the app running                   | Yes                | `tests/audio.rs` `an_unplugged_device_leaves_the_set_playing_silently_and_reports_the_error`, `a_device_plugged_back_in_is_used_again_at_the_next_retry`                  |

### Owner checks

- Q09: the device list against Windows sound settings, sound from the demo Set, a live buffer and rate change, a MIDI keyboard, and unplugging a device. The line is `[~]` until Q09 arrives.
- Q10: the ASIO SDK build and an ASIO interface (D31).

### Not verified

- No sound was heard in this review, and no MIDI hardware was used. Device loss was simulated with a fake driver only.
- The `asio` feature has not been compiled. Linux (ALSA, JACK) and macOS (CoreAudio, CoreMIDI) were not compiled here; CI covers them.
- The MIDI output timing tests run against the wall clock with a 15 ms tolerance. They passed five runs in a row on this machine.
- The manual PDF was not available for items 5 to 9, so manual references come from `docs/live-manual.md` and the milestone's page list.
