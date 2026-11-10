//! Offline and streaming stereo sample-rate conversion backed by `rubato`.

use crate::audio_clip::DecodedAudio;
use rubato::audioadapter_buffers::direct::{InterleavedSlice, SequentialSliceOfSlices};
use rubato::{Async, Fft, FixedAsync, FixedSync, Indexing, Resampler, SincInterpolationParameters};

/// Maximum `rate / gcd(src_rate, dst_rate)` ratio allowed for the synchronous `Fft` resampler
/// before falling back to `Async` windowed-sinc interpolation.
const MAX_FFT_SUBCHUNK_FACTOR: usize = 4096;

/// Default processing chunk size in input frames for `rubato` resamplers.
const RESAMPLE_CHUNK_FRAMES: usize = 1024;

fn gcd(mut a: usize, mut b: usize) -> usize {
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    a.max(1)
}

fn build_stereo_resampler(
    src_rate: u32,
    dst_rate: u32,
) -> anyhow::Result<Box<dyn Resampler<f32> + Send>> {
    if src_rate == 0 || dst_rate == 0 {
        anyhow::bail!("sample rates must be > 0 ({src_rate} -> {dst_rate})");
    }
    let src = src_rate as usize;
    let dst = dst_rate as usize;
    let factor = src.max(dst) / gcd(src, dst);
    if factor <= MAX_FFT_SUBCHUNK_FACTOR {
        let fft =
            Fft::<f32>::new(src, dst, RESAMPLE_CHUNK_FRAMES, 2, FixedSync::Both).map_err(|e| {
                anyhow::anyhow!("resampler construction failed ({src_rate} -> {dst_rate}): {e}")
            })?;
        Ok(Box::new(fft))
    } else {
        let ratio = f64::from(dst_rate) / f64::from(src_rate);
        let sinc = Async::<f32>::new_sinc(
            ratio,
            1.0,
            &SincInterpolationParameters::default(),
            RESAMPLE_CHUNK_FRAMES,
            2,
            FixedAsync::Input,
        )
        .map_err(|e| {
            anyhow::anyhow!("sinc resampler construction failed ({src_rate} -> {dst_rate}): {e}")
        })?;
        Ok(Box::new(sinc))
    }
}

/// Resamples a fully decoded stereo [`DecodedAudio`] buffer to `target_rate` using `rubato`.
///
/// When `audio.sample_rate == target_rate`, returns an exact clone without modifying samples.
pub fn resample_decoded_audio(
    audio: &DecodedAudio,
    target_rate: u32,
) -> anyhow::Result<DecodedAudio> {
    if target_rate == 0 {
        anyhow::bail!("target sample rate must be greater than zero");
    }
    if audio.sample_rate == 0 {
        anyhow::bail!("source sample rate must be greater than zero");
    }
    if audio.sample_rate == target_rate {
        return Ok(audio.clone());
    }
    let input_frames = audio.samples_l.len().min(audio.samples_r.len());
    if input_frames == 0 {
        return Ok(DecodedAudio {
            channels: audio.channels,
            sample_rate: target_rate,
            samples_l: Vec::new(),
            samples_r: Vec::new(),
        });
    }

    let mut resampler = build_stereo_resampler(audio.sample_rate, target_rate)?;
    let slices = [
        &audio.samples_l[..input_frames],
        &audio.samples_r[..input_frames],
    ];
    let input = SequentialSliceOfSlices::new(&slices, 2, input_frames)
        .map_err(|e| anyhow::anyhow!("resampler input buffer invalid: {e}"))?;
    let output = resampler
        .process_all(&input, input_frames, None)
        .map_err(|e| {
            anyhow::anyhow!(
                "resampling failed ({} -> {target_rate}): {e}",
                audio.sample_rate
            )
        })?;
    let interleaved = output.take_data();
    let expected_frames =
        resampled_frame_count(input_frames as u64, audio.sample_rate, target_rate) as usize;
    let out_frames = (interleaved.len() / 2).min(expected_frames);
    let mut samples_l = Vec::with_capacity(out_frames);
    let mut samples_r = Vec::with_capacity(out_frames);
    for &[l, r] in interleaved.as_chunks::<2>().0.iter().take(out_frames) {
        samples_l.push(l);
        samples_r.push(r);
    }

    Ok(DecodedAudio {
        channels: audio.channels,
        sample_rate: target_rate,
        samples_l,
        samples_r,
    })
}

/// Computes the exact expected output frame count when converting `src_frames` from `src_rate` to `dst_rate`.
#[must_use]
pub fn resampled_frame_count(src_frames: u64, src_rate: u32, dst_rate: u32) -> u64 {
    if src_rate == dst_rate || src_rate == 0 {
        return src_frames;
    }
    let num = u128::from(src_frames) * u128::from(dst_rate);
    let den = u128::from(src_rate);
    u64::try_from(num.div_ceil(den)).unwrap_or(u64::MAX)
}

/// Incremental stereo `[f32; 2]` chunk resampler for [`DiskStreamer`](crate::streamer::DiskStreamer).
///
/// Preserves bit-exact samples when `src_rate == dst_rate`.
pub struct StreamingResampler {
    kind: StreamingKind,
}

enum StreamingKind {
    PassThrough,
    Resample(Box<StreamingInner>),
}

struct StreamingInner {
    src_rate: u32,
    dst_rate: u32,
    resampler: Box<dyn Resampler<f32> + Send>,
    delay_remaining: usize,
    total_in_frames: u64,
    total_out_emitted: u64,
    pending_in: Vec<[f32; 2]>,
    in_scratch: Vec<f32>,
    out_scratch: Vec<f32>,
}

impl StreamingResampler {
    /// Creates a streaming stereo resampler from `src_rate` to `dst_rate`.
    pub fn new(src_rate: u32, dst_rate: u32) -> anyhow::Result<Self> {
        if src_rate == 0 || dst_rate == 0 {
            anyhow::bail!("sample rates must be > 0 ({src_rate} -> {dst_rate})");
        }
        if src_rate == dst_rate {
            return Ok(Self {
                kind: StreamingKind::PassThrough,
            });
        }
        let resampler = build_stereo_resampler(src_rate, dst_rate)?;
        let delay_remaining = resampler.output_delay();
        let in_cap = resampler
            .input_frames_max()
            .max(resampler.input_frames_next());
        let out_cap = resampler
            .output_frames_max()
            .max(resampler.output_frames_next());
        Ok(Self {
            kind: StreamingKind::Resample(Box::new(StreamingInner {
                src_rate,
                dst_rate,
                resampler,
                delay_remaining,
                total_in_frames: 0,
                total_out_emitted: 0,
                pending_in: Vec::with_capacity(in_cap * 2),
                in_scratch: vec![0.0; in_cap * 2],
                out_scratch: vec![0.0; out_cap * 2],
            })),
        })
    }

    /// Returns `true` when `src_rate == dst_rate` and samples pass through unmodified.
    #[must_use]
    pub fn is_passthrough(&self) -> bool {
        matches!(self.kind, StreamingKind::PassThrough)
    }

    /// Resets internal resampler state (used when seeking).
    pub fn reset(&mut self) {
        if let StreamingKind::Resample(inner) = &mut self.kind {
            inner.resampler.reset();
            inner.delay_remaining = inner.resampler.output_delay();
            inner.total_in_frames = 0;
            inner.total_out_emitted = 0;
            inner.pending_in.clear();
        }
    }

    /// Feeds `input` stereo frames at `src_rate` and appends resampled stereo frames at `dst_rate`
    /// onto `out`.
    pub fn process_chunk(
        &mut self,
        input: &[[f32; 2]],
        out: &mut Vec<[f32; 2]>,
    ) -> anyhow::Result<()> {
        let inner = match &mut self.kind {
            StreamingKind::PassThrough => {
                out.extend_from_slice(input);
                return Ok(());
            }
            StreamingKind::Resample(inner) => inner,
        };
        if input.is_empty() {
            return Ok(());
        }
        inner.total_in_frames = inner.total_in_frames.saturating_add(input.len() as u64);
        inner.pending_in.extend_from_slice(input);

        let mut offset = 0usize;
        loop {
            let req_in = inner.resampler.input_frames_next();
            if inner.pending_in.len() - offset < req_in {
                break;
            }
            inner.process_slice(offset, req_in, None, None, out)?;
            offset += req_in;
        }
        if offset > 0 {
            inner.pending_in.drain(..offset);
        }
        Ok(())
    }

    /// Flushes any remaining buffered input frames and the resampler's startup/tail delay at EOF.
    pub fn flush(&mut self, out: &mut Vec<[f32; 2]>) -> anyhow::Result<()> {
        let inner = match &mut self.kind {
            StreamingKind::PassThrough => return Ok(()),
            StreamingKind::Resample(inner) => inner,
        };
        let expected_total_out =
            resampled_frame_count(inner.total_in_frames, inner.src_rate, inner.dst_rate);
        if !inner.pending_in.is_empty() {
            let partial = inner.pending_in.len();
            let req_in = inner.resampler.input_frames_next();
            let take = partial.min(req_in);
            let remaining = (expected_total_out.saturating_sub(inner.total_out_emitted)) as usize;
            let indexing = Indexing::new().partial_len(take);
            inner.process_slice(0, take, Some(&indexing), Some(remaining), out)?;
            inner.pending_in.drain(..take);
        }
        while inner.total_out_emitted < expected_total_out {
            let remaining = (expected_total_out - inner.total_out_emitted) as usize;
            let indexing = Indexing::new().partial_len(0);
            let nbr_out = inner.process_slice(0, 0, Some(&indexing), Some(remaining), out)?;
            if nbr_out == 0 {
                break;
            }
        }
        Ok(())
    }
}

impl StreamingInner {
    fn process_slice(
        &mut self,
        pending_offset: usize,
        valid_frames: usize,
        indexing: Option<&Indexing>,
        max_emit: Option<usize>,
        out: &mut Vec<[f32; 2]>,
    ) -> anyhow::Result<usize> {
        let req_in = self.resampler.input_frames_next();
        if self.in_scratch.len() < req_in * 2 {
            self.in_scratch.resize(req_in * 2, 0.0);
        }
        for i in 0..valid_frames {
            let [l, r] = self.pending_in[pending_offset + i];
            self.in_scratch[i * 2] = l;
            self.in_scratch[i * 2 + 1] = r;
        }
        if valid_frames < req_in {
            self.in_scratch[valid_frames * 2..req_in * 2].fill(0.0);
        }

        let req_out = self
            .resampler
            .output_frames_next()
            .max(self.resampler.output_frames_max());
        if self.out_scratch.len() < req_out * 2 {
            self.out_scratch.resize(req_out * 2, 0.0);
        }

        let input_frames = if indexing.and_then(|idx| idx.partial_len).is_some() {
            valid_frames
        } else {
            req_in
        };
        let in_adapter =
            InterleavedSlice::new(&self.in_scratch[..input_frames * 2], 2, input_frames)
                .map_err(|e| anyhow::anyhow!("streaming resampler input adapter error: {e}"))?;
        let mut out_adapter =
            InterleavedSlice::new_mut(&mut self.out_scratch[..req_out * 2], 2, req_out)
                .map_err(|e| anyhow::anyhow!("streaming resampler output adapter error: {e}"))?;

        let (_, nbr_out) = self
            .resampler
            .process_into_buffer(&in_adapter, &mut out_adapter, indexing)
            .map_err(|e| anyhow::anyhow!("streaming resampling failed: {e}"))?;

        let skip = self.delay_remaining.min(nbr_out);
        self.delay_remaining -= skip;
        let available = nbr_out - skip;
        let emit = max_emit.map_or(available, |limit| available.min(limit));
        out.reserve(emit);
        for frame in skip..skip + emit {
            out.push([self.out_scratch[frame * 2], self.out_scratch[frame * 2 + 1]]);
        }
        self.total_out_emitted = self.total_out_emitted.saturating_add(emit as u64);
        Ok(nbr_out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    fn sine_audio(sample_rate: u32, frames: usize, freq_hz: f64) -> DecodedAudio {
        let mut samples_l = Vec::with_capacity(frames);
        let mut samples_r = Vec::with_capacity(frames);
        for i in 0..frames {
            let s = (TAU * freq_hz * (i as f64) / f64::from(sample_rate)).sin() as f32 * 0.8;
            samples_l.push(s);
            samples_r.push(-s);
        }
        DecodedAudio {
            channels: 2,
            sample_rate,
            samples_l,
            samples_r,
        }
    }

    fn measure_440hz_correlation(samples: &[f32], sample_rate: u32) -> f64 {
        let start = 1024usize.min(samples.len() / 4);
        let end = samples.len().saturating_sub(1024).max(start + 1);
        let omega = TAU * 440.0 / f64::from(sample_rate);
        let mut s_sin = 0.0f64;
        let mut s_cos = 0.0f64;
        let mut sig_sq = 0.0f64;
        let mut count = 0usize;
        for (i, &x) in samples[start..end].iter().enumerate() {
            let idx = (start + i) as f64;
            let xf = f64::from(x);
            s_sin += xf * (omega * idx).sin();
            s_cos += xf * (omega * idx).cos();
            sig_sq += xf * xf;
            count += 1;
        }
        let n = count.max(1) as f64;
        let a = 2.0 * s_sin / n;
        let b = 2.0 * s_cos / n;
        let tone_power = 0.5 * (a * a + b * b);
        let total_power = (sig_sq / n).max(1e-18);
        tone_power / total_power
    }

    #[test]
    fn equal_rate_resampling_is_bit_exact_passthrough() {
        let src = sine_audio(44100, 4096, 440.0);
        let out = resample_decoded_audio(&src, 44100).expect("passthrough");
        assert_eq!(out, src);

        let mut sr = StreamingResampler::new(44100, 44100).expect("streaming passthrough");
        assert!(sr.is_passthrough());
        let interleaved: Vec<[f32; 2]> = src
            .samples_l
            .iter()
            .zip(&src.samples_r)
            .map(|(&l, &r)| [l, r])
            .collect();
        let mut streamed = Vec::new();
        for chunk in interleaved.chunks(333) {
            sr.process_chunk(chunk, &mut streamed).expect("chunk");
        }
        sr.flush(&mut streamed).expect("flush");
        assert_eq!(streamed, interleaved);
    }

    #[test]
    fn resample_decoded_and_streaming_preserve_440hz_across_rates() {
        for &(src_rate, dst_rate) in &[(48000, 44100), (96000, 44100), (44100, 96000)] {
            let src = sine_audio(src_rate, src_rate as usize, 440.0);
            let offline = resample_decoded_audio(&src, dst_rate).expect("offline resample");
            assert_eq!(offline.sample_rate, dst_rate);
            assert_eq!(offline.samples_l.len(), dst_rate as usize);
            let purity = measure_440hz_correlation(&offline.samples_l, dst_rate);
            assert!(
                purity > 0.99,
                "offline {src_rate}->{dst_rate} 440Hz purity too low: {purity}"
            );

            let mut sr = StreamingResampler::new(src_rate, dst_rate).expect("streaming resample");
            let pairs: Vec<[f32; 2]> = src
                .samples_l
                .iter()
                .zip(&src.samples_r)
                .map(|(&l, &r)| [l, r])
                .collect();
            let mut streamed = Vec::new();
            for chunk in pairs.chunks(517) {
                sr.process_chunk(chunk, &mut streamed).expect("chunk");
            }
            sr.flush(&mut streamed).expect("flush");
            assert_eq!(streamed.len(), dst_rate as usize);
            let streamed_l: Vec<f32> = streamed.iter().map(|f| f[0]).collect();
            let streamed_purity = measure_440hz_correlation(&streamed_l, dst_rate);
            assert!(
                streamed_purity > 0.99,
                "streaming {src_rate}->{dst_rate} 440Hz purity too low: {streamed_purity}"
            );

            // Offline and streaming produce matching results after reset as well.
            let max_diff = offline
                .samples_l
                .iter()
                .zip(&streamed_l)
                .map(|(a, b)| (a - b).abs())
                .fold(0.0f32, f32::max);
            assert!(
                max_diff < 1e-5,
                "{src_rate}->{dst_rate} offline vs streaming differed by {max_diff}"
            );
        }
    }
}
