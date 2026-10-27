# Owner Queue

Purpose: owner-only work (captures, measurements, legal advice, certificates) that implementation needs. Work never waits for it; an Owner review line in `CHECKLIST.md` finishes the dependent `[~]` lines once the input exists. Read when: you are the owner, or a line needs owner input.

Status values: `Open` (needed), `Arrived <date>` (input in place, not yet used), `Done <date>` (used by an Owner review). Mark a row `Arrived` and push when you have done it, or just put the file in place; the next Owner review checks.

| ID  | Needed by line | What to do (exact steps)                                                                         | Where the result goes                                 | Status             |
| --- | -------------- | ------------------------------------------------------------------------------------------------ | ----------------------------------------------------- | ------------------ |
| Q01 | M46 release    | Owner answered 2026-10-28 without legal advice. D-OWN-3 accepted; M46 gate dropped.              | `reference/DECISIONS.md#owner-decisions`, row D-OWN-3 | Arrived 2026-10-28 |
| Q02 | none (review)  | Owner answered 2026-10-28: D-OWN-1, D-OWN-2 and D-OWN-5 yes; D-OWN-6 no; D-OWN-4 default stands. | `reference/DECISIONS.md#owner-decisions`              | Arrived 2026-10-28 |

<!-- Append one row per request: next ID (Q03, Q04, ...), the checklist line ID, the exact steps copied from the task, the exact result path, Status "Open". Never delete rows. -->
