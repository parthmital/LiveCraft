//! MIDI ports through `midir` (M06 items 6 and 7, manual 17.3, External MIDI In/Out, page 375).
//!
//! midir calls each input's callback on its own thread. The callback stamps the message with the
//! shared `HostClock` and pushes it into the port's own queue, so no queue has two writers and nothing
//! locks. The engine pops every port's queue on the audio thread (see
//! `Engine::set_midi_input`). `Playback::open_with_driver` opens the ports. Listing and opening never
//! run on the audio thread.

use crate::midi::{HostClock, MidiMessage};
use midir::{MidiInput, MidiInputConnection, MidiOutput};
use serde::Serialize;

/// The client name the MIDI clients report to the system.
const CLIENT_NAME: &str = "LiveCraft";

/// Capacity of the queue from each MIDI input to the engine, in messages. A burst beyond it is dropped,
/// because the audio thread never waits.
pub const MIDI_QUEUE_CAPACITY: usize = 1024;

/// The MIDI input and output ports this machine offers, in the order the system lists them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MidiPorts {
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
}

/// The queue one input port's callback fills. It is the only writer of its ring buffer.
type Queue = rtrb::Producer<MidiMessage>;

/// The engine's end of the MIDI input: one queue of stamped messages per open port, and the clock they
/// are stamped with.
pub struct MidiFeed {
    pub messages: Vec<rtrb::Consumer<MidiMessage>>,
    pub clock: HostClock,
}

/// The open MIDI input ports. They stay open while this value lives, so dropping it closes them.
pub struct MidiInputs {
    _connections: Vec<MidiInputConnection<Queue>>,
}

impl MidiInputs {
    /// Opens the input ports named in `names` and returns them with the feed the engine reads. A name that
    /// is not an input port, or a port that cannot be opened, is skipped with a warning, so a missing
    /// keyboard never stops playback.
    pub fn open(names: &[String]) -> (Self, MidiFeed) {
        let clock = HostClock::start();
        let mut connections = Vec::new();
        let mut messages = Vec::new();
        for name in names {
            let (producer, consumer) = rtrb::RingBuffer::new(MIDI_QUEUE_CAPACITY);
            if let Some(connection) = connect(name, producer, clock) {
                connections.push(connection);
                messages.push(consumer);
            }
        }
        (
            Self {
                _connections: connections,
            },
            MidiFeed { messages, clock },
        )
    }
}

/// Connects to the input port called `name`. Its messages are stamped with `clock` and pushed into
/// `queue`, which the connection owns. The port number is the port's position in the system's list of inputs.
fn connect(name: &str, queue: Queue, clock: HostClock) -> Option<MidiInputConnection<Queue>> {
    let input = match MidiInput::new(CLIENT_NAME) {
        Ok(input) => input,
        Err(error) => {
            log::warn!("MIDI input {name} cannot be opened, because MIDI is unavailable: {error}");
            return None;
        }
    };
    let found = input.ports().into_iter().enumerate().find(|(_, port)| {
        input
            .port_name(port)
            .is_ok_and(|port_name| port_name == name)
    });
    let Some((index, port)) = found else {
        log::warn!("MIDI input {name} is not connected, so it does not feed the engine");
        return None;
    };
    let Ok(port_number) = u8::try_from(index) else {
        log::warn!("MIDI input {name} is past the 256th port, so it does not feed the engine");
        return None;
    };
    let connection = input.connect(
        &port,
        name,
        move |_stamp, bytes, producer: &mut Queue| {
            let time_us = clock.now_us();
            let Some(message) = MidiMessage::new(time_us, port_number, bytes) else {
                return;
            };
            // A full queue drops the message: the engine is behind, and waiting would not help.
            let _ = producer.push(message);
        },
        queue,
    );
    match connection {
        Ok(connection) => {
            log::info!("MIDI input {name} feeds the engine");
            Some(connection)
        }
        Err(error) => {
            log::warn!("MIDI input {name} cannot be opened: {error}");
            None
        }
    }
}

/// The MIDI input and output ports this machine offers. A port that cannot be named is skipped with a
/// warning, and a system without MIDI gives empty lists.
pub fn list_midi_ports() -> MidiPorts {
    MidiPorts {
        inputs: input_names(),
        outputs: output_names(),
    }
}

fn input_names() -> Vec<String> {
    let input = match MidiInput::new(CLIENT_NAME) {
        Ok(input) => input,
        Err(error) => {
            log::warn!("MIDI inputs cannot be listed, because MIDI is unavailable: {error}");
            return Vec::new();
        }
    };
    input
        .ports()
        .iter()
        .filter_map(|port| named(input.port_name(port)))
        .collect()
}

fn output_names() -> Vec<String> {
    let output = match MidiOutput::new(CLIENT_NAME) {
        Ok(output) => output,
        Err(error) => {
            log::warn!("MIDI outputs cannot be listed, because MIDI is unavailable: {error}");
            return Vec::new();
        }
    };
    output
        .ports()
        .iter()
        .filter_map(|port| named(output.port_name(port)))
        .collect()
}

/// The name of a port, or None with a warning when the system cannot name it.
fn named(name: Result<String, midir::PortInfoError>) -> Option<String> {
    match name {
        Ok(name) => Some(name),
        Err(error) => {
            log::warn!("A MIDI port cannot be named, so it is skipped: {error}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_that_is_no_input_port_opens_nothing_and_the_feed_is_empty() {
        let names = ["No Such MIDI Input Port".to_string()];
        let (_inputs, feed) = MidiInputs::open(&names);
        assert!(feed.messages.is_empty());
    }

    #[test]
    fn opening_no_ports_gives_a_feed_with_nothing_in_it() {
        let (_inputs, feed) = MidiInputs::open(&[]);
        assert!(feed.messages.is_empty());
    }
}
