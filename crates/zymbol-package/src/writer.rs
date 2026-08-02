//! Writing a `.zyp` archive: a deterministic ZIP containing `zyp.toml`, `zyp.json` (the
//! same manifest, pre-parsed to JSON so the web playground never has to parse TOML), and
//! the packaged source tree under `src/`.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use zip::write::{FileOptions, ZipWriter};
use zip::{CompressionMethod, DateTime};

use crate::closure::{ClosureResult, PackageWarning, WarningKind};
use crate::manifest::Manifest;
use crate::PackageError;

/// Recommended size ceiling (W010). Not enforced — packaging is permissive — only flagged.
const SIZE_WARNING_BYTES: u64 = 5 * 1024 * 1024;

/// Fixed timestamp for every entry (1980-01-01, the minimum representable DOS/ZIP time) so
/// the same source tree always produces a byte-identical archive — useful for verifying a
/// `.zyp` by hash.
fn fixed_mtime() -> DateTime {
    DateTime::from_date_and_time(1980, 1, 1, 0, 0, 0)
        .expect("1980-01-01 00:00:00 is a representable DOS date/time")
}

/// Writes `manifest` and the files in `closure` to `out` as a `.zyp` archive. Entries are
/// written in a fixed order (`zyp.toml`, `zyp.json`, then `src/*` sorted by
/// [`ClosureResult::files`], which is already sorted) so identical inputs produce an
/// identical archive byte-for-byte.
///
/// Returns any additional warnings discovered while writing (currently just W010, the
/// total-size check — it needs the actual file bytes, which `compute_closure` doesn't read).
pub fn write_zyp(manifest: &Manifest, closure: &ClosureResult, out: &Path) -> Result<Vec<PackageWarning>, PackageError> {
    let file = File::create(out)?;
    let mut zip = ZipWriter::new(file);
    let mtime = fixed_mtime();

    let stored: FileOptions<()> = FileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .last_modified_time(mtime)
        .unix_permissions(0o644);
    let deflated: FileOptions<()> = FileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(mtime)
        .unix_permissions(0o644);

    zip.start_file("zyp.toml", stored)?;
    zip.write_all(manifest.to_toml().as_bytes())?;

    zip.start_file("zyp.json", stored)?;
    zip.write_all(manifest.to_json().as_bytes())?;

    let mut total_bytes: u64 = 0;
    for packaged in &closure.files {
        validate_entry_name(&packaged.rel_path)?;
        let bytes = std::fs::read(&packaged.abs_path)?;
        total_bytes += bytes.len() as u64;
        let entry_name = format!("src/{}", packaged.rel_path);
        zip.start_file(&entry_name, deflated)?;
        zip.write_all(&bytes)?;
    }

    zip.finish()?;

    let mut extra_warnings = Vec::new();
    if total_bytes > SIZE_WARNING_BYTES {
        extra_warnings.push(PackageWarning { file: PathBuf::new(), kind: WarningKind::SizeLimit(total_bytes) });
    }
    Ok(extra_warnings)
}

// Defense in depth: `compute_closure` only ever derives `rel_path` from lexical joins of
// import components, so a `..`/absolute entry name shouldn't be possible by construction —
// but a closure can also be handed to `write_zyp` directly by a future caller, so the check
// is repeated at write time rather than assumed. The rule itself lives in `path_safety`.
use crate::path_safety::validate_relative_path as validate_entry_name;
