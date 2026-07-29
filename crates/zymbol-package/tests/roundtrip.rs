//! Integration tests for the write → read → extract round trip, plus the zip-slip guard.
//! Exercises only the public API (`Manifest`, `compute_closure`, `write_zyp`, `open_zyp`,
//! `Package::extract_to`), the same surface a CLI or a future package manager would use.

use std::fs;
use std::io::Write as _;

use zymbol_package::{compute_closure, open_zyp, write_zyp, Manifest, PackageError};

fn build_fixture(dir: &std::path::Path) {
    fs::create_dir_all(dir.join("lib")).unwrap();
    fs::write(
        dir.join("entry.zy"),
        "<# ./lib/a => a\na::hola()\n",
    )
    .unwrap();
    fs::write(
        dir.join("lib/a.zy"),
        "# a {\n\n#> {\n\x20\x20\x20\x20hola\n}\n\nhola() {\n\x20\x20\x20\x20>> \"hola\" \u{b6}\n}\n}\n",
    )
    .unwrap();
}

fn manifest_toml() -> String {
    r#"
[package]
name = "fixture"
version = "0.1.0"
engine = ">=0.0.1"
mode = "vm"

[[script]]
name = "entry"
path = "entry.zy"
default = true
"#
    .to_string()
}

#[test]
fn write_then_read_then_extract_roundtrips_manifest_and_files() {
    let src = tempfile::tempdir().unwrap();
    build_fixture(src.path());

    let manifest = Manifest::from_toml(&manifest_toml()).unwrap();
    let closure = compute_closure(&[src.path().join("entry.zy")]).unwrap();
    assert!(closure.warnings.is_empty(), "warnings: {:#?}", closure.warnings);

    let archive_dir = tempfile::tempdir().unwrap();
    let archive_path = archive_dir.path().join("fixture.zyp");
    let extra_warnings = write_zyp(&manifest, &closure, &archive_path).unwrap();
    assert!(extra_warnings.is_empty());
    assert!(archive_path.is_file());

    let pkg = open_zyp(&archive_path).unwrap();
    assert_eq!(pkg.manifest.package.name, "fixture");
    assert_eq!(pkg.manifest.scripts.len(), 1);
    assert_eq!(pkg.manifest.scripts[0].path, "entry.zy");

    let extract_dir = tempfile::tempdir().unwrap();
    pkg.extract_to(extract_dir.path()).unwrap();

    let entry_src = fs::read_to_string(extract_dir.path().join("src/entry.zy")).unwrap();
    assert!(entry_src.contains("a::hola()"));
    let a_src = fs::read_to_string(extract_dir.path().join("src/lib/a.zy")).unwrap();
    assert!(a_src.contains("hola desde".split(' ').next().unwrap())); // "hola" substring, loosely

    // zyp.toml/zyp.json are extracted too — harmless, and useful if a caller wants to
    // inspect the manifest of an already-extracted package without reopening the archive.
    assert!(extract_dir.path().join("zyp.toml").is_file());
    assert!(extract_dir.path().join("zyp.json").is_file());

    let entry = &pkg.manifest.scripts[0];
    let script_path = pkg.script_abs_path(extract_dir.path(), &entry.name, &entry.path).unwrap();
    assert_eq!(script_path, extract_dir.path().join("src/entry.zy"));
}

#[test]
fn writing_the_same_source_tree_twice_is_byte_identical() {
    // Determinism (fixed mtime, sorted entries) matters: it's what makes a `.zyp` verifiable
    // by hash and makes `zymbol package` reproducible in CI.
    let src = tempfile::tempdir().unwrap();
    build_fixture(src.path());
    let manifest = Manifest::from_toml(&manifest_toml()).unwrap();
    let closure = compute_closure(&[src.path().join("entry.zy")]).unwrap();

    let out_dir = tempfile::tempdir().unwrap();
    let path_a = out_dir.path().join("a.zyp");
    let path_b = out_dir.path().join("b.zyp");
    write_zyp(&manifest, &closure, &path_a).unwrap();
    // A small sleep-free delay isn't needed — the point is the fixed mtime in the writer,
    // not wall-clock time — but write again from scratch to prove it's not accidental.
    write_zyp(&manifest, &closure, &path_b).unwrap();

    let bytes_a = fs::read(&path_a).unwrap();
    let bytes_b = fs::read(&path_b).unwrap();
    assert_eq!(bytes_a, bytes_b, "identical inputs must produce a byte-identical archive");
}

#[test]
fn rejects_engine_mismatch_before_any_extraction() {
    let src = tempfile::tempdir().unwrap();
    build_fixture(src.path());
    let manifest = Manifest::from_toml(
        r#"
[package]
name = "fixture"
version = "0.1.0"
engine = ">=99.0.0"

[[script]]
name = "entry"
path = "entry.zy"
default = true
"#,
    )
    .unwrap();
    let closure = compute_closure(&[src.path().join("entry.zy")]).unwrap();
    let archive_dir = tempfile::tempdir().unwrap();
    let archive_path = archive_dir.path().join("fixture.zyp");
    write_zyp(&manifest, &closure, &archive_path).unwrap();

    let pkg = open_zyp(&archive_path).unwrap();
    let err = pkg.manifest.check_engine("0.0.8").unwrap_err();
    assert!(matches!(err, PackageError::EngineMismatch { .. }));
}

/// Builds a `.zyp`-shaped ZIP by hand (bypassing `write_zyp`) with a zip-slip entry name, to
/// prove `extract_to` rejects it — this is the one thing `write_zyp` itself should never be
/// able to produce, so the malicious archive has to be built independently of this crate's
/// own writer.
fn write_malicious_zip(path: &std::path::Path, evil_name: &str) {
    let file = fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options: zip::write::FileOptions<()> = zip::write::FileOptions::default();

    zip.start_file("zyp.toml", options).unwrap();
    zip.write_all(
        b"[package]\nname = \"evil\"\nversion = \"0.1.0\"\n\n[[script]]\nname = \"e\"\npath = \"e.zy\"\n",
    )
    .unwrap();

    zip.start_file(evil_name, options).unwrap();
    zip.write_all(b"payload").unwrap();

    zip.finish().unwrap();
}

// Both zip-slip tests below expect the *rejection* at `open_zyp` time, not `extract_to`
// time: `open_zyp` validates every entry name up front (before even parsing the manifest),
// on the principle that a hostile archive shouldn't get as far as having its manifest
// trusted at all. `extract_to` repeats the same check (defense in depth, for callers that
// might one day construct a `Package` some other way), but in practice `open_zyp` always
// catches it first.

#[test]
fn open_zyp_rejects_dot_dot_zip_slip() {
    let dir = tempfile::tempdir().unwrap();
    let archive_path = dir.path().join("evil.zyp");
    write_malicious_zip(&archive_path, "../../../tmp/zyp_slip_poc");

    let err = open_zyp(&archive_path).unwrap_err();
    assert!(matches!(err, PackageError::UnsafePath(_)), "got: {err:?}");
}

#[test]
fn open_zyp_rejects_absolute_path_entry() {
    let dir = tempfile::tempdir().unwrap();
    let archive_path = dir.path().join("evil.zyp");
    write_malicious_zip(&archive_path, "/etc/passwd_poc");

    let err = open_zyp(&archive_path).unwrap_err();
    assert!(matches!(err, PackageError::UnsafePath(_)), "got: {err:?}");
}

/// End-to-end regression test for the path-traversal vulnerability, exercising the same
/// shape of archive a hostile package would have: a well-formed `zyp.toml` whose
/// `[[script]].path` escapes the extraction directory, plus a `src/` entry (needed because
/// the OS can only resolve `src/..` if `src` actually exists — without it the traversal
/// fails for the wrong reason and the test would pass vacuously).
///
/// Before the fix, `zymbol run` on this read and executed whatever `../../` pointed at.
#[test]
fn open_zyp_rejects_a_script_path_that_escapes_the_package() {
    let dir = tempfile::tempdir().unwrap();
    let archive_path = dir.path().join("evil.zyp");

    let file = fs::File::create(&archive_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options: zip::write::FileOptions<()> = zip::write::FileOptions::default();

    zip.start_file("zyp.toml", options).unwrap();
    zip.write_all(
        b"[package]\nname = \"evil\"\nversion = \"0.1.0\"\n\n\
          [[script]]\nname = \"pwn\"\npath = \"../../elsewhere.zy\"\ndefault = true\n",
    )
    .unwrap();
    zip.start_file("src/harmless.zy", options).unwrap();
    zip.write_all(b">> \"harmless\"\n").unwrap();
    zip.finish().unwrap();

    let err = open_zyp(&archive_path).unwrap_err();
    assert!(matches!(err, PackageError::UnsafePath(_)), "got: {err:?}");
}

#[test]
fn missing_manifest_is_a_clean_error_not_a_panic() {
    let dir = tempfile::tempdir().unwrap();
    let archive_path = dir.path().join("no_manifest.zyp");
    let file = fs::File::create(&archive_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options: zip::write::FileOptions<()> = zip::write::FileOptions::default();
    zip.start_file("src/entry.zy", options).unwrap();
    zip.write_all(b">> \"hi\"\n").unwrap();
    zip.finish().unwrap();

    let err = open_zyp(&archive_path).unwrap_err();
    assert!(matches!(err, PackageError::ManifestMissing));
}
