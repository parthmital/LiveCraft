//! 10,000-case property-based lossless round-trip verification for `AlsDocument`.

mod common;

use common::assert_projects_equivalent;
use livecraft_formats::als::AlsDocument;
use livecraft_model::color::ABLETON_PALETTE;
use livecraft_model::ids::{ClipId, DeviceId, LaneId, SceneId, TrackId};
use livecraft_model::model::{
    ArrangementClip, AudioOutput, AutomationLane, AutomationPoint, AutomationTarget, Clip,
    ClipPayload, Device, DeviceType, FollowAction, FollowActions, Metronome, MetronomeRhythm,
    MetronomeSound, MidiNote, MonitoringMode, Project, Quantization, ScaleConfig, ScaleType, Scene,
    Track, TrackType, TuningSystem, WarpMode, param_table,
};
use livecraft_time::{Beats, TempoMap, TimeSignatureMap};
use proptest::prelude::*;
use proptest::sample::select;

fn arb_palette_color() -> impl Strategy<Value = String> {
    (0..ABLETON_PALETTE.len()).prop_map(|i| ABLETON_PALETTE[i].to_owned())
}

fn arb_quantization() -> impl Strategy<Value = Quantization> {
    select(vec![
        Quantization::None,
        Quantization::Bars(8),
        Quantization::Bars(4),
        Quantization::Bars(2),
        Quantization::Bars(1),
        Quantization::Note {
            num: 1,
            den: 4,
            triplet: false,
        },
        Quantization::Note {
            num: 1,
            den: 8,
            triplet: true,
        },
        Quantization::Note {
            num: 1,
            den: 16,
            triplet: false,
        },
        Quantization::Note {
            num: 1,
            den: 32,
            triplet: false,
        },
    ])
}

fn arb_follow_actions() -> impl Strategy<Value = FollowActions> {
    (
        any::<bool>(),
        select(vec![
            FollowAction::NoAction,
            FollowAction::Stop,
            FollowAction::PlayAgain,
            FollowAction::Previous,
            FollowAction::Next,
            FollowAction::First,
            FollowAction::Last,
            FollowAction::Any,
            FollowAction::Other,
            FollowAction::Jump(2),
        ]),
        select(vec![
            FollowAction::NoAction,
            FollowAction::Stop,
            FollowAction::Next,
            FollowAction::Jump(0),
        ]),
        0.0f32..=1.0,
        any::<bool>(),
        1u32..=8,
        1.0f64..=16.0,
    )
        .prop_map(
            |(enabled, action_a, action_b, chance_a, linked, multiplier, time_beats)| {
                FollowActions {
                    enabled,
                    action_a,
                    action_b,
                    chance_a: (chance_a * 100.0).round() / 100.0,
                    chance_b: ((1.0 - chance_a) * 100.0).round() / 100.0,
                    linked,
                    multiplier,
                    time_beats,
                }
            },
        )
}

fn arb_device(id: DeviceId) -> impl Strategy<Value = Device> {
    let kinds = select(vec![
        DeviceType::Operator,
        DeviceType::EqEight,
        DeviceType::Compressor,
        DeviceType::Limiter,
        DeviceType::Utility,
        DeviceType::Delay,
        DeviceType::Reverb,
        DeviceType::Saturator,
        DeviceType::AutoFilter,
        DeviceType::Chorus,
        DeviceType::AutoPan,
        DeviceType::Gate,
        DeviceType::EqThree,
        DeviceType::Redux,
        DeviceType::Simpler,
        DeviceType::DrumRack,
        DeviceType::Custom("MxDeviceAudioEffect".to_owned()),
    ]);
    (kinds, any::<bool>(), 0.1f32..1000.0).prop_map(move |(dtype, enabled, val)| {
        let mut dev = Device::new(id, "Dev", dtype.clone());
        dev.is_enabled = enabled;
        if let Some(first) = param_table(&dtype).first() {
            dev.set_param(first.name, val);
        }
        dev
    })
}

fn arb_midi_note() -> impl Strategy<Value = MidiNote> {
    (
        24u8..96,
        0.0f64..16.0,
        0.125f64..4.0,
        1u8..=127,
        0u8..=127,
        0.0f32..=1.0,
        -20.0f32..=20.0,
        any::<bool>(),
    )
        .prop_map(|(pitch, start, dur, vel, rel, prob, dev, mute)| MidiNote {
            pitch,
            start_beat: start,
            duration_beats: dur,
            velocity: f32::from(vel),
            release_velocity: f32::from(rel),
            probability: prob,
            velocity_deviation: dev,
            mute,
        })
}

fn arb_project() -> impl Strategy<Value = Project> {
    let tempos = (
        40.0f64..220.0,
        proptest::collection::vec((1.0f64..32.0, 40.0f64..220.0, any::<bool>()), 0..=2),
    );
    let sigs = (
        select(vec![(4u8, 4u8), (3, 4), (6, 8), (7, 8)]),
        proptest::collection::vec((1i32..16, select(vec![(3u8, 4u8), (5, 8), (4, 4)])), 0..=2),
    );
    let scale = (0u8..12, select(ScaleType::ALL.to_vec()), any::<bool>());
    let transport_flags = (
        any::<bool>(),
        0.0f64..16.0,
        1.0f64..32.0,
        0.0f64..32.0,
        any::<bool>(),
        any::<bool>(),
        arb_quantization(),
        any::<bool>(),
        any::<bool>(),
    );
    let track_specs = proptest::collection::vec(
        (
            select(vec![TrackType::Midi, TrackType::Audio, TrackType::Group]),
            arb_palette_color(),
            -48.0f32..=6.0,
            -1.0f32..=1.0,
            any::<bool>(),
            any::<bool>(),
            any::<bool>(),
            select(vec![
                MonitoringMode::In,
                MonitoringMode::Auto,
                MonitoringMode::Off,
            ]),
            proptest::collection::vec(arb_midi_note(), 0..=2),
            arb_follow_actions(),
            arb_device(DeviceId(1)),
        ),
        1..=2,
    );

    (
        tempos,
        sigs,
        scale,
        transport_flags,
        track_specs,
        0usize..=1,
        1usize..=2,
        any::<bool>(),
    )
        .prop_map(
            |(
                (bpm0, tempo_pts),
                ((n0, d0), sig_pts),
                (root_note, scale_type, scale_active),
                (loop_on, loop_s, loop_l, playhead, pin, pout, gq, met_on, fa_on),
                t_specs,
                ret_count,
                scene_count,
                with_tuning,
            )| {
                let mut proj = Project::new_default("PropTest");
                let mut tmap = TempoMap::constant(bpm0);
                for (b, bpm, ramp) in tempo_pts {
                    tmap.insert(Beats(b), bpm, ramp);
                }
                proj.transport.tempo = tmap;

                let mut smap = TimeSignatureMap::constant(n0, d0).expect("valid sig");
                let mut by_bar = std::collections::BTreeMap::new();
                for (bar, sig) in sig_pts {
                    by_bar.insert(bar, sig);
                }
                for (bar, (n, d)) in by_bar {
                    if smap.signature_at(smap.bar_start(bar)) != (n, d) {
                        let _ = smap.insert(bar, n, d);
                    }
                }
                proj.transport.time_signature = smap;
                proj.transport.loop_enabled = loop_on;
                proj.transport.loop_start_beats = loop_s;
                proj.transport.loop_length_beats = loop_l;
                proj.transport.playhead_beats = playhead;
                proj.transport.punch_in = pin;
                proj.transport.punch_out = pout;
                proj.transport.global_quantization = gq;
                proj.transport.metronome_enabled = met_on;
                proj.transport.follow_actions_enabled = fa_on;
                proj.transport.metronome = Metronome {
                    count_in_bars: if met_on { 2 } else { 0 },
                    rhythm: if met_on {
                        MetronomeRhythm::Division {
                            den: 8,
                            triplet: true,
                        }
                    } else {
                        MetronomeRhythm::Auto
                    },
                    sound: if met_on {
                        MetronomeSound::Wood
                    } else {
                        MetronomeSound::Classic
                    },
                    only_while_recording: met_on,
                    volume_db: if met_on { -3.0 } else { 0.0 },
                };

                proj.scale = ScaleConfig {
                    root_note,
                    scale_type,
                    is_active: scale_active,
                };
                proj.tuning = with_tuning.then(|| {
                    TuningSystem::from_scala("! p\nProp\n2\n500.0\n1200.0\n").expect("scala")
                });

                let mut next_id = 1u64;
                let mut alloc = || {
                    let v = next_id;
                    next_id += 1;
                    v
                };

                proj.return_tracks = (0..ret_count)
                    .map(|i| {
                        let mut rt =
                            Track::new(TrackId(alloc()), format!("Ret-{i}"), TrackType::Return, 0);
                        rt.sends = vec![0.0; ret_count];
                        rt
                    })
                    .collect();

                proj.tracks.clear();
                let mut first_group: Option<TrackId> = None;
                for (idx, (tt, color, vol, pan, muted, soloed, armed, mon, notes, fa, mut dev)) in
                    t_specs.into_iter().enumerate()
                {
                    let tid = TrackId(alloc());
                    if tt == TrackType::Group && first_group.is_none() {
                        first_group = Some(tid);
                    }
                    let mut trk = Track::new(tid, format!("Trk-{idx}"), tt, 1);
                    trk.color = color;
                    trk.volume_db = vol;
                    trk.pan = pan;
                    trk.is_muted = muted;
                    trk.is_soloed = soloed;
                    trk.is_armed = armed;
                    trk.monitoring = mon;
                    trk.sends = vec![0.25; ret_count];
                    dev.id = DeviceId(alloc());
                    trk.device_chain.push(dev);

                    if tt != TrackType::Group
                        && let Some(gid) = first_group
                        && gid != tid
                    {
                        trk.parent = Some(gid);
                        trk.routing.audio_to = AudioOutput::Track(gid);
                    }

                    if tt == TrackType::Midi {
                        let cid = ClipId(alloc());
                        let mut clip = Clip::new_midi(cid, "MClip", 4.0);
                        clip.follow = fa;
                        if let ClipPayload::Midi(ref mut md) = clip.payload {
                            md.notes = notes;
                        }
                        trk.clip_slots[0].clip = Some(clip);
                    } else if tt == TrackType::Audio {
                        let cid = ClipId(alloc());
                        let mut clip = Clip::new_audio(cid, "AClip", "samples/loop.wav", 8.0);
                        if let ClipPayload::Audio(ref mut ad) = clip.payload {
                            ad.warp_mode = WarpMode::Complex;
                            ad.pitch_cents = 50;
                        }
                        trk.arrangement_clips
                            .push(ArrangementClip::new(cid, clip, 4.0, 8.0));
                    }

                    let lane_id = LaneId(alloc());
                    let mut lane = AutomationLane::new(lane_id, AutomationTarget::TrackVolume);
                    lane.points.push(AutomationPoint::new(0.0, 0.5, 0.25));
                    trk.automation_lanes.push(lane);

                    proj.tracks.push(trk);
                }

                proj.main_track = Track::new(TrackId(alloc()), "Main", TrackType::Main, 0);
                proj.main_track.sends = vec![0.0; ret_count];

                proj.scenes = (0..scene_count)
                    .map(|i| {
                        let mut s = Scene::new(SceneId(alloc()), format!("Scene {i}"));
                        if i == 0 {
                            s.tempo = Some(128.0);
                            s.time_signature = Some((3, 4));
                        }
                        s
                    })
                    .collect();

                proj
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn random_project_roundtrips_losslessly_through_xml(proj in arb_project()) {
        let doc = AlsDocument::from_project(&proj).expect("from_project");
        let xml = doc.to_xml().expect("to_xml");
        let reloaded = AlsDocument::from_xml(&xml).expect("from_xml").into_project();
        assert_projects_equivalent(&proj, &reloaded);
    }
}
