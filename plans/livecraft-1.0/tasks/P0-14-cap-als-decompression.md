# Task P0-14: Cap `.als` decompression size (finding LC-001)

Depends on: P0-13. Size: 3 files, about 50 changed lines. Milestone: P0.

## Goal

Opening a `.als` whose decompressed XML exceeds 512 MiB fails with a clear `AlsError::TooLarge` instead of exhausting memory, proven by a test.

## Before You Start

1. `git pull --ff-only`, then `git status --short`: empty.
2. In `plans/livecraft-1.0/CHECKLIST.md`, P0-13 is ticked and P0-14 is not.
3. Run `cargo xtask ci`. Expected: `xtask: all checks passed`, 49 tests.

## Context

- Finding LC-001 (CWE-409, decompression bomb): `AlsParser::decompress_to_xml` in `crates/livecraft-core/src/als.rs` reads the whole gzip stream into a `String` with no limit. A few kilobytes of gzip can expand to gigabytes.
- Live 12.4's default template is about 180 KB of XML; real sets with many devices reach tens of MB. 512 MiB leaves a wide margin.
- Reading into bytes first (then checking UTF-8) makes the limit check exact even when the cut falls inside a multi-byte character.
- `AlsError` is defined at the top of `als.rs` and re-exported by `livecraft_core` (`pub use als::*`). `crates/livecraft-core/src/tests.rs` is the body of the crate's test module (`use super::*;` at its top).

## Steps

1. Test first. Append to the end of `crates/livecraft-core/src/tests.rs`:

   ```rust

   #[test]
   fn test_als_decompression_rejects_oversized_output() {
       use std::io::Write;
       let mut encoder =
           flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
       encoder.write_all(&[b'a'; 4096]).unwrap();
       let gz = encoder.finish().unwrap();

       let err = AlsParser::decompress_reader(gz.as_slice(), 1024).unwrap_err();
       assert!(matches!(err, AlsError::TooLarge { limit: 1024 }));

       let xml = AlsParser::decompress_reader(gz.as_slice(), 4096).unwrap();
       assert_eq!(xml.len(), 4096);
   }
   ```

2. In `crates/livecraft-core/src/als.rs`, find:

   ```rust
       #[error("Invalid Ableton file format: {0}")]
       InvalidFormat(String),
   ```

   Replace it with:

   ```rust
       #[error("Invalid Ableton file format: {0}")]
       InvalidFormat(String),
       #[error("Decompressed Live Set is larger than the {limit}-byte limit")]
       TooLarge { limit: u64 },
   ```

3. In the same file, replace the entire function `decompress_to_xml` (its doc comment line `/// Decompress a .als file into a UTF-8 XML string`, its signature `pub fn decompress_to_xml<P: AsRef<Path>>(path: P) -> Result<String, AlsError> {`, its body, and its closing `}`) with:

   ```rust
       /// Largest decompressed `.als` XML accepted, in bytes (512 MiB).
       pub const MAX_XML_BYTES: u64 = 512 * 1024 * 1024;

       /// Decompress a .als file into a UTF-8 XML string, rejecting oversized content.
       pub fn decompress_to_xml<P: AsRef<Path>>(path: P) -> Result<String, AlsError> {
           let file = File::open(path)?;
           Self::decompress_reader(file, Self::MAX_XML_BYTES)
       }

       /// Decompress gzip data from `reader` into UTF-8 text of at most `limit` bytes.
       pub fn decompress_reader<R: Read>(reader: R, limit: u64) -> Result<String, AlsError> {
           let mut bytes = Vec::new();
           GzDecoder::new(reader)
               .take(limit.saturating_add(1))
               .read_to_end(&mut bytes)
               .map_err(|e| AlsError::Decompress(format!("Failed to decompress gzip stream: {e}")))?;
           if bytes.len() as u64 > limit {
               return Err(AlsError::TooLarge { limit });
           }
           String::from_utf8(bytes)
               .map_err(|e| AlsError::Decompress(format!("Live Set XML is not UTF-8: {e}")))
       }
   ```

4. Run `cargo fmt --all`.

5. In `SECURITY.md`, change the LC-001 row: Status `Fixed`, Fix cell `512 MiB cap in AlsParser::decompress_reader, test test_als_decompression_rejects_oversized_output (Task P0-14)`. In `ARCHITECTURE.md`, delete the Known Violations row that starts with `` | `.als` decompression has no size limit``.

## Do Not

- Change files other than: `crates/livecraft-core/src/als.rs`, `crates/livecraft-core/src/tests.rs`, `SECURITY.md`, `ARCHITECTURE.md`, `plans/livecraft-1.0/CHECKLIST.md`.

## Verify

| Command                                                                        | Expected result                                                                                |
| ------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------- |
| `cargo test -p livecraft-core test_als_decompression_rejects_oversized_output` | 1 passed                                                                                       |
| `cargo xtask ci`                                                               | `xtask: all checks passed`, 50 tests                                                           |
| `cargo dev -- info .agent-local/output/p0-11/demo.als`                         | exit 0 (create it first with `cargo dev -- new .agent-local/output/p0-11/demo.als` if missing) |

## Commit And Push

1. Tick P0-14 in `plans/livecraft-1.0/CHECKLIST.md`.
2. `git add crates/livecraft-core/src/als.rs crates/livecraft-core/src/tests.rs SECURITY.md ARCHITECTURE.md plans/livecraft-1.0/CHECKLIST.md`
3. Commit:

   ```text
   Cap .als decompression at 512 MiB

   - Add AlsParser::decompress_reader with an explicit byte limit
   - Return AlsError::TooLarge for decompression bombs, with a regression test
   - Mark finding LC-001 fixed
   ```

4. `git push origin main`, then `git status --short --branch`: clean, not ahead.

## If Something Fails

- Fix within the listed files, at most 3 attempts. Still failing: append under Blockers in `CHECKLIST.md`: `P0-14, <date>: <what failed>`, the command, and the last 20 lines of output. Commit only `CHECKLIST.md`, push, stop.
- `git push` rejected: `git pull --rebase`, rerun Verify, push. If the rebase conflicts: `git rebase --abort`, log a blocker, stop.
