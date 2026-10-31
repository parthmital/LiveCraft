use clap::{Parser, Subcommand, ValueEnum};
use livecraft_devices::operator::{OperatorSynth, Waveform};
use livecraft_engine::bouncer::{OfflineBouncer, RenderOptions};
use livecraft_engine::engine::AudioEngine;
use livecraft_formats::als::AlsParser;
use livecraft_model::model::{Clip, ClipPayload, MidiNote, Project, TrackType};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(
    name = "livecraft",
    about = "Native Rust Digital Audio Workstation (Ableton Live compatible)",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Quick launch GUI with an optional project
    #[arg(short, long)]
    project: Option<PathBuf>,

    /// Verbose logging output
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Create a new Ableton Live .als or .json project
    New {
        /// Target file path (e.g. project.als or project.json)
        path: PathBuf,
        /// Project tempo (BPM)
        #[arg(short, long, default_value_t = 120.0)]
        bpm: f32,
    },
    /// Print metadata summary of an Ableton project
    Info {
        /// Path to .als or .json project
        path: PathBuf,
    },
    /// Inspect tracks, devices, and clips in detail
    Inspect {
        /// Path to .als or .json project
        path: PathBuf,
        /// Show full details in JSON format
        #[arg(long)]
        json: bool,
        /// Show detailed device chain for each track
        #[arg(short, long)]
        devices: bool,
        /// Show clip details and notes
        #[arg(short, long)]
        clips: bool,
    },
    /// Render project to a WAV audio file
    Render {
        /// Path to .als or .json project
        path: PathBuf,
        /// Output WAV file path
        #[arg(short, long, default_value = "output.wav")]
        output: PathBuf,
        /// Duration to render in musical bars
        #[arg(short, long, default_value_t = 8)]
        duration_bars: u32,
        /// Sample rate in Hz (44100, 48000, 96000)
        #[arg(short, long, default_value_t = 44100)]
        sample_rate: u32,
        /// Bit depth (16, 24, 32)
        #[arg(short, long, default_value_t = 24)]
        bits: u16,
    },
    /// Export each track as an isolated stem WAV file
    Stems {
        /// Path to .als or .json project
        path: PathBuf,
        /// Output directory for stem files
        #[arg(short, long, default_value = "./stems")]
        output_dir: PathBuf,
        /// Duration to render in musical bars
        #[arg(short, long, default_value_t = 8)]
        duration_bars: u32,
    },
    /// Play project in terminal with real-time ASCII VU meters
    Play {
        /// Path to .als or .json project
        path: Option<PathBuf>,
        /// Playhead loop duration in seconds (0 for indefinite)
        #[arg(short, long, default_value_t = 0)]
        duration: u64,
    },
    /// Generate synth tone or phrase directly from CLI
    Synth {
        /// MIDI note number (60 = C3, 69 = A3 = 440 Hz)
        #[arg(short, long, default_value_t = 60)]
        note: u8,
        /// Oscillator waveform
        #[arg(short, long, value_enum, default_value_t = CliWaveform::Saw)]
        waveform: CliWaveform,
        /// Output WAV file path (optional, plays live if omitted)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Duration in seconds
        #[arg(short, long, default_value_t = 2.0)]
        duration_sec: f32,
    },
    /// Convert between Ableton .als (gzipped XML) and JSON
    Convert {
        /// Input file (.als or .json)
        input: PathBuf,
        /// Output file (.als or .json)
        output: PathBuf,
    },
    /// Launch the desktop interface
    Gui {
        /// Optional project file to open
        project: Option<PathBuf>,

        /// Path to save a PNG screenshot and exit
        #[arg(long)]
        screenshot: Option<PathBuf>,

        /// Initial view mode ("session" or "arrangement")
        #[arg(long)]
        view: Option<String>,

        /// Initial bottom detail tab (0 = Device View, 1 = Clip View)
        #[arg(long)]
        tab: Option<usize>,

        /// Color theme ("light" or "dark")
        #[arg(long)]
        theme: Option<String>,
    },
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum CliWaveform {
    Sine,
    Triangle,
    Saw,
    Square,
    Noise,
}

impl From<CliWaveform> for Waveform {
    fn from(w: CliWaveform) -> Self {
        match w {
            CliWaveform::Sine => Waveform::Sine,
            CliWaveform::Triangle => Waveform::Triangle,
            CliWaveform::Saw => Waveform::Sawtooth,
            CliWaveform::Square => Waveform::Square,
            CliWaveform::Noise => Waveform::Noise,
        }
    }
}

fn load_project_from_path<P: AsRef<Path>>(path: P) -> anyhow::Result<Project> {
    let p = path.as_ref();
    if !p.exists() {
        anyhow::bail!("File not found: {}", p.display());
    }

    if let Some(ext) = p.extension().and_then(|s| s.to_str())
        && ext.eq_ignore_ascii_case("json")
    {
        let data = fs::read_to_string(p)?;
        let proj: Project = serde_json::from_str(&data)?;
        return Ok(proj);
    }

    // Default to .als parser
    AlsParser::load_project(p).map_err(|e| anyhow::anyhow!("ALS load error: {}", e))
}

fn main() -> anyhow::Result<()> {
    let args = Cli::parse();

    let mut logger =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("error"));
    if args.verbose {
        logger.filter_level(log::LevelFilter::Debug);
    }
    let _ = logger.try_init();

    match args.command {
        None => {
            // Default action if no subcommand is given: launch GUI
            let options = livecraft_ui::GuiOptions {
                project_path: args.project,
                ..Default::default()
            };
            if let Err(e) = livecraft_ui::run_gui_with_options(options) {
                eprintln!("Error starting GUI: {:?}", e);
                return Err(e);
            }
        }
        Some(Commands::Gui {
            project,
            screenshot,
            view,
            tab,
            theme,
        }) => {
            let options = livecraft_ui::GuiOptions {
                project_path: project,
                screenshot_path: screenshot,
                initial_view: view,
                initial_tab: tab,
                theme,
            };
            if let Err(e) = livecraft_ui::run_gui_with_options(options) {
                eprintln!("Error starting GUI: {:?}", e);
                return Err(e);
            }
        }
        Some(Commands::New { path, bpm }) => {
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("New Project");
            let mut proj = Project::new_default(name);
            proj.transport.set_bpm(bpm);

            if let Some(ext) = path.extension().and_then(|s| s.to_str())
                && ext.eq_ignore_ascii_case("json")
            {
                let json = serde_json::to_string_pretty(&proj)?;
                fs::write(&path, json)?;
                println!("Created new LiveCraft JSON project: {}", path.display());
                return Ok(());
            }

            AlsParser::save_project_to_als(&proj, &path)?;
            println!("Created new Ableton Live set: {}", path.display());
        }
        Some(Commands::Info { path }) => {
            let proj = load_project_from_path(&path)?;
            print_project_card(&proj, &path);
        }
        Some(Commands::Inspect {
            path,
            json,
            devices,
            clips,
        }) => {
            let proj = load_project_from_path(&path)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&proj)?);
            } else {
                print_project_inspection(&proj, devices, clips);
            }
        }
        Some(Commands::Render {
            path,
            output,
            duration_bars,
            sample_rate,
            bits,
        }) => {
            let proj = load_project_from_path(&path)?;
            println!("Rendering '{}' to '{}'...", proj.name, output.display());
            println!(
                "Configuration: {} bars @ {} Hz, {}-bit WAV",
                duration_bars, sample_rate, bits
            );

            let options = RenderOptions {
                sample_rate,
                duration_bars,
                bits_per_sample: bits,
            };

            let start = std::time::Instant::now();
            OfflineBouncer::render_project(&proj, &output, options)?;
            let elapsed = start.elapsed();

            println!(
                "Render complete in {:.2?}! Output saved to: {}",
                elapsed,
                output.display()
            );
        }
        Some(Commands::Stems {
            path,
            output_dir,
            duration_bars,
        }) => {
            let proj = load_project_from_path(&path)?;
            println!(
                "Exporting {} stems to '{}'...",
                proj.tracks.len(),
                output_dir.display()
            );

            let options = RenderOptions {
                sample_rate: 44100,
                duration_bars,
                bits_per_sample: 24,
            };

            let start = std::time::Instant::now();
            let stems = OfflineBouncer::render_stems(&proj, &output_dir, options)?;
            let elapsed = start.elapsed();

            println!("Exported {} stems in {:.2?}:", stems.len(), elapsed);
            for s in stems {
                println!("  - {}", s);
            }
        }
        Some(Commands::Play { path, duration }) => {
            let proj = if let Some(p) = path {
                load_project_from_path(&p)?
            } else {
                let mut default_proj = Project::new_default("Live Demo");
                // Add demo notes
                let mut clip = Clip::new_midi(1, "Lead Melody", 4.0);
                if let ClipPayload::Midi(ref mut mdata) = clip.payload {
                    mdata.notes.push(MidiNote::new(60, 0.0, 1.0, 100)); // C3
                    mdata.notes.push(MidiNote::new(62, 1.0, 1.0, 100)); // D4
                    mdata.notes.push(MidiNote::new(64, 2.0, 1.0, 100)); // E4
                    mdata.notes.push(MidiNote::new(67, 3.0, 1.0, 100)); // G4
                }
                default_proj.tracks[0].clip_slots[0].clip = Some(clip);
                default_proj
            };

            run_terminal_player(&proj, duration)?;
        }
        Some(Commands::Synth {
            note,
            waveform,
            output,
            duration_sec,
        }) => {
            run_synth_cli(note, waveform.into(), output, duration_sec)?;
        }
        Some(Commands::Convert { input, output }) => {
            let proj = load_project_from_path(&input)?;
            let out_ext = output.extension().and_then(|s| s.to_str()).unwrap_or("als");

            if out_ext.eq_ignore_ascii_case("json") {
                let json = serde_json::to_string_pretty(&proj)?;
                fs::write(&output, json)?;
                println!(
                    "Converted '{}' -> JSON: '{}'",
                    input.display(),
                    output.display()
                );
            } else {
                AlsParser::save_project_to_als(&proj, &output)?;
                println!(
                    "Converted '{}' -> Ableton .als: '{}'",
                    input.display(),
                    output.display()
                );
            }
        }
    }

    Ok(())
}

fn print_project_card(proj: &Project, path: &Path) {
    println!("============================================================");
    println!("  LIVECRAFT :: ABLETON LIVE PROJECT SUMMARY");
    println!("============================================================");
    println!("  Project Name : {}", proj.name);
    println!("  File Path    : {}", path.display());
    println!("  Creator      : {}", proj.creator);
    println!("  Version      : {}", proj.schema_version);
    println!("  Tempo        : {:.1} BPM", proj.transport.bpm());
    println!(
        "  Time Sig     : {}/{}",
        proj.transport.time_signature().0,
        proj.transport.time_signature().1
    );
    println!("------------------------------------------------------------");
    let audio_count = proj
        .tracks
        .iter()
        .filter(|t| t.track_type == TrackType::Audio)
        .count();
    let midi_count = proj
        .tracks
        .iter()
        .filter(|t| t.track_type == TrackType::Midi)
        .count();
    let total_clips: usize = proj
        .tracks
        .iter()
        .map(|t| t.clip_slots.iter().filter(|s| s.clip.is_some()).count())
        .sum();

    println!(
        "  Tracks       : {} total ({} Audio, {} MIDI)",
        proj.tracks.len(),
        audio_count,
        midi_count
    );
    println!("  Returns      : {} tracks", proj.return_tracks.len());
    println!("  Scenes       : {} scenes", proj.scenes.len());
    println!("  Active Clips : {}", total_clips);
    println!("============================================================");
}

fn print_project_inspection(proj: &Project, show_devices: bool, show_clips: bool) {
    println!("Project: {} (BPM: {:.1})", proj.name, proj.transport.bpm());
    println!("Tracks ({}):", proj.tracks.len());
    for (i, trk) in proj.tracks.iter().enumerate() {
        let type_str = match trk.track_type {
            TrackType::Audio => "Audio",
            TrackType::Midi => "MIDI",
            TrackType::Return => "Return",
            TrackType::Group => "Group",
            TrackType::Main => "Main",
        };
        let mute_flag = if trk.is_muted { " [MUTED]" } else { "" };
        let solo_flag = if trk.is_soloed { " [SOLO]" } else { "" };
        println!(
            "  #{}: {} [{}] Vol: {:.1} dB, Pan: {:.2}{}{}",
            i + 1,
            trk.name,
            type_str,
            trk.volume_db,
            trk.pan,
            mute_flag,
            solo_flag
        );

        if show_devices && !trk.device_chain.is_empty() {
            println!(
                "      Devices: {}",
                trk.device_chain
                    .iter()
                    .map(|d| d.name.as_str())
                    .collect::<Vec<_>>()
                    .join(" -> ")
            );
        }

        if show_clips {
            let clips: Vec<_> = trk
                .clip_slots
                .iter()
                .enumerate()
                .filter_map(|(s_idx, s)| s.clip.as_ref().map(|c| (s_idx, c)))
                .collect();
            if !clips.is_empty() {
                for (s_idx, c) in clips {
                    let desc = match &c.payload {
                        ClipPayload::Midi(m) => format!("MIDI ({} notes)", m.notes.len()),
                        ClipPayload::Audio(a) => format!("Audio ({})", a.file_path),
                    };
                    println!(
                        "      Slot #{}: '{}' [{}] Length: {:.1} beats",
                        s_idx + 1,
                        c.name,
                        desc,
                        c.length_beats
                    );
                }
            }
        }
    }

    if !proj.return_tracks.is_empty() {
        println!("\nReturn Tracks ({}):", proj.return_tracks.len());
        for (i, ret) in proj.return_tracks.iter().enumerate() {
            println!(
                "  Return #{}: {} (Vol: {:.1} dB)",
                i + 1,
                ret.name,
                ret.volume_db
            );
        }
    }
}

fn run_terminal_player(project: &Project, max_duration: u64) -> anyhow::Result<()> {
    println!("\nStarting LiveCraft Real-Time Audio Engine...");
    let engine = AudioEngine::new(project)?;

    // Trigger first available clip on each track
    for trk in project.tracks.iter() {
        if let Some(slot) = trk.clip_slots.first()
            && slot.clip.is_some()
        {
            engine.trigger_clip(trk.id, 0);
        }
    }
    engine.play();

    println!("============================================================");
    println!("  LIVECRAFT LIVE TERMINAL PLAYBACK");
    println!(
        "  Project: {} | Tempo: {:.1} BPM",
        project.name,
        project.transport.bpm()
    );
    println!("  Press [Enter] in this terminal or wait for timeout to stop.");
    println!("============================================================");

    let running = Arc::new(AtomicBool::new(true));
    let r_clone = Arc::clone(&running);
    thread::spawn(move || {
        let mut line = String::new();
        let _ = io::stdin().read_line(&mut line);
        r_clone.store(false, Ordering::SeqCst);
    });

    let start_time = std::time::Instant::now();

    while running.load(Ordering::SeqCst) {
        if max_duration > 0 && start_time.elapsed().as_secs() >= max_duration {
            break;
        }

        if let Ok(graph) = engine.graph.lock() {
            let beats = graph.playhead_beats;
            let bar = (beats / 4.0) as u32 + 1;
            let beat = (beats % 4.0) as u32 + 1;
            let sixteenth = ((beats * 4.0) % 4.0) as u32 + 1;

            let meter_l = graph.master_meter.peak_db_l();
            let meter_r = graph.master_meter.peak_db_r();

            let bar_meter_l = render_vu_meter_bar(meter_l);
            let bar_meter_r = render_vu_meter_bar(meter_r);

            print!(
                "\r [Bar {:03}.{:01}.{:01}] L: [{}] {:>5.1} dB | R: [{}] {:>5.1} dB ",
                bar, beat, sixteenth, bar_meter_l, meter_l, bar_meter_r, meter_r
            );
            let _ = io::stdout().flush();
        }

        thread::sleep(Duration::from_millis(50));
    }

    engine.stop();
    println!("\nPlayback stopped.");
    Ok(())
}

fn render_vu_meter_bar(db: f32) -> String {
    let width = 16;
    // Map -48 dB to 0 dB across width
    let clamped = db.clamp(-48.0, 0.0);
    let norm = (clamped + 48.0) / 48.0;
    let filled = (norm * width as f32).round() as usize;

    let mut bar = String::with_capacity(width);
    for i in 0..width {
        if i < filled {
            if i > 12 {
                bar.push('#'); // Hot/peak
            } else if i > 8 {
                bar.push('=');
            } else {
                bar.push('-');
            }
        } else {
            bar.push(' ');
        }
    }
    bar
}

fn run_synth_cli(
    note: u8,
    wave: Waveform,
    output: Option<PathBuf>,
    duration_sec: f32,
) -> anyhow::Result<()> {
    println!(
        "Generating Synth tone: MIDI note {}, waveform {:?}",
        note, wave
    );

    if let Some(out_path) = output {
        let sample_rate = 44100;
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&out_path, spec)?;

        let mut synth = OperatorSynth::new(sample_rate as f32);
        synth.osc1_wave = wave;
        synth.note_on(note, 100);

        let total_samples = (duration_sec * sample_rate as f32) as usize;
        for _ in 0..total_samples {
            let (l, r) = synth.process_sample();
            writer.write_sample((l.clamp(-1.0, 1.0) * 32767.0) as i16)?;
            writer.write_sample((r.clamp(-1.0, 1.0) * 32767.0) as i16)?;
        }

        writer.finalize()?;
        println!("Rendered synth audio to: {}", out_path.display());
    } else {
        println!("Live synth preview requires an output file path: specify -o tone.wav");
    }

    Ok(())
}
