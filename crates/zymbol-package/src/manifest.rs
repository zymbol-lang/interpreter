//! `zyp.toml` manifest: package metadata and the list of runnable `[[script]]` entries.
//!
//! Read with [`Manifest::from_toml`]; the same struct serializes to `zyp.json` via
//! [`Manifest::to_json`] so the web playground never has to parse TOML in the browser
//! (see the crate-level docs for why that matters).

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::path_safety::validate_relative_path;
use crate::PackageError;

/// A parsed `zyp.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub package: PackageMeta,
    /// `[[script]]` entries. Order is preserved from the file — useful for the web
    /// playground's script picker, which lists them in manifest order.
    #[serde(rename = "script", default)]
    pub scripts: Vec<ScriptEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageMeta {
    pub name: String,
    pub version: String,
    /// Minimum interpreter version, as a semver requirement (e.g. `">=0.0.8"`).
    /// `None` means no check is performed. A *bare* version like `"0.0.8"` is parsed by
    /// the `semver` crate as `^0.0.8`, which pre-1.0 matches only that exact version — see
    /// [`Manifest::check_engine`]. Manifests written by `zymbol package` always emit `>=`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
    /// Default execution engine for this package's scripts. `--vm`/`--tw` on the CLI
    /// override this per invocation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<EngineMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
}

/// Which execution engine a package (or a `zymbol run` invocation) should use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EngineMode {
    /// Register VM (`zymbol run --vm`). The default for `.zyp` packages when the
    /// manifest doesn't say otherwise — plain `.zy` files keep defaulting to the
    /// tree-walker so existing behavior for loose scripts is unchanged.
    Vm,
    /// Tree-walking interpreter (`zymbol run --tw` / plain `zymbol run` on a `.zy` file).
    Tw,
}

impl std::fmt::Display for EngineMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            EngineMode::Vm => "vm",
            EngineMode::Tw => "tw",
        })
    }
}

/// One runnable script declared in the manifest. `path` is relative to the archive's
/// `src/` prefix (equivalently: relative to the manifest's directory when authoring).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptEntry {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub default: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
}

impl Manifest {
    /// Parses and validates a `zyp.toml` document.
    pub fn from_toml(s: &str) -> Result<Self, PackageError> {
        let manifest: Manifest = toml::from_str(s)?;
        manifest.validate()?;
        Ok(manifest)
    }

    fn validate(&self) -> Result<(), PackageError> {
        let mut seen = HashSet::new();
        for script in &self.scripts {
            if !seen.insert(script.name.as_str()) {
                return Err(PackageError::DuplicateScriptName(script.name.clone()));
            }
            // A `[[script]].path` is joined onto the extraction directory to decide what to
            // execute, so it is exactly as untrusted as a ZIP entry name and gets the same
            // lexical rule. Without this, `path = "../../elsewhere.zy"` in a hostile package
            // escaped the extraction directory and got arbitrary source on the user's disk
            // read and run. Rejected here, at parse time, so no caller can hold a `Manifest`
            // carrying an unsafe path in the first place.
            validate_relative_path(&script.path)?;
        }

        let defaults: Vec<String> = self
            .scripts
            .iter()
            .filter(|s| s.default)
            .map(|s| s.name.clone())
            .collect();
        if defaults.len() > 1 {
            return Err(PackageError::MultipleDefaults(defaults));
        }

        Ok(())
    }

    /// Serializes back to TOML (human-authored form). Used by `zymbol package` when it
    /// synthesizes a manifest from `--script` flags and prints it for the user to save.
    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).expect("Manifest always serializes to TOML")
    }

    /// Serializes to JSON — the `zyp.json` entry written alongside `zyp.toml` inside the
    /// archive, so the web playground never has to parse TOML in the browser.
    ///
    /// Deliberately does NOT just `serde_json::to_string(self)`: `scripts` carries
    /// `#[serde(rename = "script")]` so TOML can use the `[[script]]` array-of-tables
    /// syntax, but that same rename would leak into JSON too and produce `"script": [...]`
    /// — a singular key holding an array reads as a bug to any JS consumer expecting
    /// `manifest.scripts`. This shadow struct re-serializes under the field's natural
    /// (plural) name for JSON specifically, without disturbing the TOML shape at all.
    pub fn to_json(&self) -> String {
        #[derive(Serialize)]
        struct ManifestJson<'a> {
            package: &'a PackageMeta,
            scripts: &'a [ScriptEntry],
        }
        let shadow = ManifestJson { package: &self.package, scripts: &self.scripts };
        serde_json::to_string(&shadow).expect("Manifest always serializes to JSON")
    }

    /// Validates `self.package.engine` (a semver requirement) against the running
    /// interpreter's version. A manifest with no `engine` field always passes.
    ///
    /// Note the pre-1.0 semver trap: `engine = "0.0.8"` parses as `^0.0.8`, which matches
    /// *only* `0.0.8` exactly — it does not match `0.0.9`. Always write `engine = ">=0.0.8"`.
    pub fn check_engine(&self, current: &str) -> Result<(), PackageError> {
        let Some(req_str) = &self.package.engine else {
            return Ok(());
        };
        let req = semver::VersionReq::parse(req_str).map_err(|source| PackageError::EngineReq {
            req: req_str.clone(),
            source,
        })?;
        let cur = semver::Version::parse(current).map_err(|source| PackageError::EngineVersion {
            version: current.to_string(),
            source,
        })?;
        if !req.matches(&cur) {
            return Err(PackageError::EngineMismatch {
                name: self.package.name.clone(),
                required: req_str.clone(),
                current: current.to_string(),
            });
        }
        Ok(())
    }

    /// Picks the script to run: by `name` if given, otherwise the one marked
    /// `default = true`, otherwise — if there is exactly one script — that one.
    pub fn resolve_script(&self, name: Option<&str>) -> Result<&ScriptEntry, PackageError> {
        if let Some(name) = name {
            return self
                .scripts
                .iter()
                .find(|s| s.name == name)
                .ok_or_else(|| PackageError::ScriptNotFound(name.to_string()));
        }
        self.scripts
            .iter()
            .find(|s| s.default)
            .or(match self.scripts.len() {
                1 => self.scripts.first(),
                _ => None,
            })
            .ok_or(PackageError::NoDefaultScript)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GO_TOML: &str = r#"
[package]
name = "go"
version = "1.2.0"
engine = ">=0.0.8"
mode = "vm"

[[script]]
name = "go"
path = "go.zy"
default = true
desc = "English"

[[script]]
name = "囲碁"
path = "囲碁.zy"
desc = "日本語"
"#;

    #[test]
    fn parses_a_well_formed_manifest() {
        let m = Manifest::from_toml(GO_TOML).unwrap();
        assert_eq!(m.package.name, "go");
        assert_eq!(m.package.engine.as_deref(), Some(">=0.0.8"));
        assert_eq!(m.package.mode, Some(EngineMode::Vm));
        assert_eq!(m.scripts.len(), 2);
        assert_eq!(m.scripts[1].name, "囲碁");
    }

    #[test]
    fn rejects_duplicate_script_names() {
        let toml = r#"
[package]
name = "x"
version = "0.1.0"

[[script]]
name = "a"
path = "a.zy"

[[script]]
name = "a"
path = "b.zy"
"#;
        let err = Manifest::from_toml(toml).unwrap_err();
        assert!(matches!(err, PackageError::DuplicateScriptName(name) if name == "a"));
    }

    /// Regression test for the path-traversal vulnerability: a `[[script]].path` that
    /// escapes the package is joined onto the extraction directory to decide what to
    /// execute, so before this was rejected, a hostile `.zyp` could point its entry point at
    /// arbitrary source on the user's disk and get it read and run. Rejected at parse time,
    /// so no `Manifest` can even exist carrying such a path.
    #[test]
    fn rejects_script_paths_that_escape_the_package() {
        for evil in [
            "../../elsewhere.zy",
            "a/../../elsewhere.zy",
            "/etc/passwd",
            "C:/windows/x.zy",
            "..\\..\\x.zy",
            "./x.zy",
        ] {
            let toml = format!(
                "[package]\nname = \"x\"\nversion = \"0.1.0\"\n\n[[script]]\nname = \"a\"\npath = \"{}\"\n",
                evil.replace('\\', "\\\\")
            );
            let err = Manifest::from_toml(&toml)
                .expect_err(&format!("must reject script path {evil:?}"));
            assert!(matches!(err, PackageError::UnsafePath(_)), "for {evil:?} got {err:?}");
        }
    }

    #[test]
    fn accepts_ordinary_script_paths_including_subdirectories_and_cjk() {
        for ok in ["a.zy", "試験/描画試験.zy", "言語/한국어.zy"] {
            let toml = format!(
                "[package]\nname = \"x\"\nversion = \"0.1.0\"\n\n[[script]]\nname = \"a\"\npath = \"{ok}\"\n"
            );
            assert!(Manifest::from_toml(&toml).is_ok(), "must accept {ok:?}");
        }
    }

    #[test]
    fn rejects_more_than_one_default() {
        let toml = r#"
[package]
name = "x"
version = "0.1.0"

[[script]]
name = "a"
path = "a.zy"
default = true

[[script]]
name = "b"
path = "b.zy"
default = true
"#;
        let err = Manifest::from_toml(toml).unwrap_err();
        assert!(matches!(err, PackageError::MultipleDefaults(names) if names == vec!["a".to_string(), "b".to_string()]));
    }

    #[test]
    fn resolve_script_prefers_explicit_name_then_default_then_sole_entry() {
        let m = Manifest::from_toml(GO_TOML).unwrap();
        assert_eq!(m.resolve_script(Some("囲碁")).unwrap().path, "囲碁.zy");
        assert_eq!(m.resolve_script(None).unwrap().name, "go"); // default = true

        let single = Manifest::from_toml(
            r#"
[package]
name = "x"
version = "0.1.0"

[[script]]
name = "only"
path = "only.zy"
"#,
        )
        .unwrap();
        assert_eq!(single.resolve_script(None).unwrap().name, "only");
    }

    #[test]
    fn resolve_script_errors_with_no_default_and_multiple_scripts() {
        let toml = r#"
[package]
name = "x"
version = "0.1.0"

[[script]]
name = "a"
path = "a.zy"

[[script]]
name = "b"
path = "b.zy"
"#;
        let m = Manifest::from_toml(toml).unwrap();
        assert!(matches!(m.resolve_script(None), Err(PackageError::NoDefaultScript)));
        assert!(matches!(m.resolve_script(Some("nope")), Err(PackageError::ScriptNotFound(_))));
    }

    #[test]
    fn engine_check_passes_with_satisfying_version() {
        let m = Manifest::from_toml(GO_TOML).unwrap();
        assert!(m.check_engine("0.0.8").is_ok());
        assert!(m.check_engine("0.1.0").is_ok());
    }

    #[test]
    fn engine_check_fails_below_minimum() {
        let m = Manifest::from_toml(GO_TOML).unwrap();
        assert!(m.check_engine("0.0.7").is_err());
    }

    #[test]
    fn engine_check_absent_always_passes() {
        let toml = r#"
[package]
name = "x"
version = "0.1.0"

[[script]]
name = "a"
path = "a.zy"
"#;
        let m = Manifest::from_toml(toml).unwrap();
        assert!(m.check_engine("0.0.1").is_ok());
    }

    /// Documents the pre-1.0 semver trap called out in `check_engine`'s doc comment: a
    /// bare version is parsed as a caret requirement, which for a 0.x version matches only
    /// that exact version. `zymbol package` must always synthesize `engine = ">=x.y.z"`,
    /// never a bare version, or every patch release breaks every existing package.
    #[test]
    fn bare_pre_1_0_engine_version_matches_only_itself() {
        let toml = r#"
[package]
name = "x"
version = "0.1.0"
engine = "0.0.8"

[[script]]
name = "a"
path = "a.zy"
"#;
        let m = Manifest::from_toml(toml).unwrap();
        assert!(m.check_engine("0.0.8").is_ok());
        assert!(m.check_engine("0.0.9").is_err(), "bare 0.0.8 must NOT match 0.0.9 under caret semantics");
    }

    #[test]
    fn to_toml_and_to_json_roundtrip_through_from_toml() {
        let m = Manifest::from_toml(GO_TOML).unwrap();
        let reparsed = Manifest::from_toml(&m.to_toml()).unwrap();
        assert_eq!(reparsed.package.name, m.package.name);
        assert_eq!(reparsed.scripts.len(), m.scripts.len());

        let json = m.to_json();
        assert!(json.contains("\"engine\":\">=0.0.8\""));
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["package"]["name"], "go");

        // Regression check: `scripts` carries #[serde(rename = "script")] so TOML can use
        // `[[script]]`, but JSON must NOT inherit that — a JS consumer expects the plural,
        // idiomatic `manifest.scripts` array (see `to_json`'s doc comment for the story).
        assert!(parsed.get("script").is_none(), "JSON must not have a singular 'script' key");
        let scripts = parsed["scripts"].as_array().expect("JSON must have a 'scripts' array");
        assert_eq!(scripts.len(), m.scripts.len());
        assert_eq!(scripts[1]["name"], "囲碁");
    }
}
