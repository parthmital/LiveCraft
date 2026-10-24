use crate::audio_clip::AudioSampleCache;
use crate::autofilter::AutoFilterEffect;
use crate::chorus::ChorusEffect;
use crate::compressor::CompressorEffect;
use crate::delay::DelayEffect;
use crate::drumrack::DrumRackInstrument;
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
    pub drum_rack: Option<DrumRackInstrument>,
    pub eq8: Option<EqEight>,
    pub delay: Option<DelayEffect>,
    pub reverb: Option<ReverbEffect>,
    pub compressor: Option<CompressorEffect>,
    pub limiter: Option<LimiterEffect>,
    pub utility: Option<UtilityEffect>,
    pub saturator: Option<SaturatorEffect>,
    pub auto_filter: Option<AutoFilterEffect>,
    pub chorus: Option<ChorusEffect>,
    pub audio_sample_cache: AudioSampleCache,
    pub current_audio_in: (f32, f32),
    pub active_clip_slot: Option<usize>,
    pub meter: TrackMeter,
    active_notes: [bool; 128],
}

impl TrackDspState {
    pub fn new(track: &Track, sample_rate: f32) -> Self {
        let mut state = Self {
            track_id: track.id,
            synth: None,
            simpler: None,
            drum_rack: None,
            eq8: None,
            delay: None,
            reverb: None,
            compressor: None,
            limiter: None,
            utility: None,
            saturator: None,
            auto_filter: None,
            chorus: None,
            audio_sample_cache: AudioSampleCache::new(),
            current_audio_in: (0.0, 0.0),
            active_clip_slot: None,
            meter: TrackMeter::default(),
            active_notes: [false; 128],
        };
        state.sync_devices(track, sample_rate);
        state
    }

    pub fn sync_devices(&mut self, track: &Track, sample_rate: f32) {
        let has_device =
            |dtype: DeviceType| track.device_chain.iter().any(|d| d.device_type == dtype);

        if track.track_type == TrackType::Midi {
            if has_device(DeviceType::DrumRack) {
                if self.drum_rack.is_none() {
                    self.drum_rack = Some(DrumRackInstrument::new(sample_rate));
                }
                self.synth = None;
                self.simpler = None;
            } else if has_device(DeviceType::Simpler) {
                if self.simpler.is_none() {
                    self.simpler = Some(SimplerInstrument::new(sample_rate));
                }
                self.synth = None;
                self.drum_rack = None;
            } else if self.synth.is_none() {
                self.synth = Some(OperatorSynth::new(sample_rate));
                self.drum_rack = None;
            }
        }

        if !has_device(DeviceType::EqEight) {
            self.eq8 = None;
        }
        if !has_device(DeviceType::Delay) {
            self.delay = None;
        }
        if !has_device(DeviceType::Reverb) {
            self.reverb = None;
        }
        if !has_device(DeviceType::Compressor) {
            self.compressor = None;
        }
        if !has_device(DeviceType::Limiter) {
            self.limiter = None;
        }
        if !has_device(DeviceType::Utility) {
            self.utility = None;
        }
        if !has_device(DeviceType::Saturator) {
            self.saturator = None;
        }
        if !has_device(DeviceType::AutoFilter) {
            self.auto_filter = None;
        }
        if !has_device(DeviceType::Chorus) {
            self.chorus = None;
        }

        for dev in &track.device_chain {
            match dev.device_type {
                DeviceType::Operator => {
                    if self.synth.is_none() {
                        self.synth = Some(OperatorSynth::new(sample_rate));
                    }
                    if let Some(ref mut s) = self.synth {
                        s.update_from_device(dev);
                    }
                }
                DeviceType::Simpler => {
                    if self.simpler.is_none() {
                        self.simpler = Some(SimplerInstrument::new(sample_rate));
                    }
                    if let Some(ref mut sm) = self.simpler {
                        sm.update_from_device(dev);
                    }
                }
                DeviceType::DrumRack => {
                    if self.drum_rack.is_none() {
                        self.drum_rack = Some(DrumRackInstrument::new(sample_rate));
                    }
                    if let Some(ref mut dr) = self.drum_rack {
                        dr.update_from_device(dev);
                    }
                }
                DeviceType::EqEight => {
                    if self.eq8.is_none() {
                        self.eq8 = Some(EqEight::new(sample_rate));
                    }
                    if let Some(ref mut eq) = self.eq8 {
                        eq.update_from_device(dev);
                    }
                }
                DeviceType::Compressor => {
                    if self.compressor.is_none() {
                        self.compressor = Some(CompressorEffect::new(sample_rate));
                    }
                    if let Some(ref mut c) = self.compressor {
                        c.update_from_device(dev);
                    }
                }
                DeviceType::Delay => {
                    if self.delay.is_none() {
                        self.delay = Some(DelayEffect::new(sample_rate));
                    }
                    if let Some(ref mut d) = self.delay {
                        d.update_from_device(dev);
                    }
                }
                DeviceType::Reverb => {
                    if self.reverb.is_none() {
                        self.reverb = Some(ReverbEffect::new(sample_rate));
                    }
                    if let Some(ref mut r) = self.reverb {
                        r.update_from_device(dev);
                    }
                }
                DeviceType::Limiter => {
                    if self.limiter.is_none() {
                        self.limiter = Some(LimiterEffect::new(sample_rate));
                    }
                    if let Some(ref mut l) = self.limiter {
                        l.update_from_device(dev);
                    }
                }
                DeviceType::Utility => {
                    if self.utility.is_none() {
                        self.utility = Some(UtilityEffect::new());
                    }
                    if let Some(ref mut u) = self.utility {
                        u.update_from_device(dev);
                    }
                }
                DeviceType::Saturator => {
                    if self.saturator.is_none() {
                        self.saturator = Some(SaturatorEffect::new(sample_rate));
                    }
                    if let Some(ref mut sat) = self.saturator {
                        sat.update_from_device(dev);
                    }
                }
                DeviceType::AutoFilter => {
                    if self.auto_filter.is_none() {
                        self.auto_filter = Some(AutoFilterEffect::new(sample_rate));
                    }
                    if let Some(ref mut af) = self.auto_filter {
                        af.update_from_device(dev);
                    }
                }
                DeviceType::Chorus => {
                    if self.chorus.is_none() {
                        self.chorus = Some(ChorusEffect::new(sample_rate));
                    }
                    if let Some(ref mut ch) = self.chorus {
                        ch.update_from_device(dev);
                    }
                }
                _ => {}
            }
        }
    }

    pub fn process_sample(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        let mut l = in_l;
        let mut r = in_r;

        // Sound source: Synth, Simpler, or Drum Rack
        if let Some(ref mut s) = self.synth {
            let (sl, sr) = s.process_sample();
            l += sl;
            r += sr;
        } else if let Some(ref mut sm) = self.simpler {
            let (sl, sr) = sm.process_sample();
            l += sl;
            r += sr;
        } else if let Some(ref mut dr) = self.drum_rack {
            let (sl, sr) = dr.process_sample();
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

        if let Some(ref mut sat) = self.saturator {
            let (sl, sr) = sat.process_stereo(l, r);
            l = sl;
            r = sr;
        }

        if let Some(ref mut af) = self.auto_filter {
            let (fl, fr) = af.process_stereo(l, r);
            l = fl;
            r = fr;
        }

        if let Some(ref mut ch) = self.chorus {
            let (cl, cr) = ch.process_stereo(l, r);
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
        if let Some(ref mut sm) = self.simpler {
            sm.all_notes_off();
        }
        if let Some(ref mut dr) = self.drum_rack {
            dr.all_notes_off();
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

    pub fn sync_project(&mut self, project: &Project) {
        self.bpm = project.transport.bpm;

        // Tracks
        while self.track_states.len() < project.tracks.len() {
            let idx = self.track_states.len();
            self.track_states
                .push(TrackDspState::new(&project.tracks[idx], self.sample_rate));
        }
        if self.track_states.len() > project.tracks.len() {
            self.track_states.truncate(project.tracks.len());
        }
        for (idx, trk) in project.tracks.iter().enumerate() {
            if let Some(state) = self.track_states.get_mut(idx) {
                state.track_id = trk.id;
                state.sync_devices(trk, self.sample_rate);
            }
        }

        // Return tracks
        while self.return_states.len() < project.return_tracks.len() {
            let idx = self.return_states.len();
            self.return_states.push(TrackDspState::new(
                &project.return_tracks[idx],
                self.sample_rate,
            ));
        }
        if self.return_states.len() > project.return_tracks.len() {
            self.return_states.truncate(project.return_tracks.len());
        }
        for (idx, trk) in project.return_tracks.iter().enumerate() {
            if let Some(state) = self.return_states.get_mut(idx) {
                state.track_id = trk.id;
                state.sync_devices(trk, self.sample_rate);
            }
        }

        // Master track
        self.master_state
            .sync_devices(&project.master_track, self.sample_rate);
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
                        let (raw_l, raw_r) = state
                            .process_sample(state.current_audio_in.0, state.current_audio_in.1);

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
        let bpm = project.transport.bpm;
        for (trk_idx, trk) in project.tracks.iter().enumerate() {
            if let Some(state) = self.track_states.get_mut(trk_idx) {
                if let Some(slot_idx) = state.active_clip_slot {
                    if let Some(slot) = trk.clip_slots.get(slot_idx) {
                        if let Some(ref clip) = slot.clip {
                            let clip_len = clip.length_beats.max(1.0);
                            let clip_pos = (self.playhead_beats % clip_len).abs();
                            Self::process_clip_payload(state, clip, clip_pos, bpm);
                        }
                    }
                } else if !trk.arrangement_clips.is_empty() {
                    let mut clip_found = false;
                    for arr_clip in &trk.arrangement_clips {
                        let c_start = arr_clip.start_beat;
                        let c_end = arr_clip.start_beat + arr_clip.length_beats;
                        if self.playhead_beats >= c_start && self.playhead_beats < c_end {
                            clip_found = true;
                            let rel_beat = self.playhead_beats - c_start;
                            let clip_len = arr_clip.clip.length_beats.max(1.0);
                            let clip_pos = if arr_clip.clip.is_looping {
                                rel_beat % clip_len
                            } else {
                                rel_beat
                            };
                            Self::process_clip_payload(state, &arr_clip.clip, clip_pos, bpm);
                        }
                    }
                    if !clip_found {
                        state.stop_all_notes();
                        state.current_audio_in = (0.0, 0.0);
                    }
                } else {
                    let mut current_beat = 0.0;
                    let mut clip_found = false;
                    for slot in &trk.clip_slots {
                        if let Some(ref clip) = slot.clip {
                            let c_start = current_beat;
                            let c_end = current_beat + clip.length_beats;
                            if self.playhead_beats >= c_start && self.playhead_beats < c_end {
                                clip_found = true;
                                let clip_pos = self.playhead_beats - c_start;
                                Self::process_clip_payload(state, clip, clip_pos, bpm);
                                break;
                            }
                            current_beat += clip.length_beats;
                        }
                    }
                    if !clip_found {
                        if current_beat > 0.0 {
                            state.stop_all_notes();
                        }
                        state.current_audio_in = (0.0, 0.0);
                    }
                }
            }
        }
    }

    fn process_clip_payload(state: &mut TrackDspState, clip: &Clip, clip_pos: f64, bpm: f32) {
        match clip.payload {
            ClipPayload::Midi(ref midi_data) => {
                state.current_audio_in = (0.0, 0.0);
                for note in &midi_data.notes {
                    if !note.muted {
                        let note_start = note.start_beat;
                        let note_end = note.start_beat + note.duration_beats;
                        let p = note.pitch as usize;

                        if clip_pos >= note_start && clip_pos < note_end {
                            if p < 128 && !state.active_notes[p] {
                                state.active_notes[p] = true;
                                if let Some(ref mut s) = state.synth {
                                    s.note_on(note.pitch, note.velocity);
                                }
                                if let Some(ref mut sm) = state.simpler {
                                    sm.note_on(note.pitch, note.velocity);
                                }
                                if let Some(ref mut dr) = state.drum_rack {
                                    dr.note_on(note.pitch, note.velocity);
                                }
                            }
                        } else if p < 128 && state.active_notes[p] {
                            state.active_notes[p] = false;
                            if let Some(ref mut s) = state.synth {
                                s.note_off(note.pitch);
                            }
                            if let Some(ref mut sm) = state.simpler {
                                sm.note_off(note.pitch);
                            }
                            if let Some(ref mut dr) = state.drum_rack {
                                dr.note_off(note.pitch);
                            }
                        }
                    }
                }
            }
            ClipPayload::Audio(_) => {
                state.current_audio_in = state.audio_sample_cache.sample_clip(clip, clip_pos, bpm);
            }
        }
    }
}
