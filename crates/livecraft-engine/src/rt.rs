//! Real-time checks for the audio thread (M05 item 1).
//!
//! `checked` runs a closure with allocation forbidden on this thread: an allocation or free inside
//! it aborts the process. The check needs the binary or test to set `AllocDisabler` as its global
//! allocator. The crate's default `disable_release` feature turns the check off in release builds,
//! and the `rt-checks` feature (on by default) turns the whole wrapper off.

#[cfg(feature = "rt-checks")]
/// Runs `func` with allocation forbidden on this thread.
pub fn checked<T, F: FnOnce() -> T>(func: F) -> T {
    assert_no_alloc::assert_no_alloc(func)
}

#[cfg(not(feature = "rt-checks"))]
/// Runs `func`. Allocation checks are compiled out without the `rt-checks` feature.
pub fn checked<T, F: FnOnce() -> T>(func: F) -> T {
    func()
}
