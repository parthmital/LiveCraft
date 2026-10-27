# M06: Audio and MIDI I/O and device settings

Purpose: scope, decisions, and acceptance criteria for hardware audio and MIDI. Read when: expanding or reviewing M06.
Phase: 1. Depends on: M05. Manual: 2.3 Live's Settings (31 to 34; Audio and Link, Tempo and MIDI tabs), 17.2 External Audio In/Out (375), 17.3 External MIDI In/Out (375 to 379), 41.2 Audio Setup (1005), 41.3 Connecting MIDI Devices (1005). Parity rows: 17.2, 17.3.

## Goal

Users pick audio driver type, input and output devices, sample rate, and buffer size, see input and output latency, use audio inputs, and connect MIDI inputs and outputs, with settings persisted and device loss handled without crashing.

## Decisions

- Audio: `cpal` 0.18 hosts. Windows: WASAPI (shared; exclusive when cpal exposes it, otherwise documented as a gap), plus `asio` behind the opt-in feature (DECISIONS D14, D-OWN-2). macOS: CoreAudio. Linux: ALSA and JACK (cpal `jack` feature on, since JACK is LGPL-licensed client library loaded dynamically; confirm with cargo-deny in the task).
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
