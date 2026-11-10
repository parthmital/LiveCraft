//! Integration tests for lossless `.als` I/O, partial-edit preservation,
//! `LiveSetDiff` (B10), and 5-set gzip round-trips (M09 Step B).

mod common;

use std::path::Path;

use common::assert_projects_equivalent;
use livecraft_formats::als::diff::{ClipLocation, diff_projects};
use livecraft_formats::als::read::decompress_als_or_xml;
use livecraft_formats::als::{AlsDocument, AlsParser, diff_als_files, format_diff_summary};
use livecraft_model::ids::{ClipId, DeviceId, LaneId, TrackId};
use livecraft_model::model::{
    ArrangementClip, AudioInput, AudioOutput, AutomationLane, AutomationPoint, AutomationTarget,
    Clip, ClipLaunch, ClipPayload, Device, DeviceType, FollowAction, FollowActions, LaunchMode,
    MidiNote, MidiOutput, Project, Quantization, Routing, ScaleConfig, ScaleType, TapPoint, Track,
    TrackType, TuningSystem, WarpMarker, WarpMode,
};
use livecraft_time::{Beats, TempoMap, TimeSignatureMap};

const RICH_FIXTURE_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Ableton MajorVersion="5" MinorVersion="12.0_12117" SchemaChangeCount="10" Creator="LiveCraft Fixture 1.0" Revision="fixture-rev">
	<LiveSet>
		<NextPointeeId Value="5000" />
		<LomId Value="0" />
		<CustomUnknownRootAttr KeepMe='{"raw": [1, 2, 3]}' />
		<!-- Preserve root comment -->
		<Tracks>
			<MidiTrack Id="12" SelectedToolPanel="7">
				<LomId Value="0" />
				<Name>
					<EffectiveName Value="1-MIDI" />
					<UserName Value="" />
					<Annotation Value="Lead channel" />
				</Name>
				<Color Value="19" />
				<AutomationEnvelopes>
					<Envelopes />
				</AutomationEnvelopes>
				<TrackGroupId Value="-1" />
				<TrackDelay Value="0" />
				<DeviceChain>
					<AudioInputRouting>
						<Target Value="AudioIn/External/S0" />
						<UpperDisplayString Value="Ext. In" />
					</AudioInputRouting>
					<AudioOutputRouting>
						<Target Value="AudioOut/Main" />
					</AudioOutputRouting>
					<MidiOutputRouting>
						<Target Value="MidiOut/None" />
					</MidiOutputRouting>
					<Mixer>
						<Sends>
							<TrackSendHolder Id="0">
								<Send>
									<Manual Value="0.0003162277571" />
									<AutomationTarget Id="101" />
								</Send>
							</TrackSendHolder>
						</Sends>
						<Speaker>
							<Manual Value="true" />
						</Speaker>
						<SoloSink Value="false" />
						<Pan>
							<Manual Value="0" />
							<AutomationTarget Id="102" />
						</Pan>
						<Volume>
							<Manual Value="1" />
							<AutomationTarget Id="103" />
						</Volume>
					</Mixer>
					<MainSequencer>
						<ClipSlotList>
							<ClipSlot Id="0">
								<ClipSlot>
									<Value>
										<MidiClip Id="200" Time="0">
											<Name Value="Riff" />
											<Color Value="2" />
											<CurrentStart Value="0" />
											<CurrentEnd Value="4" />
											<Loop>
												<LoopStart Value="0" />
												<LoopEnd Value="4" />
												<StartRelative Value="0" />
												<LoopOn Value="true" />
												<OutMarker Value="4" />
											</Loop>
											<Notes>
												<KeyTracks>
													<KeyTrack Id="201">
														<Notes>
															<MidiNoteEvent Time="0" Duration="1" Velocity="110" VelocityDeviation="0" OffVelocity="64" Probability="1" IsEnabled="true" NoteId="1" />
														</Notes>
														<MidiKey Value="60" />
													</KeyTrack>
												</KeyTracks>
												<NoteIdGenerator>
													<NextId Value="2" />
												</NoteIdGenerator>
											</Notes>
										</MidiClip>
									</Value>
								</ClipSlot>
								<HasStop Value="true" />
							</ClipSlot>
						</ClipSlotList>
						<MonitoringEnum Value="1" />
					</MainSequencer>
					<DeviceChain>
						<Devices>
							<Operator Id="300">
								<UserName Value="Lead Op" />
								<On>
									<Manual Value="true" />
								</On>
								<FilterCutoff>
									<Manual Value="2400" />
								</FilterCutoff>
							</Operator>
							<MxDeviceAudioEffect Id="301">
								<UserName Value="M4L Shimmer" />
								<On>
									<Manual Value="true" />
								</On>
								<PatcherBlob><![CDATA[<maxpat>{opaque_data}</maxpat>]]></PatcherBlob>
							</MxDeviceAudioEffect>
						</Devices>
					</DeviceChain>
				</DeviceChain>
			</MidiTrack>
			<ReturnTrack Id="2">
				<Name>
					<EffectiveName Value="A-Reverb" />
					<UserName Value="" />
				</Name>
				<Color Value="1" />
				<TrackGroupId Value="-1" />
				<DeviceChain>
					<Mixer>
						<Sends>
							<TrackSendHolder Id="0">
								<Send>
									<Manual Value="0.0003162277571" />
								</Send>
							</TrackSendHolder>
						</Sends>
						<Speaker>
							<Manual Value="true" />
						</Speaker>
						<SoloSink Value="false" />
						<Pan>
							<Manual Value="0" />
						</Pan>
						<Volume>
							<Manual Value="1" />
						</Volume>
					</Mixer>
					<DeviceChain>
						<Devices />
					</DeviceChain>
				</DeviceChain>
			</ReturnTrack>
		</Tracks>
		<MainTrack>
			<Name>
				<EffectiveName Value="Main" />
				<UserName Value="" />
			</Name>
			<Color Value="4" />
			<AutomationEnvelopes>
				<Envelopes>
					<AutomationEnvelope Id="1">
						<EnvelopeTarget>
							<PointeeId Value="8" />
						</EnvelopeTarget>
						<Automation>
							<Events>
								<FloatEvent Id="0" Time="-63072000" Value="120" />
							</Events>
						</Automation>
					</AutomationEnvelope>
				</Envelopes>
			</AutomationEnvelopes>
			<TrackGroupId Value="-1" />
			<DeviceChain>
				<Mixer>
					<Speaker>
						<Manual Value="true" />
					</Speaker>
					<SoloSink Value="false" />
					<Pan>
						<Manual Value="0" />
					</Pan>
					<Volume>
						<Manual Value="1" />
					</Volume>
					<Tempo>
						<Manual Value="120" />
						<AutomationTarget Id="8" />
					</Tempo>
					<TimeSignature>
						<Manual Value="201" />
						<AutomationTarget Id="10" />
					</TimeSignature>
				</Mixer>
				<DeviceChain>
					<Devices />
				</DeviceChain>
			</DeviceChain>
		</MainTrack>
		<Scenes>
			<Scene Id="0">
				<Name Value="" />
				<Color Value="-1" />
				<Tempo Value="120" />
				<IsTempoEnabled Value="false" />
				<TimeSignatureId Value="201" />
				<IsTimeSignatureEnabled Value="false" />
			</Scene>
		</Scenes>
		<Transport>
			<LoopOn Value="false" />
			<LoopStart Value="0" />
			<LoopLength Value="16" />
			<CurrentTime Value="0" />
			<PunchIn Value="false" />
			<PunchOut Value="false" />
		</Transport>
		<GlobalQuantisation Value="4" />
		<ScaleInformation>
			<Root Value="0" />
			<Name Value="0" />
		</ScaleInformation>
		<InKey Value="false" />
		<TuningSystems />
	</LiveSet>
</Ableton>
"#;

#[test]
fn unmodified_default_live_set_roundtrips_with_zero_xml_diff_when_present() {
    let template_path =
        r"C:\ProgramData\Ableton\Live 12 Suite\Resources\Builtin\Templates\DefaultLiveSet.als";
    if !Path::new(template_path).exists() {
        return;
    }
    let original_xml = decompress_als_or_xml(template_path).expect("decompress DefaultLiveSet");
    let mut doc = AlsDocument::open(template_path).expect("open DefaultLiveSet");
    assert!(
        doc.project().validate().is_empty(),
        "DefaultLiveSet validation errors: {:?}",
        doc.project().validate()
    );
    let unchanged = doc.project().clone();
    doc.apply_project(&unchanged)
        .expect("apply unchanged project");
    let roundtripped = doc.to_xml().expect("serialize");
    assert_eq!(
        roundtripped.as_bytes(),
        original_xml.as_bytes(),
        "DefaultLiveSet.als must round-trip with ZERO XML diff when unmodified"
    );
}

#[test]
fn unmodified_rich_fixture_preserves_unknown_nodes_quotes_and_m4l_bytes_exactly() {
    let mut doc = AlsDocument::from_xml(RICH_FIXTURE_XML).expect("parse fixture");
    assert!(doc.project().validate().is_empty());
    let m4l = &doc.project().tracks[0].device_chain[1];
    assert_eq!(
        m4l.device_type,
        DeviceType::Custom("MxDeviceAudioEffect".to_owned())
    );
    assert_eq!(m4l.name, "M4L Shimmer");

    let unchanged = doc.project().clone();
    doc.apply_project(&unchanged).expect("apply unchanged");
    let roundtripped = doc.to_xml().expect("to_xml");
    assert_eq!(roundtripped, RICH_FIXTURE_XML);
}

#[test]
fn partial_edit_preserves_unknown_nodes_and_m4l_device_and_updates_next_pointee_id() {
    let mut doc = AlsDocument::from_xml(RICH_FIXTURE_XML).expect("parse fixture");
    let orig_next = doc.next_pointee_id();
    let mut edited = doc.project().clone();

    edited.tracks[0].name = "Renamed Bass".to_owned();
    edited.transport.set_bpm(138.0);
    if let Some(ref mut clip) = edited.tracks[0].clip_slots[0].clip
        && let ClipPayload::Midi(ref mut m) = clip.payload
    {
        m.notes.push(MidiNote::new(64, 1.0, 0.5, 95));
    }

    let mut new_audio = Track::new(TrackId(90), "Pad Audio", TrackType::Audio, 1);
    new_audio.sends = vec![0.25];
    edited.tracks.push(new_audio);

    doc.apply_project(&edited).expect("apply edited");
    let xml = doc.to_xml().expect("to_xml");

    assert!(xml.contains("Renamed Bass"));
    assert!(xml.contains("<Manual Value=\"138\" />"));
    assert!(xml.contains("Pad Audio"));
    assert!(xml.contains("CustomUnknownRootAttr KeepMe='{\"raw\": [1, 2, 3]}'"));
    assert!(xml.contains("<!-- Preserve root comment -->"));
    assert!(xml.contains("<PatcherBlob><![CDATA[<maxpat>{opaque_data}</maxpat>]]></PatcherBlob>"));
    assert!(xml.contains("<TrackDelay Value=\"0\" />"));
    assert!(doc.next_pointee_id() >= orig_next);

    // Deleting a clip preserves everything else
    let mut removed_clip_proj = edited.clone();
    removed_clip_proj.tracks[0].clip_slots[0].clip = None;
    doc.apply_project(&removed_clip_proj).expect("remove clip");
    let reloaded = AlsDocument::from_xml(&doc.to_xml().expect("xml"))
        .expect("reload")
        .into_project();
    assert!(reloaded.tracks[0].clip_slots[0].clip.is_none());
    assert_eq!(reloaded.tracks[0].device_chain.len(), 2);
    assert_eq!(
        reloaded.tracks[0].device_chain[1].device_type,
        DeviceType::Custom("MxDeviceAudioEffect".to_owned())
    );
}

#[test]
fn liveset_diff_summarizes_files_and_model_edits() {
    let tmp = std::env::temp_dir().join("livecraft_m09_diff_test");
    let _ = std::fs::create_dir_all(&tmp);
    let path_a = tmp.join("a.als");
    let path_b = tmp.join("b.als");

    let mut proj_a = Project::new_default("A");
    let mut clip = Clip::new_midi(401, "Chords", 4.0);
    if let ClipPayload::Midi(ref mut md) = clip.payload {
        md.notes.push(MidiNote::new(60, 0.0, 1.0, 100));
    }
    proj_a.tracks[0].clip_slots[0].clip = Some(clip);
    AlsParser::save_project_to_als(&proj_a, &path_a).expect("save a");

    let mut proj_b = proj_a.clone();
    proj_b.transport.set_bpm(145.0);
    proj_b.scale = ScaleConfig::new(5, ScaleType::HarmonicMinor);
    proj_b.tracks[1].is_muted = true;
    if let Some(ref mut c) = proj_b.tracks[0].clip_slots[0].clip
        && let ClipPayload::Midi(ref mut md) = c.payload
    {
        md.notes.push(MidiNote::new(67, 2.0, 1.0, 120));
    }
    AlsParser::save_project_to_als(&proj_b, &path_b).expect("save b");

    let diff = diff_als_files(&path_a, &path_b).expect("diff files");
    assert_eq!(diff, diff_projects(&proj_a, &proj_b));
    assert!(!diff.is_empty());
    assert_eq!(diff.bpm_changed, Some((120.0, 145.0)));
    assert!(diff.scale_changed.is_some());
    assert_eq!(diff.tracks_modified.len(), 2);
    assert!(matches!(
        diff.tracks_modified[0].clips_modified[0].location,
        ClipLocation::SessionSlot(0)
    ));

    let summary = format_diff_summary(&diff);
    assert!(summary.contains("120.00 BPM -> 145.00 BPM"));
    assert!(summary.contains("Harmonic Minor"));
    assert!(summary.contains("G3 @ 2"));

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn five_distinct_livecraft_sets_roundtrip_via_gzip_als() {
    let tmp = std::env::temp_dir().join("livecraft_m09_five_sets");
    let _ = std::fs::create_dir_all(&tmp);

    // 1. Empty set
    let s1 = Project::new_default("Set1_Empty");

    // 2. MIDI + Operator + full-featured notes
    let mut s2 = Project::new_default("Set2_MidiOperator");
    let mut op = Device::new(DeviceId(601), "Bass Op", DeviceType::Operator);
    op.set_param("filter_cutoff", 1850.0);
    op.set_param("algorithm", 3.0);
    s2.tracks[0].device_chain = vec![op];
    let mut mclip = Clip::new_midi(602, "Arp", 8.0);
    mclip.launch = ClipLaunch {
        mode: LaunchMode::Toggle,
        quantization: Quantization::Bars(2),
        legato: true,
        velocity_amount: 0.4,
    };
    mclip.follow = FollowActions {
        enabled: true,
        action_a: FollowAction::Jump(3),
        action_b: FollowAction::Stop,
        chance_a: 0.75,
        chance_b: 0.25,
        linked: false,
        multiplier: 2,
        time_beats: 8.0,
    };
    if let ClipPayload::Midi(ref mut md) = mclip.payload {
        md.notes.push(MidiNote {
            pitch: 60,
            start_beat: 0.0,
            duration_beats: 0.75,
            velocity: 102.5,
            release_velocity: 48.0,
            probability: 0.85,
            velocity_deviation: -12.0,
            mute: true,
        });
    }
    s2.tracks[0].clip_slots[0].clip = Some(mclip);

    // 3. Audio + AudioClip + WarpMarkers + gain/pitch
    let mut s3 = Project::new_default("Set3_AudioWarp");
    let mut aclip = Clip::new_audio(701, "Vocal Chop", "Samples/vox.wav", 8.0);
    if let ClipPayload::Audio(ref mut ad) = aclip.payload {
        ad.warp_mode = WarpMode::ComplexPro;
        ad.is_warped = true;
        ad.gain_db = -3.5;
        ad.pitch_cents = -150;
        ad.sample_rate = 48000;
        ad.channels = 1;
        ad.original_bpm = 128.0;
        ad.warp_markers = vec![
            WarpMarker {
                sample_time_sec: 0.0,
                beat_time: 0.0,
            },
            WarpMarker {
                sample_time_sec: 1.875,
                beat_time: 4.0,
            },
        ];
    }
    s3.tracks[2].clip_slots[1].clip = Some(aclip);

    // 4. Multi-track + sends + returns + track & clip automation
    let mut s4 = Project::new_default("Set4_SendsAutomation");
    s4.tracks[0].sends = vec![0.6, 0.35];
    let mut vol_lane = AutomationLane::new(LaneId(801), AutomationTarget::TrackVolume);
    vol_lane.points = vec![
        AutomationPoint::new(0.0, 0.2, 0.0),
        AutomationPoint::new(4.0, 0.9, 0.4),
    ];
    s4.tracks[0].automation_lanes.push(vol_lane);
    let mut dev_lane = AutomationLane::new(
        LaneId(802),
        AutomationTarget::DeviceParam {
            device_id: DeviceId(101),
            param_key: "filter_cutoff".to_owned(),
        },
    );
    dev_lane.points = vec![
        AutomationPoint::new(0.0, 500.0, 0.0),
        AutomationPoint::new(8.0, 4500.0, -0.3),
    ];
    s4.tracks[0].automation_lanes.push(dev_lane);

    // 5. Group + child tracks + arrangement clips + tempo/meter maps + tuning
    let mut s5 = Project::new_default("Set5_GroupTuning");
    let mut grp = Track::new(TrackId(50), "Synth Bus", TrackType::Group, 0);
    grp.sends = vec![0.0, 0.0];
    s5.tracks.insert(0, grp);
    s5.tracks[1].parent = Some(TrackId(50));
    s5.tracks[1].routing = Routing {
        audio_from: AudioInput::ExternalIn { channel: 2 },
        audio_to: AudioOutput::Track(TrackId(50)),
        midi_to: MidiOutput::External {
            port: 1,
            channel: 3,
        },
    };
    s5.tracks[2].routing.audio_from = AudioInput::Track {
        track: TrackId(1),
        tap: TapPoint::PostFx,
    };
    let arr = ArrangementClip::new(
        ClipId(901),
        Clip::new_midi(901, "Chorus Line", 4.0),
        8.0,
        4.0,
    );
    s5.tracks[1].arrangement_clips.push(arr);
    let mut tmap = TempoMap::constant(110.0);
    tmap.insert(Beats(0.0), 110.0, true);
    tmap.insert(Beats(8.0), 132.0, false);
    s5.transport.tempo = tmap;
    let mut smap = TimeSignatureMap::constant(4, 4).expect("4/4");
    smap.insert(2, 6, 8).expect("6/8");
    s5.transport.time_signature = smap;
    s5.scenes[0].tempo = Some(126.0);
    s5.scenes[0].time_signature = Some((3, 4));
    s5.tuning =
        Some(TuningSystem::from_scala("! test\nPyth\n3\n204.0\n408.0\n1200.0\n").expect("scala"));

    for (idx, set) in [&s1, &s2, &s3, &s4, &s5].iter().enumerate() {
        assert!(
            set.validate().is_empty(),
            "set {} invalid: {:?}",
            idx + 1,
            set.validate()
        );
        let p = tmp.join(format!("set_{}.als", idx + 1));
        AlsDocument::from_project(set)
            .expect("from_project")
            .save_als(&p)
            .expect("save_als");
        let loaded = AlsDocument::open(&p).expect("open").into_project();
        assert!(
            loaded.validate().is_empty(),
            "reloaded set {} invalid: {:?}",
            idx + 1,
            loaded.validate()
        );
        assert_projects_equivalent(set, &loaded);
    }

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn audio_clip_relative_path_lossless_als_roundtrip() {
    let tmp = std::env::temp_dir().join(format!(
        "livecraft_audio_rel_roundtrip_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).expect("mkdir");

    // 1. Existing XML without <RelativePath> and with relative_path = None remains untouched.
    let no_rel_xml = concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "<Ableton MajorVersion=\"5\" MinorVersion=\"12.0_12117\">\n",
        "\t<LiveSet>\n",
        "\t\t<Tracks>\n",
        "\t\t\t<AudioTrack Id=\"1\">\n",
        "\t\t\t\t<Name><EffectiveName Value=\"Audio\" /></Name>\n",
        "\t\t\t\t<DeviceChain><MainSequencer><ClipSlotList>\n",
        "\t\t\t\t\t<ClipSlot Id=\"0\"><ClipSlot><Value>\n",
        "\t\t\t\t\t\t<AudioClip Id=\"10\">\n",
        "\t\t\t\t\t\t\t<Name Value=\"Dry\" />\n",
        "\t\t\t\t\t\t\t<SampleRef><FileRef><Path Value=\"D:/Audio/Dry.wav\" /></FileRef></SampleRef>\n",
        "\t\t\t\t\t\t</AudioClip>\n",
        "\t\t\t\t\t</Value></ClipSlot></ClipSlot>\n",
        "\t\t\t\t</ClipSlotList></MainSequencer></DeviceChain>\n",
        "\t\t\t</AudioTrack>\n",
        "\t\t</Tracks>\n",
        "\t</LiveSet>\n",
        "</Ableton>\n"
    );
    let mut doc = AlsDocument::from_xml(no_rel_xml).expect("parse no_rel_xml");
    let proj = doc.project().clone();
    if let Some(ref clip) = proj.tracks[0].clip_slots[0].clip
        && let ClipPayload::Audio(ref a) = clip.payload
    {
        assert_eq!(a.file_path, "D:/Audio/Dry.wav");
        assert_eq!(a.relative_path, None);
    } else {
        panic!("expected audio clip");
    }
    doc.apply_project(&proj).expect("apply unchanged");
    let unchanged_xml = doc.to_xml().expect("to_xml unchanged");
    assert!(
        !unchanged_xml.contains("RelativePath"),
        "unchanged XML must not gain RelativePath tags:\n{unchanged_xml}"
    );

    // 2. Setting relative_path patches <RelativePath> and <RelativePathType Value="3" /> and round-trips.
    let mut edited = proj;
    if let Some(ref mut clip) = edited.tracks[0].clip_slots[0].clip
        && let ClipPayload::Audio(ref mut a) = clip.payload
    {
        a.relative_path = Some("Samples/Imported/Dry.wav".to_owned());
    }
    doc.apply_project(&edited).expect("apply relative_path");
    let patched_xml = doc.to_xml().expect("to_xml patched");
    assert!(patched_xml.contains("<RelativePathType Value=\"3\""));
    assert!(patched_xml.contains("<RelativePath Value=\"Samples/Imported/Dry.wav\""));

    let als_path = tmp.join("rel_roundtrip.als");
    doc.save_als(&als_path).expect("save_als");
    let reopened = AlsDocument::open(&als_path).expect("reopen");
    if let Some(ref clip) = reopened.project().tracks[0].clip_slots[0].clip
        && let ClipPayload::Audio(ref a) = clip.payload
    {
        assert_eq!(a.file_path, "D:/Audio/Dry.wav");
        assert_eq!(a.relative_path.as_deref(), Some("Samples/Imported/Dry.wav"));
    } else {
        panic!("expected audio clip on reopen");
    }

    let _ = std::fs::remove_dir_all(&tmp);
}
