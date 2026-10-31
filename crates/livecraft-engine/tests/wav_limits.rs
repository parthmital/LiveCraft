use livecraft_engine::audio_clip::load_file_with_limit;
use std::path::PathBuf;

fn write_test_wav(name: &str, frames: u32) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 44_100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(&path, spec).unwrap();
    for i in 0..frames {
        let value = (i % 100) as i16 * 100;
        writer.write_sample(value).unwrap();
        writer.write_sample(-value).unwrap();
    }
    writer.finalize().unwrap();
    path
}

#[test]
fn wav_within_limit_loads() {
    let path = write_test_wav("wav_within_limit.wav", 1000);
    let audio = load_file_with_limit(&path, 2000).unwrap();
    assert_eq!(audio.channels, 2);
    assert_eq!(audio.sample_rate, 44_100);
    assert_eq!(audio.samples_l.len(), 1000);
    assert_eq!(audio.samples_r.len(), 1000);
}

#[test]
fn wav_over_limit_is_rejected() {
    let path = write_test_wav("wav_over_limit.wav", 1000);
    let err = load_file_with_limit(&path, 1999).unwrap_err();
    assert!(err.to_string().contains("over the limit"), "{err}");
}
