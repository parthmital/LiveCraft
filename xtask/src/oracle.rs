//! M08 audio oracle harness (`cargo xtask oracle [<name> | --all]`).
//!
//! Renders `fixtures/oracle/<name>.als` via `livecraft render` and compares the output against
//! `.agent-local/reference/oracle/<name>.wav` using the metrics in `livecraft_dsp::metrics`.

use crate::{Result, cargo_program};
use livecraft_dsp::metrics::{
    integrated_lufs, max_third_octave_diff_db, relative_residual_rms_dbfs, residual_rms_dbfs,
    true_peak_dbtp,
};
use std::path::{Path, PathBuf};
use std::process::Command;

const ORACLE_FIXTURES_DIR: &str = "fixtures/oracle";
const ORACLE_REFERENCE_DIR: &str = ".agent-local/reference/oracle";
const ORACLE_TMP_DIR: &str = ".agent-local/tmp/oracle";
const LINEAR_RESIDUAL_MAX_DBFS: f64 = -90.0;
const NONLINEAR_BAND_DIFF_MAX_DB: f64 = 1.0;
const NONLINEAR_LUFS_DIFF_MAX: f64 = 0.5;
const NONLINEAR_PEAK_DIFF_MAX_DB: f64 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OracleKind {
    Linear,
    Nonlinear,
}

impl OracleKind {
    fn label(self) -> &'static str {
        match self {
            Self::Linear => "Linear",
            Self::Nonlinear => "Nonlinear",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum OracleSelection {
    All,
    Single(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OracleArgs {
    selection: OracleSelection,
    kind_override: Option<OracleKind>,
}

#[derive(Debug, Clone)]
struct WavData {
    sample_rate: u32,
    channels: usize,
    samples: Vec<f32>,
}

#[derive(Debug, Clone, Copy)]
struct OracleMetrics {
    null_dbfs: f64,
    band_diff_db: f64,
    lufs_diff: f64,
    peak_diff_db: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum OracleStatus {
    Pass,
    Fail(String),
    Skip,
}

impl OracleStatus {
    fn label(&self) -> String {
        match self {
            Self::Pass => "PASS".to_string(),
            Self::Fail(reason) => format!("FAIL ({reason})"),
            Self::Skip => "SKIP (missing reference)".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
struct OracleRow {
    name: String,
    kind: OracleKind,
    metrics: Option<OracleMetrics>,
    status: OracleStatus,
}

impl OracleRow {
    fn skipped(name: impl Into<String>, kind: OracleKind) -> Self {
        Self {
            name: name.into(),
            kind,
            metrics: None,
            status: OracleStatus::Skip,
        }
    }
}

#[derive(Debug, Clone)]
struct OraclePaths {
    fixtures_dir: PathBuf,
    reference_dir: PathBuf,
    tmp_dir: PathBuf,
}

impl OraclePaths {
    fn workspace() -> Self {
        Self {
            fixtures_dir: PathBuf::from(ORACLE_FIXTURES_DIR),
            reference_dir: PathBuf::from(ORACLE_REFERENCE_DIR),
            tmp_dir: PathBuf::from(ORACLE_TMP_DIR),
        }
    }
}

pub fn oracle_command(args: &[String]) -> Result {
    let parsed = parse_oracle_args(args)?;
    run_oracle_with(&parsed, &OraclePaths::workspace(), render_oracle_set)
}

fn parse_oracle_args(args: &[String]) -> Result<OracleArgs> {
    let mut selection: Option<OracleSelection> = None;
    let mut kind_override: Option<OracleKind> = None;

    for arg in args {
        match arg.as_str() {
            "--all" => {
                if selection.is_some() {
                    return Err("usage: cargo xtask oracle [<name> | --all]".to_string());
                }
                selection = Some(OracleSelection::All);
            }
            "--nonlinear" => kind_override = Some(OracleKind::Nonlinear),
            other if other.starts_with('-') => {
                return Err(format!(
                    "unknown oracle flag `{other}`; usage: cargo xtask oracle [<name> | --all]"
                ));
            }
            name => {
                if selection.is_some() {
                    return Err("usage: cargo xtask oracle [<name> | --all]".to_string());
                }
                let trimmed = name.strip_suffix(".als").unwrap_or(name);
                if trimmed.is_empty() {
                    return Err("oracle set name cannot be empty".to_string());
                }
                selection = Some(OracleSelection::Single(trimmed.to_string()));
            }
        }
    }

    Ok(OracleArgs {
        selection: selection.unwrap_or(OracleSelection::All),
        kind_override,
    })
}

fn discover_oracle_sets(fixtures_dir: &Path) -> Result<Vec<String>> {
    if !fixtures_dir.is_dir() {
        return Err(format!(
            "oracle fixtures directory not found: {}",
            fixtures_dir.display()
        ));
    }
    let entries = std::fs::read_dir(fixtures_dir)
        .map_err(|e| format!("could not read {}: {e}", fixtures_dir.display()))?;
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("could not read directory entry: {e}"))?;
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("als"))
            && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
        {
            names.push(stem.to_string());
        }
    }
    names.sort();
    if names.is_empty() {
        return Err(format!(
            "no .als oracle fixtures found in {}",
            fixtures_dir.display()
        ));
    }
    Ok(names)
}

fn oracle_kind_for_set(als_path: &Path, override_kind: Option<OracleKind>) -> Result<OracleKind> {
    if let Some(kind) = override_kind {
        return Ok(kind);
    }
    let sidecar = als_path.with_extension("json");
    if !sidecar.exists() {
        return Ok(OracleKind::Linear);
    }
    let text = std::fs::read_to_string(&sidecar)
        .map_err(|e| format!("could not read {}: {e}", sidecar.display()))?;
    let json: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("invalid JSON in {}: {e}", sidecar.display()))?;
    match json.get("kind").and_then(|v| v.as_str()) {
        None | Some("linear" | "Linear") => Ok(OracleKind::Linear),
        Some("nonlinear" | "Nonlinear") => Ok(OracleKind::Nonlinear),
        Some(other) => Err(format!(
            "{}: unsupported oracle kind `{other}` (expected `linear` or `nonlinear`)",
            sidecar.display()
        )),
    }
}

fn render_oracle_set(als_path: &Path, out_wav: &Path) -> Result {
    if let Some(parent) = out_wav.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("could not create {}: {e}", parent.display()))?;
    }
    let status = Command::new(cargo_program())
        .args(["run", "--quiet", "--bin", "livecraft", "--", "render"])
        .arg(als_path)
        .arg("-o")
        .arg(out_wav)
        .args([
            "--bits",
            "32",
            "--sample-rate",
            "44100",
            "--length-beats",
            "4",
            "--no-report",
        ])
        .status()
        .map_err(|e| format!("could not run livecraft render: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "livecraft render failed for {}",
            als_path.display()
        ))
    }
}

fn read_wav_f32(path: &Path) -> Result<WavData> {
    let mut reader = hound::WavReader::open(path)
        .map_err(|e| format!("{}: failed to open WAV: {e}", path.display()))?;
    let spec = reader.spec();
    if spec.channels == 0 {
        return Err(format!("{}: WAV has 0 channels", path.display()));
    }
    if spec.sample_rate == 0 {
        return Err(format!("{}: WAV has 0 Hz sample rate", path.display()));
    }

    let samples = match spec.sample_format {
        hound::SampleFormat::Float => {
            if spec.bits_per_sample != 32 {
                return Err(format!(
                    "{}: unsupported float bit depth {}",
                    path.display(),
                    spec.bits_per_sample
                ));
            }
            reader
                .samples::<f32>()
                .collect::<std::result::Result<Vec<f32>, _>>()
                .map_err(|e| format!("{}: failed reading f32 samples: {e}", path.display()))?
        }
        hound::SampleFormat::Int => {
            let scale: f64 = match spec.bits_per_sample {
                16 => 32_768.0,
                24 => 8_388_608.0,
                32 => 2_147_483_648.0,
                other => {
                    return Err(format!(
                        "{}: unsupported integer PCM bit depth {other}",
                        path.display()
                    ));
                }
            };
            let mut out = Vec::new();
            for sample in reader.samples::<i32>() {
                let value = sample
                    .map_err(|e| format!("{}: failed reading PCM samples: {e}", path.display()))?;
                out.push((f64::from(value) / scale) as f32);
            }
            out
        }
    };

    Ok(WavData {
        sample_rate: spec.sample_rate,
        channels: usize::from(spec.channels),
        samples,
    })
}

fn abs_level_diff(a: f64, b: f64) -> f64 {
    if a.is_infinite() && b.is_infinite() && a.signum() == b.signum() {
        0.0
    } else if !a.is_finite() || !b.is_finite() {
        f64::INFINITY
    } else {
        (a - b).abs()
    }
}

fn evaluate_oracle_wavs(
    name: &str,
    kind: OracleKind,
    reference: &WavData,
    candidate: &WavData,
) -> OracleRow {
    if reference.channels != candidate.channels || reference.sample_rate != candidate.sample_rate {
        return OracleRow {
            name: name.to_string(),
            kind,
            metrics: None,
            status: OracleStatus::Fail(format!(
                "format mismatch: ref {}Hz/{}ch != rendered {}Hz/{}ch",
                reference.sample_rate,
                reference.channels,
                candidate.sample_rate,
                candidate.channels
            )),
        };
    }

    let (ch, sr) = (reference.channels, reference.sample_rate);
    let null_dbfs = relative_residual_rms_dbfs(&reference.samples, &candidate.samples);
    let residual_dbfs = residual_rms_dbfs(&reference.samples, &candidate.samples);
    let band_diff_db = max_third_octave_diff_db(&reference.samples, &candidate.samples, ch, sr);
    let lufs_diff = abs_level_diff(
        integrated_lufs(&reference.samples, ch, sr),
        integrated_lufs(&candidate.samples, ch, sr),
    );
    let peak_diff_db = abs_level_diff(
        true_peak_dbtp(&reference.samples, ch, sr),
        true_peak_dbtp(&candidate.samples, ch, sr),
    );

    let metrics = OracleMetrics {
        null_dbfs,
        band_diff_db,
        lufs_diff,
        peak_diff_db,
    };

    let status = match kind {
        OracleKind::Linear => {
            let ref_silent = reference.samples.iter().all(|&s| s == 0.0);
            let pass = null_dbfs <= LINEAR_RESIDUAL_MAX_DBFS
                || (ref_silent && residual_dbfs <= LINEAR_RESIDUAL_MAX_DBFS);
            if pass {
                OracleStatus::Pass
            } else {
                OracleStatus::Fail(format!(
                    "null {} > {:.1} dBFS (abs {} dBFS)",
                    format_metric(null_dbfs),
                    LINEAR_RESIDUAL_MAX_DBFS,
                    format_metric(residual_dbfs),
                ))
            }
        }
        OracleKind::Nonlinear => {
            let checks = [
                ("band", band_diff_db, NONLINEAR_BAND_DIFF_MAX_DB, "dB"),
                ("LUFS", lufs_diff, NONLINEAR_LUFS_DIFF_MAX, "LU"),
                ("peak", peak_diff_db, NONLINEAR_PEAK_DIFF_MAX_DB, "dB"),
            ];
            let failures: Vec<String> = checks
                .into_iter()
                .filter(|(_, val, limit, _)| val.is_nan() || *val > *limit)
                .map(|(label, val, limit, unit)| {
                    format!("{label} {} > {limit:.1} {unit}", format_metric(val))
                })
                .collect();
            if failures.is_empty() {
                OracleStatus::Pass
            } else {
                OracleStatus::Fail(failures.join(", "))
            }
        }
    };

    OracleRow {
        name: name.to_string(),
        kind,
        metrics: Some(metrics),
        status,
    }
}

fn evaluate_oracle_files(
    name: &str,
    kind: OracleKind,
    rendered_wav: &Path,
    reference_wav: &Path,
) -> Result<OracleRow> {
    if !reference_wav.exists() {
        return Ok(OracleRow::skipped(name, kind));
    }
    let reference = read_wav_f32(reference_wav)?;
    let candidate = read_wav_f32(rendered_wav)?;
    Ok(evaluate_oracle_wavs(name, kind, &reference, &candidate))
}

fn run_oracle_with<F>(args: &OracleArgs, paths: &OraclePaths, mut renderer: F) -> Result
where
    F: FnMut(&Path, &Path) -> Result,
{
    std::fs::create_dir_all(&paths.tmp_dir)
        .map_err(|e| format!("could not create {}: {e}", paths.tmp_dir.display()))?;

    let names = match &args.selection {
        OracleSelection::All => discover_oracle_sets(&paths.fixtures_dir)?,
        OracleSelection::Single(name) => {
            let als = paths.fixtures_dir.join(format!("{name}.als"));
            if !als.is_file() {
                return Err(format!("oracle set not found: {}", als.display()));
            }
            vec![name.clone()]
        }
    };

    let mut rows = Vec::with_capacity(names.len());
    for name in names {
        let als_path = paths.fixtures_dir.join(format!("{name}.als"));
        let ref_wav = paths.reference_dir.join(format!("{name}.wav"));
        let tmp_wav = paths.tmp_dir.join(format!("{name}.wav"));
        let kind = oracle_kind_for_set(&als_path, args.kind_override)?;

        if !ref_wav.exists() {
            println!(
                "xtask: oracle `{name}`: SKIP (missing reference: {})",
                ref_wav.display()
            );
            rows.push(OracleRow::skipped(name, kind));
            continue;
        }

        renderer(&als_path, &tmp_wav)?;
        rows.push(evaluate_oracle_files(&name, kind, &tmp_wav, &ref_wav)?);
    }

    print!("{}", format_oracle_table(&rows));

    let (mut passed, mut skipped, mut failed) = (0usize, 0usize, 0usize);
    for row in &rows {
        match row.status {
            OracleStatus::Pass => passed += 1,
            OracleStatus::Skip => skipped += 1,
            OracleStatus::Fail(_) => failed += 1,
        }
    }
    println!("xtask: oracle summary: {passed} passed, {skipped} skipped, {failed} failed");

    if failed > 0 {
        Err(format!("oracle comparison failed for {failed} set(s)"))
    } else {
        Ok(())
    }
}

fn format_metric(value: f64) -> String {
    if value == f64::NEG_INFINITY {
        "-inf".to_string()
    } else if value == f64::INFINITY {
        "+inf".to_string()
    } else if value.is_nan() {
        "NaN".to_string()
    } else {
        format!("{value:.2}")
    }
}

fn format_oracle_table(rows: &[OracleRow]) -> String {
    let nw = rows.iter().map(|r| r.name.len()).max().unwrap_or(0).max(4);
    let cols = |c: [&str; 7]| {
        format!(
            "{:<nw$} | {:<9} | {:>9} | {:>11} | {:>9} | {:>9} | {}\n",
            c[0], c[1], c[2], c[3], c[4], c[5], c[6]
        )
    };
    let mut out = cols([
        "Name",
        "Kind",
        "Null dBFS",
        "BandDiff dB",
        "LUFS Diff",
        "Peak Diff",
        "Status",
    ]);
    out.push_str(&format!(
        "{:-<nw$}-+-----------+-----------+-------------+-----------+-----------+-------------------------\n",
        ""
    ));
    for row in rows {
        let (n, b, l, p) = match row.metrics {
            Some(m) => (
                format_metric(m.null_dbfs),
                format_metric(m.band_diff_db),
                format_metric(m.lufs_diff),
                format_metric(m.peak_diff_db),
            ),
            None => ("-".into(), "-".into(), "-".into(), "-".into()),
        };
        out.push_str(&cols([
            &row.name,
            row.kind.label(),
            &n,
            &b,
            &l,
            &p,
            &row.status.label(),
        ]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn test_tmp_dir(name: &str) -> PathBuf {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../.agent-local/tmp/xtask_tests")
            .join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create test temp dir");
        dir
    }

    fn gen_stereo_sine(freq_hz: f64, amplitude: f64, sample_rate: u32, seconds: f64) -> Vec<f32> {
        let frames = (f64::from(sample_rate) * seconds).round() as usize;
        let mut out = Vec::with_capacity(frames * 2);
        for n in 0..frames {
            let s =
                (amplitude * (2.0 * PI * freq_hz * n as f64 / f64::from(sample_rate)).sin()) as f32;
            out.push(s);
            out.push(s);
        }
        out
    }

    fn write_wav<T: hound::Sample + Copy>(
        path: &Path,
        sample_rate: u32,
        channels: u16,
        bits_per_sample: u16,
        sample_format: hound::SampleFormat,
        samples: &[T],
    ) {
        let spec = hound::WavSpec {
            channels,
            sample_rate,
            bits_per_sample,
            sample_format,
        };
        let mut writer = hound::WavWriter::create(path, spec).expect("create wav");
        for &s in samples {
            writer.write_sample(s).expect("write wav sample");
        }
        writer.finalize().expect("finalize wav");
    }

    fn test_args(selection: OracleSelection) -> OracleArgs {
        OracleArgs {
            selection,
            kind_override: None,
        }
    }

    fn scale_samples(samples: &[f32], factor: f32) -> Vec<f32> {
        samples.iter().map(|&s| s * factor).collect()
    }

    fn test_paths(dir: &Path, dummy_als: &str) -> OraclePaths {
        let paths = OraclePaths {
            fixtures_dir: dir.join("fixtures"),
            reference_dir: dir.join("ref"),
            tmp_dir: dir.join("tmp"),
        };
        std::fs::create_dir_all(&paths.fixtures_dir).expect("mkdir fixtures");
        std::fs::write(
            paths.fixtures_dir.join(format!("{dummy_als}.als")),
            b"dummy",
        )
        .expect("write dummy als");
        paths
    }

    fn eval_samples(
        dir: &Path,
        file_name: &str,
        kind: OracleKind,
        samples: &[f32],
        ref_path: &Path,
    ) -> OracleRow {
        let p = dir.join(file_name);
        write_f32_wav(&p, 44100, 2, samples);
        evaluate_oracle_files(file_name, kind, &p, ref_path).expect("evaluate wavs")
    }

    fn write_f32_wav(path: &Path, sr: u32, ch: u16, samples: &[f32]) {
        write_wav(path, sr, ch, 32, hound::SampleFormat::Float, samples);
    }

    #[test]
    fn wav_reader_decodes_i16_i24_i32_and_f32() {
        let dir = test_tmp_dir("wav_formats");
        let int_cases: &[(&str, u32, u16, u16, &[i32])] = &[
            ("s16.wav", 44100, 2, 16, &[0, 16_384, -16_384, -32_768]),
            (
                "s24.wav",
                48000,
                2,
                24,
                &[0, 4_194_304, -4_194_304, -8_388_608],
            ),
            (
                "s32.wav",
                44100,
                1,
                32,
                &[0, 1_073_741_824, -1_073_741_824, i32::MIN],
            ),
        ];
        for &(name, sr, ch, bits, raw) in int_cases {
            let p = dir.join(name);
            write_wav(&p, sr, ch, bits, hound::SampleFormat::Int, raw);
            let wav = read_wav_f32(&p).expect("read int wav");
            assert_eq!(
                (wav.sample_rate, wav.channels, wav.samples),
                (sr, usize::from(ch), vec![0.0, 0.5, -0.5, -1.0])
            );
        }

        let pf32 = dir.join("f32.wav");
        write_f32_wav(&pf32, 44100, 2, &[0.0, 0.5, -1.25, 1.5]);
        assert_eq!(
            read_wav_f32(&pf32).expect("read f32").samples,
            vec![0.0, 0.5, -1.25, 1.5]
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn oracle_identical_and_attenuated_linear_pass_at_or_below_neg90_dbfs() {
        let dir = test_tmp_dir("linear_pass");
        let sine = gen_stereo_sine(1000.0, 0.5, 44100, 1.0);
        let ref_path = dir.join("ref.wav");
        write_f32_wav(&ref_path, 44100, 2, &sine);

        let ident_row = eval_samples(&dir, "ident.wav", OracleKind::Linear, &sine, &ref_path);
        assert_eq!(ident_row.status, OracleStatus::Pass);
        assert_eq!(
            ident_row.metrics.expect("metrics").null_dbfs,
            f64::NEG_INFINITY
        );

        let attenuated = scale_samples(&sine, 1.0 - 1e-5);
        let atten_row = eval_samples(
            &dir,
            "atten.wav",
            OracleKind::Linear,
            &attenuated,
            &ref_path,
        );
        assert_eq!(atten_row.status, OracleStatus::Pass);
        let atten_null = atten_row.metrics.expect("metrics").null_dbfs;
        assert!(
            atten_null <= LINEAR_RESIDUAL_MAX_DBFS,
            "expected <= -90 dBFS, got {atten_null}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn oracle_mismatched_linear_and_nonlinear_fail() {
        let dir = test_tmp_dir("mismatch_fail");
        let sine = gen_stereo_sine(1000.0, 0.5, 44100, 1.0);
        let ref_path = dir.join("ref.wav");
        write_f32_wav(&ref_path, 44100, 2, &sine);

        let scaled_bad = scale_samples(&sine, 0.9);
        let lin_row = eval_samples(
            &dir,
            "bad_lin.wav",
            OracleKind::Linear,
            &scaled_bad,
            &ref_path,
        );
        assert!(matches!(lin_row.status, OracleStatus::Fail(_)));

        let nl_ok = scale_samples(&sine, -10.0_f32.powf(-0.2 / 20.0));
        assert_eq!(
            eval_samples(&dir, "nl_ok.wav", OracleKind::Nonlinear, &nl_ok, &ref_path).status,
            OracleStatus::Pass
        );

        let nl_bad = scale_samples(&sine, 10.0_f32.powf(-3.0 / 20.0));
        let nl_bad_row = eval_samples(
            &dir,
            "nl_bad.wav",
            OracleKind::Nonlinear,
            &nl_bad,
            &ref_path,
        );
        assert!(matches!(nl_bad_row.status, OracleStatus::Fail(_)));

        let paths = test_paths(&dir, "case");
        std::fs::create_dir_all(&paths.reference_dir).expect("mkdir ref");
        write_f32_wav(&paths.reference_dir.join("case.wav"), 44100, 2, &sine);
        let res = run_oracle_with(
            &test_args(OracleSelection::Single("case".into())),
            &paths,
            |_als, out_wav| {
                write_f32_wav(out_wav, 44100, 2, &scaled_bad);
                Ok(())
            },
        );
        assert!(res.is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn oracle_missing_reference_skips_cleanly() {
        let dir = test_tmp_dir("missing_ref_skip");
        let paths = test_paths(&dir, "sine_unity");

        let row = evaluate_oracle_files(
            "sine_unity",
            OracleKind::Linear,
            &dir.join("rendered.wav"),
            &paths.reference_dir.join("sine_unity.wav"),
        )
        .expect("missing reference should not error");
        assert_eq!(row.status, OracleStatus::Skip);
        assert_eq!(row.status.label(), "SKIP (missing reference)");
        assert!(format_oracle_table(&[row]).contains("SKIP (missing reference)"));

        let mut renderer_called = false;
        let run_res = run_oracle_with(&test_args(OracleSelection::All), &paths, |_als, _out| {
            renderer_called = true;
            Ok(())
        });
        assert!(run_res.is_ok(), "missing reference must exit cleanly");
        assert!(
            !renderer_called,
            "missing reference should skip without invoking livecraft render"
        );
        assert!(
            paths.tmp_dir.is_dir(),
            "tmp dir should be created automatically"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn oracle_cli_parsing_and_fixture_discovery() {
        assert_eq!(
            parse_oracle_args(&[]).expect("empty args"),
            test_args(OracleSelection::All)
        );
        assert_eq!(
            parse_oracle_args(&["--all".into()]).expect("--all"),
            test_args(OracleSelection::All)
        );
        assert_eq!(
            parse_oracle_args(&["sine_unity.als".into()]).expect("single .als"),
            test_args(OracleSelection::Single("sine_unity".into()))
        );
        assert!(parse_oracle_args(&["--unknown".into()]).is_err());

        let fixtures_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/oracle");
        assert_eq!(
            discover_oracle_sets(&fixtures_dir).expect("discover fixtures"),
            vec![
                "panned_sine",
                "send_to_return_neg6db",
                "sine_unity",
                "two_tracks_summed"
            ]
        );

        let sine_wav =
            read_wav_f32(&fixtures_dir.join("audio/sine_1khz.wav")).expect("read sine_1khz.wav");
        assert_eq!((sine_wav.sample_rate, sine_wav.channels), (44100, 2));
        assert_eq!(sine_wav.samples.len(), 88_200 * 2);
    }
}
