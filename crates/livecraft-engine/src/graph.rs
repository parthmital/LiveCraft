use crate::audio_clip::AudioSampleCache;
use crate::engine::AudioParamMsg;
use livecraft_devices::autofilter::AutoFilterEffect;
use livecraft_devices::autopan::AutoPanEffect;
use livecraft_devices::chorus::ChorusEffect;
use livecraft_devices::compressor::CompressorEffect;
use livecraft_devices::delay::DelayEffect;
use livecraft_devices::drumrack::DrumRackInstrument;
use livecraft_devices::eq3::EqThreeEffect;
use livecraft_devices::eq8::EqEight;
use livecraft_devices::gate::GateEffect;
use livecraft_devices::limiter::{LimiterEffect, UtilityEffect};
use livecraft_devices::operator::OperatorSynth;
use livecraft_devices::processor::{DspProcessor, create_processor};
use livecraft_devices::redux::ReduxEffect;
use livecraft_devices::reverb::ReverbEffect;
use livecraft_devices::saturator::SaturatorEffect;
use livecraft_devices::simpler::SimplerInstrument;
use livecraft_model::ids::{DeviceId, SceneId, TrackId};
use livecraft_model::model::*;
use livecraft_time::beats_per_sample;

/// Rounds a note velocity to the 0 to 127 integer that the instruments take.
/// Fractional velocities reach the instruments once they accept `f32` (M32, M33).
#[inline]
fn midi_velocity(velocity: f32) -> u8 {
    velocity.round().clamp(0.0, 127.0) as u8
}

/// Replaces NaN and infinity with silence; finite samples pass through bit-exactly.
#[inline]
pub fn sanitize_output_sample(sample: f32) -> f32 {
    if sample.is_finite() { sample } else { 0.0 }
}

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

#[derive(Debug, Clone, PartialEq)]
pub struct ActiveNoteInstance {
    pub pitch: u8,
    pub note_index: usize,
    pub start_beat: f64,
    pub duration_beats: f64,
}

pub struct TrackDspState {
    pub track_id: TrackId,
    pub volume_db: f32,
    pub pan: f32,
    pub is_muted: bool,
    pub is_soloed: bool,
    pub is_armed: bool,
    pub monitoring: MonitoringMode,
    pub sends: Vec<f32>,
    pub clip_slots: Vec<ClipSlot>,
    pub arrangement_clips: Vec<ArrangementClip>,
    pub device_chain: Vec<Device>,
    pub synth: Option<OperatorSynth>,
    pub simpler: Option<SimplerInstrument>,
    pub drum_rack: Option<DrumRackInstrument>,
    pub processors: Vec<Box<dyn DspProcessor>>,
    pub eq8: Option<EqEight>,
    pub delay: Option<DelayEffect>,
    pub reverb: Option<ReverbEffect>,
    pub compressor: Option<CompressorEffect>,
    pub limiter: Option<LimiterEffect>,
    pub utility: Option<UtilityEffect>,
    pub saturator: Option<SaturatorEffect>,
    pub auto_filter: Option<AutoFilterEffect>,
    pub chorus: Option<ChorusEffect>,
    pub auto_pan: Option<AutoPanEffect>,
    pub gate: Option<GateEffect>,
    pub eq3: Option<EqThreeEffect>,
    pub redux: Option<ReduxEffect>,
    pub audio_sample_cache: AudioSampleCache,
    pub current_audio_in: (f32, f32),
    pub active_clip_slot: Option<usize>,
    pub meter: TrackMeter,
    pub active_note_instances: Vec<ActiveNoteInstance>,
    pub audio_block_l: [f32; 32],
    pub audio_block_r: [f32; 32],
}

impl TrackDspState {
    pub fn new(track: &Track, sample_rate: f32) -> Self {
        let mut state = Self {
            track_id: track.id,
            volume_db: track.volume_db,
            pan: track.pan,
            is_muted: track.is_muted,
            is_soloed: track.is_soloed,
            is_armed: track.is_armed,
            monitoring: track.monitoring,
            sends: track.sends.clone(),
            clip_slots: track.clip_slots.clone(),
            arrangement_clips: track.arrangement_clips.clone(),
            device_chain: track.device_chain.clone(),
            synth: None,
            simpler: None,
            drum_rack: None,
            processors: Vec::new(),
            eq8: None,
            delay: None,
            reverb: None,
            compressor: None,
            limiter: None,
            utility: None,
            saturator: None,
            auto_filter: None,
            chorus: None,
            auto_pan: None,
            gate: None,
            eq3: None,
            redux: None,
            audio_sample_cache: AudioSampleCache::new(),
            current_audio_in: (0.0, 0.0),
            active_clip_slot: None,
            meter: TrackMeter::default(),
            active_note_instances: Vec::new(),
            audio_block_l: [0.0; 32],
            audio_block_r: [0.0; 32],
        };
        state.sync_devices(track, sample_rate);
        state
    }

    pub fn sync_devices(&mut self, track: &Track, sample_rate: f32) {
        self.track_id = track.id;
        self.volume_db = track.volume_db;
        self.pan = track.pan;
        self.is_muted = track.is_muted;
        self.is_soloed = track.is_soloed;
        self.is_armed = track.is_armed;
        self.monitoring = track.monitoring;
        self.sends = track.sends.clone();
        self.clip_slots = track.clip_slots.clone();
        self.arrangement_clips = track.arrangement_clips.clone();
        self.device_chain = track.device_chain.clone();

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
        if !has_device(DeviceType::AutoPan) {
            self.auto_pan = None;
        }
        if !has_device(DeviceType::Gate) {
            self.gate = None;
        }
        if !has_device(DeviceType::EqThree) {
            self.eq3 = None;
        }
        if !has_device(DeviceType::Redux) {
            self.redux = None;
        }

        let mut new_processors: Vec<Box<dyn DspProcessor>> =
            Vec::with_capacity(track.device_chain.len());
        for dev in &track.device_chain {
            if let Some(pos) = self.processors.iter().position(|p| p.device_id() == dev.id) {
                let mut existing = self.processors.remove(pos);
                existing.set_enabled(dev.is_enabled);
                for (k, v) in dev.named_params() {
                    existing.update_param(k, v);
                }
                new_processors.push(existing);
            } else {
                if let Some(proc) = create_processor(dev, sample_rate) {
                    new_processors.push(proc);
                }
            }
        }
        self.processors = new_processors;

        for dev in &track.device_chain {
            self.sync_single_device(dev, sample_rate);
        }
    }

    pub fn set_device_param(
        &mut self,
        device_id: DeviceId,
        param_key: &str,
        value: f32,
        sample_rate: f32,
    ) {
        if let Some(dev) = self
            .device_chain
            .iter_mut()
            .find(|d| d.id == device_id || device_id.0 == 0)
        {
            dev.set_param(param_key, value);
            let dev_clone = dev.clone();
            self.sync_single_device(&dev_clone, sample_rate);
        }
        for proc in &mut self.processors {
            if proc.device_id() == device_id || device_id.0 == 0 {
                proc.update_param(param_key, value);
            }
        }
    }

    pub fn sync_single_device(&mut self, dev: &Device, sample_rate: f32) {
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
            DeviceType::AutoPan => {
                if self.auto_pan.is_none() {
                    self.auto_pan = Some(AutoPanEffect::new(sample_rate));
                }
                if let Some(ref mut ap) = self.auto_pan {
                    ap.amount = dev.get_param("amount");
                    ap.rate_hz = dev.get_param("rate_hz");
                    ap.phase_deg = dev.get_param("phase_deg");
                    ap.shape = dev.get_param("shape");
                    ap.mode = dev.get_param("mode");
                }
            }
            DeviceType::Gate => {
                if self.gate.is_none() {
                    self.gate = Some(GateEffect::new(sample_rate));
                }
                if let Some(ref mut g) = self.gate {
                    g.threshold_db = dev.get_param("threshold_db");
                    g.return_db = dev.get_param("return_db");
                    g.attack_ms = dev.get_param("attack_ms");
                    g.hold_ms = dev.get_param("hold_ms");
                    g.release_ms = dev.get_param("release_ms");
                    g.floor_db = dev.get_param("floor_db");
                }
            }
            DeviceType::EqThree => {
                if self.eq3.is_none() {
                    self.eq3 = Some(EqThreeEffect::new(sample_rate));
                }
                if let Some(ref mut eq) = self.eq3 {
                    eq.low_gain_db = dev.get_param("low_gain_db");
                    eq.mid_gain_db = dev.get_param("mid_gain_db");
                    eq.high_gain_db = dev.get_param("high_gain_db");
                    eq.low_freq = dev.get_param("low_freq");
                    eq.high_freq = dev.get_param("high_freq");
                    eq.low_on = dev.get_param("low_on") > 0.5;
                    eq.mid_on = dev.get_param("mid_on") > 0.5;
                    eq.high_on = dev.get_param("high_on") > 0.5;
                }
            }
            DeviceType::Redux => {
                if self.redux.is_none() {
                    self.redux = Some(ReduxEffect::new(sample_rate));
                }
                if let Some(ref mut rd) = self.redux {
                    rd.rate_factor = dev.get_param("rate_factor");
                    rd.bits = dev.get_param("bits");
                    rd.jitter = dev.get_param("jitter");
                    rd.dry_wet = dev.get_param("dry_wet");
                }
            }
            _ => {}
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

        // Modular effect chain in sequential topological order
        for proc in &mut self.processors {
            if proc.is_enabled() {
                let (el, er) = proc.process_stereo(l, r);
                l = el;
                r = er;
            }
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
        self.active_note_instances.clear();
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
    pub tuning: Option<TuningSystem>,
    pub scene_ids: Vec<SceneId>,
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
        let master_state = TrackDspState::new(&project.main_track, sample_rate);

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
            bpm: project.transport.bpm(),
            scratch_l,
            scratch_r,
            tuning: project.tuning.clone(),
            scene_ids: project.scenes.iter().map(|scene| scene.id).collect(),
        }
    }

    pub fn trigger_clip(&mut self, track: TrackId, slot_idx: usize) {
        if let Some(state) = self.track_state_mut(track) {
            state.trigger_clip(slot_idx);
        }
    }

    pub fn stop_clip(&mut self, track: TrackId) {
        if let Some(state) = self.track_state_mut(track) {
            state.stop_clip();
        }
    }

    pub fn trigger_scene(&mut self, scene: SceneId) {
        let Some(row) = self.scene_ids.iter().position(|id| *id == scene) else {
            return;
        };
        for state in self.track_states.iter_mut() {
            if row < state.clip_slots.len() && state.clip_slots[row].clip.is_some() {
                state.trigger_clip(row);
            }
        }
    }

    fn track_state_mut(&mut self, track: TrackId) -> Option<&mut TrackDspState> {
        self.track_states
            .iter_mut()
            .find(|state| state.track_id == track)
    }

    pub fn apply_param_msg(&mut self, msg: AudioParamMsg) {
        match msg {
            AudioParamMsg::SetTempo(bpm) => {
                self.bpm = bpm;
            }
            AudioParamMsg::SetTrackVolume { track, volume_db } => {
                if let Some(state) = self.track_state_mut(track) {
                    state.volume_db = volume_db;
                }
            }
            AudioParamMsg::SetTrackPan { track, pan } => {
                if let Some(state) = self.track_state_mut(track) {
                    state.pan = pan;
                }
            }
            AudioParamMsg::SetTrackMute { track, muted } => {
                if let Some(state) = self.track_state_mut(track) {
                    state.is_muted = muted;
                }
            }
            AudioParamMsg::SetTrackSolo { track, soloed } => {
                if let Some(state) = self.track_state_mut(track) {
                    state.is_soloed = soloed;
                }
            }
            AudioParamMsg::SetTrackSend {
                track,
                send_idx,
                amount,
            } => {
                if let Some(state) = self.track_state_mut(track) {
                    while state.sends.len() <= send_idx {
                        state.sends.push(0.0);
                    }
                    state.sends[send_idx] = amount;
                }
            }
            AudioParamMsg::SetMasterVolume { volume_db } => {
                self.master_state.volume_db = volume_db;
            }
            AudioParamMsg::SetMasterPan { pan } => {
                self.master_state.pan = pan;
            }
            AudioParamMsg::SetDeviceParam {
                track,
                device,
                param_key,
                value,
            } => {
                let sample_rate = self.sample_rate;
                if track == self.master_state.track_id {
                    self.master_state
                        .set_device_param(device, &param_key, value, sample_rate);
                } else if let Some(state) = self.track_state_mut(track) {
                    state.set_device_param(device, &param_key, value, sample_rate);
                }
            }
            AudioParamMsg::TriggerClip { track, slot_idx } => {
                self.trigger_clip(track, slot_idx);
            }
            AudioParamMsg::StopClip { track } => {
                self.stop_clip(track);
            }
            AudioParamMsg::TriggerScene { scene } => {
                self.trigger_scene(scene);
            }
            AudioParamMsg::StopAll => {
                self.stop_all();
            }
            AudioParamMsg::SetPlayhead(beats) => {
                self.playhead_beats = beats;
            }
            AudioParamMsg::SetPlaying(playing) => {
                self.is_playing = playing;
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
        self.bpm = project.transport.bpm();
        self.tuning = project.tuning.clone();
        self.scene_ids = project.scenes.iter().map(|scene| scene.id).collect();

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
            .sync_devices(&project.main_track, self.sample_rate);
    }

    pub fn process_buffer_interleaved(&mut self, data: &mut [f32], channels: usize) {
        let frames = data.len() / channels;
        let mut scratch_l = std::mem::take(&mut self.scratch_l);
        let mut scratch_r = std::mem::take(&mut self.scratch_r);
        if frames > scratch_l.len() {
            scratch_l.resize(frames, 0.0);
            scratch_r.resize(frames, 0.0);
        }
        self.process_buffer(&mut scratch_l[..frames], &mut scratch_r[..frames]);

        for (frame_idx, chunk) in data.chunks_mut(channels).enumerate() {
            let l = sanitize_output_sample(scratch_l[frame_idx]);
            let r = sanitize_output_sample(scratch_r[frame_idx]);
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

    pub fn process_buffer_interleaved_i16(&mut self, data: &mut [i16], channels: usize) {
        let frames = data.len() / channels;
        let mut scratch_l = std::mem::take(&mut self.scratch_l);
        let mut scratch_r = std::mem::take(&mut self.scratch_r);
        if frames > scratch_l.len() {
            scratch_l.resize(frames, 0.0);
            scratch_r.resize(frames, 0.0);
        }
        self.process_buffer(&mut scratch_l[..frames], &mut scratch_r[..frames]);

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

    pub fn process_buffer_interleaved_u16(&mut self, data: &mut [u16], channels: usize) {
        let frames = data.len() / channels;
        let mut scratch_l = std::mem::take(&mut self.scratch_l);
        let mut scratch_r = std::mem::take(&mut self.scratch_r);
        if frames > scratch_l.len() {
            scratch_l.resize(frames, 0.0);
            scratch_r.resize(frames, 0.0);
        }
        self.process_buffer(&mut scratch_l[..frames], &mut scratch_r[..frames]);

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

    pub fn process_buffer(&mut self, buffer_l: &mut [f32], buffer_r: &mut [f32]) {
        let num_samples = buffer_l.len();
        let dt_beats = if self.is_playing {
            beats_per_sample(self.bpm as f64, self.sample_rate as f64)
        } else {
            0.0
        };

        let sub_block_size = 32;
        let mut s_idx = 0;
        while s_idx < num_samples {
            let block_end = (s_idx + sub_block_size).min(num_samples);
            let block_len = block_end - s_idx;

            if self.is_playing {
                self.dispatch_clip_events(block_len, dt_beats);
                self.playhead_beats += dt_beats * block_len as f64;
            } else {
                for state in self.track_states.iter_mut() {
                    state.audio_block_l[0..block_len].fill(0.0);
                    state.audio_block_r[0..block_len].fill(0.0);
                    state.current_audio_in = (0.0, 0.0);
                }
            }

            let any_solo = self.track_states.iter().any(|t| t.is_soloed);
            let num_returns = self.return_states.len();

            for (sub_i, i) in (s_idx..block_end).enumerate() {
                let mut master_l = 0.0f32;
                let mut master_r = 0.0f32;
                let mut send_sums_l = [0.0f32; 16];
                let mut send_sums_r = [0.0f32; 16];

                for state in self.track_states.iter_mut() {
                    let in_l = state.audio_block_l[sub_i];
                    let in_r = state.audio_block_r[sub_i];
                    let (raw_l, raw_r) = state.process_sample(in_l, in_r);

                    // Skip muted tracks or un-soloed tracks if someone is soloed
                    if state.is_muted || (any_solo && !state.is_soloed) {
                        continue;
                    }

                    // Volume (dB to linear)
                    let vol_lin = if state.volume_db <= -59.0 {
                        0.0
                    } else {
                        10.0f32.powf(state.volume_db / 20.0)
                    };

                    // Pan (constant power)
                    let pan_norm = (state.pan.clamp(-1.0, 1.0) + 1.0) * 0.5;
                    let angle = pan_norm * std::f32::consts::FRAC_PI_2;
                    let pan_l = angle.cos();
                    let pan_r = angle.sin();

                    let track_out_l = raw_l * vol_lin * pan_l;
                    let track_out_r = raw_r * vol_lin * pan_r;

                    // Accumulate to Master
                    master_l += track_out_l;
                    master_r += track_out_r;

                    // Accumulate to dynamic Return tracks
                    for (ret_idx, &send_level) in state.sends.iter().enumerate() {
                        if ret_idx < 16 && ret_idx < num_returns {
                            send_sums_l[ret_idx] += track_out_l * send_level;
                            send_sums_r[ret_idx] += track_out_r * send_level;
                        }
                    }
                }

                // Process all Return tracks dynamically and accumulate into Master
                for (ret_idx, ret_state) in self.return_states.iter_mut().enumerate() {
                    let s_l = if ret_idx < 16 {
                        send_sums_l[ret_idx]
                    } else {
                        0.0
                    };
                    let s_r = if ret_idx < 16 {
                        send_sums_r[ret_idx]
                    } else {
                        0.0
                    };
                    let (ret_l, ret_r) = ret_state.process_sample(s_l, s_r);

                    let ret_vol = if ret_state.volume_db <= -59.0 {
                        0.0
                    } else {
                        10.0f32.powf(ret_state.volume_db / 20.0)
                    };
                    master_l += ret_l * ret_vol;
                    master_r += ret_r * ret_vol;
                }

                // Master channel processing
                let (final_l, final_r) = self.master_state.process_sample(master_l, master_r);
                let master_vol = if self.master_state.volume_db <= -59.0 {
                    0.0
                } else {
                    10.0f32.powf(self.master_state.volume_db / 20.0)
                };
                let out_l = final_l * master_vol;
                let out_r = final_r * master_vol;
                self.master_meter.update(out_l, out_r);

                buffer_l[i] = out_l;
                buffer_r[i] = out_r;
            }
            s_idx = block_end;
        }
    }

    fn dispatch_clip_events(&mut self, block_len: usize, dt_beats: f64) {
        let bpm = self.bpm;
        for state in self.track_states.iter_mut() {
            let should_play_clips = match state.monitoring {
                MonitoringMode::In => false,
                MonitoringMode::Auto => !state.is_armed,
                MonitoringMode::Off => true,
            };

            if !should_play_clips {
                state.stop_all_notes();
                state.audio_block_l[0..block_len].fill(0.0);
                state.audio_block_r[0..block_len].fill(0.0);
                state.current_audio_in = (0.0, 0.0);
                continue;
            }

            if let Some(slot_idx) = state.active_clip_slot {
                let maybe_clip = state.clip_slots.get(slot_idx).and_then(|s| s.clip.clone());
                if let Some(clip) = maybe_clip {
                    let clip_len = clip.length_beats.max(1.0);
                    let clip_pos = (self.playhead_beats % clip_len).abs();
                    Self::process_clip_payload_with_tuning(
                        state,
                        &clip,
                        clip_pos,
                        bpm,
                        block_len,
                        dt_beats,
                        self.tuning.as_ref(),
                    );
                }
            } else if !state.arrangement_clips.is_empty() {
                let mut matched_clip = None;
                for arr_clip in &state.arrangement_clips {
                    let c_start = arr_clip.start_beat;
                    let c_end = arr_clip.start_beat + arr_clip.length_beats;
                    if self.playhead_beats >= c_start && self.playhead_beats < c_end {
                        let rel_beat = self.playhead_beats - c_start;
                        let clip_len = arr_clip.clip.length_beats.max(1.0);
                        let clip_pos = if arr_clip.clip.is_looping {
                            rel_beat % clip_len
                        } else {
                            rel_beat
                        };
                        matched_clip = Some((arr_clip.clip.clone(), clip_pos));
                        break;
                    }
                }
                if let Some((clip, clip_pos)) = matched_clip {
                    Self::process_clip_payload_with_tuning(
                        state,
                        &clip,
                        clip_pos,
                        bpm,
                        block_len,
                        dt_beats,
                        self.tuning.as_ref(),
                    );
                } else {
                    state.stop_all_notes();
                    state.audio_block_l[0..block_len].fill(0.0);
                    state.audio_block_r[0..block_len].fill(0.0);
                    state.current_audio_in = (0.0, 0.0);
                }
            } else {
                let mut current_beat = 0.0;
                let mut matched_clip = None;
                let mut has_clips = false;
                for slot in &state.clip_slots {
                    if let Some(ref clip) = slot.clip {
                        has_clips = true;
                        let c_start = current_beat;
                        let c_end = current_beat + clip.length_beats;
                        if self.playhead_beats >= c_start && self.playhead_beats < c_end {
                            let clip_pos = self.playhead_beats - c_start;
                            matched_clip = Some((clip.clone(), clip_pos));
                            break;
                        }
                        current_beat += clip.length_beats;
                    }
                }
                if let Some((clip, clip_pos)) = matched_clip {
                    Self::process_clip_payload_with_tuning(
                        state,
                        &clip,
                        clip_pos,
                        bpm,
                        block_len,
                        dt_beats,
                        self.tuning.as_ref(),
                    );
                } else {
                    if has_clips {
                        state.stop_all_notes();
                    }
                    state.audio_block_l[0..block_len].fill(0.0);
                    state.audio_block_r[0..block_len].fill(0.0);
                    state.current_audio_in = (0.0, 0.0);
                }
            }
        }
    }

    pub fn process_clip_payload(
        state: &mut TrackDspState,
        clip: &Clip,
        clip_pos: f64,
        bpm: f32,
        block_len: usize,
        dt_beats: f64,
    ) {
        Self::process_clip_payload_with_tuning(
            state, clip, clip_pos, bpm, block_len, dt_beats, None,
        );
    }

    pub fn process_clip_payload_with_tuning(
        state: &mut TrackDspState,
        clip: &Clip,
        clip_pos: f64,
        bpm: f32,
        block_len: usize,
        dt_beats: f64,
        tuning: Option<&TuningSystem>,
    ) {
        match clip.payload {
            ClipPayload::Midi(ref midi_data) => {
                state.audio_block_l[0..block_len].fill(0.0);
                state.audio_block_r[0..block_len].fill(0.0);
                state.current_audio_in = (0.0, 0.0);
                for (idx, note) in midi_data.notes.iter().enumerate() {
                    if !note.mute {
                        let note_start = note.start_beat;
                        let note_end = note.start_beat + note.duration_beats;
                        let is_active_now = clip_pos >= note_start && clip_pos < note_end;
                        let existing_pos = state
                            .active_note_instances
                            .iter()
                            .position(|inst| inst.note_index == idx && inst.pitch == note.pitch);

                        if is_active_now {
                            if existing_pos.is_none() {
                                state.active_note_instances.push(ActiveNoteInstance {
                                    pitch: note.pitch,
                                    note_index: idx,
                                    start_beat: note.start_beat,
                                    duration_beats: note.duration_beats,
                                });
                                let freq = if let Some(t) = tuning {
                                    t.note_to_freq(note.pitch) as f32
                                } else {
                                    440.0 * 2.0f32.powf((note.pitch as f32 - 69.0) / 12.0)
                                };
                                if let Some(ref mut s) = state.synth {
                                    s.note_on_with_freq(
                                        note.pitch,
                                        midi_velocity(note.velocity),
                                        freq,
                                    );
                                }
                                if let Some(ref mut sm) = state.simpler {
                                    sm.note_on_with_freq(
                                        note.pitch,
                                        midi_velocity(note.velocity),
                                        freq,
                                    );
                                }
                                if let Some(ref mut dr) = state.drum_rack {
                                    dr.note_on(note.pitch, midi_velocity(note.velocity));
                                }
                            }
                        } else if let Some(pos) = existing_pos {
                            state.active_note_instances.remove(pos);
                            let pitch_still_active = state
                                .active_note_instances
                                .iter()
                                .any(|inst| inst.pitch == note.pitch);
                            if !pitch_still_active {
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
            }
            ClipPayload::Audio(_) => {
                state.audio_sample_cache.render_clip_samples(
                    clip,
                    clip_pos,
                    dt_beats,
                    bpm,
                    &mut state.audio_block_l[0..block_len],
                    &mut state.audio_block_r[0..block_len],
                );
                state.current_audio_in = (state.audio_block_l[0], state.audio_block_r[0]);
            }
        }
    }
}
