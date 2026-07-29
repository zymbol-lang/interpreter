//! Error type for the whole crate: manifest parsing, engine checks, script resolution,
//! and archive read/write.

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum PackageError {
    #[error("invalid zyp.toml: {0}")]
    ManifestParse(#[from] toml::de::Error),

    #[error("invalid engine requirement '{req}' in zyp.toml: {source}")]
    EngineReq { req: String, #[source] source: semver::Error },

    #[error("invalid interpreter version '{version}': {source}")]
    EngineVersion { version: String, #[source] source: semver::Error },

    #[error("package '{name}' requires engine {required}, this interpreter is {current}")]
    EngineMismatch { name: String, required: String, current: String },

    #[error("no script named '{0}' in this package")]
    ScriptNotFound(String),

    #[error("no default script declared in zyp.toml, and none selected with --script (use --script <name>)")]
    NoDefaultScript,

    #[error("more than one [[script]] is marked default = true: {}", .0.join(", "))]
    MultipleDefaults(Vec<String>),

    #[error("duplicate [[script]] name '{0}' in zyp.toml")]
    DuplicateScriptName(String),

    #[error(
        "script '{name}' ({path}) is a module file (has a `#` module declaration) — \
         modules are imported with <#, not run directly"
    )]
    ScriptIsModule { name: String, path: String },

    // The field is named `cause`, not `source`, on purpose: thiserror treats any field
    // *named* `source` as the error's source even without a `#[source]` attribute, and
    // anyhow then prints that whole chain — re-printing this message's own io::Error text
    // verbatim under a "Caused by:" heading. One failure should read as one line.
    #[error("cannot open package '{path}': {cause}")]
    ArchiveOpen { path: String, cause: std::io::Error },

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error(
        "unsafe path in package: '{0}' — paths must be relative and stay inside the \
         package (no '..', no leading '/', no drive letter, no backslash)"
    )]
    UnsafePath(String),

    #[error("script '{name}' is declared in zyp.toml as '{path}', but that file is not in the archive")]
    ScriptMissingFromArchive { name: String, path: String },

    #[error("archive entry '{0}' is too large (exceeds the {1}-byte decompression limit)")]
    SizeLimit(String, u64),

    #[error("zip64 archives are not supported (entry '{0}' is too large or the archive has too many entries)")]
    Zip64Unsupported(String),

    #[error("zyp.toml not found in archive (expected at the archive root)")]
    ManifestMissing,

    #[error("zyp.toml not found at {0} (pass a directory containing one, or use --script to synthesize one)")]
    ManifestFileMissing(PathBuf),
}
