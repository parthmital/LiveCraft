//! Standard MIDI File (SMF format 0 and 1) reader and writer.

use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use livecraft_model::ids::ClipId;
use livecraft_model::model::{Clip, ClipPayload, MidiNote};

/// Maximum Standard MIDI File size accepted (32 MiB).
pub const MAX_MIDI_BYTES: u64 = 32 * 1024 * 1024;
const DEFAULT_PPQN: u16 = 480;
const MAX_VLQ_VALUE: u64 = 0x0FFF_FFFF;

/// SMF file format variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SmfFormat {
    /// Single multi-channel track (`MThd` format 0).
    Format0,
    /// Multi-track synchronous file (`MThd` format 1).
    Format1,
}

/// Errors returned while reading or writing Standard MIDI Files.
#[derive(Debug, thiserror::Error)]
pub enum SmfError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("MIDI file exceeds {limit}-byte size limit")]
    TooLarge { limit: u64 },
    #[error("Invalid SMF header or track data: {0}")]
    InvalidFormat(String),
    #[error("Unexpected end of MIDI data")]
    UnexpectedEof,
    #[error("Unsupported SMF feature: {0}")]
    Unsupported(String),
}

/// Parsed contents of a Standard MIDI File.
#[derive(Debug, Clone)]
pub struct MidiFileData {
    pub format: SmfFormat,
    pub ppqn: u16,
    pub bpm: f64,
    pub time_signature: (u8, u8),
    pub tracks: Vec<Vec<MidiNote>>,
    pub merged_notes: Vec<MidiNote>,
}

struct ParsedSmfInternal {
    data: MidiFileData,
    first_track_name: Option<String>,
}

/// Reads and parses an in-memory Standard MIDI File (Format 0 or 1).
pub fn read_smf(bytes: &[u8]) -> Result<MidiFileData, SmfError> {
    parse_smf_internal(bytes).map(|parsed| parsed.data)
}

/// Reads and parses a Standard MIDI File from disk up to [`MAX_MIDI_BYTES`].
pub fn read_smf_file(path: &Path) -> Result<MidiFileData, SmfError> {
    let bytes = read_limited_file(path)?;
    read_smf(&bytes)
}

/// Imports a Standard MIDI File into a LiveCraft [`Clip`].
pub fn read_smf_to_clip(path: &Path) -> Result<Clip, SmfError> {
    let bytes = read_limited_file(path)?;
    let parsed = parse_smf_internal(&bytes)?;
    let fallback = path
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("MIDI Import");
    let clip_name = parsed
        .first_track_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(fallback)
        .to_owned();

    let max_end = parsed
        .data
        .merged_notes
        .iter()
        .map(|n| (n.start_beat + n.duration_beats).max(0.0))
        .fold(0.0_f64, f64::max);
    let mut clip = Clip::new_midi(
        ClipId(1),
        clip_name,
        if max_end > 0.0 { max_end } else { 4.0 },
    );
    if let ClipPayload::Midi(ref mut mdata) = clip.payload {
        mdata.notes = parsed.data.merged_notes;
    }
    Ok(clip)
}

/// Encodes `notes`, tempo, and time signature into a 480-PPQN Standard MIDI File.
pub fn write_smf(
    notes: &[MidiNote],
    bpm: f64,
    time_sig: (u8, u8),
    format: SmfFormat,
    track_name: Option<&str>,
) -> Result<Vec<u8>, SmfError> {
    if !bpm.is_finite() || bpm <= 0.0 {
        return Err(SmfError::InvalidFormat(
            "BPM must be finite and positive".to_owned(),
        ));
    }
    let (ts_num, ts_den) = time_sig;
    if ts_num == 0 || ts_den == 0 || !ts_den.is_power_of_two() || ts_den > 128 {
        return Err(SmfError::InvalidFormat(format!(
            "invalid time signature {ts_num}/{ts_den}"
        )));
    }
    let us = (60_000_000.0 / bpm).round().clamp(1.0, 16_777_215.0) as u32;
    let tempo_bytes = [(us >> 16) as u8, (us >> 8) as u8, us as u8];
    let ts_bytes = [ts_num, ts_den.trailing_zeros() as u8, 24, 8];
    let note_events = build_sorted_note_events(notes)?;

    let (fmt_code, num_tracks): (u16, u16) = match format {
        SmfFormat::Format0 => (0, 1),
        SmfFormat::Format1 => (1, 2),
    };
    let mut out = Vec::with_capacity(64 + note_events.len() * 6);
    out.extend_from_slice(b"MThd");
    out.extend_from_slice(&6u32.to_be_bytes());
    out.extend_from_slice(&fmt_code.to_be_bytes());
    out.extend_from_slice(&num_tracks.to_be_bytes());
    out.extend_from_slice(&DEFAULT_PPQN.to_be_bytes());

    match format {
        SmfFormat::Format0 => {
            let mut trk = Vec::new();
            if let Some(name) = track_name {
                write_meta_event(&mut trk, 0, 0x03, name.as_bytes())?;
            }
            write_meta_event(&mut trk, 0, 0x51, &tempo_bytes)?;
            write_meta_event(&mut trk, 0, 0x58, &ts_bytes)?;
            write_note_events(&mut trk, &note_events)?;
            write_meta_event(&mut trk, 0, 0x2F, &[])?;
            append_mtrk_chunk(&mut out, &trk)?;
        }
        SmfFormat::Format1 => {
            let mut conductor = Vec::new();
            write_meta_event(&mut conductor, 0, 0x51, &tempo_bytes)?;
            write_meta_event(&mut conductor, 0, 0x58, &ts_bytes)?;
            write_meta_event(&mut conductor, 0, 0x2F, &[])?;
            append_mtrk_chunk(&mut out, &conductor)?;

            let mut note_trk = Vec::new();
            if let Some(name) = track_name {
                write_meta_event(&mut note_trk, 0, 0x03, name.as_bytes())?;
            }
            write_note_events(&mut note_trk, &note_events)?;
            write_meta_event(&mut note_trk, 0, 0x2F, &[])?;
            append_mtrk_chunk(&mut out, &note_trk)?;
        }
    }
    Ok(out)
}

/// Exports a LiveCraft MIDI [`Clip`] as a Standard MIDI File on disk.
pub fn export_clip_to_smf(
    clip: &Clip,
    path: &Path,
    bpm: f64,
    time_sig: (u8, u8),
    format: SmfFormat,
) -> Result<(), SmfError> {
    let ClipPayload::Midi(ref mdata) = clip.payload else {
        return Err(SmfError::Unsupported(
            "cannot export an Audio clip to Standard MIDI File".to_owned(),
        ));
    };
    let name = (!clip.name.is_empty()).then_some(clip.name.as_str());
    let bytes = write_smf(&mdata.notes, bpm, time_sig, format, name)?;
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, bytes)?;
    Ok(())
}

fn read_limited_file(path: &Path) -> Result<Vec<u8>, SmfError> {
    let file = File::open(path)?;
    let mut bytes = Vec::new();
    BufReader::new(file)
        .take(MAX_MIDI_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_MIDI_BYTES {
        return Err(SmfError::TooLarge {
            limit: MAX_MIDI_BYTES,
        });
    }
    Ok(bytes)
}

fn parse_smf_internal(bytes: &[u8]) -> Result<ParsedSmfInternal, SmfError> {
    if bytes.len() as u64 > MAX_MIDI_BYTES {
        return Err(SmfError::TooLarge {
            limit: MAX_MIDI_BYTES,
        });
    }
    let mut cursor = 0usize;
    if &read_bytes::<4>(bytes, &mut cursor)? != b"MThd" {
        return Err(SmfError::InvalidFormat(
            "missing `MThd` header chunk".to_owned(),
        ));
    }
    let hdr_len = read_u32_be(bytes, &mut cursor)? as usize;
    if hdr_len < 6 {
        return Err(SmfError::InvalidFormat(format!(
            "invalid MThd header length {hdr_len}"
        )));
    }
    let hdr_end = cursor.checked_add(hdr_len).ok_or(SmfError::UnexpectedEof)?;
    let hdr_data = bytes.get(cursor..hdr_end).ok_or(SmfError::UnexpectedEof)?;
    cursor = hdr_end;

    let format = match u16::from_be_bytes([hdr_data[0], hdr_data[1]]) {
        0 => SmfFormat::Format0,
        1 => SmfFormat::Format1,
        2 => return Err(SmfError::Unsupported("SMF Format 2 is unsupported".into())),
        other => return Err(SmfError::InvalidFormat(format!("unknown format {other}"))),
    };
    let num_tracks = u16::from_be_bytes([hdr_data[2], hdr_data[3]]);
    if num_tracks == 0 {
        return Err(SmfError::InvalidFormat("SMF declares 0 tracks".into()));
    }
    let division = u16::from_be_bytes([hdr_data[4], hdr_data[5]]);
    if division & 0x8000 != 0 {
        return Err(SmfError::Unsupported(
            "SMPTE time division is not supported".into(),
        ));
    }
    let ppqn = division & 0x7FFF;
    if ppqn == 0 {
        return Err(SmfError::InvalidFormat("PPQN cannot be zero".into()));
    }

    let mut meta = TrackMetaState {
        bpm: 120.0,
        seen_tempo: false,
        time_signature: (4, 4),
        seen_time_sig: false,
        first_track_name: None,
    };
    let mut tracks = Vec::new();
    while tracks.len() < usize::from(num_tracks) {
        let chunk_id = read_bytes::<4>(bytes, &mut cursor)?;
        let chunk_len = read_u32_be(bytes, &mut cursor)? as usize;
        let chunk_end = cursor
            .checked_add(chunk_len)
            .ok_or(SmfError::UnexpectedEof)?;
        let chunk_data = bytes
            .get(cursor..chunk_end)
            .ok_or(SmfError::UnexpectedEof)?;
        cursor = chunk_end;
        if &chunk_id != b"MTrk" {
            continue;
        }
        let mut trk_ctx = TrackParseState::new(ppqn);
        parse_mtrk(chunk_data, &mut trk_ctx, &mut meta)?;
        tracks.push(trk_ctx.notes);
    }

    let mut merged_notes: Vec<MidiNote> = tracks.iter().flatten().cloned().collect();
    sort_notes(&mut merged_notes);
    Ok(ParsedSmfInternal {
        data: MidiFileData {
            format,
            ppqn,
            bpm: meta.bpm,
            time_signature: meta.time_signature,
            tracks,
            merged_notes,
        },
        first_track_name: meta.first_track_name,
    })
}

struct TrackMetaState {
    bpm: f64,
    seen_tempo: bool,
    time_signature: (u8, u8),
    seen_time_sig: bool,
    first_track_name: Option<String>,
}

struct TrackParseState {
    ppqn_f64: f64,
    active: HashMap<(u8, u8), VecDeque<(u64, u8)>>,
    notes: Vec<MidiNote>,
}

impl TrackParseState {
    fn new(ppqn: u16) -> Self {
        Self {
            ppqn_f64: f64::from(ppqn),
            active: HashMap::new(),
            notes: Vec::new(),
        }
    }

    fn note_on(&mut self, channel: u8, pitch: u8, velocity: u8, tick: u64) {
        if velocity == 0 {
            self.note_off(channel, pitch, 64, tick);
            return;
        }
        self.active
            .entry((channel, pitch))
            .or_default()
            .push_back((tick, velocity));
    }

    fn note_off(&mut self, channel: u8, pitch: u8, release_vel: u8, tick: u64) {
        let Some((start_tick, vel)) = self
            .active
            .get_mut(&(channel, pitch))
            .and_then(VecDeque::pop_front)
        else {
            return;
        };
        let dur_ticks = tick.saturating_sub(start_tick).max(1);
        let mut note = MidiNote::new(
            pitch,
            start_tick as f64 / self.ppqn_f64,
            dur_ticks as f64 / self.ppqn_f64,
            vel,
        );
        note.release_velocity = f32::from(release_vel);
        self.notes.push(note);
    }

    fn close_hanging(&mut self, final_tick: u64) {
        let keys: Vec<(u8, u8)> = self.active.keys().copied().collect();
        for (ch, pitch) in keys {
            if let Some(mut queue) = self.active.remove(&(ch, pitch)) {
                while let Some((start_tick, vel)) = queue.pop_front() {
                    let dur_ticks = final_tick.saturating_sub(start_tick).max(1);
                    self.notes.push(MidiNote::new(
                        pitch,
                        start_tick as f64 / self.ppqn_f64,
                        dur_ticks as f64 / self.ppqn_f64,
                        vel,
                    ));
                }
            }
        }
    }
}

fn parse_mtrk(
    data: &[u8],
    state: &mut TrackParseState,
    meta: &mut TrackMetaState,
) -> Result<(), SmfError> {
    let mut cursor = 0usize;
    let mut tick: u64 = 0;
    let mut running_status: Option<u8> = None;

    while cursor < data.len() {
        tick = tick.saturating_add(read_vlq(data, &mut cursor)?);
        let next = *data.get(cursor).ok_or(SmfError::UnexpectedEof)?;
        let status = if next >= 0x80 {
            cursor += 1;
            // SysEx and Meta events cancel running status per SMF 1.0.
            running_status = (next < 0xF0).then_some(next);
            next
        } else {
            running_status
                .ok_or_else(|| SmfError::InvalidFormat("data byte without running status".into()))?
        };

        match status & 0xF0 {
            0x80 => {
                let pitch = read_u8(data, &mut cursor)? & 0x7F;
                let off_vel = read_u8(data, &mut cursor)? & 0x7F;
                state.note_off(status & 0x0F, pitch, off_vel, tick);
            }
            0x90 => {
                let pitch = read_u8(data, &mut cursor)? & 0x7F;
                let vel = read_u8(data, &mut cursor)? & 0x7F;
                state.note_on(status & 0x0F, pitch, vel, tick);
            }
            0xA0 | 0xB0 | 0xE0 => {
                let _ = read_bytes::<2>(data, &mut cursor)?;
            }
            0xC0 | 0xD0 => {
                let _ = read_u8(data, &mut cursor)?;
            }
            0xF0 => match status {
                0xF0 | 0xF7 => {
                    let len = read_vlq(data, &mut cursor)? as usize;
                    let end = cursor.checked_add(len).ok_or(SmfError::UnexpectedEof)?;
                    if end > data.len() {
                        return Err(SmfError::UnexpectedEof);
                    }
                    cursor = end;
                }
                0xFF => {
                    let meta_type = read_u8(data, &mut cursor)?;
                    let len = read_vlq(data, &mut cursor)? as usize;
                    let end = cursor.checked_add(len).ok_or(SmfError::UnexpectedEof)?;
                    let payload = data.get(cursor..end).ok_or(SmfError::UnexpectedEof)?;
                    cursor = end;
                    if apply_meta_event(meta_type, payload, meta) {
                        break;
                    }
                }
                0xF1 | 0xF3 => {
                    let _ = read_u8(data, &mut cursor)?;
                }
                0xF2 => {
                    let _ = read_bytes::<2>(data, &mut cursor)?;
                }
                _ => {
                    return Err(SmfError::InvalidFormat(format!(
                        "unsupported system byte 0x{status:02X}"
                    )));
                }
            },
            _ => {
                return Err(SmfError::InvalidFormat(format!(
                    "invalid MIDI status 0x{status:02X}"
                )));
            }
        }
    }

    state.close_hanging(tick);
    sort_notes(&mut state.notes);
    Ok(())
}

fn apply_meta_event(meta_type: u8, payload: &[u8], meta: &mut TrackMetaState) -> bool {
    match meta_type {
        0x03 => {
            let decoded = String::from_utf8_lossy(payload).into_owned();
            if !decoded.is_empty() && meta.first_track_name.is_none() {
                meta.first_track_name = Some(decoded);
            }
        }
        0x51 if payload.len() == 3 && !meta.seen_tempo => {
            let us = (u32::from(payload[0]) << 16)
                | (u32::from(payload[1]) << 8)
                | u32::from(payload[2]);
            if us > 0 {
                meta.bpm = 60_000_000.0 / f64::from(us);
                meta.seen_tempo = true;
            }
        }
        0x58 if payload.len() >= 2 && !meta.seen_time_sig => {
            let (num, den_pow) = (payload[0], payload[1]);
            if num > 0 && den_pow <= 7 {
                meta.time_signature = (num, 1u8 << den_pow);
                meta.seen_time_sig = true;
            }
        }
        0x2F => return true,
        _ => {}
    }
    false
}

fn sort_notes(notes: &mut [MidiNote]) {
    notes.sort_by(|a, b| {
        a.start_beat
            .total_cmp(&b.start_beat)
            .then_with(|| a.pitch.cmp(&b.pitch))
            .then_with(|| a.duration_beats.total_cmp(&b.duration_beats))
    });
}

#[derive(Clone, Copy)]
struct RawNoteEvent {
    tick: u64,
    is_on: bool,
    pitch: u8,
    velocity: u8,
    seq: usize,
}

fn build_sorted_note_events(notes: &[MidiNote]) -> Result<Vec<RawNoteEvent>, SmfError> {
    let mut events = Vec::with_capacity(notes.len() * 2);
    for (idx, note) in notes.iter().enumerate() {
        if !note.start_beat.is_finite() || !note.duration_beats.is_finite() {
            return Err(SmfError::InvalidFormat(
                "MIDI note timing must be finite".into(),
            ));
        }
        let start_f = note.start_beat.max(0.0) * f64::from(DEFAULT_PPQN);
        let dur_f = (note.duration_beats.max(0.0) * f64::from(DEFAULT_PPQN))
            .round()
            .max(1.0);
        if start_f > MAX_VLQ_VALUE as f64 || start_f + dur_f > MAX_VLQ_VALUE as f64 {
            return Err(SmfError::InvalidFormat(
                "MIDI note tick exceeds 28-bit range".into(),
            ));
        }
        let start_tick = start_f.round() as u64;
        let end_tick = start_tick.saturating_add(dur_f as u64);
        let pitch = note.pitch.min(127);
        let vel = if note.velocity.is_finite() {
            (note.velocity.round() as i32).clamp(1, 127) as u8
        } else {
            100
        };
        let off_vel = if note.release_velocity.is_finite() {
            (note.release_velocity.round() as i32).clamp(0, 127) as u8
        } else {
            64
        };
        events.push(RawNoteEvent {
            tick: start_tick,
            is_on: true,
            pitch,
            velocity: vel,
            seq: idx * 2,
        });
        events.push(RawNoteEvent {
            tick: end_tick,
            is_on: false,
            pitch,
            velocity: off_vel,
            seq: idx * 2 + 1,
        });
    }
    events.sort_by(|a, b| {
        a.tick
            .cmp(&b.tick)
            .then_with(|| a.is_on.cmp(&b.is_on))
            .then_with(|| a.pitch.cmp(&b.pitch))
            .then_with(|| a.seq.cmp(&b.seq))
    });
    Ok(events)
}

fn write_note_events(buf: &mut Vec<u8>, events: &[RawNoteEvent]) -> Result<(), SmfError> {
    let mut prev_tick = 0u64;
    for ev in events {
        write_vlq(buf, ev.tick.saturating_sub(prev_tick))?;
        buf.extend_from_slice(&[if ev.is_on { 0x90 } else { 0x80 }, ev.pitch, ev.velocity]);
        prev_tick = ev.tick;
    }
    Ok(())
}

fn write_meta_event(
    buf: &mut Vec<u8>,
    delta: u64,
    meta_type: u8,
    payload: &[u8],
) -> Result<(), SmfError> {
    write_vlq(buf, delta)?;
    buf.extend_from_slice(&[0xFF, meta_type]);
    write_vlq(buf, payload.len() as u64)?;
    buf.extend_from_slice(payload);
    Ok(())
}

fn append_mtrk_chunk(out: &mut Vec<u8>, track_data: &[u8]) -> Result<(), SmfError> {
    let len = u32::try_from(track_data.len())
        .map_err(|_| SmfError::InvalidFormat("MTrk chunk exceeds 4 GiB".into()))?;
    out.extend_from_slice(b"MTrk");
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(track_data);
    Ok(())
}

fn write_vlq(buf: &mut Vec<u8>, mut value: u64) -> Result<(), SmfError> {
    if value > MAX_VLQ_VALUE {
        return Err(SmfError::InvalidFormat("VLQ exceeds 28-bit limit".into()));
    }
    let mut encoded = [0u8; 4];
    let mut len = 1usize;
    encoded[0] = (value & 0x7F) as u8;
    value >>= 7;
    while value > 0 {
        encoded[len] = ((value & 0x7F) as u8) | 0x80;
        len += 1;
        value >>= 7;
    }
    for i in (0..len).rev() {
        buf.push(encoded[i]);
    }
    Ok(())
}

fn read_vlq(data: &[u8], cursor: &mut usize) -> Result<u64, SmfError> {
    let mut value = 0u64;
    for byte_idx in 0..4 {
        let byte = read_u8(data, cursor)?;
        value = (value << 7) | u64::from(byte & 0x7F);
        if byte & 0x80 == 0 {
            return Ok(value);
        }
        if byte_idx == 3 {
            return Err(SmfError::InvalidFormat("VLQ exceeds 4 bytes".into()));
        }
    }
    Err(SmfError::InvalidFormat("VLQ exceeds 4 bytes".into()))
}

fn read_u8(data: &[u8], cursor: &mut usize) -> Result<u8, SmfError> {
    let b = *data.get(*cursor).ok_or(SmfError::UnexpectedEof)?;
    *cursor += 1;
    Ok(b)
}

fn read_u32_be(data: &[u8], cursor: &mut usize) -> Result<u32, SmfError> {
    Ok(u32::from_be_bytes(read_bytes::<4>(data, cursor)?))
}

fn read_bytes<const N: usize>(data: &[u8], cursor: &mut usize) -> Result<[u8; N], SmfError> {
    let end = cursor.checked_add(N).ok_or(SmfError::UnexpectedEof)?;
    let slice = data.get(*cursor..end).ok_or(SmfError::UnexpectedEof)?;
    let mut out = [0u8; N];
    out.copy_from_slice(slice);
    *cursor = end;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::collections::BTreeMap;

    #[test]
    fn vlq_encode_decode_boundaries() {
        for val in [
            0u64,
            1,
            0x7F,
            0x80,
            0x3FFF,
            0x4000,
            0x001F_FFFF,
            MAX_VLQ_VALUE,
        ] {
            let mut buf = Vec::new();
            write_vlq(&mut buf, val).expect("write vlq");
            let mut cursor = 0;
            assert_eq!(read_vlq(&buf, &mut cursor).expect("read vlq"), val);
            assert_eq!(cursor, buf.len());
        }
        let mut cursor = 0;
        assert!(matches!(
            read_vlq(&[0x80, 0x80, 0x80, 0x80], &mut cursor).unwrap_err(),
            SmfError::InvalidFormat(_)
        ));
    }

    #[test]
    fn read_smf_handles_running_status_cc_pitchbend_aftertouch_and_vel0_off() {
        let mut trk = Vec::new();
        trk.extend_from_slice(&[0x00, 0xFF, 0x03, 0x04, b'L', b'e', b'a', b'd']);
        trk.extend_from_slice(&[0x00, 0xFF, 0x51, 0x03, 0x07, 0x53, 0x00]); // 125 BPM
        trk.extend_from_slice(&[0x00, 0xFF, 0x58, 0x04, 0x03, 0x03, 0x18, 0x08]); // 3/8
        trk.extend_from_slice(&[0x00, 0x90, 60, 100, 120, 64, 90]); // NoteOn + running status
        trk.extend_from_slice(&[0x00, 0xB0, 1, 64, 0x00, 0xD0, 50, 0x00, 0xC0, 5]);
        trk.extend_from_slice(&[0x00, 0xE0, 0x00, 0x40, 0x00, 0xA0, 60, 40]);
        trk.extend_from_slice(&[0x00, 0xF0, 0x02, 0x7E, 0xF7]); // SysEx cancels running status
        trk.extend_from_slice(&[120, 0x90, 60, 0, 120, 64, 0]); // vel 0 -> NoteOff + running status
        trk.extend_from_slice(&[0x00, 0xFF, 0x2F, 0x00]);

        let mut smf = Vec::new();
        smf.extend_from_slice(b"MThd");
        smf.extend_from_slice(&6u32.to_be_bytes());
        smf.extend_from_slice(&0u16.to_be_bytes());
        smf.extend_from_slice(&1u16.to_be_bytes());
        smf.extend_from_slice(&480u16.to_be_bytes());
        append_mtrk_chunk(&mut smf, &trk).expect("append track");

        let parsed = parse_smf_internal(&smf).expect("parse running status SMF");
        assert_eq!(parsed.first_track_name.as_deref(), Some("Lead"));
        assert!((parsed.data.bpm - 125.0).abs() < 1e-4);
        assert_eq!(parsed.data.time_signature, (3, 8));
        assert_eq!(parsed.data.merged_notes.len(), 2);
        assert!((parsed.data.merged_notes[0].duration_beats - 0.5).abs() < 1e-9);
        assert!((parsed.data.merged_notes[1].start_beat - 0.25).abs() < 1e-9);
    }

    #[test]
    fn smpte_division_is_rejected_cleanly() {
        let mut smf = Vec::from(*b"MThd\x00\x00\x00\x06\x00\x00\x00\x01\xE7\x28");
        assert!(matches!(
            read_smf(&smf).unwrap_err(),
            SmfError::Unsupported(_)
        ));
        smf[12] = 0x00;
        smf[13] = 0x00;
        assert!(matches!(
            read_smf(&smf).unwrap_err(),
            SmfError::InvalidFormat(_)
        ));
    }

    fn arb_non_overlapping_notes() -> impl Strategy<Value = Vec<MidiNote>> {
        proptest::collection::vec(
            (0u8..=127, 0u16..=640, 1u16..=96, 1u8..=127, 0u8..=127),
            0..=32,
        )
        .prop_map(|raw| {
            let mut last_end_by_pitch: BTreeMap<u8, u64> = BTreeMap::new();
            let mut out = Vec::with_capacity(raw.len());
            for (pitch, gap_16ths, dur_16ths, vel, rel_vel) in raw {
                let start_ticks =
                    last_end_by_pitch.get(&pitch).copied().unwrap_or(0) + u64::from(gap_16ths) * 30;
                let dur_ticks = u64::from(dur_16ths) * 30;
                last_end_by_pitch.insert(pitch, start_ticks + dur_ticks);
                let mut note = MidiNote::new(
                    pitch,
                    start_ticks as f64 / f64::from(DEFAULT_PPQN),
                    dur_ticks as f64 / f64::from(DEFAULT_PPQN),
                    vel,
                );
                note.release_velocity = f32::from(rel_vel);
                out.push(note);
            }
            sort_notes(&mut out);
            out
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(512))]

        #[test]
        fn arbitrary_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..1024)) {
            let _ = read_smf(&bytes);
        }

        #[test]
        fn smf_roundtrip_preserves_notes_tempo_and_time_sig(
            notes in arb_non_overlapping_notes(),
            bpm in 40.0f64..=240.0,
            ts_num in 1u8..=12,
            ts_den in prop_oneof![Just(1u8), Just(2), Just(4), Just(8), Just(16), Just(32)],
            format in prop_oneof![Just(SmfFormat::Format0), Just(SmfFormat::Format1)],
        ) {
            let encoded = write_smf(&notes, bpm, (ts_num, ts_den), format, Some("PropTrack"))
                .expect("write_smf");
            let decoded = read_smf(&encoded).expect("read_smf");

            prop_assert_eq!(decoded.format, format);
            prop_assert_eq!(decoded.ppqn, DEFAULT_PPQN);
            prop_assert!((decoded.bpm - bpm).abs() < 0.05);
            prop_assert_eq!(decoded.time_signature, (ts_num, ts_den));
            prop_assert_eq!(decoded.merged_notes.len(), notes.len());

            for (orig, back) in notes.iter().zip(&decoded.merged_notes) {
                prop_assert_eq!(orig.pitch, back.pitch);
                prop_assert!((orig.start_beat - back.start_beat).abs() < 1e-6);
                prop_assert!((orig.duration_beats - back.duration_beats).abs() < 1e-6);
                prop_assert_eq!(orig.velocity, back.velocity);
                prop_assert_eq!(orig.release_velocity, back.release_velocity);
            }
        }
    }
}
