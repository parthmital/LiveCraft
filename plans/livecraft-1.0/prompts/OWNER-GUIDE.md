# Owner Guide

Purpose: what the repository owner does that no executor can: start the run, fill the owner queue, and answer decisions. Read when: starting the plan, or when `OWNER-QUEUE.md` has new rows.

## Running The Plan

1. Commit and push this plan folder before starting (the executor pulls before every task).
2. Open Antigravity, new conversation, Gemini 3.8 Flash High, and paste the one `/goal` prompt from `START-HERE.md#antigravity-primary-executor-gemini-38-flash-high`. It runs every line, Expand lines included, to the release.
3. The run continues without you. When you have time, open `OWNER-QUEUE.md`, do the `Open` rows (types below), put each result where its row says, set the row to `Arrived <date>`, and push. The next Reference pass uses them. Pull before you push: the executor pushes often.
4. If the conversation ends before the run is complete (context or quota limit), start a new conversation and paste the same prompt; it resumes at the first unticked line.
5. If the run ends on a blocker in `CHECKLIST.md`, use the Blocked Run prompt in `START-HERE.md` with a stronger model, or fix it yourself, then paste the run prompt again.
6. After the run, if `[~]` lines remain, finish their queue rows and paste the run prompt once more: it runs the final Reference pass, including the v1.0.0 release steps that wait on Q01 and the blind tests.

## Owner Work Types

### Reference captures of Live (M12 onward)

- Follow `reference/UI-FIDELITY.md#capture-protocol` exactly (100 percent Windows scaling, Live zoom 100 percent, 1920x1080 client area, listed themes).
- Open the named fixture set from `fixtures/ui/` in Live, perform the listed steps, capture the window client area only (Windows: Snipping Tool window mode, then crop to the client area if needed).
- Save as `.agent-local/reference/live/<area>__<state>__<theme>.png` with the JSON sidecar described in `UI-FIDELITY.md`.
- Never commit these files or share them publicly.

### Oracle renders from Live (M08 onward)

- Open the named fixture `.als` from `fixtures/oracle/` in Live 12.4.
- File > Export Audio/Video with the exact settings the task lists (usually: Rendered Track Main, render start 1.1.1, the listed length, Sample Rate 48000, PCM WAV, Bit Depth 32, Dither none, Normalize off, Include Return and Main Effects on, Create Analysis File off).
- Save to `.agent-local/reference/oracle/<fixture name>.wav`.

### Saved sets and clips from Live (M09 onward)

- Build the set the task describes in Live, save it into `.agent-local/reference/sets/` (or `.agent-local/reference/midi-tools/` as `.alc` for M19 and M31).
- Use only your own or royalty-free samples; never Core Library content in anything you might share.

### Opening LiveCraft files in Live

- Open each listed LiveCraft-saved file in Live 12.4 and report: any error dialog (exact text), anything missing or different.

### Measurements (M13, M20, M22)

- Follow the measurement form in the task (for example: drag a number box 100 px and note the value change; record a 10-second screen video of meters at 60 fps).

## Owner Decisions

The executor uses each default from `reference/DECISIONS.md#owner-decisions` and never waits. Answer any time; features whose default is off are still built behind a feature or setting, so an answer only flips the default. Record the answer by editing the row in `reference/DECISIONS.md#owner-decisions` (Answer column text in the Question cell, for example "Answered 2027-01-10: Yes, ship with Link under GPL").

### D-OWN-1

Ship Ableton Link? Link is GPL-2.0-or-later unless you get a proprietary licence from Ableton (link-devs@ableton.com). Saying yes under the GPL makes every distributed LiveCraft binary built with the `link` feature GPL. Options: no Link (MIDI clock and Tempo Follower only); GPL builds with Link as a separate download; request a licence from Ableton.

### D-OWN-2

Ship ASIO support? The Steinberg ASIO SDK is GPLv3 or proprietary. WASAPI gives low latency on most Windows interfaces; ASIO is what most professional interfaces recommend. Options: no ASIO; GPLv3 builds with ASIO as a separate download; obtain the proprietary ASIO licence from Steinberg.

### D-OWN-3

Exact replication of Live's interface and device names carries legal risk (trade dress, trademarks) even with clean-room code and original assets. Get legal advice before a public 1.0. Options: proceed as planned; rename devices and adjust visual details where advised.

### D-OWN-4

Accept the font chosen by measurement in M13 (scores are in `DESIGN.md`), or name another openly licensed font.

### D-OWN-5

Allow LiveCraft to download the about 200 MB stem separation model (Apache-2.0) after asking the user? Options: yes (feature `stems` on in release builds); no (feature off).

### D-OWN-6

Allow LiveCraft to read your installed Live Core Library and Packs (read only) in its Browser? This is for your own use; it never redistributes content. Options: yes; no.
