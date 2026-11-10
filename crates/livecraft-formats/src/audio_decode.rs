//! Multi-format audio decoding and frame-accurate disk streaming backed by `symphonia`.

use std::fs::File;
use std::path::{Path, PathBuf};

use symphonia::core::audio::GenericAudioBufferRef;
use symphonia::core::codecs::audio::well_known::{
    CODEC_ID_AAC, CODEC_ID_MP1, CODEC_ID_MP2, CODEC_ID_MP3, CODEC_ID_VORBIS,
};
use symphonia::core::codecs::audio::{
    AudioCodecId, AudioCodecParameters, AudioDecoder, AudioDecoderOptions,
};
use symphonia::core::errors::{Error as SymphoniaError, SeekErrorKind};
use symphonia::core::formats::{
    FormatOptions, FormatReader, SeekMode, SeekTo, TrackType, probe::Hint,
};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::units::{Duration, Time, TimeBase, Timestamp};

/// Maximum total samples (frames * channels) permitted when decoding a file into RAM.
pub const MAX_RAM_AUDIO_SAMPLES: u64 = 1 << 28;

/// Cap initial per-channel vector reservation so malformed container headers cannot trigger OOM.
const MAX_INITIAL_RESERVE_FRAMES: usize = 1 << 20;

/// Fully decoded stereo `f32` audio held in memory.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DecodedAudio {
    pub channels: u16,
    pub sample_rate: u32,
    pub samples_l: Vec<f32>,
    pub samples_r: Vec<f32>,
}

/// Lightweight metadata probed from an audio file container without decoding its payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioInfo {
    pub sample_rate: u32,
    pub channels: u16,
    pub total_frames: Option<u64>,
}

/// Errors raised while probing, decoding, or seeking audio files.
#[derive(Debug, thiserror::Error)]
pub enum AudioDecodeError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("audio probe/demux/decode error: {0}")]
    Symphonia(#[from] SymphoniaError),
    #[error("no audio track found")]
    NoAudioTrack,
    #[error("invalid audio metadata: {0}")]
    InvalidMetadata(String),
    #[error("audio exceeds sample limit ({samples} > {limit})")]
    ExceedsLimit { samples: u64, limit: u64 },
    #[error("seek error: {0}")]
    Seek(String),
}

struct OpenedTrack {
    format: Box<dyn FormatReader>,
    track_id: u32,
    params: AudioCodecParameters,
    time_base: Option<TimeBase>,
    info: AudioInfo,
}

fn open_format_reader(path: &Path) -> Result<Box<dyn FormatReader>, AudioDecodeError> {
    let file = File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let format = symphonia::default::get_probe().probe(
        &hint,
        mss,
        FormatOptions::default(),
        MetadataOptions::default(),
    )?;
    Ok(format)
}

fn duration_to_frames(dur: Duration, tb: TimeBase, sample_rate: u32) -> u64 {
    if tb.numer.get() == 1 && tb.denom.get() == sample_rate {
        return dur.get();
    }
    let num = u128::from(dur.get()) * u128::from(tb.numer.get()) * u128::from(sample_rate);
    let den = u128::from(tb.denom.get());
    u64::try_from(num / den).unwrap_or(u64::MAX)
}

fn ts_to_frames(ts: Timestamp, time_base: Option<TimeBase>, sample_rate: u32) -> i64 {
    let Some(tb) = time_base else {
        return ts.get();
    };
    if tb.numer.get() == 1 && tb.denom.get() == sample_rate {
        return ts.get();
    }
    let num = i128::from(ts.get()) * i128::from(tb.numer.get()) * i128::from(sample_rate);
    let den = i128::from(tb.denom.get());
    i64::try_from(num.div_euclid(den)).unwrap_or(if num < 0 { i64::MIN } else { i64::MAX })
}

fn extract_info_from_params(
    params: &AudioCodecParameters,
    num_frames: Option<u64>,
    duration: Option<Duration>,
    time_base: Option<TimeBase>,
) -> Result<AudioInfo, AudioDecodeError> {
    let sample_rate = params
        .sample_rate
        .filter(|&sr| sr > 0)
        .ok_or_else(|| AudioDecodeError::InvalidMetadata("missing or zero sample rate".into()))?;
    let ch_count = params
        .channels
        .as_ref()
        .map(|c| c.count())
        .filter(|&c| c > 0)
        .ok_or_else(|| {
            AudioDecodeError::InvalidMetadata("missing or zero audio channels".into())
        })?;
    let channels = u16::try_from(ch_count).map_err(|_| {
        AudioDecodeError::InvalidMetadata(format!("channel count {ch_count} exceeds u16"))
    })?;
    let total_frames = num_frames.or_else(|| {
        duration.and_then(|d| time_base.map(|tb| duration_to_frames(d, tb, sample_rate)))
    });
    Ok(AudioInfo {
        sample_rate,
        channels,
        total_frames,
    })
}

fn open_track(
    path: &Path,
    require_decoder: bool,
) -> Result<(OpenedTrack, Option<Box<dyn AudioDecoder>>), AudioDecodeError> {
    let format = open_format_reader(path)?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or(AudioDecodeError::NoAudioTrack)?;
    let track_id = track.id;
    let time_base = track.time_base;
    let num_frames = track.num_frames;
    let duration = track.duration;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|cp| cp.audio())
        .ok_or(AudioDecodeError::NoAudioTrack)?
        .clone();

    let needs_decoder_for_meta = params.sample_rate.is_none_or(|sr| sr == 0)
        || params.channels.as_ref().is_none_or(|c| c.count() == 0);

    if require_decoder || needs_decoder_for_meta {
        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(&params, &AudioDecoderOptions::default())?;
        let eff_params = decoder.codec_params().clone();
        let merged = AudioCodecParameters {
            sample_rate: eff_params.sample_rate.or(params.sample_rate),
            channels: eff_params.channels.or_else(|| params.channels.clone()),
            ..eff_params
        };
        let info = extract_info_from_params(&merged, num_frames, duration, time_base)?;
        Ok((
            OpenedTrack {
                format,
                track_id,
                params: merged,
                time_base,
                info,
            },
            Some(decoder),
        ))
    } else {
        let info = extract_info_from_params(&params, num_frames, duration, time_base)?;
        Ok((
            OpenedTrack {
                format,
                track_id,
                params,
                time_base,
                info,
            },
            None,
        ))
    }
}

/// Probes an audio file's sample rate, channel count, and optional total frame count.
pub fn probe_audio_info(path: &Path) -> Result<AudioInfo, AudioDecodeError> {
    let (opened, _) = open_track(path, false)?;
    Ok(opened.info)
}

/// Decodes an entire audio file into [`DecodedAudio`] using [`MAX_RAM_AUDIO_SAMPLES`].
pub fn decode_audio_file(path: &Path) -> Result<DecodedAudio, AudioDecodeError> {
    decode_audio_file_with_limit(path, MAX_RAM_AUDIO_SAMPLES)
}

/// Decodes an entire audio file into [`DecodedAudio`], rejecting inputs exceeding `max_samples`.
pub fn decode_audio_file_with_limit(
    path: &Path,
    max_samples: u64,
) -> Result<DecodedAudio, AudioDecodeError> {
    let (mut opened, decoder_opt) = open_track(path, true)?;
    let mut decoder = decoder_opt.ok_or(AudioDecodeError::NoAudioTrack)?;
    let info = opened.info;

    if let Some(total_frames) = info.total_frames {
        let total_samples = total_frames.saturating_mul(u64::from(info.channels));
        if total_samples > max_samples {
            return Err(AudioDecodeError::ExceedsLimit {
                samples: total_samples,
                limit: max_samples,
            });
        }
    }

    let reserve_frames = info
        .total_frames
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or(0)
        .min(MAX_INITIAL_RESERVE_FRAMES);

    let mut samples_l = Vec::with_capacity(reserve_frames);
    let mut samples_r = Vec::with_capacity(reserve_frames);
    let mut planar: Vec<Vec<f32>> = Vec::new();
    let mut decoded_frames: u64 = 0;

    loop {
        let packet = match opened.format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(SymphoniaError::ResetRequired) => {
                let Some(track) = opened.format.default_track(TrackType::Audio) else {
                    break;
                };
                let Some(new_params) = track.codec_params.as_ref().and_then(|cp| cp.audio()) else {
                    break;
                };
                opened.track_id = track.id;
                decoder = symphonia::default::get_codecs()
                    .make_audio_decoder(new_params, &AudioDecoderOptions::default())?;
                continue;
            }
            Err(e) => return Err(AudioDecodeError::Symphonia(e)),
        };
        if packet.track_id != opened.track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(buf) => buf,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(e) => return Err(AudioDecodeError::Symphonia(e)),
        };

        let pkt_frames = decoded.frames();
        if pkt_frames == 0 {
            continue;
        }
        let pkt_planes = decoded.num_planes();
        if pkt_planes == 0 {
            return Err(AudioDecodeError::InvalidMetadata(
                "decoded packet has zero audio channels".into(),
            ));
        }

        let new_frames = decoded_frames.saturating_add(pkt_frames as u64);
        let new_samples = new_frames.saturating_mul(u64::from(info.channels));
        if new_samples > max_samples {
            return Err(AudioDecodeError::ExceedsLimit {
                samples: new_samples,
                limit: max_samples,
            });
        }

        decoded.copy_to_vecs_planar::<f32>(&mut planar);
        match planar.as_slice() {
            [] => {
                return Err(AudioDecodeError::InvalidMetadata(
                    "decoded packet has zero audio channels".into(),
                ));
            }
            [mono] => {
                samples_l.extend_from_slice(mono);
                samples_r.extend_from_slice(mono);
            }
            [left, right, ..] => {
                samples_l.extend_from_slice(left);
                samples_r.extend_from_slice(right);
            }
        }
        decoded_frames = new_frames;
    }

    Ok(DecodedAudio {
        channels: info.channels,
        sample_rate: info.sample_rate,
        samples_l,
        samples_r,
    })
}

fn convert_buffer_to_stereo(
    decoded: GenericAudioBufferRef<'_>,
    planar: &mut Vec<Vec<f32>>,
    out_stereo: &mut Vec<[f32; 2]>,
) -> Result<(), AudioDecodeError> {
    out_stereo.clear();
    let n_frames = decoded.frames();
    if n_frames == 0 {
        return Ok(());
    }
    if decoded.num_planes() == 0 {
        return Err(AudioDecodeError::InvalidMetadata(
            "decoded packet has zero audio channels".into(),
        ));
    }
    decoded.copy_to_vecs_planar::<f32>(planar);
    out_stereo.reserve(n_frames);
    match planar.as_slice() {
        [] => {
            return Err(AudioDecodeError::InvalidMetadata(
                "decoded packet has zero audio channels".into(),
            ));
        }
        [mono] => {
            for &s in mono {
                out_stereo.push([s, s]);
            }
        }
        [left, right, ..] => {
            for (&l, &r) in left.iter().zip(right.iter()) {
                out_stereo.push([l, r]);
            }
        }
    }
    Ok(())
}

/// Incremental, seekable audio reader that streams stereo `f32` frames in bounded memory.
pub struct AudioStreamReader {
    path: PathBuf,
    format: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track_id: u32,
    codec: AudioCodecId,
    time_base: Option<TimeBase>,
    sample_rate: u32,
    channels: u16,
    total_frames: Option<u64>,
    position_frames: u64,
    pending_stereo: Vec<[f32; 2]>,
    pending_offset: usize,
    planar_scratch: Vec<Vec<f32>>,
    eof: bool,
}

impl AudioStreamReader {
    /// Opens an audio file for incremental streaming and frame-accurate seeking.
    pub fn open(path: &Path) -> Result<Self, AudioDecodeError> {
        let (opened, decoder_opt) = open_track(path, true)?;
        let decoder = decoder_opt.ok_or(AudioDecodeError::NoAudioTrack)?;
        Ok(Self {
            path: path.to_path_buf(),
            format: opened.format,
            decoder,
            track_id: opened.track_id,
            codec: opened.params.codec,
            time_base: opened.time_base,
            sample_rate: opened.info.sample_rate,
            channels: opened.info.channels,
            total_frames: opened.info.total_frames,
            position_frames: 0,
            pending_stereo: Vec::new(),
            pending_offset: 0,
            planar_scratch: Vec::new(),
            eof: false,
        })
    }

    #[must_use]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    #[must_use]
    pub fn channels(&self) -> u16 {
        self.channels
    }

    #[must_use]
    pub fn total_frames(&self) -> Option<u64> {
        self.total_frames
    }

    #[must_use]
    pub fn position_frames(&self) -> u64 {
        self.position_frames
    }

    /// Seeks to `target_frame` with frame accuracy across seekable and unseekable codecs.
    pub fn seek(&mut self, target_frame: u64) -> Result<(), AudioDecodeError> {
        if target_frame == self.position_frames {
            return Ok(());
        }
        if target_frame == 0 {
            return self.reopen_and_skip(0);
        }
        if let Some(total) = self.total_frames {
            if target_frame > total {
                return Err(AudioDecodeError::Seek(format!(
                    "target frame {target_frame} exceeds total frames {total}"
                )));
            }
            if target_frame == total {
                self.pending_stereo.clear();
                self.pending_offset = 0;
                self.position_frames = total;
                self.eof = true;
                return Ok(());
            }
        }

        // Lossy MDCT / bit-reservoir codecs depend on prior frames to avoid boundary artifacts.
        let preroll = match self.codec {
            CODEC_ID_AAC => 2048u64,
            CODEC_ID_MP1 | CODEC_ID_MP2 | CODEC_ID_MP3 => 4608u64,
            CODEC_ID_VORBIS => 4096u64,
            _ => 0u64,
        };
        let seek_frame = target_frame.saturating_sub(preroll);
        let seek_to = self.build_seek_to(seek_frame)?;

        match self.format.seek(SeekMode::Accurate, seek_to) {
            Ok(seeked) => {
                let actual_frame = ts_to_frames(seeked.actual_ts, self.time_base, self.sample_rate);
                if actual_frame > target_frame as i64 {
                    return self.reopen_and_skip(target_frame);
                }
                match self.align_after_seek(target_frame) {
                    Err(AudioDecodeError::Symphonia(SymphoniaError::DecodeError(msg)))
                        if msg.contains("no atom pending read") =>
                    {
                        self.reopen_and_seek(seek_frame, target_frame)
                    }
                    other => other,
                }
            }
            Err(SymphoniaError::SeekError(
                SeekErrorKind::Unseekable | SeekErrorKind::ForwardOnly,
            )) => self.reopen_and_skip(target_frame),
            Err(SymphoniaError::SeekError(SeekErrorKind::OutOfRange)) => Err(
                AudioDecodeError::Seek(format!("seek target frame {target_frame} is out of range")),
            ),
            Err(e) => Err(AudioDecodeError::Symphonia(e)),
        }
    }

    fn reopen_and_seek(
        &mut self,
        seek_frame: u64,
        target_frame: u64,
    ) -> Result<(), AudioDecodeError> {
        let (opened, decoder_opt) = open_track(&self.path, true)?;
        let decoder = decoder_opt.ok_or(AudioDecodeError::NoAudioTrack)?;
        self.format = opened.format;
        self.decoder = decoder;
        self.track_id = opened.track_id;
        self.codec = opened.params.codec;
        self.time_base = opened.time_base;
        self.position_frames = 0;
        self.pending_stereo.clear();
        self.pending_offset = 0;
        self.eof = false;

        let seek_to = self.build_seek_to(seek_frame)?;
        match self.format.seek(SeekMode::Accurate, seek_to) {
            Ok(seeked) => {
                let actual_frame = ts_to_frames(seeked.actual_ts, self.time_base, self.sample_rate);
                if actual_frame > target_frame as i64 {
                    return self.reopen_and_skip(target_frame);
                }
                self.align_after_seek(target_frame)
            }
            Err(SymphoniaError::SeekError(
                SeekErrorKind::Unseekable | SeekErrorKind::ForwardOnly,
            )) => self.reopen_and_skip(target_frame),
            Err(SymphoniaError::SeekError(SeekErrorKind::OutOfRange)) => Err(
                AudioDecodeError::Seek(format!("seek target frame {target_frame} is out of range")),
            ),
            Err(e) => Err(AudioDecodeError::Symphonia(e)),
        }
    }

    /// Clears `out_stereo` and reads up to `max_frames` stereo frames, returning the frame count read.
    pub fn read_frames(
        &mut self,
        max_frames: usize,
        out_stereo: &mut Vec<[f32; 2]>,
    ) -> Result<usize, AudioDecodeError> {
        out_stereo.clear();
        if max_frames == 0 || self.eof {
            return Ok(0);
        }

        while out_stereo.len() < max_frames {
            if self.pending_offset < self.pending_stereo.len() {
                let avail = self.pending_stereo.len() - self.pending_offset;
                let take = avail.min(max_frames - out_stereo.len());
                out_stereo.extend_from_slice(
                    &self.pending_stereo[self.pending_offset..self.pending_offset + take],
                );
                self.pending_offset += take;
                self.position_frames += take as u64;
                if self.pending_offset == self.pending_stereo.len() {
                    self.pending_stereo.clear();
                    self.pending_offset = 0;
                }
            } else if !self.decode_next_packet()? {
                self.eof = true;
                break;
            }
        }

        Ok(out_stereo.len())
    }

    fn build_seek_to(&self, seek_frame: u64) -> Result<SeekTo, AudioDecodeError> {
        if let Some(tb) = self.time_base {
            if tb.numer.get() == 1 && tb.denom.get() == self.sample_rate {
                let ts = Timestamp::try_from(seek_frame).map_err(|_| {
                    AudioDecodeError::Seek("seek target overflows i64 timestamp".into())
                })?;
                Ok(SeekTo::Timestamp {
                    ts,
                    track_id: self.track_id,
                })
            } else {
                let secs = seek_frame as f64 / f64::from(self.sample_rate);
                let time = Time::try_from_secs_f64(secs)
                    .ok_or_else(|| AudioDecodeError::Seek("seek time out of range".into()))?;
                Ok(SeekTo::Time {
                    time,
                    track_id: Some(self.track_id),
                })
            }
        } else {
            let ts = Timestamp::try_from(seek_frame).map_err(|_| {
                AudioDecodeError::Seek("seek target overflows i64 timestamp".into())
            })?;
            Ok(SeekTo::Timestamp {
                ts,
                track_id: self.track_id,
            })
        }
    }

    fn align_after_seek(&mut self, target_frame: u64) -> Result<(), AudioDecodeError> {
        self.decoder.reset();
        self.pending_stereo.clear();
        self.pending_offset = 0;
        self.eof = false;

        let mut saw_packet = false;
        let mut decoded_end_frame: i64 = 0;

        loop {
            let packet = match self.format.next_packet() {
                Ok(Some(packet)) => packet,
                Ok(None) => {
                    if !saw_packet && target_frame > 0 {
                        return self.reopen_and_skip(target_frame);
                    }
                    if decoded_end_frame == target_frame as i64 {
                        self.position_frames = target_frame;
                        self.eof = true;
                        return Ok(());
                    }
                    return Err(AudioDecodeError::Seek(format!(
                        "target frame {target_frame} exceeds stream length ({})",
                        decoded_end_frame.max(0)
                    )));
                }
                Err(SymphoniaError::ResetRequired) => {
                    if !self.handle_reset_required()? {
                        return Err(AudioDecodeError::NoAudioTrack);
                    }
                    continue;
                }
                Err(e) => return Err(AudioDecodeError::Symphonia(e)),
            };
            if packet.track_id != self.track_id {
                continue;
            }

            let start_ts = packet.pts.saturating_add(packet.trim_start);
            let pkt_start_frame = ts_to_frames(start_ts, self.time_base, self.sample_rate);
            let decoded = match self.decoder.decode(&packet) {
                Ok(buf) => buf,
                Err(SymphoniaError::DecodeError(_)) => continue,
                Err(e) => return Err(AudioDecodeError::Symphonia(e)),
            };
            let n_frames = decoded.frames();
            if n_frames == 0 {
                continue;
            }

            if !saw_packet && pkt_start_frame > target_frame as i64 {
                return self.reopen_and_skip(target_frame);
            }
            saw_packet = true;

            let pkt_end_frame = pkt_start_frame + n_frames as i64;
            decoded_end_frame = pkt_end_frame;

            if pkt_end_frame <= target_frame as i64 {
                self.position_frames = pkt_end_frame.max(0) as u64;
                continue;
            }

            let skip = (target_frame as i64 - pkt_start_frame).max(0) as usize;
            convert_buffer_to_stereo(decoded, &mut self.planar_scratch, &mut self.pending_stereo)?;
            self.pending_offset = skip.min(self.pending_stereo.len());
            if self.pending_offset == self.pending_stereo.len() {
                self.pending_stereo.clear();
                self.pending_offset = 0;
            }
            self.position_frames = target_frame;
            return Ok(());
        }
    }

    fn reopen_and_skip(&mut self, target_frame: u64) -> Result<(), AudioDecodeError> {
        let (opened, decoder_opt) = open_track(&self.path, true)?;
        let decoder = decoder_opt.ok_or(AudioDecodeError::NoAudioTrack)?;
        self.format = opened.format;
        self.decoder = decoder;
        self.track_id = opened.track_id;
        self.codec = opened.params.codec;
        self.time_base = opened.time_base;
        self.position_frames = 0;
        self.pending_stereo.clear();
        self.pending_offset = 0;
        self.eof = false;

        while self.position_frames < target_frame {
            if self.pending_offset < self.pending_stereo.len() {
                let remaining = target_frame - self.position_frames;
                let avail = (self.pending_stereo.len() - self.pending_offset) as u64;
                let skip = avail.min(remaining) as usize;
                self.pending_offset += skip;
                self.position_frames += skip as u64;
                if self.pending_offset == self.pending_stereo.len() {
                    self.pending_stereo.clear();
                    self.pending_offset = 0;
                }
            } else if !self.decode_next_packet()? {
                self.eof = true;
                return Err(AudioDecodeError::Seek(format!(
                    "target frame {target_frame} exceeds stream length ({})",
                    self.position_frames
                )));
            }
        }
        Ok(())
    }

    fn decode_next_packet(&mut self) -> Result<bool, AudioDecodeError> {
        self.pending_stereo.clear();
        self.pending_offset = 0;

        loop {
            let packet = match self.format.next_packet() {
                Ok(Some(packet)) => packet,
                Ok(None) => return Ok(false),
                Err(SymphoniaError::ResetRequired) => {
                    if !self.handle_reset_required()? {
                        return Ok(false);
                    }
                    continue;
                }
                Err(e) => return Err(AudioDecodeError::Symphonia(e)),
            };
            if packet.track_id != self.track_id {
                continue;
            }
            let decoded = match self.decoder.decode(&packet) {
                Ok(buf) => buf,
                Err(SymphoniaError::DecodeError(_)) => continue,
                Err(e) => return Err(AudioDecodeError::Symphonia(e)),
            };
            if decoded.frames() == 0 {
                continue;
            }
            convert_buffer_to_stereo(decoded, &mut self.planar_scratch, &mut self.pending_stereo)?;
            if !self.pending_stereo.is_empty() {
                return Ok(true);
            }
        }
    }

    fn handle_reset_required(&mut self) -> Result<bool, AudioDecodeError> {
        let Some(track) = self.format.default_track(TrackType::Audio) else {
            return Ok(false);
        };
        let Some(params) = track.codec_params.as_ref().and_then(|cp| cp.audio()) else {
            return Ok(false);
        };
        self.track_id = track.id;
        self.time_base = track.time_base;
        self.codec = params.codec;
        self.decoder = symphonia::default::get_codecs()
            .make_audio_decoder(params, &AudioDecoderOptions::default())?;
        Ok(true)
    }
}
