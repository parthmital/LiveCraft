//! MIDI ports through `midir` (M06 items 6 and 7, manual 17.3, External MIDI In/Out, page 375).
//!
//! midir calls each input's callback on its own thread. The callback stamps the message with the
//! shared `HostClock` and pushes it into the port's own queue, so no queue has two writers and nothing
//! locks. The engine pops every port's queue on the audio thread (see
//! `Engine::set_midi_input`). The engine sends the notes of tracks routed to an output port through a
//! queue to the `MidiOutputs` thread, which sends each when its frame is heard. `Playback::open_with_driver`
//! opens the ports. Listing, opening, and sending never run on the audio thread.

use crate::midi::{HostClock, MidiMessage, MidiOutEvent, MidiOutScheduler};
use midir::{MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use serde::Serialize;
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// The client name the MIDI clients report to the system.
const CLIENT_NAME: &str = "LiveCraft";

/// Capacity of the queue from each MIDI input to the engine, in messages. A burst beyond it is dropped,
/// because the audio thread never waits.
pub const MIDI_QUEUE_CAPACITY: usize = 1024;

/// The MIDI input and output ports this machine offers, in the order the system lists them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
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

/// Capacity of the queue from the engine to the MIDI output thread, in events.
pub const MIDI_OUT_QUEUE_CAPACITY: usize = 4096;

/// The longest the MIDI output thread sleeps between looks at its queue.
const MIDI_OUT_POLL: Duration = Duration::from_millis(1);

/// Where the MIDI output thread sends a message. `port` is the index of the output port in
/// `list_midi_ports().outputs`.
pub trait MidiSink: Send {
    fn send(&mut self, port: u8, bytes: &[u8]);
}

/// The open midir output connections, by output port index.
struct MidirSink {
    connections: Vec<(u8, MidiOutputConnection)>,
}

impl MidiSink for MidirSink {
    fn send(&mut self, port: u8, bytes: &[u8]) {
        if let Some((_, connection)) = self.connections.iter_mut().find(|(at, _)| *at == port)
            && let Err(error) = connection.send(bytes)
        {
            log::warn!("A MIDI message to output port {port} was not sent: {error}");
        }
    }
}

/// The engine's end of the MIDI output: the queue of notes stamped with their frames, and the anchor
/// that maps the frames to host time (`Engine::set_midi_output`).
pub struct MidiOutFeed {
    pub events: rtrb::Producer<MidiOutEvent>,
    pub anchor: triple_buffer::Input<Option<(u64, u64)>>,
}

/// The MIDI output thread. It sends each note when its frame is heard, so a note leaves the port in
/// time with the audio (manual 40.3). Dropping it stops the thread and closes the ports.
pub struct MidiOutputs {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl MidiOutputs {
    /// Opens the output ports named in `names` and starts the thread. A name that is not an output
    /// port, or a port that cannot be opened, is skipped with a warning. `clock` must be the clock the
    /// engine's buffers start on, and `output_delay` the driver's output delay in frames.
    pub fn open(
        names: &[String],
        clock: HostClock,
        sample_rate: f32,
        output_delay: Arc<AtomicU32>,
    ) -> io::Result<(Self, MidiOutFeed)> {
        let connections = names
            .iter()
            .filter_map(|name| connect_output(name))
            .collect();
        Self::with_sink(
            Box::new(MidirSink { connections }),
            clock,
            sample_rate,
            output_delay,
        )
    }

    /// As `open`, sending through `sink`. Tests pass a sink that records what it is sent.
    pub fn with_sink(
        mut sink: Box<dyn MidiSink>,
        clock: HostClock,
        sample_rate: f32,
        output_delay: Arc<AtomicU32>,
    ) -> io::Result<(Self, MidiOutFeed)> {
        let (events, mut queue) = rtrb::RingBuffer::new(MIDI_OUT_QUEUE_CAPACITY);
        let (anchor, mut anchors) = triple_buffer::triple_buffer(&None);
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = Arc::clone(&stop);
        let rate = f64::from(sample_rate);
        let thread = thread::Builder::new()
            .name("livecraft-midi-out".into())
            .spawn(move || {
                let mut scheduler = MidiOutScheduler::default();
                while !stopping.load(Ordering::Relaxed) {
                    while let Ok(event) = queue.pop() {
                        scheduler.push(event);
                    }
                    let Some(anchor) = *anchors.read() else {
                        thread::sleep(MIDI_OUT_POLL);
                        continue;
                    };
                    let delay = output_delay.load(Ordering::Relaxed);
                    let latency_us = (f64::from(delay) * 1_000_000.0 / rate).round() as u64;
                    let now = clock.now_us();
                    for event in scheduler.due(now, anchor, rate, latency_us) {
                        sink.send(event.port, &event.bytes[..usize::from(event.len)]);
                    }
                    let wait = scheduler.next_send_us(anchor, rate, latency_us).map_or(
                        MIDI_OUT_POLL,
                        |next| {
                            Duration::from_micros(next.saturating_sub(clock.now_us()))
                                .min(MIDI_OUT_POLL)
                        },
                    );
                    thread::sleep(wait);
                }
            })?;
        Ok((
            Self {
                stop,
                thread: Some(thread),
            },
            MidiOutFeed { events, anchor },
        ))
    }
}

impl Drop for MidiOutputs {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            log::error!("The MIDI output thread panicked");
        }
    }
}

/// Connects to the output port called `name`, with the port's index in the list of outputs.
fn connect_output(name: &str) -> Option<(u8, MidiOutputConnection)> {
    let output = match MidiOutput::new(CLIENT_NAME) {
        Ok(output) => output,
        Err(error) => {
            log::warn!("MIDI output {name} cannot be opened, because MIDI is unavailable: {error}");
            return None;
        }
    };
    let found = output.ports().into_iter().enumerate().find(|(_, port)| {
        output
            .port_name(port)
            .is_ok_and(|port_name| port_name == name)
    });
    let Some((index, port)) = found else {
        log::warn!("MIDI output {name} is not connected, so no notes are sent to it");
        return None;
    };
    let Ok(port_number) = u8::try_from(index) else {
        log::warn!("MIDI output {name} is past the 256th port, so no notes are sent to it");
        return None;
    };
    match output.connect(&port, name) {
        Ok(connection) => {
            log::info!("MIDI output {name} receives the notes routed to it");
            Some((port_number, connection))
        }
        Err(error) => {
            log::warn!("MIDI output {name} cannot be opened: {error}");
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

    /// Records what it is sent, with the clock time it was sent at.
    struct Loopback {
        clock: HostClock,
        sent: std::sync::mpsc::Sender<(u64, u8, Vec<u8>)>,
    }

    impl MidiSink for Loopback {
        fn send(&mut self, port: u8, bytes: &[u8]) {
            let _ = self.sent.send((self.clock.now_us(), port, bytes.to_vec()));
        }
    }

    fn loopback(delay_frames: u32) -> (MidiOutputs, MidiOutFeed, HostClock, Receiver) {
        let clock = HostClock::start();
        let (sent, received) = std::sync::mpsc::channel();
        let (outputs, feed) = MidiOutputs::with_sink(
            Box::new(Loopback { clock, sent }),
            clock,
            44100.0,
            Arc::new(AtomicU32::new(delay_frames)),
        )
        .expect("start the MIDI output thread");
        (outputs, feed, clock, received)
    }

    type Receiver = std::sync::mpsc::Receiver<(u64, u8, Vec<u8>)>;

    fn note(frame: u64, port: u8, pitch: u8) -> MidiOutEvent {
        MidiOutEvent {
            frame,
            port,
            len: 3,
            bytes: [0x92, pitch, 100],
        }
    }

    #[test]
    fn the_loopback_receives_each_note_in_order_when_its_frame_is_heard() {
        let (_outputs, mut feed, clock, received) = loopback(0);
        // Frame 0 is heard 20 ms from now, so frames 441 and 882 at 30 ms and 40 ms.
        let start = clock.now_us() + 20_000;
        for (frame, pitch) in [(0, 60), (441, 62), (882, 64)] {
            feed.events
                .push(note(frame, 1, pitch))
                .expect("room in the queue");
        }
        feed.anchor.write(Some((0, start)));
        for (index, pitch) in [60u8, 62, 64].into_iter().enumerate() {
            let (at, port, bytes) = received
                .recv_timeout(Duration::from_secs(2))
                .expect("the note is sent");
            let due = start + index as u64 * 10_000;
            assert_eq!(port, 1);
            assert_eq!(bytes, vec![0x92, pitch, 100]);
            assert!(at >= due, "note {pitch} sent {} us early", due - at);
            assert!(at < due + 15_000, "note {pitch} sent {} us late", at - due);
        }
    }

    #[test]
    fn the_output_delay_holds_each_note_until_its_sample_is_heard() {
        // 4410 frames of output delay is 100 ms.
        let (_outputs, mut feed, clock, received) = loopback(4410);
        let start = clock.now_us();
        feed.events.push(note(0, 0, 60)).expect("room in the queue");
        feed.anchor.write(Some((0, start)));
        let (at, _, _) = received
            .recv_timeout(Duration::from_secs(2))
            .expect("the note is sent");
        assert!(
            at >= start + 100_000,
            "sent {} us early",
            start + 100_000 - at
        );
    }

    #[test]
    fn nothing_is_sent_before_the_engine_has_published_an_anchor() {
        let (outputs, mut feed, _clock, received) = loopback(0);
        feed.events.push(note(0, 0, 60)).expect("room in the queue");
        assert!(received.recv_timeout(Duration::from_millis(50)).is_err());
        drop(outputs);
    }
}
