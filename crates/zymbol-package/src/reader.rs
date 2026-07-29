//! Reading a `.zyp` archive: parse its manifest and safely extract its `src/` tree to a
//! directory (typically a caller-owned temp dir — see `zymbol run`'s handling of `.zyp`).

use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use crate::manifest::Manifest;
use crate::PackageError;

/// Per-entry and total decompressed-size ceiling. Chosen as a pragmatic zip-bomb guard:
/// rather than parsing zip64 extra fields to detect "is this entry zip64-encoded", any
/// entry (or running total) over this size is rejected outright — which also covers the
/// zip64 case in practice, since zip64 only exists to describe files/archives past the
/// 4 GiB boundary and this ceiling is far below that.
const MAX_ENTRY_BYTES: u64 = 100 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 100 * 1024 * 1024;

/// An opened `.zyp`: its parsed manifest, plus every archive entry's bytes already read
/// into memory (archives are small source trees, not asset bundles — see the `include`
/// TODO in the crate docs for why non-`.zy` assets aren't in scope yet).
#[derive(Debug)]
pub struct Package {
    pub manifest: Manifest,
    entries: Vec<(String, Vec<u8>)>,
}

/// Opens and validates a `.zyp` file: reads every entry, rejects anything that looks like a
/// zip-slip attempt or a decompression bomb, and parses `zyp.toml`.
pub fn open_zyp(path: &Path) -> Result<Package, PackageError> {
    // Named error rather than the blanket `?` into PackageError::Io: opening the archive is
    // the one I/O failure a user hits routinely (a mistyped path), and letting it fall
    // through produced three stacked lines all saying "No such file or directory" — the
    // caller's `failed to open X` context, `io error: …`, and the raw `io::Error` beneath it.
    let file = fs::File::open(path).map_err(|cause| PackageError::ArchiveOpen {
        path: path.display().to_string(),
        cause,
    })?;
    let mut archive = zip::ZipArchive::new(file)?;

    let mut manifest_toml: Option<String> = None;
    let mut entries = Vec::with_capacity(archive.len());
    let mut total_bytes: u64 = 0;

    for i in 0..archive.len() {
        let mut zf = archive.by_index(i)?;
        let name = zf.name().to_string();

        if zf.is_dir() {
            continue;
        }

        let size = zf.size();
        if size > MAX_ENTRY_BYTES {
            return Err(PackageError::SizeLimit(name, MAX_ENTRY_BYTES));
        }
        total_bytes += size;
        if total_bytes > MAX_TOTAL_BYTES {
            return Err(PackageError::SizeLimit(name, MAX_TOTAL_BYTES));
        }

        // Validate now, before anything is written anywhere — extract_to() re-validates
        // per-entry too (defense in depth), but failing here means a hostile archive never
        // even gets its manifest parsed.
        validate_entry_name(&name)?;

        let mut buf = Vec::with_capacity(size as usize);
        zf.read_to_end(&mut buf)?;

        if name == "zyp.toml" {
            manifest_toml = Some(String::from_utf8_lossy(&buf).into_owned());
        }
        entries.push((name, buf));
    }

    let manifest_toml = manifest_toml.ok_or(PackageError::ManifestMissing)?;
    let manifest = Manifest::from_toml(&manifest_toml)?;

    Ok(Package { manifest, entries })
}

impl Package {
    /// Extracts every archive entry under `dest_root`, creating parent directories as
    /// needed. Every entry name is validated lexically *before* any path is joined with
    /// `dest_root` — no `..`, no absolute prefix, no backslash, no NUL, no Windows drive
    /// letter, and the joined path is confirmed to still start with `dest_root` (checked
    /// as a plain path comparison, not `canonicalize()`, since the destination file doesn't
    /// exist yet).
    pub fn extract_to(&self, dest_root: &Path) -> Result<(), PackageError> {
        for (name, bytes) in &self.entries {
            let dest = safe_join(dest_root, name)?;
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&dest, bytes)?;
        }
        Ok(())
    }

    /// Absolute path of `script.path`'s source once `extract_to` has run.
    ///
    /// Fallible on purpose: `script_path` comes from the archive's own `zyp.toml`, so it is
    /// as untrusted as any ZIP entry name and gets the same containment check. `Manifest`
    /// already rejects unsafe script paths at parse time, so reaching an error here means a
    /// caller built a `Manifest` some other way — the check stays regardless, because the
    /// original vulnerability in this crate was exactly a path that "couldn't happen"
    /// arriving through a second, unguarded door.
    ///
    /// Returns [`PackageError::ScriptMissingFromArchive`] when the path is safe but the
    /// file simply isn't in the archive, so the caller can say so plainly instead of
    /// surfacing a bare "no such file" against an opaque temp-directory path.
    pub fn script_abs_path(
        &self,
        dest_root: &Path,
        script_name: &str,
        script_path: &str,
    ) -> Result<PathBuf, PackageError> {
        let src_root = dest_root.join("src");
        let resolved = safe_join(&src_root, script_path)?;
        if !resolved.is_file() {
            return Err(PackageError::ScriptMissingFromArchive {
                name: script_name.to_string(),
                path: script_path.to_string(),
            });
        }
        Ok(resolved)
    }
}

use crate::path_safety::validate_relative_path as validate_entry_name;

fn safe_join(dest_root: &Path, name: &str) -> Result<PathBuf, PackageError> {
    validate_entry_name(name)?;
    let mut result = dest_root.to_path_buf();
    for part in name.split('/') {
        if part.is_empty() {
            continue;
        }
        result.push(part);
    }
    if !result.starts_with(dest_root) {
        return Err(PackageError::UnsafePath(name.to_string()));
    }
    Ok(result)
}
