//! Tempo-synced delay times equal the original f32 arithmetic at every sampled tempo, so the
//! move to `livecraft_time::seconds_per_beat` changes no output bits.

use livecraft_devices::delay::DelayEffect;

#[test]
fn tempo_sync_matches_the_original_f32_arithmetic() {
    let mut delay = DelayEffect::new(44_100.0);
    for step in 0..200_000u32 {
        let bpm = step as f32 * 0.005;
        delay.set_tempo_synced(bpm, 0.5);
        let beat_sec = 60.0 / bpm.max(20.0);
        let expected = (beat_sec * 0.5 * 1000.0).clamp(1.0, 1900.0);
        assert_eq!(delay.delay_time_ms, expected, "bpm {bpm}");
    }
}
