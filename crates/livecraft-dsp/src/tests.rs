use crate::biquad::*;

#[test]
fn test_biquad_all_filter_types() {
    let types = [
        FilterType::LowPass,
        FilterType::HighPass,
        FilterType::BandPass,
        FilterType::Notch,
        FilterType::Peaking,
        FilterType::LowShelf,
        FilterType::HighShelf,
    ];

    for &ft in &types {
        let mut filter = BiquadFilter::new(ft, 44100.0, 1000.0, 3.0, 0.707);
        for _ in 0..50 {
            let (l, r) = filter.process_stereo(0.5, 0.5);
            assert!(l.is_finite());
            assert!(r.is_finite());
        }
    }
}
