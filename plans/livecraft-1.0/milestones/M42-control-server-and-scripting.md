# M42: Control server, MCP, and scripting

Purpose: scope, decisions, and acceptance criteria for programmability beyond Live. Read when: expanding or reviewing M42.
Phase: 7. Depends on: M41. Manual: 32 Max for Live (824 to 829; replaced, decision D04), 32.4 Building Max for Live MIDI Tools (828). Parity rows: 32.1 to 32.6 (marked Out of Scope with this milestone as the replacement). Beyond: B03, B04, B05, B10.

## Goal

Every registry command is reachable from a local JSON-RPC server and an MCP adapter, the CLI covers batch workflows, and users can write MIDI Tools and control scripts in Rhai.

## Decisions

- New crate `livecraft-server` (depends on session). JSON-RPC 2.0 over stdio (`livecraft serve --stdio`) and localhost TCP (`livecraft serve --port <p>`, bound to 127.0.0.1 only, random token required in every request, printed at start and written to a file readable only by the user). No remote network access (security).
- Methods: `commands.list`, `commands.run`, `set.open`, `set.save`, `set.query` (JSONPath-like selectors over the model), `transport.*`, `render.*`, `events.subscribe` (snapshot stream at a requested rate).
- MCP: `livecraft mcp` exposes the same methods as MCP tools through `rmcp` 3.8.0 (D26) in a separate binary mode; every tool that changes the set requires the session to be unsaved-changes-safe (it creates an undo transaction).
- Scripting: `rhai` 1.27.0 with a sandbox (no file or network access, operation limits); MIDI Tool scripts implement the M19 tool trait and appear in the Transform and Generate panels; control scripts drive M38 surfaces.
- CLI: `livecraft batch <script.json>` runs a list of commands over sets for CI pipelines; exit codes documented.

## Task Outline

1. Create `livecraft-server`; JSON-RPC framing and dispatch over stdio with tests.
2. TCP transport with token authentication and localhost binding; security tests (wrong token, non-local address).
3. Methods for commands, set, transport, render, events.
4. MCP adapter binary mode.
5. Rhai runtime sandbox and limits with tests.
6. Scripted MIDI Tools in the Transform and Generate panels; three example scripts.
7. Control scripts API for M38.
8. `livecraft batch` and exit codes; `livecraft diff` polish (B10).
9. Docs: `docs/control-protocol.md` (new topic doc) and `docs/scripting.md`; SECURITY.md threat model updated for the server.
10. Milestone verification.

## Acceptance Criteria

- Every registry command is callable through JSON-RPC (test enumerates the registry).
- Security tests pass; SECURITY.md updated with the server entry point.
- Beyond rows B03, B04, B05, B10 Done with evidence.
