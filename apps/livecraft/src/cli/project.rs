use std::fs;
use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};
use livecraft_formats::als::{AlsDocument, AlsParser};
use livecraft_formats::health::{
    HealthCheckOptions, check_project_health, format_health_report, inspect_set_health,
};
use livecraft_formats::live_clip::{LiveClipFile, save_live_clip_in_context};
use livecraft_formats::pack::install_pack;
use livecraft_formats::project_dir::{
    CollectAllOptions, CollectReport, UnusedFilesReport, find_unused_files, save_set_into_project,
};
use livecraft_model::ids::ClipId;
use livecraft_model::model::Project;
use livecraft_session::Session;

use super::convert::{is_json_path, load_project_from_path};
use super::render::resolve_single_track;

#[derive(Args, Debug)]
pub struct HealthArgs {
    /// Path to .als, .xml, or .json Live Set
    pub set: PathBuf,
    /// Print the health report as JSON
    #[arg(long)]
    pub json: bool,
    /// Expected sample rate in Hz for mismatch checks
    #[arg(long, value_name = "HZ")]
    pub sample_rate: Option<u32>,
    /// User Library root directory for sample classification
    #[arg(long, value_name = "DIR")]
    pub user_library: Option<PathBuf>,
    /// Exit with a non-zero status code if any health issues are found
    #[arg(long)]
    pub strict: bool,
}

#[derive(Args, Debug)]
pub struct CollectArgs {
    /// Path to .als Live Set inside a Live Project folder
    pub set: PathBuf,
    /// Also copy samples located in the User Library
    #[arg(long)]
    pub user_library: bool,
    /// Also copy samples located in Factory Packs
    #[arg(long)]
    pub factory_packs: bool,
    /// Copy samples from all external sources (including User Library and Factory Packs)
    #[arg(long)]
    pub all: bool,
    /// User Library root directory for sample classification
    #[arg(long, value_name = "DIR")]
    pub user_lib_dir: Option<PathBuf>,
    /// Print the collection report as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct UnusedArgs {
    /// Path to a Live Project folder (containing Ableton Project Info)
    pub project_dir: PathBuf,
    /// Print the unused files report as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Subcommand, Debug)]
pub enum PackAction {
    /// Extract an Ableton Live Pack (.alp) into a destination directory
    Install(PackInstallArgs),
}

#[derive(Args, Debug)]
pub struct PackInstallArgs {
    /// Path to .alp Live Pack archive
    pub pack: PathBuf,
    /// Destination directory where Pack contents will be extracted
    pub dest_dir: PathBuf,
    /// Print the installation report as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Subcommand, Debug)]
pub enum MidiAction {
    /// Import a Standard MIDI File (.mid) into a Live Set
    Import(MidiImportArgs),
    /// Export a MIDI clip from a Live Set as a Standard MIDI File (.mid)
    Export(MidiExportArgs),
}

#[derive(Args, Debug)]
pub struct MidiImportArgs {
    /// Input .als, .xml, or .json Live Set
    pub set: PathBuf,
    /// Standard MIDI File (.mid) to import
    pub mid: PathBuf,
    /// Target MIDI track numeric ID or name (creates a new MIDI track when omitted)
    #[arg(long, value_name = "ID")]
    pub track: Option<String>,
    /// Zero-based Session clip slot index
    #[arg(long, value_name = "IDX", conflicts_with = "beat")]
    pub slot: Option<usize>,
    /// Arrangement start position in quarter-note beats
    #[arg(long, value_name = "POS", conflicts_with = "slot")]
    pub beat: Option<f64>,
    /// Output .als Live Set path
    #[arg(short, long, value_name = "OUT")]
    pub out: PathBuf,
}

#[derive(Args, Debug)]
pub struct MidiExportArgs {
    /// Input .als, .xml, or .json Live Set
    pub set: PathBuf,
    /// Numeric clip ID to export
    #[arg(long, value_name = "ID")]
    pub clip: u64,
    /// Output Standard MIDI File (.mid) path
    #[arg(short, long, value_name = "OUT")]
    pub out: PathBuf,
}

#[derive(Subcommand, Debug)]
pub enum ClipAction {
    /// Import an Ableton Live Clip (.alc) into a Live Set
    ImportAlc(ClipImportAlcArgs),
    /// Export a clip and its track device chain as an Ableton Live Clip (.alc)
    ExportAlc(ClipExportAlcArgs),
}

#[derive(Args, Debug)]
pub struct ClipImportAlcArgs {
    /// Input .als, .xml, or .json Live Set
    pub set: PathBuf,
    /// Ableton Live Clip (.alc) file to import
    pub alc: PathBuf,
    /// Target track numeric ID or name (creates a new track when omitted)
    #[arg(long, value_name = "ID")]
    pub track: Option<String>,
    /// Zero-based Session clip slot index
    #[arg(long, value_name = "IDX")]
    pub slot: Option<usize>,
    /// Output .als Live Set path
    #[arg(short, long, value_name = "OUT")]
    pub out: PathBuf,
}

#[derive(Args, Debug)]
pub struct ClipExportAlcArgs {
    /// Input .als, .xml, or .json Live Set
    pub set: PathBuf,
    /// Track numeric ID or name holding the clip
    #[arg(long, value_name = "ID")]
    pub track: String,
    /// Numeric clip ID to export
    #[arg(long, value_name = "ID")]
    pub clip: u64,
    /// Output Ableton Live Clip (.alc) path
    #[arg(short, long, value_name = "OUT")]
    pub out: PathBuf,
    /// Copy dependent audio sample into <project-dir>/Samples/Imported/
    #[arg(long, value_name = "DIR")]
    pub copy_into: Option<PathBuf>,
}

/// Creates either a standalone `.als`/`.json` Set or a `<Name> Project/<Name>.als` folder.
pub fn run_new(path: &Path, bpm: f32, as_project: bool) -> anyhow::Result<()> {
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("New Project");
    let mut proj = Project::new_default(name);
    proj.transport.set_bpm(bpm);

    if as_project {
        if is_json_path(path) {
            anyhow::bail!(
                "--project cannot be used with a .json path; pass a .als path or project name"
            );
        }
        let doc = AlsDocument::from_project(&proj)
            .map_err(|e| anyhow::anyhow!("ALS build error: {e}"))?;
        let saved = save_set_into_project(&doc, path)?;
        println!(
            "Created new Live Project '{}' with set: {}",
            saved.project_dir.root.display(),
            saved.set_path.display()
        );
        return Ok(());
    }

    if is_json_path(path) {
        let json = serde_json::to_string_pretty(&proj)?;
        fs::write(path, json)?;
        println!("Created new LiveCraft JSON project: {}", path.display());
        return Ok(());
    }

    AlsParser::save_project_to_als(&proj, path)?;
    println!("Created new Ableton Live set: {}", path.display());
    Ok(())
}

/// Runs the one-pass B11 project health check on a Live Set.
pub fn run_health(args: HealthArgs) -> anyhow::Result<()> {
    let options = HealthCheckOptions {
        set_path: Some(args.set.clone()),
        expected_sample_rate: args.sample_rate,
        user_library: args.user_library,
        ..HealthCheckOptions::default()
    };
    let report = if is_json_path(&args.set) {
        let project = load_project_from_path(&args.set)?;
        check_project_health(&project, &options)?
    } else {
        inspect_set_health(&args.set, &options)?
    };
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("{}", format_health_report(&report));
    }
    if args.strict && !report.is_healthy() {
        anyhow::bail!(
            "project health check found {} issue(s)",
            report.issue_count()
        );
    }
    Ok(())
}

/// Runs Collect All and Save (manual 5.7) on a `.als` Live Set inside a Live Project folder.
pub fn run_collect(args: CollectArgs) -> anyhow::Result<()> {
    if !args.set.exists() {
        anyhow::bail!("File not found: {}", args.set.display());
    }
    let mut session =
        Session::open_als(&args.set).map_err(|e| anyhow::anyhow!("ALS load error: {e}"))?;
    if let Some(ref user_lib_dir) = args.user_lib_dir {
        let mut prefs = session.preferences().library.clone();
        prefs.user_library = Some(user_lib_dir.clone());
        session.set_library_preferences(prefs)?;
    }
    let opts = if args.all {
        CollectAllOptions::all()
    } else {
        CollectAllOptions {
            collect_user_library: args.user_library,
            collect_factory_packs: args.factory_packs,
            ..CollectAllOptions::default()
        }
    };
    let report = session.collect_all_and_save(&opts)?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_collect_report(&args.set, &report);
    }
    Ok(())
}

/// Scans a Live Project folder for audio files unreferenced by any `.als`/`.alc`/`.adv`/`.adg`.
pub fn run_unused(args: UnusedArgs) -> anyhow::Result<()> {
    let report = find_unused_files(&args.project_dir)?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_unused_report(&report);
    }
    Ok(())
}

/// Extracts an Ableton Live Pack (`.alp`) with zip-slip and size protection.
pub fn run_pack_install(args: PackInstallArgs) -> anyhow::Result<()> {
    let report = install_pack(&args.pack, &args.dest_dir)?;
    if args.json {
        let payload = serde_json::json!({
            "extracted_files": report.extracted_files,
            "sets_found": report.sets_found,
            "total_bytes": report.total_bytes,
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!(
            "Installed Pack '{}' into '{}' ({} file(s), {} bytes)",
            args.pack.display(),
            args.dest_dir.display(),
            report.extracted_files.len(),
            report.total_bytes
        );
        if !report.sets_found.is_empty() {
            println!("Live Sets found ({}):", report.sets_found.len());
            for set_path in &report.sets_found {
                println!("  - {}", set_path.display());
            }
        }
    }
    Ok(())
}

/// Imports a `.mid` file into a Live Set and saves the updated `.als`.
pub fn run_midi_import(args: MidiImportArgs) -> anyhow::Result<()> {
    let mut session = open_session_from_path(&args.set)?;
    let track_id = args
        .track
        .as_deref()
        .map(|sel| resolve_single_track(session.project(), sel))
        .transpose()?;
    let clip_id = session.import_midi_file(track_id, args.slot, args.beat, &args.mid)?;
    session
        .save(&args.out)
        .map_err(|e| anyhow::anyhow!("ALS save error: {e}"))?;
    println!(
        "Imported MIDI '{}' as clip {} and saved {}",
        args.mid.display(),
        clip_id,
        args.out.display()
    );
    Ok(())
}

/// Exports a MIDI clip from a Live Set to a `.mid` Standard MIDI File.
pub fn run_midi_export(args: MidiExportArgs) -> anyhow::Result<()> {
    let session = open_session_from_path(&args.set)?;
    let clip_id = ClipId(args.clip);
    session.export_midi_clip(clip_id, &args.out)?;
    println!("Exported MIDI clip {} to {}", clip_id, args.out.display());
    Ok(())
}

/// Imports an Ableton Live Clip (`.alc`) into a Live Set and saves the updated `.als`.
pub fn run_clip_import_alc(args: ClipImportAlcArgs) -> anyhow::Result<()> {
    let mut session = open_session_from_path(&args.set)?;
    let track_id = args
        .track
        .as_deref()
        .map(|sel| resolve_single_track(session.project(), sel))
        .transpose()?;
    let clip_id = session.import_live_clip(track_id, args.slot, None, &args.alc)?;
    session
        .save(&args.out)
        .map_err(|e| anyhow::anyhow!("ALS save error: {e}"))?;
    println!(
        "Imported Live Clip '{}' as clip {} and saved {}",
        args.alc.display(),
        clip_id,
        args.out.display()
    );
    Ok(())
}

/// Exports a clip and its track device chain as an Ableton Live Clip (`.alc`).
pub fn run_clip_export_alc(args: ClipExportAlcArgs) -> anyhow::Result<()> {
    let session = open_session_from_path(&args.set)?;
    let project = session.project();
    let track_id = resolve_single_track(project, &args.track)?;
    let track = project
        .track(track_id)
        .ok_or_else(|| anyhow::anyhow!("Track {} not found", track_id))?;
    let clip_id = ClipId(args.clip);
    let clip = track
        .clip_slots
        .iter()
        .filter_map(|slot| slot.clip.as_ref())
        .chain(track.arrangement_clips.iter().map(|arr| &arr.clip))
        .find(|clip| clip.id == clip_id)
        .ok_or_else(|| anyhow::anyhow!("Clip {} not found on track {}", clip_id, track_id))?;
    let live_clip = LiveClipFile {
        clip: clip.clone(),
        devices: track.device_chain.clone(),
    };
    save_live_clip_in_context(
        &live_clip,
        &args.out,
        args.copy_into.as_deref(),
        Some(session.project_context()),
    )
    .map_err(|e| anyhow::anyhow!("ALC save error: {e}"))?;
    println!(
        "Exported clip {} from track {} to {}",
        clip_id,
        track_id,
        args.out.display()
    );
    Ok(())
}

fn open_session_from_path(path: &Path) -> anyhow::Result<Session> {
    if !path.exists() {
        anyhow::bail!("File not found: {}", path.display());
    }
    if is_json_path(path) {
        let project = load_project_from_path(path)?;
        return Ok(Session::new(project, None));
    }
    Session::open_als(path).map_err(|e| anyhow::anyhow!("ALS load error: {e}"))
}

fn print_collect_report(set_path: &Path, report: &CollectReport) {
    println!(
        "Collect All and Save: {} ({} file(s) collected, {} reference(s) updated)",
        set_path.display(),
        report.collected.len(),
        report.updated_references
    );
    for item in &report.collected {
        let reused = if item.reused_existing {
            " [reused]"
        } else {
            ""
        };
        println!(
            "  + {} -> {}{}",
            item.original_path.display(),
            item.project_relative_path,
            reused
        );
    }
    if !report.skipped_by_policy.is_empty() {
        println!("Skipped by policy ({}):", report.skipped_by_policy.len());
        for (path, loc) in &report.skipped_by_policy {
            println!("  - {} ({:?})", path.display(), loc);
        }
    }
    if !report.missing.is_empty() {
        println!("Missing ({}):", report.missing.len());
        for m in &report.missing {
            println!("  ! {m}");
        }
    }
}

fn print_unused_report(report: &UnusedFilesReport) {
    println!(
        "Unused Files Report: {} ({} document(s) scanned)",
        report.project_root.display(),
        report.scanned_documents.len()
    );
    if report.unused_files.is_empty() {
        println!("No unused files found.");
        return;
    }
    println!("Unused files ({}):", report.unused_files.len());
    for entry in &report.unused_files {
        println!(
            "  - {} ({:?}, {} bytes)",
            entry.relative_path, entry.category, entry.size_bytes
        );
    }
}
