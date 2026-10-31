//! The collector thread frees what the audio thread has released (M05 items 6 and 7). Dropping a
//! graph, or the last `Shared` value of a clip, runs destructors and frees memory, so it happens here
//! and never on the audio thread.

use crate::compiled::CompiledGraph;
use std::time::Duration;

/// How long the collector sleeps when nothing is waiting. It is not the audio thread.
const IDLE: Duration = Duration::from_millis(10);
/// How many idle rounds the collector waits, after the engine is gone, for the session to release
/// its handles before it gives up and leaks what is left.
const SHUTDOWN_ROUNDS: u32 = 100;

/// Starts the collector thread. It drops retired graphs and collects shared data until the engine is
/// dropped, then waits for the session to release its handles and frees what is left.
pub fn spawn(
    mut retired: rtrb::Consumer<Box<CompiledGraph>>,
    mut shared: basedrop::Collector,
) -> std::io::Result<std::thread::JoinHandle<()>> {
    std::thread::Builder::new()
        .name("livecraft-collector".to_string())
        .spawn(move || {
            loop {
                while let Ok(graph) = retired.pop() {
                    drop(graph);
                }
                shared.collect();
                if retired.is_abandoned() {
                    break;
                }
                std::thread::sleep(IDLE);
            }
            for _ in 0..SHUTDOWN_ROUNDS {
                match shared.try_cleanup() {
                    Ok(()) => return,
                    Err(waiting) => {
                        shared = waiting;
                        shared.collect();
                        std::thread::sleep(IDLE);
                    }
                }
            }
            log::warn!("Shared audio data is still held by the session, so it is leaked at exit");
        })
}
