//! A stereo delay line of a fixed length, used for latency compensation (M05 item 10, manual 23.3).
//!
//! The compiler sizes every delay line from the devices' latencies and allocates it there. Processing
//! a frame only reads and writes the ring buffer, so it never allocates.

/// A stereo delay of `len` frames. A zero-length line passes its input through unchanged.
#[derive(Debug, Clone, Default)]
pub struct DelayLine {
    left: Box<[f32]>,
    right: Box<[f32]>,
    write: usize,
}

impl DelayLine {
    /// A delay line of `frames` frames, silent at the start.
    pub fn new(frames: usize) -> Self {
        Self {
            left: vec![0.0; frames].into_boxed_slice(),
            right: vec![0.0; frames].into_boxed_slice(),
            write: 0,
        }
    }

    /// The delay in frames.
    pub fn len(&self) -> usize {
        self.left.len()
    }

    pub fn is_empty(&self) -> bool {
        self.left.is_empty()
    }

    /// Delays one frame: returns the frame that went in `len` calls ago, and stores this one.
    pub fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        if self.left.is_empty() {
            return (left, right);
        }
        let out = (self.left[self.write], self.right[self.write]);
        self.left[self.write] = left;
        self.right[self.write] = right;
        self.write = (self.write + 1) % self.left.len();
        out
    }

    /// Delays a block in place.
    pub fn apply(&mut self, left: &mut [f32], right: &mut [f32]) {
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            (*l, *r) = self.process(*l, *r);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_comes_out_after_the_delay_length() {
        let mut line = DelayLine::new(3);
        let outputs: Vec<f32> = (1..=6)
            .map(|frame| line.process(frame as f32, -(frame as f32)).0)
            .collect();
        assert_eq!(outputs, vec![0.0, 0.0, 0.0, 1.0, 2.0, 3.0]);
    }

    #[test]
    fn a_zero_length_line_passes_the_input_through() {
        let mut line = DelayLine::default();
        assert!(line.is_empty());
        assert_eq!(line.process(0.25, -0.5), (0.25, -0.5));
    }

    #[test]
    fn a_block_is_delayed_in_place_across_calls() {
        let mut line = DelayLine::new(2);
        let mut left = [1.0, 2.0, 3.0];
        let mut right = [-1.0, -2.0, -3.0];
        line.apply(&mut left, &mut right);
        assert_eq!(left, [0.0, 0.0, 1.0]);
        assert_eq!(right, [0.0, 0.0, -1.0]);
        let mut next_left = [4.0];
        let mut next_right = [-4.0];
        line.apply(&mut next_left, &mut next_right);
        assert_eq!(next_left, [2.0]);
        assert_eq!(next_right, [-2.0]);
    }
}
