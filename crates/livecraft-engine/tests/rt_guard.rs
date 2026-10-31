//! The real-time guard passes allocation-free work and aborts when a checked scope allocates.

use livecraft_engine::rt::checked;

/// The allocator that makes `checked` detect allocations. The crate exports it only when its
/// checks are compiled in, which is every build except release.
#[cfg(debug_assertions)]
#[global_allocator]
static ALLOCATOR: assert_no_alloc::AllocDisabler = assert_no_alloc::AllocDisabler;

/// Set in the child process that runs the allocating scope, which must abort.
#[cfg(debug_assertions)]
const CHILD_ENV: &str = "LIVECRAFT_RT_GUARD_CHILD";

#[test]
fn a_checked_scope_without_allocation_passes() {
    let mut samples = [0.0f32; 64];
    checked(|| samples.iter_mut().for_each(|s| *s = 0.5));
    assert!(samples.iter().all(|&s| s == 0.5));
}

#[cfg(debug_assertions)]
#[test]
fn an_allocation_inside_a_checked_scope_aborts() {
    if std::env::var_os(CHILD_ENV).is_some() {
        checked(|| std::hint::black_box(vec![1u8; 64]).len());
        return;
    }
    let status = std::process::Command::new(std::env::current_exe().expect("test binary path"))
        .args([
            "--exact",
            "an_allocation_inside_a_checked_scope_aborts",
            "--nocapture",
        ])
        .env(CHILD_ENV, "1")
        .status()
        .expect("run the child test");
    assert!(!status.success(), "an allocating checked scope must abort");
}
