# Owner Queue

Purpose: owner-only work the executor needs. The run never waits for it; a Reference pass finishes the dependent `[~]` lines once the input exists. Read when: you are the owner, at any time, including while the run is going. How to do each kind of item: `prompts/OWNER-GUIDE.md`.

Status values: `Open` (needed), `Arrived <date>` (input in place, not yet used), `Done <date>` (used by a Reference pass). Mark a row `Arrived` and push when you have done it, or just put the file in place; the next Reference pass checks.

| ID  | Needed by line | What to do (exact steps)                                                                                                             | Where the result goes                                 | Status |
| --- | -------------- | ------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------- | ------ |
| Q01 | M46 release    | Get legal advice on reproducing Live's interface layout and device names (D-OWN-3), then record the answer.                          | `reference/DECISIONS.md#owner-decisions`, row D-OWN-3 | Open   |
| Q02 | none (review)  | Confirm or change the defaults the executor uses for D-OWN-1, D-OWN-2, D-OWN-4, D-OWN-5, D-OWN-6. No reply means the defaults stand. | `reference/DECISIONS.md#owner-decisions`              | Open   |

<!-- Executor: append one row per request: next ID (Q03, Q04, ...), the checklist line ID, the exact steps copied from the task, the exact result path, Status "Open". Never delete rows. -->
