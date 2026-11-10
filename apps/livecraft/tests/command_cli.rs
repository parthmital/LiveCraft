//! The `command` subcommands run registered commands on a Live Set (M04 item 10).

use livecraft_formats::als::read::decompress_als_or_xml;
use livecraft_formats::als::{AlsDocument, AlsParser};
use livecraft_model::ids::TrackId;
use livecraft_model::model::{DeviceType, Project};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn livecraft(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_livecraft"))
        .args(args)
        .output()
        .expect("run the livecraft binary")
}

/// Writes a default Set to a test folder and returns the folder and the Set's path.
fn input_set(folder: &str) -> (PathBuf, PathBuf) {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(folder);
    std::fs::create_dir_all(&dir).expect("create the test folder");
    let input = dir.join("input.als");
    AlsParser::save_project_to_als(&Project::new_default("Input"), &input).expect("write input");
    (dir, input)
}

fn path_str(path: &Path) -> &str {
    path.to_str().expect("test paths are valid UTF-8")
}

#[test]
fn command_list_prints_every_registered_name() {
    let output = livecraft(&["command", "list"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let printed: Vec<&str> = stdout.lines().collect();
    let registered: Vec<&str> = livecraft_session::commands::registry().names().collect();
    assert_eq!(printed, registered);
    assert!(printed.contains(&"track.set_volume"));
}

#[test]
fn command_run_applies_the_command_and_saves_the_set() {
    let (dir, input) = input_set("command_run");
    let output_path = dir.join("output.als");
    let run = livecraft(&[
        "command",
        "run",
        path_str(&input),
        "track.set_volume",
        r#"{"track": 1, "volume_db": -6.0}"#,
        "-o",
        path_str(&output_path),
    ]);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let saved = AlsParser::load_project(&output_path).expect("read the output Set");
    assert_eq!(saved.track(TrackId(1)).map(|t| t.volume_db), Some(-6.0));
}

#[test]
fn command_run_names_an_unknown_command_in_its_error() {
    let (dir, input) = input_set("command_unknown");
    let output_path = dir.join("unused.als");
    let run = livecraft(&[
        "command",
        "run",
        path_str(&input),
        "track.no_such_command",
        "{}",
        "-o",
        path_str(&output_path),
    ]);
    assert!(!run.status.success());
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(stderr.contains("track.no_such_command"), "stderr: {stderr}");
    assert!(
        !output_path.exists(),
        "no output is written for a failed command"
    );
}

#[test]
fn command_run_preserves_unknown_xml_and_devices() {
    let fixture_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Ableton MajorVersion="5" MinorVersion="12.0_12117" SchemaChangeCount="10" Creator="LiveCraft Fixture" Revision="keep-rev">
	<LiveSet>
		<NextPointeeId Value="4000" />
		<LomId Value="0" />
		<CustomUnknownRoot KeepMe='{"raw": [1, 2, 3]}' />
		<!-- Root comment must survive -->
		<Tracks>
			<MidiTrack Id="1" SelectedToolPanel="7">
				<LomId Value="0" />
				<Name>
					<EffectiveName Value="1-MIDI" />
					<UserName Value="" />
				</Name>
				<Color Value="19" />
				<TrackGroupId Value="-1" />
				<TrackDelay Value="0" />
				<DeviceChain>
					<Mixer>
						<Sends />
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
							<ClipSlot Id="0">
								<ClipSlot>
									<Value />
								</ClipSlot>
								<HasStop Value="true" />
							</ClipSlot>
						</ClipSlotList>
					</MainSequencer>
					<DeviceChain>
						<Devices>
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
		</Tracks>
		<MainTrack>
			<Name>
				<EffectiveName Value="Main" />
				<UserName Value="" />
			</Name>
			<Color Value="4" />
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
					</Tempo>
					<TimeSignature>
						<Manual Value="201" />
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
			</Scene>
		</Scenes>
		<Transport>
			<LoopOn Value="false" />
			<LoopStart Value="0" />
			<LoopLength Value="16" />
			<CurrentTime Value="0" />
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

    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("command_run_lossless");
    std::fs::create_dir_all(&dir).expect("create test folder");
    let input = dir.join("input_unknown.als");
    let output_path = dir.join("output_unknown.als");

    AlsDocument::from_xml(fixture_xml)
        .expect("parse fixture")
        .save_als(&input)
        .expect("save input .als");

    let run = livecraft(&[
        "command",
        "run",
        path_str(&input),
        "track.set_volume",
        r#"{"track": 1, "volume_db": -6.0}"#,
        "-o",
        path_str(&output_path),
    ]);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );

    let saved_xml = decompress_als_or_xml(&output_path).expect("decompress output .als");
    assert!(saved_xml.contains(r#"Revision="keep-rev""#));
    assert!(saved_xml.contains(r#"SelectedToolPanel="7""#));
    assert!(saved_xml.contains(r#"<CustomUnknownRoot KeepMe='{"raw": [1, 2, 3]}' />"#));
    assert!(saved_xml.contains("<!-- Root comment must survive -->"));
    assert!(saved_xml.contains(r#"<TrackDelay Value="0" />"#));
    assert!(saved_xml.contains(r#"<MxDeviceAudioEffect Id="301">"#));
    assert!(
        saved_xml.contains("<PatcherBlob><![CDATA[<maxpat>{opaque_data}</maxpat>]]></PatcherBlob>")
    );

    let reopened = AlsDocument::open(&output_path)
        .expect("reopen output .als")
        .into_project();
    let track = reopened.track(TrackId(1)).expect("track 1 exists");
    assert!((track.volume_db - (-6.0)).abs() < 1e-3);
    assert_eq!(track.device_chain.len(), 1);
    assert_eq!(
        track.device_chain[0].device_type,
        DeviceType::Custom("MxDeviceAudioEffect".to_owned())
    );
    assert_eq!(track.device_chain[0].name, "M4L Shimmer");
}
