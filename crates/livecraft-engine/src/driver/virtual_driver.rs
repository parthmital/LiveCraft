//! The offline driver (M06 item 2): it renders the engine into the caller's buffer, with no thread
//! and no clock.
//!
//! The caller sets the pace, so the output does not depend on timing. M08 renders Sets through it.

use super::{AudioDriver, DriverError, DriverStatus, Latency};
use crate::executor::Engine;

/// Renders the engine on demand, in steps of `buffer_frames` frames.
pub struct VirtualDriver {
    sample_rate: u32,
    buffer_frames: u32,
    channels: usize,
    engine: Option<Engine>,
}

impl VirtualDriver {
    /// A driver that renders at `sample_rate`, in steps of `buffer_frames` frames of `channels`
    /// channels.
    pub fn new(sample_rate: u32, buffer_frames: u32, channels: usize) -> Self {
        Self {
            sample_rate: sample_rate.max(1),
            buffer_frames: buffer_frames.max(1),
            channels: channels.max(1),
            engine: None,
        }
    }

    /// Fills `out` with the next interleaved samples. The engine is driven in steps of
    /// `buffer_frames` frames, the steps a device would use. Before `start`, `out` is silent.
    pub fn render(&mut self, out: &mut [f32]) {
        let Some(engine) = self.engine.as_mut() else {
            out.fill(0.0);
            return;
        };
        let step = self.buffer_frames as usize * self.channels;
        for chunk in out.chunks_mut(step) {
            engine.process_offline(chunk, self.channels);
        }
    }

    /// Advances the engine by `frames` frames in `buffer_frames` steps, discarding the audio.
    pub fn skip_frames(&mut self, frames: usize) {
        let Some(engine) = self.engine.as_mut() else {
            return;
        };
        if frames == 0 {
            return;
        }
        let step_frames = self.buffer_frames as usize;
        let mut scratch = vec![0.0f32; frames.min(step_frames) * self.channels];
        let mut remaining = frames;
        while remaining > 0 {
            let chunk_frames = remaining.min(step_frames);
            let slice = &mut scratch[..chunk_frames * self.channels];
            engine.process_offline(slice, self.channels);
            remaining -= chunk_frames;
        }
    }

    /// Returns a mutable reference to the attached engine, if started.
    pub fn engine_mut(&mut self) -> Option<&mut Engine> {
        self.engine.as_mut()
    }

    /// Consumes the driver and returns its engine, if started.
    pub fn into_engine(self) -> Option<Engine> {
        self.engine
    }
}

impl AudioDriver for VirtualDriver {
    fn name(&self) -> &str {
        "Virtual"
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn buffer_frames(&self) -> u32 {
        self.buffer_frames
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn latency(&self) -> Latency {
        Latency::default()
    }

    fn start(&mut self, engine: Engine) -> Result<(), DriverError> {
        if self.engine.is_some() {
            return Err(DriverError::AlreadyStarted);
        }
        self.engine = Some(engine);
        Ok(())
    }

    fn status(&self) -> DriverStatus {
        if self.engine.is_some() {
            DriverStatus::Running
        } else {
            DriverStatus::Stopped
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::EngineCommand;
    use crate::output::Playback;
    use livecraft_model::ids::TrackId;
    use livecraft_model::model::{Clip, ClipPayload, MidiNote, Project};

    /// A playback of a Set whose first track plays a chord clip, and its engine.
    fn started_set() -> (Playback, Engine) {
        let mut project = Project::new_default("Virtual");
        let mut clip = Clip::new_midi(900, "Chord", 4.0);
        if let ClipPayload::Midi(ref mut data) = clip.payload {
            data.notes.push(MidiNote::new(60, 0.0, 2.0, 100));
            data.notes.push(MidiNote::new(67, 1.0, 2.0, 100));
        }
        project.tracks[0].clip_slots[0].clip = Some(clip);
        let (mut playback, engine) = Playback::start(&project, 44100.0).expect("start the engine");
        playback
            .handle
            .send(EngineCommand::TriggerClip {
                track: TrackId(1),
                slot: 0,
            })
            .expect("launch the clip");
        playback
            .handle
            .send(EngineCommand::SetPlaying(true))
            .expect("start the transport");
        (playback, engine)
    }

    #[test]
    fn render_gives_the_samples_of_an_engine_driven_in_the_same_steps() {
        let (_first, engine) = started_set();
        let (_second, mut twin) = started_set();
        let mut driver = VirtualDriver::new(44100, 64, 2);
        driver.start(engine).expect("start the driver");

        // 1000 frames is not a whole number of 64-frame steps, so the last step is shorter.
        let mut rendered = vec![0.0f32; 1000 * 2];
        driver.render(&mut rendered);
        let mut expected = vec![0.0f32; 1000 * 2];
        for chunk in expected.chunks_mut(64 * 2) {
            twin.process(chunk, 2);
        }

        let bits = |samples: &[f32]| samples.iter().map(|s| s.to_bits()).collect::<Vec<_>>();
        assert!(
            rendered.iter().any(|&sample| sample != 0.0),
            "the clip should sound"
        );
        assert_eq!(bits(&rendered), bits(&expected));
    }

    #[test]
    fn render_before_start_writes_silence() {
        let mut driver = VirtualDriver::new(44100, 64, 2);
        let mut out = vec![1.0f32; 128];
        driver.render(&mut out);
        assert!(out.iter().all(|&sample| sample == 0.0));
    }

    #[test]
    fn a_virtual_driver_reports_running_once_it_has_an_engine() {
        let (_playback, engine) = started_set();
        let mut driver = VirtualDriver::new(44100, 64, 2);
        assert_eq!(driver.status(), DriverStatus::Stopped);
        driver.start(engine).expect("start the driver");
        assert_eq!(driver.status(), DriverStatus::Running);
        let (_other, second) = started_set();
        assert!(matches!(
            driver.start(second),
            Err(DriverError::AlreadyStarted)
        ));
    }

    #[test]
    fn skip_frames_and_engine_accessors_work() {
        let (_first, engine) = started_set();
        let (_second, twin) = started_set();
        let mut skipped = VirtualDriver::new(44100, 64, 2);
        assert!(skipped.engine_mut().is_none());
        skipped.start(engine).expect("start");
        assert!(skipped.engine_mut().is_some());
        skipped.skip_frames(200);
        let mut after_skip = vec![0.0f32; 300 * 2];
        skipped.render(&mut after_skip);

        let mut full = VirtualDriver::new(44100, 64, 2);
        full.start(twin).expect("start");
        let mut prefix = vec![0.0f32; 200 * 2];
        full.render(&mut prefix);
        let mut after_full = vec![0.0f32; 300 * 2];
        full.render(&mut after_full);

        let bits = |samples: &[f32]| samples.iter().map(|s| s.to_bits()).collect::<Vec<_>>();
        assert_eq!(bits(&after_skip), bits(&after_full));
        assert!(skipped.into_engine().is_some());
    }
}
