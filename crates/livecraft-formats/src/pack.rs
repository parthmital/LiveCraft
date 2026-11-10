//! Ableton Live Pack (`.alp`) safe extraction with zip-slip and size limits.

use std::fs::{self, File};
use std::io::{BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};

use zip::ZipArchive;

/// Maximum uncompressed size allowed for a single Pack entry (512 MiB).
pub const MAX_ENTRY_BYTES: u64 = 512 * 1024 * 1024;

/// Maximum total uncompressed size allowed across an entire Pack (2 GiB).
pub const MAX_TOTAL_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Errors raised while inspecting or extracting an `.alp` Pack archive.
#[derive(Debug, thiserror::Error)]
pub enum PackError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid Pack archive: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("Unsafe path traversal or forbidden entry in Pack: {0}")]
    PathTraversal(String),
    #[error("Pack entry `{name}` exceeds {limit}-byte limit")]
    EntryTooLarge { name: String, limit: u64 },
    #[error("Pack total uncompressed size exceeds {limit}-byte limit")]
    TotalTooLarge { limit: u64 },
}

/// Summary returned after successfully extracting an `.alp` Pack.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PackInstallReport {
    pub extracted_files: Vec<PathBuf>,
    pub sets_found: Vec<PathBuf>,
    pub total_bytes: u64,
}

#[derive(Debug, Clone)]
struct ValidatedEntry {
    index: usize,
    raw_name: String,
    rel_path: PathBuf,
    is_dir: bool,
}

/// Extracts an Ableton Live Pack (`.alp`) into `dest_dir` with zip-slip, symlink,
/// and zip-bomb protection.
pub fn install_pack(alp_path: &Path, dest_dir: &Path) -> Result<PackInstallReport, PackError> {
    install_pack_with_limits(alp_path, dest_dir, MAX_ENTRY_BYTES, MAX_TOTAL_BYTES)
}

pub(crate) fn install_pack_with_limits(
    alp_path: &Path,
    dest_dir: &Path,
    max_entry_bytes: u64,
    max_total_bytes: u64,
) -> Result<PackInstallReport, PackError> {
    let file = File::open(alp_path)?;
    let mut archive = ZipArchive::new(BufReader::new(file))?;

    // Pass 1: validate every entry and declared sizes before touching the filesystem.
    let mut entries = Vec::with_capacity(archive.len());
    let mut declared_total: u64 = 0;

    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        if entry.name_raw().contains(&0) {
            return Err(PackError::PathTraversal(
                "entry name contains null byte".to_owned(),
            ));
        }
        let raw_name = entry.name()?.into_owned();
        if entry.is_symlink() {
            return Err(PackError::PathTraversal(format!(
                "symbolic link entry is forbidden: `{raw_name}`"
            )));
        }
        if entry.encrypted() {
            return Err(PackError::Zip(zip::result::ZipError::UnsupportedArchive(
                "Encrypted Pack entries are not supported",
            )));
        }

        let is_dir = entry.is_dir();
        let rel_path = validate_entry_path(&raw_name, entry.enclosed_name(), is_dir)?;

        let declared_size = entry.size();
        if declared_size > max_entry_bytes {
            return Err(PackError::EntryTooLarge {
                name: raw_name,
                limit: max_entry_bytes,
            });
        }
        declared_total =
            declared_total
                .checked_add(declared_size)
                .ok_or(PackError::TotalTooLarge {
                    limit: max_total_bytes,
                })?;
        if declared_total > max_total_bytes {
            return Err(PackError::TotalTooLarge {
                limit: max_total_bytes,
            });
        }

        entries.push(ValidatedEntry {
            index,
            raw_name,
            rel_path,
            is_dir,
        });
    }

    fs::create_dir_all(dest_dir)?;
    let canon_dest = fs::canonicalize(dest_dir)?;

    let mut report = PackInstallReport::default();
    let mut buf = [0u8; 16 * 1024];

    // Pass 2: extract validated entries while verifying canonical containment and live byte counts.
    for item in entries {
        if item.rel_path.as_os_str().is_empty() {
            continue;
        }
        if item.is_dir {
            ensure_contained_dir(&canon_dest, &item.rel_path, &item.raw_name)?;
            continue;
        }

        let parent_rel = item.rel_path.parent().unwrap_or_else(|| Path::new(""));
        let parent_canon = if parent_rel.as_os_str().is_empty() {
            canon_dest.clone()
        } else {
            ensure_contained_dir(&canon_dest, parent_rel, &item.raw_name)?
        };

        let file_name = item.rel_path.file_name().ok_or_else(|| {
            PackError::PathTraversal(format!("entry `{}` has no file name", item.raw_name))
        })?;
        let canon_target = parent_canon.join(file_name);
        if !canon_target.starts_with(&canon_dest) {
            return Err(PackError::PathTraversal(item.raw_name));
        }

        if let Ok(meta) = fs::symlink_metadata(&canon_target)
            && meta.is_symlink()
        {
            return Err(PackError::PathTraversal(format!(
                "destination `{}` is a symbolic link",
                item.raw_name
            )));
        }

        let mut entry = archive.by_index(item.index)?;
        let mut out_file = File::create(&canon_target)?;
        let extract_res: Result<(), PackError> = (|| {
            let mut entry_written: u64 = 0;
            loop {
                let read_len = entry.read(&mut buf)?;
                if read_len == 0 {
                    break;
                }
                entry_written = entry_written.saturating_add(read_len as u64);
                if entry_written > max_entry_bytes {
                    return Err(PackError::EntryTooLarge {
                        name: item.raw_name.clone(),
                        limit: max_entry_bytes,
                    });
                }
                report.total_bytes = report.total_bytes.saturating_add(read_len as u64);
                if report.total_bytes > max_total_bytes {
                    return Err(PackError::TotalTooLarge {
                        limit: max_total_bytes,
                    });
                }
                out_file.write_all(&buf[..read_len])?;
            }
            out_file.flush()?;
            Ok(())
        })();
        if let Err(err) = extract_res {
            drop(out_file);
            let _ = fs::remove_file(&canon_target);
            return Err(err);
        }

        let reported_path = dest_dir.join(&item.rel_path);
        if reported_path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("als"))
        {
            report.sets_found.push(reported_path.clone());
        }
        report.extracted_files.push(reported_path);
    }

    Ok(report)
}

fn validate_entry_path(
    raw_name: &str,
    enclosed: Option<PathBuf>,
    is_dir: bool,
) -> Result<PathBuf, PackError> {
    if raw_name.is_empty() || raw_name.contains('\0') {
        return Err(PackError::PathTraversal(
            "empty or null-containing Pack entry path".to_owned(),
        ));
    }
    if raw_name.starts_with('/') || raw_name.starts_with('\\') {
        return Err(PackError::PathTraversal(raw_name.to_owned()));
    }
    if raw_name
        .split(['/', '\\'])
        .any(|seg| seg == ".." || seg.contains(':'))
    {
        return Err(PackError::PathTraversal(raw_name.to_owned()));
    }

    for comp in Path::new(raw_name).components() {
        match comp {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(PackError::PathTraversal(raw_name.to_owned()));
            }
            Component::CurDir | Component::Normal(_) => {}
        }
    }

    let rel = enclosed.ok_or_else(|| PackError::PathTraversal(raw_name.to_owned()))?;
    for comp in rel.components() {
        if !matches!(comp, Component::Normal(_)) {
            return Err(PackError::PathTraversal(raw_name.to_owned()));
        }
    }
    if !is_dir && rel.as_os_str().is_empty() {
        return Err(PackError::PathTraversal(raw_name.to_owned()));
    }
    Ok(rel)
}

fn ensure_contained_dir(
    canon_dest: &Path,
    rel_dir: &Path,
    raw_name: &str,
) -> Result<PathBuf, PackError> {
    let mut curr = canon_dest.to_path_buf();
    for comp in rel_dir.components() {
        let Component::Normal(seg) = comp else {
            return Err(PackError::PathTraversal(raw_name.to_owned()));
        };
        curr.push(seg);
        if !curr.starts_with(canon_dest) {
            return Err(PackError::PathTraversal(raw_name.to_owned()));
        }
        match fs::symlink_metadata(&curr) {
            Ok(meta) => {
                if meta.is_symlink() {
                    return Err(PackError::PathTraversal(format!(
                        "directory component `{raw_name}` is a symbolic link"
                    )));
                }
                let next_canon = fs::canonicalize(&curr)?;
                if !next_canon.starts_with(canon_dest) {
                    return Err(PackError::PathTraversal(raw_name.to_owned()));
                }
                curr = next_canon;
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&curr)?;
                let next_canon = fs::canonicalize(&curr)?;
                if !next_canon.starts_with(canon_dest) {
                    return Err(PackError::PathTraversal(raw_name.to_owned()));
                }
                curr = next_canon;
            }
            Err(err) => return Err(PackError::Io(err)),
        }
    }
    Ok(curr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::io::{Cursor, Write};
    use zip::CompressionMethod;
    use zip::write::{SimpleFileOptions, ZipWriter};

    fn make_temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("livecraft_pack_unit_{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn write_zip_entries(path: &Path, entries: &[(&str, &[u8], bool)]) {
        let file = File::create(path).expect("create zip");
        let mut zw = ZipWriter::new(file);
        for &(name, data, is_symlink) in entries {
            if is_symlink {
                zw.add_symlink(
                    name,
                    std::str::from_utf8(data).unwrap_or("../evil"),
                    SimpleFileOptions::default(),
                )
                .expect("add symlink");
            } else {
                let opts =
                    SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
                zw.start_file(name, opts).expect("start zip file");
                zw.write_all(data).expect("write zip data");
            }
        }
        zw.finish().expect("finish zip");
    }

    fn spoof_zip_uncompressed_sizes(bytes: &mut [u8], fake_size: u32) {
        let fake = fake_size.to_le_bytes();
        let mut i = 0;
        while i + 30 <= bytes.len() {
            if &bytes[i..i + 4] == b"PK\x03\x04" {
                bytes[i + 22..i + 26].copy_from_slice(&fake);
                i += 30;
            } else if i + 46 <= bytes.len() && &bytes[i..i + 4] == b"PK\x01\x02" {
                bytes[i + 24..i + 28].copy_from_slice(&fake);
                i += 46;
            } else {
                i += 1;
            }
        }
    }

    #[test]
    fn rejects_traversal_backslash_absolute_and_symlink_entries() {
        let root = make_temp_dir("traversal_cases");
        for (idx, bad_name) in [
            "../../evil",
            r"..\evil",
            "/tmp/evil",
            r"foo\..\../evil",
            r"C:\evil.txt",
        ]
        .iter()
        .enumerate()
        {
            let zip_path = root.join(format!("bad_{idx}.alp"));
            let dest = root.join(format!("dest_{idx}"));
            write_zip_entries(&zip_path, &[(bad_name, b"pwn", false)]);
            let err = install_pack(&zip_path, &dest).unwrap_err();
            assert!(
                matches!(err, PackError::PathTraversal(_)),
                "expected PathTraversal for {bad_name}, got {err:?}"
            );
            assert!(
                !dest.exists(),
                "destination should not be created when pre-flight rejects {bad_name}"
            );
        }

        let sym_zip = root.join("symlink.alp");
        let sym_dest = root.join("sym_dest");
        write_zip_entries(&sym_zip, &[("link.als", b"../outside", true)]);
        let err = install_pack(&sym_zip, &sym_dest).unwrap_err();
        assert!(matches!(err, PackError::PathTraversal(_)));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn enforces_entry_and_total_size_limits() {
        let root = make_temp_dir("size_limits");
        let zip_entry = root.join("big_entry.alp");
        write_zip_entries(&zip_entry, &[("big.bin", &[b'A'; 2048], false)]);
        let err = install_pack_with_limits(&zip_entry, &root.join("out1"), 1024, 8192).unwrap_err();
        assert!(matches!(err, PackError::EntryTooLarge { .. }));

        let zip_total = root.join("big_total.alp");
        write_zip_entries(
            &zip_total,
            &[
                ("a.bin", &[b'A'; 900], false),
                ("b.bin", &[b'B'; 900], false),
            ],
        );
        let err = install_pack_with_limits(&zip_total, &root.join("out2"), 1024, 1500).unwrap_err();
        assert!(matches!(err, PackError::TotalTooLarge { .. }));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_zip_decompression_bomb_with_spoofed_small_header_size() {
        let root = make_temp_dir("decompression_bomb");

        // Single entry declaring 100 bytes in headers while actual stream inflates to 64 KiB.
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut zw = ZipWriter::new(&mut cursor);
            let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
            zw.start_file("bomb.bin", opts).expect("start bomb");
            zw.write_all(&[b'A'; 65_536]).expect("write bomb");
            zw.finish().expect("finish bomb");
        }
        let mut zip_bytes = cursor.into_inner();
        spoof_zip_uncompressed_sizes(&mut zip_bytes, 100);
        let bomb_path = root.join("bomb_entry.alp");
        fs::write(&bomb_path, &zip_bytes).expect("write spoofed bomb zip");

        let out_entry = root.join("out_entry");
        let err = install_pack_with_limits(&bomb_path, &out_entry, 512, 65_536).unwrap_err();
        assert!(
            matches!(err, PackError::EntryTooLarge { limit: 512, .. })
                || matches!(&err, PackError::Io(io) if io.kind() == std::io::ErrorKind::InvalidData),
            "expected EntryTooLarge or InvalidData bomb rejection, got {err:?}"
        );
        assert!(
            !out_entry.join("bomb.bin").exists(),
            "partial bomb file should be removed when decompression bomb is rejected"
        );

        // Two entries each declaring 100 bytes and within per-entry limit (32 KiB),
        // while actual inflated streams exceed declared/total limit (30 KiB).
        let mut total_cursor = Cursor::new(Vec::new());
        {
            let mut zw = ZipWriter::new(&mut total_cursor);
            let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
            zw.start_file("part1.bin", opts).expect("start p1");
            zw.write_all(&[b'A'; 24_576]).expect("write p1");
            zw.start_file("part2.bin", opts).expect("start p2");
            zw.write_all(&[b'B'; 24_576]).expect("write p2");
            zw.finish().expect("finish total bomb");
        }
        let mut total_bytes = total_cursor.into_inner();
        spoof_zip_uncompressed_sizes(&mut total_bytes, 100);
        let total_path = root.join("bomb_total.alp");
        fs::write(&total_path, &total_bytes).expect("write spoofed total zip");

        let out_total = root.join("out_total");
        let err = install_pack_with_limits(&total_path, &out_total, 32_768, 30_000).unwrap_err();
        assert!(
            matches!(err, PackError::TotalTooLarge { limit: 30_000 })
                || matches!(&err, PackError::Io(io) if io.kind() == std::io::ErrorKind::InvalidData),
            "expected TotalTooLarge or InvalidData bomb rejection, got {err:?}"
        );
        assert!(
            !out_total.join("part1.bin").exists() && !out_total.join("part2.bin").exists(),
            "partial bomb files should be removed when rejected"
        );

        let _ = fs::remove_dir_all(&root);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        #[test]
        fn arbitrary_bytes_never_panic_in_install_pack(
            bytes in proptest::collection::vec(any::<u8>(), 0..1024)
        ) {
            let root = std::env::temp_dir().join(format!(
                "livecraft_pack_prop_{}",
                std::process::id()
            ));
            let _ = fs::create_dir_all(&root);
            let alp = root.join("fuzz.alp");
            let dest = root.join("dest");
            let _ = fs::remove_dir_all(&dest);
            let _ = fs::write(&alp, &bytes);
            let _ = install_pack(&alp, &dest);
            let _ = fs::remove_dir_all(&dest);
        }
    }
}
