//! User preferences stored as TOML in the platform config folder (M06 item 1).
//!
//! The file is `LiveCraft/Preferences.toml` under the user's config directory, never inside the
//! repository. A missing file means the defaults. Unknown keys and missing sections are ignored,
//! so a file written by another build still loads.

use livecraft_engine::driver::DriverConfig;
use livecraft_formats::analysis_cache::AnalysisCacheLocation;
use livecraft_formats::project_dir::CollectFilesOnExport;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

fn default_true() -> bool {
    true
}

/// The settings that survive a restart.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct Preferences {
    /// Audio driver, devices, sample rate, and buffer size.
    pub audio: AudioPreferences,
    /// MIDI input and output ports and their switches.
    pub midi: MidiPreferences,
    /// User Library, Factory Packs roots, and clip-export sample collection policy.
    pub library: LibraryPreferences,
}

/// User Library, Factory Packs roots, analysis cache, and Live Clip export sample-copying policy (manual 5.1.2, 5.7.1).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(default)]
pub struct LibraryPreferences {
    /// Custom User Library folder. `None` uses the Documents/LiveCraft/User Library default.
    pub user_library: Option<PathBuf>,
    /// Roots searched and classified as Factory Packs.
    pub factory_packs_roots: Vec<PathBuf>,
    /// Whether exporting a `.alc` copies dependent audio files into the destination folder.
    pub collect_files_on_export: CollectFilesOnExport,
    /// Whether `.lcanalysis` waveform overview sidecar files are written automatically (manual 5.1.2).
    #[serde(default = "default_true")]
    pub create_analysis_files: bool,
    /// Optional central folder for `.lcanalysis` files. `None` writes beside each sample.
    #[serde(default)]
    pub analysis_cache_folder: Option<PathBuf>,
}

impl Default for LibraryPreferences {
    fn default() -> Self {
        Self {
            user_library: None,
            factory_packs_roots: Vec::new(),
            collect_files_on_export: CollectFilesOnExport::default(),
            create_analysis_files: true,
            analysis_cache_folder: None,
        }
    }
}

impl LibraryPreferences {
    /// Returns the configured User Library path, or falls back to
    /// `<Documents>/LiveCraft/User Library` when unset or empty.
    #[must_use]
    pub fn effective_user_library(&self) -> Option<PathBuf> {
        self.user_library
            .as_ref()
            .filter(|path| !path.as_os_str().is_empty())
            .cloned()
            .or_else(|| {
                directories::UserDirs::new().and_then(|u| {
                    u.document_dir()
                        .map(|d| d.join("LiveCraft").join("User Library"))
                })
            })
    }

    /// Returns where `.lcanalysis` waveform overview caches are stored (manual 5.1.2).
    #[must_use]
    pub fn analysis_cache_location(&self) -> AnalysisCacheLocation {
        match self
            .analysis_cache_folder
            .as_ref()
            .filter(|path| !path.as_os_str().is_empty())
        {
            Some(dir) => AnalysisCacheLocation::Folder(dir.clone()),
            None => AnalysisCacheLocation::BesideSample,
        }
    }
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

impl From<&AudioPreferences> for DriverConfig {
    /// The audio driver settings that the preferences name. The driver type is the host.
    fn from(audio: &AudioPreferences) -> Self {
        Self {
            host: audio.driver_type.clone(),
            output_device: audio.output_device.clone(),
            input_device: audio.input_device.clone(),
            sample_rate: audio.sample_rate,
            buffer_frames: audio.buffer_size,
            ..Self::default()
        }
    }
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

impl MidiPreferences {
    /// The names of the input ports that feed the engine: the ports whose Track switch is on (manual 2.3).
    /// With no MIDI preference yet, every port in `available` feeds it.
    pub fn input_ports_to_open(&self, available: &[String]) -> Vec<String> {
        if self.inputs.is_empty() {
            return available.to_vec();
        }
        self.inputs
            .iter()
            .filter(|port| port.track)
            .map(|port| port.name.clone())
            .collect()
    }

    /// The names of the output ports that receive the notes of tracks routed to them: the ports whose
    /// Track switch is on (manual 2.3). With no output preference, no port is opened, so no note goes to
    /// a device nobody chose.
    pub fn output_ports_to_open(&self) -> Vec<String> {
        self.outputs
            .iter()
            .filter(|port| port.track)
            .map(|port| port.name.clone())
            .collect()
    }
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
            library: LibraryPreferences {
                user_library: Some(PathBuf::from("D:/Samples/User Library")),
                factory_packs_roots: vec![PathBuf::from("D:/Packs")],
                collect_files_on_export: CollectFilesOnExport::Ask,
                create_analysis_files: false,
                analysis_cache_folder: Some(PathBuf::from("D:/AnalysisCache")),
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
    fn the_audio_preferences_become_the_driver_config_they_name() {
        let config = DriverConfig::from(&sample().audio);
        assert_eq!(config.host.as_deref(), Some("WASAPI"));
        assert_eq!(config.output_device.as_deref(), Some("Speakers"));
        assert_eq!(config.input_device, None);
        assert_eq!(config.sample_rate, Some(48_000));
        assert_eq!(config.buffer_frames, Some(128));
    }

    #[test]
    fn the_default_path_is_preferences_toml_in_a_livecraft_folder() {
        let path = Preferences::default_path().expect("a home folder on this machine");
        assert!(path.ends_with(Path::new("LiveCraft").join("Preferences.toml")));
    }

    #[test]
    fn with_no_midi_preference_every_available_input_feeds_the_engine() {
        let available = ["Keyboard".to_string(), "Pad".to_string()];
        let feeding = MidiPreferences::default().input_ports_to_open(&available);
        assert_eq!(feeding, available);
    }

    #[test]
    fn only_the_inputs_with_track_on_feed_the_engine() {
        let port = |name: &str, track: bool| MidiPortPreference {
            name: name.to_string(),
            track,
            ..MidiPortPreference::default()
        };
        let midi = MidiPreferences {
            inputs: vec![port("Keyboard", true), port("Pad", false)],
            outputs: Vec::new(),
        };
        let available = [
            "Keyboard".to_string(),
            "Pad".to_string(),
            "Synth".to_string(),
        ];
        assert_eq!(
            midi.input_ports_to_open(&available),
            vec!["Keyboard".to_string()]
        );
    }

    #[test]
    fn only_the_outputs_with_track_on_receive_notes_and_none_open_by_default() {
        assert!(MidiPreferences::default().output_ports_to_open().is_empty());
        let port = |name: &str, track: bool| MidiPortPreference {
            name: name.to_string(),
            track,
            ..MidiPortPreference::default()
        };
        let midi = MidiPreferences {
            inputs: Vec::new(),
            outputs: vec![port("Synth", true), port("Drum Machine", false)],
        };
        assert_eq!(midi.output_ports_to_open(), vec!["Synth".to_string()]);
    }

    #[test]
    fn existing_toml_without_library_section_loads_with_defaults() {
        let dir = ScratchDir::new("no-library");
        let path = dir.write(
            "Preferences.toml",
            "[audio]\ndriver_type = \"WASAPI\"\nbuffer_size = 256\n\n[[midi.inputs]]\nname = \"Key\"\ntrack = true\n",
        );
        let loaded = Preferences::load(&path).expect("load existing TOML");
        assert_eq!(loaded.audio.buffer_size, Some(256));
        assert_eq!(loaded.midi.inputs.len(), 1);
        assert_eq!(loaded.library, LibraryPreferences::default());
        assert_eq!(
            loaded.library.collect_files_on_export,
            CollectFilesOnExport::Always
        );
    }

    #[test]
    fn effective_user_library_uses_configured_path_or_documents_fallback() {
        let custom = LibraryPreferences {
            user_library: Some(PathBuf::from("E:/MyLibrary")),
            ..LibraryPreferences::default()
        };
        assert_eq!(
            custom.effective_user_library(),
            Some(PathBuf::from("E:/MyLibrary"))
        );

        let default_lib = LibraryPreferences::default();
        let expected_fallback = directories::UserDirs::new().and_then(|u| {
            u.document_dir()
                .map(|d| d.join("LiveCraft").join("User Library"))
        });
        assert_eq!(default_lib.effective_user_library(), expected_fallback);
    }

    #[test]
    fn library_section_omitting_analysis_fields_defaults_create_analysis_files_to_true() {
        let dir = ScratchDir::new("library-partial");
        let path = dir.write(
            "Preferences.toml",
            "[library]\nuser_library = \"D:/Samples\"\ncollect_files_on_export = \"Never\"\n",
        );
        let loaded = Preferences::load(&path).expect("load partial [library]");
        assert_eq!(
            loaded.library.user_library,
            Some(PathBuf::from("D:/Samples"))
        );
        assert_eq!(
            loaded.library.collect_files_on_export,
            CollectFilesOnExport::Never
        );
        assert!(loaded.library.create_analysis_files);
        assert_eq!(loaded.library.analysis_cache_folder, None);
        assert_eq!(
            loaded.library.analysis_cache_location(),
            AnalysisCacheLocation::BesideSample
        );
    }

    #[test]
    fn analysis_cache_location_uses_folder_when_non_empty_or_falls_back_beside_sample() {
        let folder_prefs = LibraryPreferences {
            create_analysis_files: false,
            analysis_cache_folder: Some(PathBuf::from("E:/Cache")),
            ..LibraryPreferences::default()
        };
        assert_eq!(
            folder_prefs.analysis_cache_location(),
            AnalysisCacheLocation::Folder(PathBuf::from("E:/Cache"))
        );

        let empty_folder_prefs = LibraryPreferences {
            analysis_cache_folder: Some(PathBuf::new()),
            ..LibraryPreferences::default()
        };
        assert_eq!(
            empty_folder_prefs.analysis_cache_location(),
            AnalysisCacheLocation::BesideSample
        );
    }
}
