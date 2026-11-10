//! Background decode queue for preparing RAM-mode [`DecodedAudio`] via `basedrop::Shared`
//! off the session, UI, and audio threads.

use crate::audio_clip::{DecodedAudio, load_file};
use crate::resample::resample_decoded_audio;
use basedrop::{Handle, Shared};
use crossbeam_channel::{Receiver, Sender, unbounded};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

struct DecodeJob {
    path: PathBuf,
    target_sample_rate: u32,
}

/// Result produced by [`DecodeQueue`] once a background decode (and optional resample) finishes.
pub struct DecodedReady {
    pub path: PathBuf,
    pub target_sample_rate: u32,
    pub result: anyhow::Result<Shared<DecodedAudio>>,
}

/// Off-thread worker pool that decodes RAM-mode audio files, resamples them to the target
/// sample rate if needed, and wraps them in `basedrop::Shared<DecodedAudio>`.
pub struct DecodeQueue {
    job_tx: Option<Sender<DecodeJob>>,
    ready_rx: Receiver<DecodedReady>,
    in_flight: Arc<AtomicUsize>,
    workers: Vec<JoinHandle<()>>,
}

impl DecodeQueue {
    /// Starts a [`DecodeQueue`] with a single background worker thread bound to `handle`.
    #[must_use]
    pub fn new(handle: Handle) -> Self {
        Self::with_workers(handle, 1)
    }

    /// Starts a [`DecodeQueue`] with `worker_count` (at least 1) background worker threads.
    #[must_use]
    pub fn with_workers(handle: Handle, worker_count: usize) -> Self {
        let (job_tx, job_rx) = unbounded::<DecodeJob>();
        let (ready_tx, ready_rx) = unbounded::<DecodedReady>();
        let in_flight = Arc::new(AtomicUsize::new(0));
        let count = worker_count.max(1);
        let mut workers = Vec::with_capacity(count);

        for idx in 0..count {
            let rx = job_rx.clone();
            let tx = ready_tx.clone();
            let worker_handle = handle.clone();
            let pending = Arc::clone(&in_flight);
            let builder = thread::Builder::new().name(format!("livecraft-decode-{idx}"));
            match builder.spawn(move || {
                while let Ok(job) = rx.recv() {
                    let result = decode_and_wrap(&job, &worker_handle);
                    if tx
                        .send(DecodedReady {
                            path: job.path,
                            target_sample_rate: job.target_sample_rate,
                            result,
                        })
                        .is_err()
                    {
                        pending.fetch_sub(1, Ordering::AcqRel);
                    }
                }
            }) {
                Ok(thread) => workers.push(thread),
                Err(err) => {
                    log::error!("Failed to spawn decode worker thread {idx}: {err}");
                }
            }
        }

        Self {
            job_tx: Some(job_tx),
            ready_rx,
            in_flight,
            workers,
        }
    }

    /// Enqueues `path` for background decoding and optional resampling to `target_sample_rate`.
    /// Passing `target_sample_rate == 0` keeps the file's native sample rate.
    pub fn submit(&self, path: impl Into<PathBuf>, target_sample_rate: u32) {
        let Some(tx) = &self.job_tx else {
            return;
        };
        let path = path.into();
        self.in_flight.fetch_add(1, Ordering::AcqRel);
        if tx
            .send(DecodeJob {
                path,
                target_sample_rate,
            })
            .is_err()
        {
            self.in_flight.fetch_sub(1, Ordering::AcqRel);
        }
    }

    /// Non-blocking poll for the next finished decode job.
    #[must_use]
    pub fn try_recv_ready(&self) -> Option<DecodedReady> {
        let ready = self.ready_rx.try_recv().ok()?;
        self.in_flight.fetch_sub(1, Ordering::AcqRel);
        Some(ready)
    }

    /// Waits up to `timeout` for the next finished decode job.
    #[must_use]
    pub fn recv_ready_timeout(&self, timeout: Duration) -> Option<DecodedReady> {
        let ready = self.ready_rx.recv_timeout(timeout).ok()?;
        self.in_flight.fetch_sub(1, Ordering::AcqRel);
        Some(ready)
    }

    /// Number of submitted jobs whose results have not yet been received.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.in_flight.load(Ordering::Acquire)
    }
}

impl Drop for DecodeQueue {
    fn drop(&mut self) {
        drop(self.job_tx.take());
        for worker in self.workers.drain(..) {
            if worker.join().is_err() {
                log::error!("Decode worker thread panicked during shutdown");
            }
        }
    }
}

fn decode_and_wrap(job: &DecodeJob, handle: &Handle) -> anyhow::Result<Shared<DecodedAudio>> {
    let mut decoded = load_file(&job.path)?;
    if job.target_sample_rate > 0 && decoded.sample_rate != job.target_sample_rate {
        decoded = resample_decoded_audio(&decoded, job.target_sample_rate)?;
    }
    Ok(Shared::new(handle, decoded))
}

#[cfg(test)]
mod tests {
    use super::*;
    use basedrop::Collector;
    use std::path::Path;

    fn fixtures_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("fixtures")
            .join("audio")
    }

    #[test]
    fn decode_queue_prepares_shared_audio_off_thread_with_optional_resample() {
        let mut collector = Collector::new();
        {
            let queue = DecodeQueue::new(collector.handle());
            let flac_path = fixtures_dir().join("sine_440.flac");
            queue.submit(&flac_path, 48000);

            let ready = queue
                .recv_ready_timeout(Duration::from_secs(5))
                .expect("decoded result");
            assert_eq!(ready.path, flac_path);
            assert_eq!(ready.target_sample_rate, 48000);
            let shared = ready.result.expect("decode and resample ok");
            assert_eq!(shared.sample_rate, 48000);
            assert_eq!(shared.channels, 2);
            assert!(!shared.samples_l.is_empty());
            assert_eq!(queue.pending_count(), 0);
        }
        collector.collect();
        assert!(collector.try_cleanup().is_ok());
    }
}
