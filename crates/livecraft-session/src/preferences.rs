//! User preferences stored as TOML in the platform config folder (M06 item 1).
//!
//! The file is `LiveCraft/Preferences.toml` under the user's config directory, never inside the
//! repository. A missing file means the defaults. Unknown keys and missing sections are ignored,
//! so a file written by another build still loads.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// The settings that survive a restart.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct Preferences {
    /// Audio driver, devices, sample rate, and buffer size.
    pub audio: AudioPreferences,
    /// MIDI input and output ports and their switches.
    pub midi: MidiPreferences,
}

/// Audio settings. `None` means the system default.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct AudioPreferences {
    /// The host (driver type) name, such as `WASAPI` or `ALSA`.
    pub driver_type: Option<String>,
    /// The input device name.
    pub input_device: Option<String>,
    /// The output device name.
    pub output_device: Option<String>,
    /// The sample rate in hertz.
    pub sample_rate: Option<u32>,
    /// The buffer size in frames.
    pub buffer_size: Option<u32>,
}

/// MIDI input and output ports, each with its switches.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct MidiPreferences {
    /// The MIDI input ports.
    pub inputs: Vec<MidiPortPreference>,
    /// The MIDI output ports.
    pub outputs: Vec<MidiPortPreference>,
}

/// One MIDI port and the switches of Live's MIDI Ports table (manual 2.3).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct MidiPortPreference {
    /// The port name as the operating system reports it.
    pub name: String,
    /// Track: the port feeds the armed or monitoring MIDI tracks.
    pub track: bool,
    /// Sync: the port sends or receives MIDI clock.
    pub sync: bool,
    /// Remote: the port receives remote control messages.
    pub remote: bool,
}

/// Why a preferences file could not be read or written.
#[derive(Debug, thiserror::Error)]
pub enum PreferencesError {
    /// The file could not be read or written, or its folder could not be created.
    #[error("cannot read or write the preferences file: {0}")]
    Io(#[from] io::Error),
    /// The file is not valid TOML for these preferences.
    #[error("the preferences file is not valid: {0}")]
    Parse(#[from] toml::de::Error),
    /// The preferences could not be written as TOML.
    #[error("cannot write the preferences as TOML: {0}")]
    Serialize(#[from] toml::ser::Error),
}

impl Preferences {
    /// The file the app reads at start and writes after each change, or `None` when the system
    /// has no home folder.
    pub fn default_path() -> Option<PathBuf> {
        let dirs = directories::BaseDirs::new()?;
        Some(dirs.config_dir().join("LiveCraft").join("Preferences.toml"))
    }

    /// Reads the preferences at `path`. A missing file gives the defaults. An unreadable or
    /// invalid file is an error, so the caller can keep its own settings and report the problem.
    pub fn load(path: &Path) -> Result<Preferences, PreferencesError> {
        match fs::read_to_string(path) {
            Ok(text) => Ok(toml::from_str(&text)?),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Preferences::default()),
            Err(error) => Err(error.into()),
        }
    }

    /// Writes the preferences to `path`, creating its folder. The text goes to a temporary file
    /// next to `path` first and then replaces `path`, so an interrupted save keeps the old file.
    pub fn save(&self, path: &Path) -> Result<(), PreferencesError> {
        let text = toml::to_string(self)?;
        if let Some(folder) = path.parent() {
            fs::create_dir_all(folder)?;
        }
        let temporary = path.with_extension("toml.tmp");
        {
            let mut file = fs::File::create(&temporary)?;
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
        }
        fs::rename(&temporary, path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A folder under the system temp directory, removed when the test ends.
    struct ScratchDir(PathBuf);

    impl ScratchDir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "livecraft-preferences-{}-{name}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&path);
            Self(path)
        }

        /// Writes `text` to `name` in this folder, creating the folder first.
        fn write(&self, name: &str, text: &str) -> PathBuf {
            fs::create_dir_all(&self.0).expect("create the test folder");
            let path = self.0.join(name);
            fs::write(&path, text).expect("write the test file");
            path
        }
    }

    impl Drop for ScratchDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn sample() -> Preferences {
        Preferences {
            audio: AudioPreferences {
                driver_type: Some("WASAPI".to_string()),
                input_device: None,
                output_device: Some("Speakers".to_string()),
                sample_rate: Some(48_000),
                buffer_size: Some(128),
            },
            midi: MidiPreferences {
                inputs: vec![MidiPortPreference {
                    name: "Keyboard".to_string(),
                    track: true,
                    sync: false,
                    remote: true,
                }],
                outputs: vec![MidiPortPreference {
                    name: "Synth".to_string(),
                    track: false,
                    sync: true,
                    remote: false,
                }],
            },
        }
    }

    #[test]
    fn a_missing_file_loads_the_defaults() {
        let dir = ScratchDir::new("missing");
        let path = dir.0.join("Preferences.toml");
        let loaded = Preferences::load(&path).expect("a missing file is not an error");
        assert_eq!(loaded, Preferences::default());
    }

    #[test]
    fn preferences_survive_a_save_and_load() {
        let dir = ScratchDir::new("round-trip");
        let path = dir.0.join("Preferences.toml");
        sample().save(&path).expect("save the preferences");
        let loaded = Preferences::load(&path).expect("load the preferences");
        assert_eq!(loaded, sample());
    }

    #[test]
    fn invalid_toml_is_a_parse_error() {
        let dir = ScratchDir::new("invalid");
        let path = dir.write("Preferences.toml", "[audio\nsample_rate = ");
        assert!(matches!(
            Preferences::load(&path),
            Err(PreferencesError::Parse(_))
        ));
    }

    #[test]
    fn unknown_keys_and_missing_sections_still_load() {
        let dir = ScratchDir::new("forward");
        let path = dir.write(
            "Preferences.toml",
            "future_setting = true\n\n[audio]\nbuffer_size = 128\nfuture_key = \"x\"\n",
        );
        let loaded = Preferences::load(&path).expect("load the preferences");
        assert_eq!(loaded.audio.buffer_size, Some(128));
        assert_eq!(loaded.midi, MidiPreferences::default());
    }

    #[test]
    fn save_creates_the_missing_folder() {
        let dir = ScratchDir::new("folder");
        let path = dir
            .0
            .join("nested")
            .join("LiveCraft")
            .join("Preferences.toml");
        sample().save(&path).expect("save the preferences");
        assert!(path.is_file());
    }

    #[test]
    fn save_replaces_an_existing_file_and_leaves_no_temporary_file() {
        let dir = ScratchDir::new("replace");
        let path = dir.write("Preferences.toml", "[audio]\nbuffer_size = 512\n");
        sample().save(&path).expect("save the preferences");
        let loaded = Preferences::load(&path).expect("load the preferences");
        assert_eq!(loaded, sample());
        assert!(!dir.0.join("Preferences.toml.tmp").exists());
    }

    #[test]
    fn the_default_path_is_preferences_toml_in_a_livecraft_folder() {
        let path = Preferences::default_path().expect("a home folder on this machine");
        assert!(path.ends_with(Path::new("LiveCraft").join("Preferences.toml")));
    }
}
