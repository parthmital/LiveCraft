//! The UI-independent app core (M04): named, serialisable edits with undo.
//!
//! Every change to a Live Set is an `Operation`. The GUI, the CLI, and later the control server
//! dispatch operations by name, and the `UndoHistory` records them so each one can be undone.

mod audio_setup;
pub mod command;
pub mod commands;
mod engine_sync;
pub mod history;
pub mod preferences;
mod project_ops;
pub mod registry;
pub mod session;
pub mod transport;

pub use command::{Command, CommandError, Operation};
pub use commands::clip::{ReplaceClipSample, SetClipRamMode};
pub use commands::project::ReplaceSampleReference;
pub use history::{HistoryEntry, MERGE_GAP, OPENING_LABEL, Transaction, UndoHistory};
pub use livecraft_engine::driver::{DeviceInfo, DeviceList, HostInfo, Latency};
pub use livecraft_engine::midi_io::MidiPorts;
pub use livecraft_formats::als::{AlsDocument, AlsError};
pub use livecraft_formats::analysis_cache::{
    AnalysisCacheLocation, CachedSampleAnalysis, PeakFrame, WaveformPyramid,
};
pub use livecraft_formats::audio_decode::{AudioDecodeError, AudioInfo, probe_audio_info};
pub use livecraft_formats::health::{
    ExternalFile, HealthCheckOptions, HealthError, MissingPlugin, ProjectHealthReport,
    SampleRateMismatch,
};
pub use livecraft_formats::pack::{PackError, PackInstallReport};
pub use livecraft_formats::project_dir::{
    AutoRepairReport, AutomaticSearchOptions, ClipLocationRef, CollectAllOptions,
    CollectFilesOnExport, CollectReport, CollectedFile, MissingFile, ProjectContext, ProjectDir,
    ProjectDirError, SampleLocation, SampleReference, SampleResolution, SavedSetLocation,
    SearchCandidateOutcome, UnusedFileCategory, UnusedFileEntry, UnusedFilesReport,
    automatic_search,
};
pub use preferences::{
    AudioPreferences, LibraryPreferences, MidiPortPreference, MidiPreferences, Preferences,
    PreferencesError,
};
pub use registry::CommandRegistry;
pub use session::{
    AUDIO_RETRY, DriverFactory, EngineStatus, ExportClipOutcome, Session, SessionAudioError,
    SessionFormatError, TrackMeter, demo_set, load_set,
};
