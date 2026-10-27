# Task P0-15: One WAV loader with a size limit (finding LC-002)

Depends on: P0-14. Size: 4 files, about 90 changed lines (net reduction). Milestone: P0.

## Goal

There is one WAV decoding function, it rejects files holding more than 2^28 samples before reading sample data, and Simpler uses it instead of its duplicate copy; two integration tests prove the limit.

## Before You Start

1. `git pull --ff-only`, then `git status --short`: empty.
2. In `plans/livecraft-1.0/CHECKLIST.md`, P0-14 is `[x]` or `[~]` and P0-15 is `[ ]`.
3. Run `cargo xtask ci`. Expected: `xtask: all checks passed`, 50 tests.

## Context

- Finding LC-002 (CWE-400): WAV files are decoded fully into memory with no size check.
- Duplication: `AudioSampleCache::load_file` in `crates/livecraft-dsp/src/audio_clip.rs` and `SimplerInstrument::load_wav` in `crates/livecraft-dsp/src/simpler.rs` contain the same decoding loop, and `SimplerSample` has the same four fields as `DecodedAudio` (`channels: u16`, `sample_rate: u32`, `samples_l: Vec<f32>`, `samples_r: Vec<f32>`). The only difference: Simpler scales integers by `2^(bits-1) - 1`, the cache by `2^(bits-1)`. The cache's convention (full scale is `2^(bits-1)`) is kept; the level change in Simpler is below 0.003 percent and intentional.
- A type alias keeps every existing `SimplerSample { channels, sample_rate, samples_l, samples_r }` struct expression compiling (Rust allows struct literals through aliases).
- 2^28 samples is 1 GiB as `f32`; longer files need disk streaming (milestone M11).
- Integration tests can write scratch files to `env!("CARGO_TARGET_TMPDIR")`, which Cargo provides inside `target/`.

## Steps

1. Test first. Create `crates/livecraft-dsp/tests/wav_limits.rs` with exactly:

   ```rust
   use livecraft_dsp::audio_clip::AudioSampleCache;
   use std::path::PathBuf;

   fn write_test_wav(name: &str, frames: u32) -> PathBuf {
       let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
       let spec = hound::WavSpec {
           channels: 2,
           sample_rate: 44_100,
           bits_per_sample: 16,
           sample_format: hound::SampleFormat::Int,
       };
       let mut writer = hound::WavWriter::create(&path, spec).unwrap();
       for i in 0..frames {
           let value = (i % 100) as i16 * 100;
           writer.write_sample(value).unwrap();
           writer.write_sample(-value).unwrap();
       }
       writer.finalize().unwrap();
       path
   }

   #[test]
   fn wav_within_limit_loads() {
       let path = write_test_wav("wav_within_limit.wav", 1000);
       let audio = AudioSampleCache::load_file_with_limit(&path, 2000).unwrap();
       assert_eq!(audio.channels, 2);
       assert_eq!(audio.sample_rate, 44_100);
       assert_eq!(audio.samples_l.len(), 1000);
       assert_eq!(audio.samples_r.len(), 1000);
   }

   #[test]
   fn wav_over_limit_is_rejected() {
       let path = write_test_wav("wav_over_limit.wav", 1000);
       let err = AudioSampleCache::load_file_with_limit(&path, 1999).unwrap_err();
       assert!(err.to_string().contains("over the limit"), "{err}");
   }
   ```

2. In `crates/livecraft-dsp/src/audio_clip.rs`, find:

   ```rust
       fn load_file(path: &Path) -> anyhow::Result<DecodedAudio> {
           let mut reader = WavReader::open(path)?;
           let spec = reader.spec();
   ```

   Replace it with:

   ```rust
       /// Most samples (all channels together) a WAV file may hold to be decoded into memory:
       /// 2^28 samples, 1 GiB as `f32`. Longer files need disk streaming (milestone M11).
       pub const MAX_WAV_SAMPLES: u64 = 1 << 28;

       /// Decodes a WAV file into de-interleaved left and right channels (mono is copied to both).
       pub fn load_file(path: &Path) -> anyhow::Result<DecodedAudio> {
           Self::load_file_with_limit(path, Self::MAX_WAV_SAMPLES)
       }

       /// Like [`Self::load_file`], but rejects a file holding more than `max_samples` samples
       /// before reading any sample data.
       pub fn load_file_with_limit(path: &Path, max_samples: u64) -> anyhow::Result<DecodedAudio> {
           let mut reader = WavReader::open(path)?;
           let spec = reader.spec();
           let total = u64::from(reader.duration()) * u64::from(spec.channels);
           if total > max_samples {
               anyhow::bail!(
                   "{} holds {total} samples, over the limit of {max_samples}",
                   path.display()
               );
           }
           if spec.channels == 0 {
               anyhow::bail!("{} has no audio channels", path.display());
           }
   ```

3. In `crates/livecraft-dsp/src/simpler.rs`, find:

   ```rust
   #[derive(Debug, Clone)]
   pub struct SimplerSample {
       pub channels: u16,
       pub sample_rate: u32,
       pub samples_l: Vec<f32>,
       pub samples_r: Vec<f32>,
   }
   ```

   Replace it with:

   ```rust
   /// Decoded sample data played by Simpler; the same type the audio clip cache uses.
   pub type SimplerSample = crate::audio_clip::DecodedAudio;
   ```

4. In the same file, replace the entire function `load_wav` (from the line `    pub fn load_wav<P: AsRef<Path>>(&mut self, path: P) -> anyhow::Result<()> {` through its closing line `    }` that follows `        Ok(())`) with:

   ```rust
       pub fn load_wav<P: AsRef<Path>>(&mut self, path: P) -> anyhow::Result<()> {
           self.sample = Some(crate::audio_clip::AudioSampleCache::load_file(path.as_ref())?);
           Ok(())
       }
   ```

5. Run `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings`. If clippy reports an unused import in `simpler.rs`, delete that import line.

6. In `SECURITY.md`, change the LC-002 row: Status `Fixed`, Location `` `AudioSampleCache::load_file_with_limit` in `crates/livecraft-dsp/src/audio_clip.rs` ``, Fix `2^28-sample cap before decoding; tests in crates/livecraft-dsp/tests/wav_limits.rs (Task P0-15)`.

## Do Not

- Change files other than: `crates/livecraft-dsp/tests/wav_limits.rs` (new), `crates/livecraft-dsp/src/audio_clip.rs`, `crates/livecraft-dsp/src/simpler.rs`, `SECURITY.md`, `plans/livecraft-1.0/CHECKLIST.md`.

## Verify

| Command                                                                      | Expected result                      |
| ---------------------------------------------------------------------------- | ------------------------------------ |
| `cargo test -p livecraft-dsp --test wav_limits`                              | 2 passed                             |
| `grep -c "for sample in reader.samples" crates/livecraft-dsp/src/simpler.rs` | `0`                                  |
| `cargo xtask ci`                                                             | `xtask: all checks passed`, 52 tests |

## Commit And Push

1. Tick P0-15 in `plans/livecraft-1.0/CHECKLIST.md`.
2. `git add crates/livecraft-dsp/tests/wav_limits.rs crates/livecraft-dsp/src/audio_clip.rs crates/livecraft-dsp/src/simpler.rs SECURITY.md plans/livecraft-1.0/CHECKLIST.md`
3. Commit:

   ```text
   Use one WAV loader with a 2^28-sample limit

   - Reject oversized WAV files before decoding, with integration tests
   - Make Simpler use the shared loader and DecodedAudio type
   - Mark finding LC-002 fixed
   ```

4. `git push origin main`, then `git status --short --branch`: clean, not ahead. Continue with the next line.

## If Something Fails

- `test_simpler_playback_modes` fails: log a blocker with its output (the scaling change was expected to be inaudible).
- Other failures: fix within the listed files, at most 3 attempts. Still failing after the Unblock Procedure in `plans/livecraft-1.0/START-HERE.md`: append under Blockers in `CHECKLIST.md`: `P0-15, <date>: <what failed>`, the command, and the last 20 lines of output. Commit only `CHECKLIST.md`, push, end the run.
- `git push` rejected: `git pull --rebase`, rerun Verify, push. If the rebase conflicts: `git rebase --abort`, then the Unblock Procedure in `plans/livecraft-1.0/START-HERE.md`.
