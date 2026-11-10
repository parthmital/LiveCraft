//! Shared fixture XML and model-equivalence helpers for `livecraft-session` lossless `.als` tests.

use livecraft_engine::driver::{AudioDriver, DriverConfig, DriverError, NullDriver};
use livecraft_model::model::{Clip, ClipPayload, MidiNote, Project, Track};
use livecraft_session::DriverFactory;
use livecraft_time::Beats;
use std::path::PathBuf;

pub const RICH_UNKNOWN_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Ableton MajorVersion="5" MinorVersion="12.0_12117" SchemaChangeCount="10" Creator="LiveCraft Fixture 1.0" Revision="fixture-rev">
	<LiveSet>
		<NextPointeeId Value="5000" />
		<LomId Value="0" />
		<CustomUnknownRootTag KeepMe='{"raw": [1, 2, 3]}' />
		<!-- Preserve root comment -->
		<Tracks>
			<MidiTrack Id="1" SelectedToolPanel="7">
				<LomId Value="0" />
				<Name>
					<EffectiveName Value="1-MIDI" />
					<UserName Value="" />
					<Annotation Value="Lead channel" />
				</Name>
				<Color Value="2" />
				<AutomationEnvelopes>
					<Envelopes />
				</AutomationEnvelopes>
				<TrackGroupId Value="-1" />
				<TrackDelay Value="0" />
				<CustomTrackMeta Foo="bar" />
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
									<Manual Value="0" />
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
							<ClipSlot Id="1000">
								<LomId Value="0" />
								<ClipSlot>
									<Value />
								</ClipSlot>
								<HasStop Value="true" />
							</ClipSlot>
							<ClipSlot Id="1001">
								<LomId Value="0" />
								<ClipSlot>
									<Value />
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
								<CustomParam Attr='single-quoted' />
							</MxDeviceAudioEffect>
						</Devices>
					</DeviceChain>
				</DeviceChain>
			</MidiTrack>
			<AudioTrack Id="2" CustomTrackAttr='keep-single-quote'>
				<LomId Value="0" />
				<Name>
					<EffectiveName Value="2-Audio" />
					<UserName Value="" />
				</Name>
				<Color Value="10" />
				<TrackGroupId Value="-1" />
				<AudioSpecificExtra Tag="untouched" />
				<DeviceChain>
					<Mixer>
						<Sends>
							<TrackSendHolder Id="0">
								<Send>
									<Manual Value="0" />
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
					<MainSequencer>
						<ClipSlotList>
							<ClipSlot Id="2000">
								<LomId Value="0" />
								<ClipSlot>
									<Value />
								</ClipSlot>
								<HasStop Value="true" />
							</ClipSlot>
						</ClipSlotList>
						<MonitoringEnum Value="1" />
					</MainSequencer>
					<DeviceChain>
						<Devices />
					</DeviceChain>
				</DeviceChain>
			</AudioTrack>
			<ReturnTrack Id="5">
				<Name>
					<EffectiveName Value="A-Reverb" />
					<UserName Value="" />
				</Name>
				<Color Value="14" />
				<TrackGroupId Value="-1" />
				<DeviceChain>
					<Mixer>
						<Sends>
							<TrackSendHolder Id="0">
								<Send>
									<Manual Value="0" />
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
			<Color Value="-1" />
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

pub fn temp_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("lossless_als")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create test temp dir");
    dir
}

pub fn null_factory() -> DriverFactory {
    Box::new(
        |config: &DriverConfig| -> Result<Box<dyn AudioDriver>, DriverError> {
            Ok(Box::new(NullDriver::new(
                config.sample_rate.unwrap_or(44_100),
                config.buffer_frames.unwrap_or(512),
                2,
            )))
        },
    )
}

pub fn assert_approx_f32(a: f32, b: f32, tol: f32, ctx: &str) {
    assert!(
        (a - b).abs() <= tol,
        "{ctx}: expected {a}, got {b} (diff {})",
        (a - b).abs()
    );
}

pub fn assert_approx_f64(a: f64, b: f64, tol: f64, ctx: &str) {
    assert!(
        (a - b).abs() <= tol,
        "{ctx}: expected {a}, got {b} (diff {})",
        (a - b).abs()
    );
}

pub fn assert_clips_equivalent(a: &Clip, b: &Clip) {
    assert_eq!(a.id, b.id, "clip id");
    assert_eq!(a.name, b.name, "clip name");
    assert_eq!(a.color, b.color, "clip color");
    assert_approx_f64(a.length_beats, b.length_beats, 1e-6, "clip length");
    assert_approx_f64(a.loop_start_beat, b.loop_start_beat, 1e-6, "loop start");
    assert_approx_f64(a.loop_end_beat, b.loop_end_beat, 1e-6, "loop end");
    assert_eq!(a.is_looping, b.is_looping, "is_looping");
    assert_approx_f64(
        a.start_marker_beat,
        b.start_marker_beat,
        1e-6,
        "start marker",
    );
    assert_approx_f64(a.end_marker_beat, b.end_marker_beat, 1e-6, "end marker");

    match (&a.payload, &b.payload) {
        (ClipPayload::Midi(ma), ClipPayload::Midi(mb)) => {
            let mut na = ma.notes.clone();
            let mut nb = mb.notes.clone();
            let key = |n: &MidiNote| {
                (
                    n.pitch,
                    (n.start_beat * 1e6) as i64,
                    (n.duration_beats * 1e6) as i64,
                )
            };
            na.sort_by_key(key);
            nb.sort_by_key(key);
            assert_eq!(na.len(), nb.len(), "note count");
            for (x, y) in na.iter().zip(&nb) {
                assert_eq!(x.pitch, y.pitch);
                assert_approx_f64(x.start_beat, y.start_beat, 1e-6, "note start");
                assert_approx_f64(x.duration_beats, y.duration_beats, 1e-6, "note dur");
                assert_approx_f32(x.velocity, y.velocity, 1e-4, "note vel");
                assert_approx_f32(x.release_velocity, y.release_velocity, 1e-4, "note rel vel");
                assert_approx_f32(x.probability, y.probability, 1e-4, "note prob");
                assert_approx_f32(
                    x.velocity_deviation,
                    y.velocity_deviation,
                    1e-4,
                    "note vel dev",
                );
                assert_eq!(x.mute, y.mute);
            }
        }
        (ClipPayload::Audio(aa), ClipPayload::Audio(ab)) => {
            assert_eq!(aa.file_path, ab.file_path);
        }
        _ => panic!("mismatched clip payload types"),
    }
}

pub fn assert_tracks_equivalent(a: &Track, b: &Track) {
    assert_eq!(a.id, b.id, "track id");
    assert_eq!(a.name, b.name, "track name");
    assert_eq!(a.track_type, b.track_type, "track type");
    assert_eq!(a.parent, b.parent, "track parent");
    assert_eq!(a.color, b.color, "track color");
    assert_approx_f32(a.volume_db, b.volume_db, 1e-3, "track volume");
    assert_approx_f32(a.pan, b.pan, 1e-4, "track pan");
    assert_eq!(a.is_muted, b.is_muted, "track muted");
    assert_eq!(a.is_soloed, b.is_soloed, "track soloed");
    assert_eq!(a.is_armed, b.is_armed, "track armed");
    assert_eq!(a.monitoring, b.monitoring, "track monitoring");
    assert_eq!(a.sends.len(), b.sends.len(), "track sends len");
    for (i, (&x, &y)) in a.sends.iter().zip(&b.sends).enumerate() {
        assert_approx_f32(x, y, 1e-4, &format!("send[{i}]"));
    }
    assert_eq!(a.clip_slots.len(), b.clip_slots.len(), "slots len");
    for (sa, sb) in a.clip_slots.iter().zip(&b.clip_slots) {
        assert_eq!(sa.id, sb.id);
        assert_eq!(sa.has_stop_button, sb.has_stop_button);
        match (&sa.clip, &sb.clip) {
            (None, None) => {}
            (Some(ca), Some(cb)) => assert_clips_equivalent(ca, cb),
            _ => panic!("slot clip presence mismatch on track {:?}", a.id),
        }
    }
    assert_eq!(a.device_chain.len(), b.device_chain.len(), "devices len");
    for (da, db) in a.device_chain.iter().zip(&b.device_chain) {
        assert_eq!(da.id, db.id);
        assert_eq!(da.name, db.name);
        assert_eq!(da.device_type, db.device_type);
        assert_eq!(da.is_enabled, db.is_enabled);
        let pa: Vec<_> = da.named_params().collect();
        let pb: Vec<_> = db.named_params().collect();
        assert_eq!(pa.len(), pb.len(), "param count for {:?}", da.device_type);
        for ((ka, va), (kb, vb)) in pa.iter().zip(&pb) {
            assert_eq!(ka, kb);
            assert_approx_f32(*va, *vb, 1e-4, ka);
        }
    }
}

pub fn assert_projects_equivalent(expected: &Project, actual: &Project) {
    for beat in [0.0, 4.0, 8.0] {
        assert_approx_f64(
            expected.transport.tempo.bpm_at(Beats(beat)),
            actual.transport.tempo.bpm_at(Beats(beat)),
            1e-5,
            "tempo bpm_at",
        );
    }
    for bar in 0..=4 {
        assert_eq!(
            expected
                .transport
                .time_signature
                .signature_at(expected.transport.time_signature.bar_start(bar)),
            actual
                .transport
                .time_signature
                .signature_at(actual.transport.time_signature.bar_start(bar)),
        );
    }
    assert_eq!(
        expected.transport.loop_enabled,
        actual.transport.loop_enabled
    );
    assert_approx_f64(
        expected.transport.loop_start_beats,
        actual.transport.loop_start_beats,
        1e-6,
        "loop start",
    );
    assert_approx_f64(
        expected.transport.loop_length_beats,
        actual.transport.loop_length_beats,
        1e-6,
        "loop len",
    );
    assert_eq!(
        expected.transport.metronome_enabled,
        actual.transport.metronome_enabled
    );
    assert_eq!(expected.scale, actual.scale);

    assert_eq!(expected.scenes.len(), actual.scenes.len());
    for (sa, sb) in expected.scenes.iter().zip(&actual.scenes) {
        assert_eq!(sa.id, sb.id);
        assert_eq!(sa.name, sb.name);
        assert_eq!(sa.color, sb.color);
        assert_eq!(sa.time_signature, sb.time_signature);
        match (sa.tempo, sb.tempo) {
            (None, None) => {}
            (Some(ta), Some(tb)) => assert_approx_f32(ta, tb, 1e-4, "scene tempo"),
            _ => panic!("scene tempo mismatch"),
        }
    }

    assert_eq!(expected.tracks.len(), actual.tracks.len());
    for (ta, tb) in expected.tracks.iter().zip(&actual.tracks) {
        assert_tracks_equivalent(ta, tb);
    }
    assert_eq!(expected.return_tracks.len(), actual.return_tracks.len());
    for (ra, rb) in expected.return_tracks.iter().zip(&actual.return_tracks) {
        assert_tracks_equivalent(ra, rb);
    }
    assert_tracks_equivalent(&expected.main_track, &actual.main_track);
}
