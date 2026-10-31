//! Helpers that check device processors: block size invariance and parameter effects (M05).

use crate::device_processor::{AudioIo, DeviceProcessor, EventList, EventSink, ProcessContext};
use livecraft_model::param::ParamId;

pub fn context(frames: usize) -> ProcessContext {
    ProcessContext {
        sample_rate: 44100.0,
        frames,
        bpm: 120.0,
        playhead_beats: 0.0,
        beats_per_frame: 0.0,
    }
}

/// A fixed pseudo-random test signal in -1 to 1, so every run sees the same samples.
pub fn signal(frames: usize, seed: u32) -> (Vec<f32>, Vec<f32>) {
    let mut state = seed;
    let mut next = move || {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (state >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
    };
    let left = (0..frames).map(|_| next()).collect();
    let right = (0..frames).map(|_| next()).collect();
    (left, right)
}

/// Runs `processor` over `input` in blocks of `block` frames and returns the output.
pub fn run(
    processor: &mut dyn DeviceProcessor,
    input: &(Vec<f32>, Vec<f32>),
    block: usize,
) -> (Vec<f32>, Vec<f32>) {
    let (mut left, mut right) = input.clone();
    let events = EventList::default();
    let mut sink = EventSink::default();
    for start in (0..left.len()).step_by(block) {
        let end = (start + block).min(left.len());
        let mut io = AudioIo {
            left: &mut left[start..end],
            right: &mut right[start..end],
        };
        processor.process(&context(end - start), &mut io, &events, &mut sink);
    }
    (left, right)
}

/// The bit patterns of `samples`, so comparisons are exact.
pub fn bits(samples: &[f32]) -> Vec<u32> {
    samples.iter().map(|s| s.to_bits()).collect()
}

/// Sets `params` on two new processors and checks that one 512-frame block per call and seven
/// frames per call give the same bits. Each `(id, value)` pair is a `ParamId` position.
pub fn assert_block_size_does_not_change_output<P: DeviceProcessor>(
    mut make: impl FnMut() -> P,
    params: &[(u16, f32)],
) {
    let input = signal(8192, 17);
    let mut whole = make();
    let mut blocks = make();
    for &(id, value) in params {
        whole.set_param(ParamId(id), value, 0);
        blocks.set_param(ParamId(id), value, 0);
    }
    let (whole_l, whole_r) = run(&mut whole, &input, 512);
    let (block_l, block_r) = run(&mut blocks, &input, 7);
    assert_eq!(bits(&whole_l), bits(&block_l));
    assert_eq!(bits(&whole_r), bits(&block_r));
}

/// Checks that setting parameter `id` to `value` changes the output of a new processor.
pub fn assert_parameter_changes_output<P: DeviceProcessor>(
    mut make: impl FnMut() -> P,
    id: u16,
    value: f32,
) {
    let input = signal(8192, 23);
    let (default_l, _) = run(&mut make(), &input, 512);
    let mut changed = make();
    changed.set_param(ParamId(id), value, 0);
    let (changed_l, _) = run(&mut changed, &input, 512);
    assert_ne!(bits(&default_l), bits(&changed_l));
}
