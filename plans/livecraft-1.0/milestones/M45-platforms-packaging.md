# M45: Platforms, packaging, updates

Purpose: scope, decisions, and acceptance criteria for distribution. Read when: expanding or reviewing M45.
Phase: 7. Depends on: M44. Manual: 2.1 Installation and Authorization (24; authorisation is out of scope, B14). Beyond: B01, B14.

## Goal

Signed installers for Windows, macOS, and Linux, file associations for `.als`, an opt-in update check, and no account or telemetry.

## Decisions

- Build and package through `cargo xtask package <target>`: Windows MSI (WiX toolset, research licence at expansion) and portable ZIP; macOS universal app bundle in a DMG, notarised (owner provides Apple Developer credentials as CI secrets; HUMAN); Linux AppImage and tarball, plus Flatpak if time allows.
- Code signing: Windows Authenticode certificate provided by the owner (HUMAN); unsigned builds are labelled as such.
- Updates: an opt-in check of the GitHub Releases API with the user's consent stored in Preferences; no automatic download without a click; no telemetry of any kind (B14).
- File associations: `.als`, `.alc`, `.adg`, `.adv` open in LiveCraft only if the user chooses it during install (Live may be installed too).
- Release workflow: GitHub Actions on tags builds every package, attaches SHA256SUMS, and drafts release notes from `CHANGELOG.md` (created by this milestone per `project-docs`).

## Task Outline

1. `cargo xtask package` for Windows ZIP and MSI.
2. macOS app bundle and DMG; notarisation workflow (HUMAN credentials).
3. Linux AppImage and tarball; native Linux CI job running the full test suite (B01).
4. Signing integration (HUMAN certificates).
5. Opt-in update check.
6. File associations in installers.
7. Release workflow and `CHANGELOG.md`.
8. Milestone verification: install, launch, open a set, uninstall on each OS (HUMAN checklist).

## Acceptance Criteria

- Packages build in CI for all three OSes; checksums published.
- HUMAN install checklist passes on Windows 11, macOS (latest), Ubuntu LTS.
- Beyond rows B01, B14 Done with evidence.
