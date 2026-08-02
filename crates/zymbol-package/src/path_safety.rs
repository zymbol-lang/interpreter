//! One lexical rule for "is this relative path safe to join onto a directory we control?",
//! shared by every place a `.zyp` supplies a path.
//!
//! There are three such places, and an earlier version of this crate checked only two of
//! them — ZIP entry names on read ([`crate::open_zyp`]) and on write ([`crate::write_zyp`]),
//! each with its own copy of the rule. The third, `[[script]].path` in `zyp.toml`, went
//! unchecked, which made a hostile `.zyp` able to point its entry point outside the
//! extraction directory (`path = "../../elsewhere.zy"`) and get arbitrary source on the
//! user's disk read and executed. Two copies of a security rule is how the third site gets
//! forgotten, so there is now exactly one.
//!
//! This is deliberately a *lexical* check — no `canonicalize()`, no filesystem access. The
//! destination file does not exist yet at validation time, and resolving symlinks would
//! also normalize Unicode on some platforms (macOS/APFS returns NFD), corrupting the
//! CJK/Hangul path components a `.zyp` is expected to carry.

use crate::PackageError;

/// Validates a path supplied by an archive (a ZIP entry name, or a `[[script]].path`)
/// before it is joined onto a directory we control.
///
/// Rejects: empty paths, NUL bytes, backslashes (Windows separators — also blocks
/// `..\..\` traversal), absolute paths, Windows drive letters (`C:/...`), and any `.` or
/// `..` component.
pub(crate) fn validate_relative_path(path: &str) -> Result<(), PackageError> {
    if path.is_empty()
        || path.contains('\0')
        || path.contains('\\')
        || path.starts_with('/')
    {
        return Err(PackageError::UnsafePath(path.to_string()));
    }
    // Windows drive letter, e.g. "C:/..." — the `C:\...` form is already caught by the
    // backslash check above.
    if path.as_bytes().get(1) == Some(&b':') {
        return Err(PackageError::UnsafePath(path.to_string()));
    }
    if path.split('/').any(|part| part == ".." || part == ".") {
        return Err(PackageError::UnsafePath(path.to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_ordinary_relative_paths_including_non_ascii() {
        for ok in ["a.zy", "src/a.zy", "核/盤.zy", "言語/한국어.zy", "a/b/c/d.zy"] {
            assert!(validate_relative_path(ok).is_ok(), "should accept {ok}");
        }
    }

    #[test]
    fn rejects_traversal_and_absolute_and_windows_forms() {
        for bad in [
            "",
            "../x.zy",
            "a/../../x.zy",
            "./x.zy",
            "/etc/passwd",
            "C:/windows/x.zy",
            "a\\b.zy",
            "..\\..\\x.zy",
            "a\0b.zy",
            "..",
            ".",
        ] {
            assert!(
                validate_relative_path(bad).is_err(),
                "should reject {bad:?}"
            );
        }
    }
}
