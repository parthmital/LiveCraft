//! The collector thread frees the graphs the audio thread has replaced (M05 item 6). Dropping a
//! graph runs destructors and frees memory, so it happens here and never on the audio thread.

use crate::compiled::CompiledGraph;
use std::time::Duration;

/// How long the collector sleeps when no graph is waiting. It is not the audio thread.
const IDLE: Duration = Duration::from_millis(10);

/// Starts the collector thread. It exits once the engine has been dropped and the queue is empty.
pub fn spawn(
    mut retired: rtrb::Consumer<Box<CompiledGraph>>,
) -> std::io::Result<std::thread::JoinHandle<()>> {
    std::thread::Builder::new()
        .name("livecraft-collector".to_string())
        .spawn(move || {
            loop {
                while let Ok(graph) = retired.pop() {
                    drop(graph);
                }
                if retired.is_abandoned() {
                    break;
                }
                std::thread::sleep(IDLE);
            }
        })
}
