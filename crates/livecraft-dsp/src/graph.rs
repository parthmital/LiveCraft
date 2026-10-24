use crate::compressor::CompressorEffect;
use crate::delay::DelayEffect;
use crate::eq8::EqEight;
use crate::limiter::{LimiterEffect, UtilityEffect};
use crate::operator::OperatorSynth;
use crate::reverb::ReverbEffect;
use crate::saturator::SaturatorEffect;
use crate::simpler::SimplerInstrument;
use livecraft_core::model::*;

#[derive(Debug, Clone, Default)]
pub struct TrackMeter {
    pub peak_l: f32,
    pub peak_r: f32,
    pub rms_l: f32,
    pub rms_r: f32,
}

impl TrackMeter {
    pub fn update(&mut self, l: f32, r: f32) {
        let abs_l = l.abs();
        let abs_r = r.abs();
        // Decay peak
        self.peak_l = abs_l.max(self.peak_l * 0.95);
        self.peak_r = abs_r.max(self.peak_r * 0.95);
        // Simple smoothing for RMS
        self.rms_l = (self.rms_l * 0.9) + (abs_l * 0.1);
        self.rms_r = (self.rms_r * 0.9) + (abs_r * 0.1);
    }

    pub fn peak_db_l(&self) -> f32 {
        if self.peak_l <= 1e-5 {
            -60.0
        } else {
            20.0 * self.peak_l.log10()
        }
    }

    pub fn peak_db_r(&self) -> f32 {
        if self.peak_r <= 1e-5 {
            -60.0
        } else {
            20.0 * self.peak_r.log10()
        }
    }
}

pub struct TrackDspState {
    pub track_id: u64,
    pub synth: Option<OperatorSynth>,
    pub simpler: Option<SimplerInstrument>,
    pub eq8: Option<EqEight>,
    pub delay: Option<DelayEffect>,
    pub reverb: Option<ReverbEffect>,
    pub compressor: Option<CompressorEffect>,
    pub limiter: Option<LimiterEffect>,
    pub utility: Option<UtilityEffect>,
    pub saturator: Option<SaturatorEffect>,
    pub active_clip_slot: Option<usize>,
    pub meter: TrackMeter,
    active_notes: [bool; 128],
}

impl TrackDspState {
    pub fn new(track: &Track, sample_rate: f32) -> Self {
        let mut synth = None;
        let mut simpler = None;
        let mut eq8 = None;
        let mut delay = None;
        let mut reverb = None;
        let mut compressor = None;
        let mut limiter = None;
        let mut utility = None;
        let mut saturator = None;

        if track.track_type == TrackType::Midi {
            synth = Some(OperatorSynth::new(sample_rate));
        }

        for dev in &track.device_chain {
            match dev.device_type {
                DeviceType::Operator => synth = Some(OperatorSynth::new(sample_rate)),
                DeviceType::Simpler => simpler = Some(SimplerInstrument::new(sample_rate)),
                DeviceType::EqEight => eq8 = Some(EqEight::new(sample_rate)),
                DeviceType::Delay => delay = Some(DelayEffect::new(sample_rate)),
                DeviceType::Reverb => reverb = Some(ReverbEffect::new(sample_rate)),
                DeviceType::Compressor => compressor = Some(CompressorEffect::new(sample_rate)),
                DeviceType::Limiter => limiter = Some(LimiterEffect::new(sample_rate)),
                DeviceType::Utility => utility = Some(UtilityEffect::new()),
                DeviceType::Saturator => saturator = Some(SaturatorEffect::new(sample_rate)),
                _ => {}
            }
        }

        Self {
            track_id: track.id,
            synth,
            simpler,
            eq8,
            delay,
            reverb,
            compressor,
            limiter,
            utility,
            saturator,
            active_clip_slot: None,
            meter: TrackMeter::default(),
            active_notes: [false; 128],
        }
    }

    pub fn process_sample(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        let mut l = in_l;
        let mut r = in_r;

        // Sound source
        if let Some(ref mut s) = self.synth {
            let (sl, sr) = s.process_sample();
            l += sl;
            r += sr;
        } else if let Some(ref mut sm) = self.simpler {
            let (sl, sr) = sm.process_sample();
            l += sl;
            r += sr;
        }

        // Effect chain
        if let Some(ref mut eq) = self.eq8 {
            let (el, er) = eq.process_stereo(l, r);
            l = el;
            r = er;
        }

        if let Some(ref mut comp) = self.compressor {
            let (cl, cr) = comp.process_stereo(l, r);
            l = cl;
            r = cr;
        }

        if let Some(ref mut del) = self.delay {
            let (dl, dr) = del.process_stereo(l, r);
            l = dl;
            r = dr;
        }

        if let Some(ref mut rev) = self.reverb {
            let (rl, rr) = rev.process_stereo(l, r);
            l = rl;
            r = rr;
        }

        if let Some(ref mut lim) = self.limiter {
            let (ll, lr) = lim.process_stereo(l, r);
            l = ll;
            r = lr;
        }

        if let Some(ref ut) = self.utility {
            let (ul, ur) = ut.process_stereo(l, r);
            l = ul;
            r = ur;
        }

        if let Some(ref mut sat) = self.saturator {
            let (sl, sr) = sat.process_stereo(l, r);
            l = sl;
            r = sr;
        }

        self.meter.update(l, r);
        (l, r)
    }

    pub fn trigger_clip(&mut self, slot_idx: usize) {
        self.active_clip_slot = Some(slot_idx);
        self.stop_all_notes();
    }

    pub fn stop_clip(&mut self) {
        self.active_clip_slot = None;
        self.stop_all_notes();
    }

    pub fn stop_all_notes(&mut self) {
        if let Some(ref mut s) = self.synth {
            s.all_notes_off();
        }
        self.active_notes = [false; 128];
    }
}

pub struct AudioGraph {
    pub sample_rate: f32,
    pub track_states: Vec<TrackDspState>,
    pub return_states: Vec<TrackDspState>,
    pub master_state: TrackDspState,
    pub master_meter: TrackMeter,
    pub playhead_beats: f64,
    pub is_playing: bool,
    pub bpm: f32,
    pub scratch_l: Vec<f32>,
    pub scratch_r: Vec<f32>,
}

impl AudioGraph {
    pub fn new(project: &Project, sample_rate: f32) -> Self {
        let track_states = project
            .tracks
            .iter()
            .map(|t| TrackDspState::new(t, sample_rate))
            .collect();
        let return_states = project
            .return_tracks
            .iter()
            .map(|t| TrackDspState::new(t, sample_rate))
            .collect();
        let master_state = TrackDspState::new(&project.master_track, sample_rate);

        let mut scratch_l = Vec::with_capacity(8192);
        scratch_l.resize(8192, 0.0);
        let mut scratch_r = Vec::with_capacity(8192);
        scratch_r.resize(8192, 0.0);

        Self {
            sample_rate,
            track_states,
            return_states,
            master_state,
            master_meter: TrackMeter::default(),
            playhead_beats: 0.0,
            is_playing: false,
            bpm: project.transport.bpm,
            scratch_l,
            scratch_r,
        }
    }

    pub fn trigger_clip(&mut self, track_idx: usize, slot_idx: usize) {
        if let Some(state) = self.track_states.get_mut(track_idx) {
            state.trigger_clip(slot_idx);
        }
    }

    pub fn stop_clip(&mut self, track_idx: usize) {
        if let Some(state) = self.track_states.get_mut(track_idx) {
            state.stop_clip();
        }
    }

    pub fn trigger_scene(&mut self, scene_idx: usize, project: &Project) {
        for (trk_idx, trk) in project.tracks.iter().enumerate() {
            if scene_idx < trk.clip_slots.len() && trk.clip_slots[scene_idx].clip.is_some() {
                self.trigger_clip(trk_idx, scene_idx);
            }
        }
    }

    pub fn stop_all(&mut self) {
        for state in &mut self.track_states {
            state.stop_clip();
        }
        self.is_playing = false;
        self.playhead_beats = 0.0;
    }

    pub fn process_buffer_interleaved(
        &mut self,
        data: &mut [f32],
        channels: usize,
        project: &Project,
    ) {
        let frames = data.len() / channels;
        let mut scratch_l = std::mem::take(&mut self.scratch_l);
        let mut scratch_r = std::mem::take(&mut self.scratch_r);
        if frames > scratch_l.len() {
            scratch_l.resize(frames, 0.0);
            scratch_r.resize(frames, 0.0);
        }
        self.process_buffer(&mut scratch_l[..frames], &mut scratch_r[..frames], project);

        for (frame_idx, chunk) in data.chunks_mut(channels).enumerate() {
            let l = scratch_l[frame_idx];
            let r = scratch_r[frame_idx];
            if channels >= 2 {
                chunk[0] = l;
                chunk[1] = r;
            } else if channels == 1 {
                chunk[0] = (l + r) * 0.5;
            }
        }

        self.scratch_l = scratch_l;
        self.scratch_r = scratch_r;
    }

    pub fn process_buffer_interleaved_i16(
        &mut self,
        data: &mut [i16],
        channels: usize,
        project: &Project,
    ) {
        let frames = data.len() / channels;
        let mut scratch_l = std::mem::take(&mut self.scratch_l);
        let mut scratch_r = std::mem::take(&mut self.scratch_r);
        if frames > scratch_l.len() {
            scratch_l.resize(frames, 0.0);
            scratch_r.resize(frames, 0.0);
        }
        self.process_buffer(&mut scratch_l[..frames], &mut scratch_r[..frames], project);

        for (frame_idx, chunk) in data.chunks_mut(channels).enumerate() {
            let l = scratch_l[frame_idx];
            let r = scratch_r[frame_idx];
            let s_l = (l.clamp(-1.0, 1.0) * 32767.0) as i16;
            let s_r = (r.clamp(-1.0, 1.0) * 32767.0) as i16;
            if channels >= 2 {
                chunk[0] = s_l;
                chunk[1] = s_r;
            } else if channels == 1 {
                chunk[0] = (((l + r) * 0.5).clamp(-1.0, 1.0) * 32767.0) as i16;
            }
        }

        self.scratch_l = scratch_l;
        self.scratch_r = scratch_r;
    }

    pub fn process_buffer_interleaved_u16(
        &mut self,
        data: &mut [u16],
        channels: usize,
        project: &Project,
    ) {
        let frames = data.len() / channels;
        let mut scratch_l = std::mem::take(&mut self.scratch_l);
        let mut scratch_r = std::mem::take(&mut self.scratch_r);
        if frames > scratch_l.len() {
            scratch_l.resize(frames, 0.0);
            scratch_r.resize(frames, 0.0);
        }
        self.process_buffer(&mut scratch_l[..frames], &mut scratch_r[..frames], project);

        for (frame_idx, chunk) in data.chunks_mut(channels).enumerate() {
            let l = scratch_l[frame_idx];
            let r = scratch_r[frame_idx];
            let s_l = ((l * 0.5 + 0.5).clamp(0.0, 1.0) * 65535.0) as u16;
            let s_r = ((r * 0.5 + 0.5).clamp(0.0, 1.0) * 65535.0) as u16;
            if channels >= 2 {
                chunk[0] = s_l;
                chunk[1] = s_r;
            } else if channels == 1 {
                chunk[0] = ((((l + r) * 0.5) * 0.5 + 0.5).clamp(0.0, 1.0) * 65535.0) as u16;
            }
        }

        self.scratch_l = scratch_l;
        self.scratch_r = scratch_r;
    }

    pub fn process_buffer(
        &mut self,
        buffer_l: &mut [f32],
        buffer_r: &mut [f32],
        project: &Project,
    ) {
        let num_samples = buffer_l.len();
        let dt_beats = if self.is_playing {
            (self.bpm as f64 / 60.0) / (self.sample_rate as f64)
        } else {
            0.0
        };

        let sub_block_size = 32;
        let mut s_idx = 0;
        while s_idx < num_samples {
            let block_end = (s_idx + sub_block_size).min(num_samples);
            let block_len = block_end - s_idx;

            if self.is_playing {
                self.dispatch_clip_events(project);
                self.playhead_beats += dt_beats * block_len as f64;
            }

            let any_solo = project.tracks.iter().any(|t| t.is_soloed);

            for i in s_idx..block_end {
                let mut master_l = 0.0f32;
                let mut master_r = 0.0f32;
                let mut send_a_l = 0.0f32;
                let mut send_a_r = 0.0f32;
                let mut send_b_l = 0.0f32;
                let mut send_b_r = 0.0f32;

                for (idx, trk) in project.tracks.iter().enumerate() {
                    if let Some(state) = self.track_states.get_mut(idx) {
                        let (raw_l, raw_r) = state.process_sample(0.0, 0.0);

                        // Skip muted tracks or un-soloed tracks if someone is soloed
                        if trk.is_muted || (any_solo && !trk.is_soloed) {
                            continue;
                        }

                        // Volume (dB to linear)
                        let vol_lin = if trk.volume_db <= -59.0 {
                            0.0
                        } else {
                            10.0f32.powf(trk.volume_db / 20.0)
                        };

                        // Pan (constant power)
                        let pan_norm = (trk.pan.clamp(-1.0, 1.0) + 1.0) * 0.5;
                        let angle = pan_norm * std::f32::consts::FRAC_PI_2;
                        let pan_l = angle.cos();
                        let pan_r = angle.sin();

                        let track_out_l = raw_l * vol_lin * pan_l;
                        let track_out_r = raw_r * vol_lin * pan_r;

                        // Accumulate to Master
                        master_l += track_out_l;
                        master_r += track_out_r;

                        // Sends
                        if let Some(&send_a) = trk.sends.first() {
                            send_a_l += track_out_l * send_a;
                            send_a_r += track_out_r * send_a;
                        }
                        if let Some(&send_b) = trk.sends.get(1) {
                            send_b_l += track_out_l * send_b;
                            send_b_r += track_out_r * send_b;
                        }
                    }
                }

                // Process Return A
                if let Some(ret_a_state) = self.return_states.first_mut() {
                    let (ret_l, ret_r) = ret_a_state.process_sample(send_a_l, send_a_r);
                    master_l += ret_l;
                    master_r += ret_r;
                }

                // Process Return B
                if let Some(ret_b_state) = self.return_states.get_mut(1) {
                    let (ret_l, ret_r) = ret_b_state.process_sample(send_b_l, send_b_r);
                    master_l += ret_l;
                    master_r += ret_r;
                }

                // Master channel processing
                let (final_l, final_r) = self.master_state.process_sample(master_l, master_r);
                self.master_meter.update(final_l, final_r);

                buffer_l[i] = final_l;
                buffer_r[i] = final_r;
            }
            s_idx = block_end;
        }
    }

    fn dispatch_clip_events(&mut self, project: &Project) {
        for (trk_idx, trk) in project.tracks.iter().enumerate() {
            if let Some(state) = self.track_states.get_mut(trk_idx) {
                if let Some(slot_idx) = state.active_clip_slot {
                    if let Some(slot) = trk.clip_slots.get(slot_idx) {
                        if let Some(ref clip) = slot.clip {
                            let clip_len = clip.length_beats.max(1.0);
                            let clip_pos = (self.playhead_beats % clip_len).abs();

                            if let ClipPayload::Midi(ref midi_data) = clip.payload {
                                for note in &midi_data.notes {
                                    if !note.muted {
                                        let note_start = note.start_beat;
                                        let note_end = note.start_beat + note.duration_beats;
                                        let p = note.pitch as usize;

                                        // Note is active now
                                        if clip_pos >= note_start && clip_pos < note_end {
                                            if p < 128 && !state.active_notes[p] {
                                                state.active_notes[p] = true;
                                                if let Some(ref mut s) = state.synth {
                                                    s.note_on(note.pitch, note.velocity);
                                                }
                                            }
                                        } else if p < 128 && state.active_notes[p] {
                                            // Note ended
                                            state.active_notes[p] = false;
                                            if let Some(ref mut s) = state.synth {
                                                s.note_off(note.pitch);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
