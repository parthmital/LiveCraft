//! The `command` subcommands run registered commands on a Live Set (M04 item 10).

use livecraft_formats::als::AlsParser;
use livecraft_model::ids::TrackId;
use livecraft_model::model::Project;
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
