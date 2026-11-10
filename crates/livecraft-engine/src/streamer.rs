//! Lock-free disk streaming engine for long audio clips (`ram_mode == false`).
//!
//! `DiskStreamer` owns background decoding/resampling state and communicates with audio-thread
//! [`StreamAudioPlayer`] instances strictly over wait-free `rtrb` ring buffers and atomics.
//! [`StreamerMode::Manual`] provides deterministic synchronous pumping (`pump_sync`) for offline
//! rendering (`VirtualDriver` / `OfflineBouncer`).

use crate::audio_clip::hermite_interp;
use crate::resample::{StreamingResampler, resampled_frame_count};
use livecraft_formats::audio_decode::AudioStreamReader;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Number of source stereo frames read from `AudioStreamReader` per chunk.
const READ_CHUNK_FRAMES: usize = 4096;

/// Capacity of the audio-to-disk seek command ring buffer.
const COMMAND_RING_CAPACITY: usize = 256;

/// Maximum forward gap (in frames) before triggering a `Seek` when frames are not already
/// buffered in the ring.
const FORWARD_SEEK_THRESHOLD_FRAMES: u64 = 64;

/// Maximum decoding chunks pumped per slot in a single pass so no slot starves others.
const MAX_CHUNKS_PER_PASS: usize = 32;

/// Controls whether [`DiskStreamer`] runs a background worker thread or is driven deterministically.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamerMode {
    /// Spawns a background worker thread that continuously processes seeks and refills buffers.
    Async,
    /// Performs no background threading; callers invoke [`DiskStreamer::pump_sync`] explicitly.
    Manual,
}

/// Commands sent from the audio thread (`StreamAudioPlayer`) to `DiskStreamer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamCommand {
    Seek { frame: u64, generation: u64 },
}

/// Lock-free telemetry and synchronization state shared between a `StreamSlot` and its
/// [`StreamAudioPlayer`].
pub struct StreamStats {
    pub active: AtomicBool,
    pub underruns: AtomicU64,
    pub disk_bytes_per_sec: AtomicU64,
    pub disk_load_ppm: AtomicU32,
    pub ready_generation: AtomicU64,
    pub seek_boundary_pos: AtomicU64,
    pub known_total_frames: AtomicU64,
    pub eof_reached: AtomicBool,
}

impl StreamStats {
    fn new(total_frames_est: u64, eof_reached: bool) -> Self {
        Self {
            active: AtomicBool::new(true),
            underruns: AtomicU64::new(0),
            disk_bytes_per_sec: AtomicU64::new(0),
            disk_load_ppm: AtomicU32::new(0),
            ready_generation: AtomicU64::new(0),
            seek_boundary_pos: AtomicU64::new(0),
            known_total_frames: AtomicU64::new(total_frames_est),
            eof_reached: AtomicBool::new(eof_reached),
        }
    }
}

/// Disk-side streaming slot owning the open file reader, resampler, and ring producer.
struct StreamSlot {
    reader: AudioStreamReader,
    resampler: StreamingResampler,
    src_rate: u32,
    dst_rate: u32,
    src_total_frames: Option<u64>,
    frame_tx: rtrb::Producer<[f32; 2]>,
    cmd_rx: rtrb::Consumer<StreamCommand>,
    stats: Arc<StreamStats>,
    read_scratch: Vec<[f32; 2]>,
    pending_out: Vec<[f32; 2]>,
    pending_out_offset: usize,
    total_pushed: u64,
    dst_cursor: u64,
    eof: bool,
    flushed: bool,
}

struct StreamerInner {
    mode: StreamerMode,
    slots: Mutex<Vec<StreamSlot>>,
    stop: AtomicBool,
}

/// Manages open disk-streamed audio sources in either [`StreamerMode::Async`] or
/// [`StreamerMode::Manual`] mode.
pub struct DiskStreamer {
    inner: Arc<StreamerInner>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl DiskStreamer {
    /// Creates a new [`DiskStreamer`] in the given `mode`.
    #[must_use]
    pub fn new(mode: StreamerMode) -> Arc<Self> {
        let inner = Arc::new(StreamerInner {
            mode,
            slots: Mutex::new(Vec::new()),
            stop: AtomicBool::new(false),
        });

        let worker = match mode {
            StreamerMode::Async => {
                let worker_inner = Arc::clone(&inner);
                match thread::Builder::new()
                    .name("livecraft-disk-streamer".to_string())
                    .spawn(move || run_worker(&worker_inner))
                {
                    Ok(handle) => Some(handle),
                    Err(err) => {
                        log::error!("Failed to spawn disk streamer thread: {err}");
                        None
                    }
                }
            }
            StreamerMode::Manual => None,
        };

        Arc::new(Self {
            inner,
            worker: Mutex::new(worker),
        })
    }

    /// Creates an [`Async`](StreamerMode::Async) disk streamer.
    #[must_use]
    pub fn new_async() -> Arc<Self> {
        Self::new(StreamerMode::Async)
    }

    /// Creates a [`Manual`](StreamerMode::Manual) disk streamer driven via [`Self::pump_sync`].
    #[must_use]
    pub fn new_manual() -> Arc<Self> {
        Self::new(StreamerMode::Manual)
    }

    /// Returns the operating mode of this streamer.
    #[must_use]
    pub fn mode(&self) -> StreamerMode {
        self.inner.mode
    }

    /// Returns the number of currently active stream slots.
    #[must_use]
    pub fn active_slot_count(&self) -> usize {
        self.inner
            .slots
            .lock()
            .map(|slots| {
                slots
                    .iter()
                    .filter(|s| s.stats.active.load(Ordering::Acquire))
                    .count()
            })
            .unwrap_or(0)
    }

    /// Opens `path` for disk streaming at `engine_sample_rate` and synchronously preloads the
    /// first 1.0 second of resampled stereo frames before returning `(player, total_frames, engine_rate)`.
    pub fn open_stream(
        self: &Arc<Self>,
        path: &Path,
        engine_sample_rate: u32,
    ) -> anyhow::Result<(StreamAudioPlayer, u64, u32)> {
        let dst_rate = engine_sample_rate.max(1);
        let mut reader = AudioStreamReader::open(path)?;
        let src_rate = reader.sample_rate().max(1);
        let src_total_frames = reader.total_frames();
        let mut total_frames_est = src_total_frames
            .map(|f| resampled_frame_count(f, src_rate, dst_rate))
            .unwrap_or(u64::MAX);

        let mut resampler = StreamingResampler::new(src_rate, dst_rate)?;
        let ring_capacity = (4 * dst_rate.max(44100)) as usize;
        let preload_frames = dst_rate as usize;
        let preload_with_lookahead = (preload_frames + 4).min(ring_capacity);

        let mut read_scratch = Vec::with_capacity(READ_CHUNK_FRAMES);
        let mut pending_out = Vec::with_capacity(preload_with_lookahead + READ_CHUNK_FRAMES);
        let mut eof = false;
        let mut flushed = false;

        while pending_out.len() < preload_with_lookahead && !eof {
            let n = reader.read_frames(READ_CHUNK_FRAMES, &mut read_scratch)?;
            if n == 0 {
                resampler.flush(&mut pending_out)?;
                eof = true;
                flushed = true;
                break;
            }
            resampler.process_chunk(&read_scratch[..n], &mut pending_out)?;
        }

        let (mut frame_tx, frame_rx) = rtrb::RingBuffer::<[f32; 2]>::new(ring_capacity);
        let (cmd_tx, cmd_rx) = rtrb::RingBuffer::<StreamCommand>::new(COMMAND_RING_CAPACITY);

        let initial_push_count = pending_out.len().min(ring_capacity);
        let mut pushed = 0usize;
        for &frame in &pending_out[..initial_push_count] {
            if frame_tx.push(frame).is_err() {
                break;
            }
            pushed += 1;
        }
        let head_len = pushed.min(preload_with_lookahead);
        let head_cache: Box<[[f32; 2]]> = pending_out[..head_len].to_vec().into_boxed_slice();
        let pending_out_offset = pushed;
        let dst_cursor = pushed as u64;
        let total_pushed = pushed as u64;
        let done_eof = eof && pending_out_offset >= pending_out.len();
        if done_eof {
            total_frames_est = dst_cursor;
        }

        let stats = Arc::new(StreamStats::new(total_frames_est, done_eof));
        let slot = StreamSlot {
            reader,
            resampler,
            src_rate,
            dst_rate,
            src_total_frames,
            frame_tx,
            cmd_rx,
            stats: Arc::clone(&stats),
            read_scratch,
            pending_out,
            pending_out_offset,
            total_pushed,
            dst_cursor,
            eof,
            flushed,
        };

        if let Ok(mut slots) = self.inner.slots.lock() {
            slots.retain(|s| s.stats.active.load(Ordering::Acquire));
            slots.push(slot);
        } else {
            anyhow::bail!("disk streamer slot mutex poisoned");
        }

        self.wake_worker();

        let stream_key = compute_stream_key(path, dst_rate);
        let player = StreamAudioPlayer {
            stream_key,
            frame_rx,
            cmd_tx,
            stats,
            _streamer: Arc::clone(self),
            head_cache,
            preload_frames,
            ring_capacity,
            window: [[0.0, 0.0]; 4],
            valid_count: 0,
            next_frame: 0,
            rx_frame: 0,
            total_popped: 0,
            generation: 0,
            awaiting_generation: 0,
            pending_seek_frame: 0,
            used_head_cache_since_seek: false,
            last_underruns: 0,
            block_start_underruns: 0,
            active_in_block: false,
        };

        Ok((player, total_frames_est, dst_rate))
    }

    /// Synchronously processes pending seek commands and refills active ring buffers.
    ///
    /// Called deterministically before each block by `VirtualDriver` when running in
    /// [`StreamerMode::Manual`], or directly in tests.
    pub fn pump_sync(&self) {
        let _ = pump_all(&self.inner);
    }

    fn wake_worker(&self) {
        if self.inner.mode == StreamerMode::Async
            && let Ok(guard) = self.worker.lock()
            && let Some(handle) = guard.as_ref()
        {
            handle.thread().unpark();
        }
    }
}

impl Drop for DiskStreamer {
    fn drop(&mut self) {
        self.inner.stop.store(true, Ordering::Release);
        self.wake_worker();
        if let Ok(mut guard) = self.worker.lock()
            && let Some(handle) = guard.take()
            && handle.join().is_err()
        {
            log::error!("Disk streamer thread panicked during shutdown");
        }
    }
}

fn run_worker(inner: &StreamerInner) {
    while !inner.stop.load(Ordering::Acquire) {
        let (did_work, needs_urgent_refill) = pump_all(inner);
        if inner.stop.load(Ordering::Acquire) {
            break;
        }
        let sleep_dur = if needs_urgent_refill {
            Duration::from_millis(1)
        } else if did_work {
            Duration::from_millis(2)
        } else {
            Duration::from_millis(10)
        };
        thread::park_timeout(sleep_dur);
    }
}

/// Drops inactive slots and pumps all active slots once.
/// Returns `(did_work, needs_urgent_refill)`.
fn pump_all(inner: &StreamerInner) -> (bool, bool) {
    let Ok(mut slots) = inner.slots.lock() else {
        return (false, false);
    };
    slots.retain(|slot| slot.stats.active.load(Ordering::Acquire));
    let mut any_work = false;
    let mut needs_urgent = false;
    for slot in slots.iter_mut() {
        let (worked, urgent) = pump_slot(slot);
        any_work |= worked;
        needs_urgent |= urgent;
    }
    (any_work, needs_urgent)
}

fn pump_slot(slot: &mut StreamSlot) -> (bool, bool) {
    let start = Instant::now();
    let mut did_work = false;
    let mut frames_decoded = 0u64;

    // 1. Drain queued seek commands, coalescing to the latest request.
    let mut latest_seek: Option<(u64, u64)> = None;
    while let Ok(cmd) = slot.cmd_rx.pop() {
        match cmd {
            StreamCommand::Seek { frame, generation } => {
                latest_seek = Some((frame, generation));
            }
        }
    }

    if let Some((dst_frame, generation)) = latest_seek {
        did_work = true;
        let seek_boundary = slot.total_pushed;
        let src_frame = if slot.src_rate == slot.dst_rate {
            dst_frame
        } else {
            ((dst_frame as f64) * f64::from(slot.src_rate) / f64::from(slot.dst_rate)).floor()
                as u64
        };
        let clamped_src = slot
            .src_total_frames
            .map_or(src_frame, |total| src_frame.min(total));

        slot.resampler.reset();
        slot.pending_out.clear();
        slot.pending_out_offset = 0;
        slot.eof = false;
        slot.flushed = false;
        slot.dst_cursor = dst_frame;

        if let Err(err) = slot.reader.seek(clamped_src) {
            log::warn!("Stream seek to frame {clamped_src} failed: {err}");
            slot.eof = true;
            slot.flushed = true;
        }

        let target_fill = (slot.dst_rate as usize + 4).min(slot.frame_tx.slots());
        let mut pushed_after_seek = 0usize;
        while pushed_after_seek < target_fill {
            pushed_after_seek += push_pending_to_ring(slot);
            if slot.pending_out_offset < slot.pending_out.len() || slot.eof {
                break;
            }
            match decode_one_chunk(slot) {
                Ok(n) => frames_decoded += n as u64,
                Err(err) => {
                    log::warn!("Stream decode error after seek: {err}");
                    slot.eof = true;
                    slot.flushed = true;
                    break;
                }
            }
        }
        let _ = push_pending_to_ring(slot);

        let done_eof = slot.eof && slot.pending_out_offset >= slot.pending_out.len();
        slot.stats.eof_reached.store(done_eof, Ordering::Release);
        if done_eof && slot.stats.known_total_frames.load(Ordering::Acquire) == u64::MAX {
            slot.stats
                .known_total_frames
                .store(slot.dst_cursor, Ordering::Release);
        }
        // Store boundary BEFORE ready_generation so the audio thread never reads a stale boundary.
        slot.stats
            .seek_boundary_pos
            .store(seek_boundary, Ordering::Release);
        slot.stats
            .ready_generation
            .store(generation, Ordering::Release);
    }

    // 2. Refill ring buffer toward capacity.
    let mut chunks = 0usize;
    while slot.frame_tx.slots() > 0 && chunks < MAX_CHUNKS_PER_PASS {
        if push_pending_to_ring(slot) > 0 {
            did_work = true;
        }
        if slot.pending_out_offset < slot.pending_out.len() {
            break;
        }
        if slot.eof {
            break;
        }
        match decode_one_chunk(slot) {
            Ok(n) => {
                did_work = true;
                frames_decoded += n as u64;
                chunks += 1;
            }
            Err(err) => {
                log::warn!("Stream decode error: {err}");
                slot.eof = true;
                slot.flushed = true;
                break;
            }
        }
    }
    if push_pending_to_ring(slot) > 0 {
        did_work = true;
    }

    let done_eof = slot.eof && slot.pending_out_offset >= slot.pending_out.len();
    if done_eof {
        slot.stats.eof_reached.store(true, Ordering::Release);
        if slot.stats.known_total_frames.load(Ordering::Acquire) == u64::MAX {
            slot.stats
                .known_total_frames
                .store(slot.dst_cursor, Ordering::Release);
        }
    } else {
        slot.stats.eof_reached.store(false, Ordering::Release);
    }

    if did_work {
        let elapsed_secs = start.elapsed().as_secs_f64().max(1e-9);
        let bytes = frames_decoded * 8;
        slot.stats
            .disk_bytes_per_sec
            .store((bytes as f64 / elapsed_secs) as u64, Ordering::Relaxed);
        let audio_secs = frames_decoded as f64 / f64::from(slot.src_rate.max(1));
        let ppm = if audio_secs > 0.0 {
            ((elapsed_secs / audio_secs) * 1_000_000.0).min(2_000_000.0) as u32
        } else {
            0
        };
        slot.stats.disk_load_ppm.store(ppm, Ordering::Relaxed);
    } else {
        slot.stats.disk_load_ppm.store(0, Ordering::Relaxed);
    }

    let needs_urgent = !done_eof && slot.frame_tx.slots() >= (slot.dst_rate as usize * 2);
    (did_work, needs_urgent)
}

fn push_pending_to_ring(slot: &mut StreamSlot) -> usize {
    let mut pushed = 0usize;
    while slot.pending_out_offset < slot.pending_out.len() && slot.frame_tx.slots() > 0 {
        let frame = slot.pending_out[slot.pending_out_offset];
        if slot.frame_tx.push(frame).is_err() {
            break;
        }
        slot.pending_out_offset += 1;
        slot.total_pushed += 1;
        slot.dst_cursor += 1;
        pushed += 1;
    }
    if slot.pending_out_offset >= slot.pending_out.len() {
        slot.pending_out.clear();
        slot.pending_out_offset = 0;
    }
    pushed
}

fn decode_one_chunk(slot: &mut StreamSlot) -> anyhow::Result<usize> {
    if slot.eof {
        return Ok(0);
    }
    let n = slot
        .reader
        .read_frames(READ_CHUNK_FRAMES, &mut slot.read_scratch)?;
    if n == 0 {
        if !slot.flushed {
            slot.resampler.flush(&mut slot.pending_out)?;
            slot.flushed = true;
        }
        slot.eof = true;
        return Ok(0);
    }
    slot.resampler
        .process_chunk(&slot.read_scratch[..n], &mut slot.pending_out)?;
    Ok(n)
}

fn compute_stream_key(path: &Path, sample_rate: u32) -> u64 {
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in canonical.to_string_lossy().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    for byte in sample_rate.to_le_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

/// Audio-thread playback side of a disk-streamed clip.
///
/// Audio-thread methods perform ZERO heap allocations, ZERO locks, ZERO logging, and ZERO I/O.
pub struct StreamAudioPlayer {
    stream_key: u64,
    frame_rx: rtrb::Consumer<[f32; 2]>,
    cmd_tx: rtrb::Producer<StreamCommand>,
    stats: Arc<StreamStats>,
    _streamer: Arc<DiskStreamer>,
    head_cache: Box<[[f32; 2]]>,
    preload_frames: usize,
    ring_capacity: usize,
    window: [[f32; 2]; 4],
    valid_count: usize,
    next_frame: u64,
    rx_frame: u64,
    total_popped: u64,
    generation: u64,
    awaiting_generation: u64,
    pending_seek_frame: u64,
    used_head_cache_since_seek: bool,
    last_underruns: u64,
    block_start_underruns: u64,
    active_in_block: bool,
}

impl StreamAudioPlayer {
    /// Deterministic identifier for the resolved file path and sample rate of this stream.
    #[must_use]
    pub fn stream_key(&self) -> u64 {
        self.stream_key
    }

    /// Marks the beginning of a render block on the audio thread (lock-free, allocation-free).
    #[inline]
    pub fn begin_block(&mut self) {
        self.active_in_block = false;
        self.block_start_underruns = self.last_underruns;
    }

    /// Returns the total frame count, refining unknown-length streams once EOF is reached.
    #[inline]
    pub fn effective_total_frames(&self, fallback: u64) -> u64 {
        let known = self.stats.known_total_frames.load(Ordering::Acquire);
        if known != u64::MAX { known } else { fallback }
    }

    /// Returns the number of pre-buffered stereo frames currently sitting in the ring buffer.
    #[inline]
    #[must_use]
    pub fn ring_available(&self) -> usize {
        self.frame_rx.slots()
    }

    /// Total underruns recorded by this player.
    #[inline]
    #[must_use]
    pub fn underruns(&self) -> u64 {
        self.stats.underruns.load(Ordering::Relaxed)
    }

    /// Lock-free disk load estimate (`0.0` when idle or full, rising toward `1.0` when starved).
    #[inline]
    #[must_use]
    pub fn disk_load(&self) -> f32 {
        if !self.active_in_block {
            return 0.0;
        }
        let eof = self.stats.eof_reached.load(Ordering::Relaxed);
        let known_total = self.stats.known_total_frames.load(Ordering::Relaxed);
        let at_end = eof && known_total != u64::MAX && self.next_frame >= known_total;
        let buffer_pressure = if at_end {
            0.0
        } else {
            let avail = self.frame_rx.slots() as f32;
            let preload = self.preload_frames.max(1) as f32;
            1.0 - (avail / preload).clamp(0.0, 1.0)
        };
        let underrun_pressure = if self.last_underruns > self.block_start_underruns {
            1.0
        } else {
            0.0
        };
        let io_load = self.stats.disk_load_ppm.load(Ordering::Relaxed) as f32 / 1_000_000.0;
        io_load.max(buffer_pressure).max(underrun_pressure)
    }

    /// Reads an interpolated stereo `[L, R]` sample at fractional frame position `index + mu`
    /// using the 4-frame [`hermite_interp`] sliding window.
    ///
    /// Executes strictly wait-free with zero allocations, locks, logs, or I/O.
    pub fn read_interpolated(
        &mut self,
        index: u64,
        mu: f32,
        fallback_total_frames: u64,
    ) -> [f32; 2] {
        self.active_in_block = true;
        let total_frames = self.effective_total_frames(fallback_total_frames);
        if total_frames == 0 || index >= total_frames {
            return [0.0, 0.0];
        }

        let last = total_frames - 1;
        let i0 = index.saturating_sub(1).min(last);
        let i1 = index.min(last);
        let i2 = (index + 1).min(last);
        let i3 = (index + 2).min(last);
        let needed_min = i0;
        let needed_max = i3;

        // 1. Synchronize any in-flight seek once DiskStreamer publishes ready_generation.
        if self.awaiting_generation != 0 {
            let ready_gen = self.stats.ready_generation.load(Ordering::Acquire);
            if ready_gen >= self.awaiting_generation {
                let boundary = self.stats.seek_boundary_pos.load(Ordering::Acquire);
                while self.total_popped < boundary {
                    if self.frame_rx.pop().is_ok() {
                        self.total_popped += 1;
                    } else {
                        return self.record_underrun();
                    }
                }
                self.awaiting_generation = 0;
            }
        }

        // 2. Detect backward jump (loop wrap / backwards seek) or far forward jump.
        let window_start = self.next_frame.saturating_sub(self.valid_count as u64);
        if self.awaiting_generation != 0 {
            if needed_min < self.pending_seek_frame
                || needed_max
                    > self
                        .pending_seek_frame
                        .saturating_add(self.ring_capacity as u64)
            {
                self.issue_seek(needed_min);
            }
        } else if needed_min < window_start {
            self.issue_seek(needed_min);
        } else if needed_max >= self.next_frame {
            let ahead = needed_max - self.next_frame + 1;
            let buffered_ahead = self.frame_rx.slots() as u64;
            if ahead > buffered_ahead && ahead > FORWARD_SEEK_THRESHOLD_FRAMES {
                self.issue_seek(needed_min);
            }
        }

        // 3. Advance sliding window up through `needed_max`.
        while self.next_frame <= needed_max {
            let target = self.next_frame;
            let mut got_frame: Option<[f32; 2]> = None;

            if self.awaiting_generation == 0 {
                while self.rx_frame < target {
                    if self.frame_rx.pop().is_ok() {
                        self.total_popped += 1;
                        self.rx_frame += 1;
                    } else {
                        break;
                    }
                }
                if self.rx_frame == target
                    && let Ok(sample) = self.frame_rx.pop()
                {
                    self.total_popped += 1;
                    self.rx_frame += 1;
                    got_frame = Some(sample);
                }
            }

            if got_frame.is_none()
                && self.used_head_cache_since_seek
                && (target as usize) < self.head_cache.len()
            {
                got_frame = Some(self.head_cache[target as usize]);
            }

            let Some(frame) = got_frame else {
                return self.record_underrun();
            };

            self.push_window(frame);
            if (target as usize) >= self.head_cache.len() {
                self.used_head_cache_since_seek = false;
            }
        }

        // 4. Read the four neighbors out of `self.window` and apply Hermite interpolation.
        let s0 = self.window_frame(i0);
        let s1 = self.window_frame(i1);
        let s2 = self.window_frame(i2);
        let s3 = self.window_frame(i3);
        [
            hermite_interp(s0[0], s1[0], s2[0], s3[0], mu),
            hermite_interp(s0[1], s1[1], s2[1], s3[1], mu),
        ]
    }

    fn issue_seek(&mut self, target_frame: u64) {
        while self.frame_rx.pop().is_ok() {
            self.total_popped += 1;
        }
        let next_gen = self.generation + 1;
        if self
            .cmd_tx
            .push(StreamCommand::Seek {
                frame: target_frame,
                generation: next_gen,
            })
            .is_ok()
        {
            self.generation = next_gen;
            self.awaiting_generation = next_gen;
            self.pending_seek_frame = target_frame;
            self.used_head_cache_since_seek = (target_frame as usize) < self.head_cache.len();
            self.rx_frame = target_frame;
            self.next_frame = target_frame;
            self.valid_count = 0;
        }
    }

    #[inline]
    fn push_window(&mut self, sample: [f32; 2]) {
        self.window[0] = self.window[1];
        self.window[1] = self.window[2];
        self.window[2] = self.window[3];
        self.window[3] = sample;
        self.valid_count = (self.valid_count + 1).min(4);
        self.next_frame += 1;
    }

    #[inline]
    fn window_frame(&self, frame: u64) -> [f32; 2] {
        let offset_from_end = self.next_frame.saturating_sub(1).saturating_sub(frame) as usize;
        let slot = 3usize.saturating_sub(offset_from_end.min(3));
        self.window[slot]
    }

    #[inline]
    fn record_underrun(&mut self) -> [f32; 2] {
        self.last_underruns += 1;
        self.stats.underruns.fetch_add(1, Ordering::Relaxed);
        [0.0, 0.0]
    }
}

impl Drop for StreamAudioPlayer {
    fn drop(&mut self) {
        self.stats.active.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixtures_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("fixtures")
            .join("audio")
    }

    fn write_test_wav(path: &Path, sample_rate: u32, frames: usize) {
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(path, spec).expect("create wav");
        for i in 0..frames {
            let v = (i as f32 * 0.01).sin() * 0.5;
            writer.write_sample(v).expect("L");
            writer.write_sample(-v).expect("R");
        }
        writer.finalize().expect("finalize");
    }

    #[test]
    fn open_stream_preloads_first_second_for_immediate_zero_underrun_playback() {
        let dir = std::env::temp_dir().join(format!("livecraft_preload_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let wav = dir.join("three_sec.wav");
        write_test_wav(&wav, 44100, 44100 * 3);

        let streamer = DiskStreamer::new_manual();
        let (mut player, total, rate) = streamer.open_stream(&wav, 44100).expect("open_stream");
        assert_eq!(rate, 44100);
        assert_eq!(total, 44100 * 3);
        assert!(player.ring_available() >= 44100);

        // Play the entire first second WITHOUT ever calling pump_sync(): 0 underruns!
        player.begin_block();
        for f in 0..44100u64 {
            let s = player.read_interpolated(f, 0.0, total);
            let expected = (f as f32 * 0.01).sin() * 0.5;
            assert!((s[0] - expected).abs() < 1e-6, "frame {f}");
            assert!((s[1] + expected).abs() < 1e-6, "frame {f}");
        }
        assert_eq!(player.underruns(), 0);

        drop(player);
        streamer.pump_sync();
        assert_eq!(streamer.active_slot_count(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dropping_player_releases_slot_and_file_handle_cleanly() {
        let wav = fixtures_dir().join("sine_440.flac");
        let streamer = DiskStreamer::new_manual();
        let (player, _, _) = streamer.open_stream(&wav, 44100).expect("open");
        assert_eq!(streamer.active_slot_count(), 1);
        drop(player);
        streamer.pump_sync();
        assert_eq!(streamer.active_slot_count(), 0);
    }
}
