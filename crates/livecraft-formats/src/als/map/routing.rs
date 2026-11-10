//! Mappers for track `<AudioInputRouting>`, `<AudioOutputRouting>`, and `<MidiOutputRouting>`.

use livecraft_model::ids::TrackId;
use livecraft_model::model::{AudioInput, AudioOutput, MidiOutput, Routing, TapPoint};

use super::super::tree::{XmlNodeId, XmlTree};
use super::{child_value_str, set_child_value};

pub(super) fn read_routing(tree: &XmlTree, dc: XmlNodeId) -> Routing {
    let audio_from = tree
        .child(dc, "AudioInputRouting")
        .and_then(|r| child_value_str(tree, r, "Target"))
        .map(parse_audio_input_target)
        .unwrap_or(AudioInput::None);

    let audio_to = tree
        .child(dc, "AudioOutputRouting")
        .and_then(|r| child_value_str(tree, r, "Target"))
        .map(parse_audio_output_target)
        .unwrap_or(AudioOutput::Main);

    let midi_to = tree
        .child(dc, "MidiOutputRouting")
        .and_then(|r| child_value_str(tree, r, "Target"))
        .map(parse_midi_output_target)
        .unwrap_or(MidiOutput::None);

    Routing {
        audio_from,
        audio_to,
        midi_to,
    }
}

pub(super) fn patch_routing(
    tree: &mut XmlTree,
    dc: XmlNodeId,
    prev: Option<&Routing>,
    cur: &Routing,
) {
    if prev.is_none_or(|p| p.audio_from != cur.audio_from)
        && let Some(r) = tree.ensure_child(dc, "AudioInputRouting")
    {
        set_child_value(tree, r, "Target", format_audio_input_target(cur.audio_from));
    }
    if prev.is_none_or(|p| p.audio_to != cur.audio_to)
        && let Some(r) = tree.ensure_child(dc, "AudioOutputRouting")
    {
        set_child_value(tree, r, "Target", format_audio_output_target(cur.audio_to));
    }
    if prev.is_none_or(|p| p.midi_to != cur.midi_to)
        && let Some(r) = tree.ensure_child(dc, "MidiOutputRouting")
    {
        set_child_value(tree, r, "Target", format_midi_output_target(cur.midi_to));
    }
}

fn parse_audio_input_target(target: &str) -> AudioInput {
    if target == "AudioIn/None" {
        return AudioInput::None;
    }
    if target == "AudioIn/Resampling" || target == "AudioIn/Main" || target == "AudioIn/Master" {
        return AudioInput::Resampling;
    }
    if let Some(rest) = target.strip_prefix("AudioIn/External/")
        && let Some(ch_str) = rest.strip_prefix('S').or_else(|| rest.strip_prefix('M'))
        && let Ok(channel) = ch_str.parse::<u16>()
    {
        return AudioInput::ExternalIn { channel };
    }
    if let Some(rest) = target.strip_prefix("AudioIn/Track.")
        && let Some((id_str, tap_str)) = rest.split_once('/')
        && let Ok(id) = id_str.parse::<u64>()
    {
        let tap = match tap_str {
            "PreFx" => TapPoint::PreFx,
            "PostFx" => TapPoint::PostFx,
            _ => TapPoint::PostMixer,
        };
        return AudioInput::Track {
            track: TrackId(id),
            tap,
        };
    }
    AudioInput::None
}

fn parse_audio_output_target(target: &str) -> AudioOutput {
    if let Some(rest) = target
        .strip_prefix("AudioOut/Track.")
        .or_else(|| target.strip_prefix("AudioOut/Track/"))
        && let Ok(id) = rest.parse::<u64>()
    {
        return AudioOutput::Track(TrackId(id));
    }
    AudioOutput::Main
}

fn parse_midi_output_target(target: &str) -> MidiOutput {
    if let Some(rest) = target.strip_prefix("MidiOut/External.")
        && let Some((port_str, ch_str)) = rest.split_once('/')
        && let (Ok(port), Ok(channel)) = (port_str.parse::<u8>(), ch_str.parse::<u8>())
    {
        return MidiOutput::External {
            port,
            channel: channel.min(15),
        };
    }
    MidiOutput::None
}

fn format_audio_input_target(input: AudioInput) -> String {
    match input {
        AudioInput::None => "AudioIn/None".to_owned(),
        AudioInput::ExternalIn { channel } => format!("AudioIn/External/S{channel}"),
        AudioInput::Resampling => "AudioIn/Resampling".to_owned(),
        AudioInput::Track { track, tap } => {
            let tap_str = match tap {
                TapPoint::PreFx => "PreFx",
                TapPoint::PostFx => "PostFx",
                TapPoint::PostMixer => "PostMixer",
            };
            format!("AudioIn/Track.{}/{tap_str}", track.0)
        }
    }
}

fn format_audio_output_target(output: AudioOutput) -> String {
    match output {
        AudioOutput::Main => "AudioOut/Main".to_owned(),
        AudioOutput::Track(id) => format!("AudioOut/Track.{}", id.0),
    }
}

fn format_midi_output_target(output: MidiOutput) -> String {
    match output {
        MidiOutput::None => "MidiOut/None".to_owned(),
        MidiOutput::External { port, channel } => format!("MidiOut/External.{port}/{channel}"),
    }
}
