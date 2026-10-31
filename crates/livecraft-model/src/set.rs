use crate::device::{Device, DeviceType};
use crate::ids::{IdAllocator, SceneId, TrackId};
use crate::scale::ScaleConfig;
use crate::scene::Scene;
use crate::track::{Track, TrackType};
use crate::tuning::TuningSystem;
use livecraft_time::{Beats, TempoMap, TimeError, TimeSignatureMap};
use serde::{Deserialize, Serialize};

/// Transport state. Tempo and time signature are `livecraft-time` maps over song time.
///
/// The JSON export keeps the tempo and signature at the song start under the keys `bpm` and
/// `time_signature`. Tempo ramps and later signature changes are not exported yet; `.als`
/// keeps them (M09).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transport {
    #[serde(rename = "bpm", with = "tempo_json")]
    pub tempo: TempoMap,
    #[serde(with = "meter_json")]
    pub time_signature: TimeSignatureMap,
    pub is_playing: bool,
    pub is_recording: bool,
    pub playhead_beats: f64,
    pub loop_enabled: bool,
    pub loop_start_beats: f64,
    pub loop_length_beats: f64,
    pub metronome_enabled: bool,
}

impl Transport {
    /// The tempo at the song start, in BPM.
    pub fn bpm(&self) -> f32 {
        self.tempo.bpm_at(Beats(0.0)) as f32
    }

    /// Sets one tempo for the whole song.
    pub fn set_bpm(&mut self, bpm: f32) {
        self.tempo = TempoMap::constant(f64::from(bpm));
    }

    /// The time signature at the song start, as (numerator, denominator).
    pub fn time_signature(&self) -> (u8, u8) {
        self.time_signature.signature_at(Beats(0.0))
    }

    /// Sets one time signature for the whole song.
    pub fn set_time_signature(&mut self, numerator: u8, denominator: u8) -> Result<(), TimeError> {
        self.time_signature = TimeSignatureMap::constant(numerator, denominator)?;
        Ok(())
    }
}

mod tempo_json {
    use livecraft_time::{Beats, TempoMap};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(map: &TempoMap, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_f64(map.bpm_at(Beats(0.0)))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<TempoMap, D::Error> {
        Ok(TempoMap::constant(f64::deserialize(deserializer)?))
    }
}

mod meter_json {
    use livecraft_time::{Beats, TimeSignatureMap};
    use serde::de::Error;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(
        map: &TimeSignatureMap,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        map.signature_at(Beats(0.0)).serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<TimeSignatureMap, D::Error> {
        let (numerator, denominator) = <(u8, u8)>::deserialize(deserializer)?;
        TimeSignatureMap::constant(numerator, denominator).map_err(D::Error::custom)
    }
}

impl Default for Transport {
    fn default() -> Self {
        Self {
            tempo: TempoMap::constant(120.0),
            time_signature: TimeSignatureMap::default(),
            is_playing: false,
            is_recording: false,
            playhead_beats: 0.0,
            loop_enabled: false,
            loop_start_beats: 0.0,
            loop_length_beats: 16.0,
            metronome_enabled: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub schema_version: String,
    pub creator: String,
    pub name: String,
    pub transport: Transport,
    #[serde(default)]
    pub scale: ScaleConfig,
    #[serde(default)]
    pub tuning: Option<TuningSystem>,
    pub tracks: Vec<Track>,
    pub return_tracks: Vec<Track>,
    #[serde(rename = "master_track")]
    pub main_track: Track,
    pub scenes: Vec<Scene>,
}

impl Project {
    pub fn new_default(name: impl Into<String>) -> Self {
        let num_slots = 8;
        let mut tracks = vec![
            Track::new(TrackId(1), "1-MIDI", TrackType::Midi, num_slots),
            Track::new(TrackId(2), "2-MIDI", TrackType::Midi, num_slots),
            Track::new(TrackId(3), "3-Audio", TrackType::Audio, num_slots),
            Track::new(TrackId(4), "4-Audio", TrackType::Audio, num_slots),
        ];

        // Give default instrument to Track 1
        tracks[0]
            .device_chain
            .push(Device::new(101.into(), "Operator", DeviceType::Operator));
        // Give Compressor and EQ Eight to Track 3
        tracks[2]
            .device_chain
            .push(Device::new(102.into(), "EQ Eight", DeviceType::EqEight));

        let return_tracks = vec![
            {
                let mut trk = Track::new(TrackId(5), "A-Reverb", TrackType::Return, num_slots);
                trk.device_chain
                    .push(Device::new(201.into(), "Reverb", DeviceType::Reverb));
                trk
            },
            {
                let mut trk = Track::new(TrackId(6), "B-Delay", TrackType::Return, num_slots);
                trk.device_chain
                    .push(Device::new(202.into(), "Delay", DeviceType::Delay));
                trk
            },
        ];

        let mut main_track = Track::new(TrackId(7), "Master", TrackType::Main, 0);
        main_track
            .device_chain
            .push(Device::new(301.into(), "Limiter", DeviceType::Limiter));

        let scenes = (1..=num_slots as u64)
            .map(|i| Scene::new(SceneId(i), format!("{}", i)))
            .collect();

        Self {
            schema_version: "LiveCraft 1.0 (Ableton Live 12.0 compatible)".to_string(),
            creator: "LiveCraft Engine".to_string(),
            name: name.into(),
            transport: Transport::default(),
            scale: ScaleConfig::default(),
            tuning: None,
            tracks,
            return_tracks,
            main_track,
            scenes,
        }
    }

    /// Every track: the tracks, then the return tracks, then the Main track.
    pub fn all_tracks(&self) -> impl Iterator<Item = &Track> {
        self.tracks
            .iter()
            .chain(&self.return_tracks)
            .chain(std::iter::once(&self.main_track))
    }

    /// Every track, mutably, in the same order as `all_tracks`.
    pub fn all_tracks_mut(&mut self) -> impl Iterator<Item = &mut Track> {
        self.tracks
            .iter_mut()
            .chain(self.return_tracks.iter_mut())
            .chain(std::iter::once(&mut self.main_track))
    }

    /// Finds a track of any kind (audio, MIDI, return, or Main) by id.
    pub fn track(&self, id: TrackId) -> Option<&Track> {
        self.all_tracks().find(|track| track.id == id)
    }

    /// Finds a track of any kind (audio, MIDI, return, or Main) by id, for editing.
    pub fn track_mut(&mut self, id: TrackId) -> Option<&mut Track> {
        self.all_tracks_mut().find(|track| track.id == id)
    }

    /// True when `to` is `from` or one of its group ancestors. A parent chain that never
    /// ends is a loop, and counts as reaching `to`.
    pub(crate) fn reaches(&self, from: TrackId, to: TrackId) -> bool {
        let limit = self.all_tracks().count() + 1;
        let mut current = Some(from);
        for _ in 0..limit {
            match current {
                Some(id) if id == to => return true,
                Some(id) => current = self.track(id).and_then(|track| track.parent),
                None => return false,
            }
        }
        true
    }

    /// Returns the row of a scene in the Session View.
    pub fn scene_row(&self, id: SceneId) -> Option<usize> {
        self.scenes.iter().position(|scene| scene.id == id)
    }

    /// Returns an allocator whose next id is above every id in the Set, so new entities never reuse one.
    pub fn id_allocator(&self) -> IdAllocator {
        let mut highest = 0u64;
        for track in self
            .tracks
            .iter()
            .chain(&self.return_tracks)
            .chain(std::iter::once(&self.main_track))
        {
            highest = highest.max(track.id.0);
            for device in &track.device_chain {
                highest = highest.max(device.id.0);
            }
            for lane in &track.automation_lanes {
                highest = highest.max(lane.id.0);
            }
            for arranged in &track.arrangement_clips {
                highest = highest.max(arranged.id.0);
            }
            let slot_clips = track
                .clip_slots
                .iter()
                .filter_map(|slot| slot.clip.as_ref());
            let arranged = track
                .arrangement_clips
                .iter()
                .map(|arranged| &arranged.clip);
            for clip in slot_clips.chain(arranged) {
                highest = highest.max(clip.id.0);
                for lane in &clip.automation_lanes {
                    highest = highest.max(lane.id.0);
                }
            }
        }
        for scene in &self.scenes {
            highest = highest.max(scene.id.0);
        }
        IdAllocator::starting_at(highest + 1)
    }
}
