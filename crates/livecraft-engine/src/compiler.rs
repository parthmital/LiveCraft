//! Compiles a Live Set into a `CompiledGraph` (M05 item 5). This runs on the session side, never on
//! the audio thread: it allocates the processors and buffers the audio thread then uses.

use crate::compiled::{CompiledGraph, DeviceSlot, Mixer, Node, NodeKind, Transfer};
use livecraft_devices::{create_device_processor, is_instrument};
use livecraft_model::ids::{DeviceId, TrackId};
use livecraft_model::model::{Device, Project, Track};

/// Compiles Sets into graphs. It remembers the layout of the last graph it compiled, so each new
/// graph lists the devices it takes over. Compile and submit graphs in the same order.
pub struct GraphCompiler {
    sample_rate: f32,
    /// The track and device ids of each node of the last graph, in node order.
    previous: Vec<(TrackId, Vec<DeviceId>)>,
}

impl GraphCompiler {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            previous: Vec::new(),
        }
    }

    /// Compiles the Set into the nodes the renderer runs. A device without a native processor makes
    /// no sound, so it is left out of the chain.
    pub fn compile(&mut self, project: &Project) -> CompiledGraph {
        let tracks = project
            .tracks
            .iter()
            .map(|track| self.node(track, NodeKind::Track));
        let returns = project
            .return_tracks
            .iter()
            .map(|track| self.node(track, NodeKind::Return));
        let main = self.node(&project.main_track, NodeKind::Main);
        let nodes: Vec<Node> = tracks.chain(returns).chain(std::iter::once(main)).collect();
        let transfers = self.transfers(&nodes);
        self.previous = nodes
            .iter()
            .map(|node| {
                (
                    node.track,
                    node.devices.iter().map(|slot| slot.id).collect(),
                )
            })
            .collect();
        CompiledGraph::new(
            nodes,
            project.tracks.len(),
            project.return_tracks.len(),
            project.scenes.iter().map(|scene| scene.id).collect(),
            transfers,
        )
    }

    /// Every device that the previous graph also had, paired with its old and new position.
    fn transfers(&self, nodes: &[Node]) -> Vec<Transfer> {
        let mut transfers = Vec::new();
        for (to_node, node) in nodes.iter().enumerate() {
            let Some(from_node) = self
                .previous
                .iter()
                .position(|(track, _)| *track == node.track)
            else {
                continue;
            };
            let previous_devices = &self.previous[from_node].1;
            for (to_slot, slot) in node.devices.iter().enumerate() {
                if let Some(from_slot) = previous_devices.iter().position(|id| *id == slot.id) {
                    transfers.push(Transfer {
                        from: (from_node, from_slot),
                        to: (to_node, to_slot),
                    });
                }
            }
        }
        transfers
    }

    fn node(&self, track: &Track, kind: NodeKind) -> Node {
        let mut chain: Vec<&Device> = track.device_chain.iter().collect();
        // Instruments come first, so they take the note events before the effects process them.
        chain.sort_by_key(|device| !is_instrument(&device.device_type));
        let devices = chain
            .into_iter()
            .filter_map(|device| {
                create_device_processor(device, self.sample_rate).map(|processor| DeviceSlot {
                    id: device.id,
                    enabled: device.is_enabled,
                    processor,
                    values: device.parameters.iter().collect(),
                })
            })
            .collect();
        let mixer = Mixer {
            volume_db: track.volume_db,
            pan: track.pan,
            is_muted: track.is_muted,
            is_soloed: track.is_soloed,
            sends: match kind {
                NodeKind::Track => track.sends.clone(),
                NodeKind::Return | NodeKind::Main => Vec::new(),
            },
        };
        Node::new(track.id, kind, mixer, devices)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiled::MAX_BLOCK;
    use livecraft_devices::{NoteEvent, ProcessContext};
    use livecraft_model::ids::{DeviceId, TrackId};
    use livecraft_model::model::DeviceType;

    fn compile(project: &Project) -> CompiledGraph {
        GraphCompiler::new(44100.0).compile(project)
    }

    fn context(frames: usize) -> ProcessContext {
        ProcessContext {
            sample_rate: 44100.0,
            frames,
            bpm: 120.0,
            playhead_beats: 0.0,
            beats_per_frame: 0.0,
        }
    }

    #[test]
    fn the_nodes_are_tracks_then_returns_then_main() {
        let project = Project::new_default("Order");
        let graph = compile(&project);
        let kinds: Vec<NodeKind> = graph.nodes().iter().map(|node| node.kind).collect();
        let expected: Vec<NodeKind> = std::iter::repeat_n(NodeKind::Track, project.tracks.len())
            .chain(std::iter::repeat_n(
                NodeKind::Return,
                project.return_tracks.len(),
            ))
            .chain(std::iter::once(NodeKind::Main))
            .collect();
        assert_eq!(kinds, expected);
    }

    #[test]
    fn an_instrument_is_placed_before_the_effects() {
        let mut project = Project::new_default("Chain");
        project.tracks[0].device_chain.insert(
            0,
            Device::new(DeviceId(500), "Utility", DeviceType::Utility),
        );
        let graph = compile(&project);
        let track = graph
            .nodes()
            .iter()
            .find(|node| node.track == TrackId(1))
            .expect("track 1");
        assert_eq!(track.devices[0].id, DeviceId(101));
        assert_eq!(track.devices[1].id, DeviceId(500));
    }

    #[test]
    fn a_silent_set_renders_silence() {
        let mut graph = compile(&Project::new_default("Silent"));
        graph.render(&context(MAX_BLOCK));
        let (left, right) = graph.output(MAX_BLOCK);
        assert!(left.iter().chain(right).all(|&sample| sample == 0.0));
    }

    #[test]
    fn a_note_in_the_block_makes_its_instrument_sound() {
        let mut graph = compile(&Project::new_default("Note"));
        let track = graph.node_mut(TrackId(1)).expect("track 1");
        assert!(track.events.push(NoteEvent::On {
            offset: 0,
            pitch: 60,
            velocity: 100,
            freq_hz: 261.6,
        }));
        graph.render(&context(MAX_BLOCK));
        let (left, _) = graph.output(MAX_BLOCK);
        assert!(left.iter().any(|&sample| sample != 0.0));
    }

    #[test]
    fn rendering_the_same_graph_twice_gives_the_same_bits() {
        let render = || {
            let mut graph = compile(&Project::new_default("Repeat"));
            let track = graph.node_mut(TrackId(1)).expect("track 1");
            assert!(track.events.push(NoteEvent::On {
                offset: 3,
                pitch: 64,
                velocity: 90,
                freq_hz: 329.6,
            }));
            let mut out = Vec::new();
            for _ in 0..4 {
                graph.render(&context(MAX_BLOCK));
                out.extend_from_slice(graph.output(MAX_BLOCK).0);
                graph.node_mut(TrackId(1)).expect("track 1").events.clear();
            }
            out
        };
        let first: Vec<u32> = render().iter().map(|s| s.to_bits()).collect();
        let second: Vec<u32> = render().iter().map(|s| s.to_bits()).collect();
        assert_eq!(first, second);
    }
}
