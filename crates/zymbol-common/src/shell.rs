//! Locating the shell that `<\ \>` and `</ />` run their commands through.
//!
//! On Unix this is `sh`, and there is nothing to decide. On Windows there is no
//! `sh` in a stock install, so we look for one and fall back to `cmd.exe` if no
//! POSIX shell is present:
//!
//! 1. `ZYMBOL_SH`, for a shell in a place we would not think to look.
//! 2. `sh.exe` on `PATH`.
//! 3. Git for Windows, which ships a full MSYS2 `sh.exe` (that is `bash` in POSIX
//!    mode). Its location is derived from `git.exe` on `PATH` when possible, and
//!    otherwise from the standard install prefixes.
//! 4. `cmd.exe`, so that `<\ \>` still does something on a machine with no POSIX
//!    shell at all.
//!
//! The order matters: a POSIX shell is preferred wherever one exists, so a script
//! written on Linux keeps running unchanged on a Windows box that has Git
//! installed. Step 4 is a genuine change of meaning — `cmd.exe` does not
//! understand `$$`, `/dev/urandom` or pipelines the same way — so taking it emits
//! a one-time note on stderr rather than letting a script fail in a way that looks
//! like a bug in the script.
//!
//! WSL is deliberately absent from the list. `wsl.exe` would find a shell, but one
//! living in a different filesystem namespace, where the script's own `D:\project`
//! is `/mnt/d/project` and any path it built by hand no longer names the file it
//! means. For the same reason we never probe a bare `bash.exe` on `PATH`: on
//! Windows that name is usually the WSL launcher stub in `System32`, not a shell
//! that shares the caller's view of the disk.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// Which shell was found, and therefore which syntax a command is interpreted in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellKind {
    /// A POSIX `sh`. Commands mean what they mean on Linux and macOS.
    Posix,
    /// Windows `cmd.exe`. POSIX syntax will not work; see the module docs.
    WindowsCmd,
}

/// No shell could be found at all — on Windows this means even `cmd.exe` was
/// missing. Carries the places that were looked in, so the error can say what to
/// install rather than just that something failed.
#[derive(Debug, Clone)]
pub struct ShellNotFound {
    /// The candidates that were tried, in the order they were tried.
    pub tried: Vec<String>,
}

impl std::fmt::Display for ShellNotFound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "no shell found — `<\\ \\>` and `</ />` run their commands through one. Tried: {}.",
            self.tried.join(", ")
        )?;
        if cfg!(windows) {
            write!(
                f,
                " Install Git for Windows (which ships `sh.exe`), or set ZYMBOL_SH to the full path of a POSIX shell."
            )?;
        }
        Ok(())
    }
}

impl std::error::Error for ShellNotFound {}

/// The shell to run commands through, resolved once per process.
///
/// The lookup is cached because on Windows a full search touches the filesystem
/// several times, and `<\ \>` inside a loop should not pay that every iteration.
pub fn shell() -> Result<(&'static Path, ShellKind), ShellNotFound> {
    static SHELL: OnceLock<Result<(PathBuf, ShellKind), ShellNotFound>> = OnceLock::new();
    match SHELL.get_or_init(find_shell) {
        Ok((path, kind)) => Ok((path.as_path(), *kind)),
        Err(e) => Err(e.clone()),
    }
}

/// A `Command` that runs `script` through the shell.
///
/// Both engines go through this one function, so the tree-walker and the VM cannot
/// drift apart on which shell they use — a divergence that would only show up on
/// the platform where the answer is not obvious.
pub fn shell_command(script: &str) -> Result<Command, ShellNotFound> {
    let (sh, kind) = shell()?;
    let mut cmd = Command::new(sh);
    match kind {
        ShellKind::Posix => cmd.arg("-c").arg(script),
        ShellKind::WindowsCmd => cmd.arg("/C").arg(script),
    };
    if kind == ShellKind::Posix {
        with_shell_utilities_on_path(&mut cmd, sh);
    }
    Ok(cmd)
}

/// Put the shell's own directory on the child's `PATH`.
///
/// Finding `sh.exe` is only half of running a POSIX command. Git for Windows keeps
/// its shell and its coreutils together in `usr\bin`, but that directory is not on
/// the Windows `PATH` — so a shell spawned directly inherits a `PATH` with no
/// `date`, no `od`, no `tr`, and the script fails one line later with
/// `command not found` instead of failing at the shell.
///
/// Prepending rather than replacing: the script can still reach everything the user
/// has installed, and the shell's own tools win a name clash, which is what a script
/// written for POSIX expects.
///
/// This is a no-op on Unix, where the shell is `sh` with no directory of its own and
/// the inherited `PATH` is already the right one.
fn with_shell_utilities_on_path(cmd: &mut Command, sh: &Path) {
    let Some(dir) = sh.parent().filter(|d| !d.as_os_str().is_empty()) else {
        return;
    };
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    let mut dirs = vec![dir.to_path_buf()];
    dirs.extend(std::env::split_paths(&inherited).filter(|p| p != dir));
    if let Ok(joined) = std::env::join_paths(dirs) {
        cmd.env("PATH", joined);
    }
}

#[cfg(not(windows))]
fn find_shell() -> Result<(PathBuf, ShellKind), ShellNotFound> {
    // Unix has a POSIX shell at a path POSIX itself specifies. Resolving it further
    // would only risk diverging from what every other tool on the system means by
    // `sh`.
    if let Some(explicit) = std::env::var_os("ZYMBOL_SH") {
        return Ok((PathBuf::from(explicit), ShellKind::Posix));
    }
    Ok((PathBuf::from("sh"), ShellKind::Posix))
}

#[cfg(windows)]
fn find_shell() -> Result<(PathBuf, ShellKind), ShellNotFound> {
    let mut tried = Vec::new();

    if let Some(explicit) = std::env::var_os("ZYMBOL_SH") {
        let path = PathBuf::from(explicit);
        if path.is_file() {
            return Ok((path, ShellKind::Posix));
        }
        tried.push(format!("ZYMBOL_SH ({})", path.display()));
    }

    if let Some(on_path) = which_on_path("sh.exe") {
        return Ok((on_path, ShellKind::Posix));
    }
    tried.push("sh.exe on PATH".to_string());

    for candidate in git_for_windows_shells() {
        if candidate.is_file() {
            return Ok((candidate, ShellKind::Posix));
        }
        tried.push(candidate.display().to_string());
    }

    // Nothing POSIX on this machine. `cmd.exe` keeps `<\ \>` working for commands
    // that happen to be portable, at the cost of the ones that are not — so say so
    // once, on stderr, instead of letting a POSIX script fail as if it were wrong.
    if let Some(cmd) = windows_cmd() {
        eprintln!(
            "zymbol: no POSIX shell found ({}); running `<\\ \\>` through cmd.exe. \
             Commands written in POSIX shell syntax will not work — install Git for Windows \
             or set ZYMBOL_SH to get the portable behaviour back.",
            tried.join(", ")
        );
        return Ok((cmd, ShellKind::WindowsCmd));
    }
    tried.push("cmd.exe".to_string());

    Err(ShellNotFound { tried })
}

/// `cmd.exe`, via `COMSPEC` if it is set (it always is in practice) and via the
/// system directory if it is not.
#[cfg(windows)]
fn windows_cmd() -> Option<PathBuf> {
    if let Some(comspec) = std::env::var_os("COMSPEC") {
        let path = PathBuf::from(comspec);
        if path.is_file() {
            return Some(path);
        }
    }
    if let Some(root) = std::env::var_os("SystemRoot") {
        let path = PathBuf::from(root).join("System32").join("cmd.exe");
        if path.is_file() {
            return Some(path);
        }
    }
    which_on_path("cmd.exe")
}

/// The `sh.exe` candidates that a Git for Windows install would put on disk.
///
/// The install prefix is derived from `git.exe` on `PATH` first: a user who
/// installed Git somewhere non-standard still has it on `PATH`, and `git.exe`
/// lives at `<prefix>\cmd\git.exe` or `<prefix>\bin\git.exe`, both one level under
/// the prefix. The fixed paths afterwards cover Git being installed but not on
/// `PATH`.
#[cfg(windows)]
fn git_for_windows_shells() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(git) = which_on_path("git.exe") {
        if let Some(prefix) = git.parent().and_then(Path::parent) {
            candidates.push(prefix.join("usr").join("bin").join("sh.exe"));
            candidates.push(prefix.join("bin").join("sh.exe"));
        }
    }

    for var in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
        if let Some(base) = std::env::var_os(var) {
            let mut root = PathBuf::from(base);
            if var == "LOCALAPPDATA" {
                root.push("Programs");
            }
            root.push("Git");
            candidates.push(root.join("usr").join("bin").join("sh.exe"));
        }
    }

    candidates.dedup();
    candidates
}

/// Look `program` up in `PATH` by hand.
///
/// `Command::new("sh")` would search `PATH` on its own, but only at spawn time and
/// without telling us where it looked — and the point of this module is to be able
/// to say what was tried when nothing is found.
#[cfg(windows)]
fn which_on_path(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(program))
        .find(|candidate| candidate.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(not(windows))]
    fn unix_resolves_to_plain_sh() {
        let (path, kind) = find_shell().unwrap();
        assert_eq!(path, PathBuf::from("sh"));
        assert_eq!(kind, ShellKind::Posix);
    }

    #[test]
    fn shell_not_found_names_what_was_tried() {
        let err = ShellNotFound {
            tried: vec![
                "sh.exe on PATH".into(),
                r"C:\Program Files\Git\usr\bin\sh.exe".into(),
            ],
        };
        let msg = err.to_string();
        assert!(msg.contains("sh.exe on PATH"));
        assert!(msg.contains(r"C:\Program Files\Git\usr\bin\sh.exe"));
    }

    #[test]
    #[cfg(windows)]
    fn windows_error_points_at_a_remedy() {
        let msg = ShellNotFound {
            tried: vec!["sh.exe on PATH".into()],
        }
        .to_string();
        assert!(msg.contains("Git for Windows"));
        assert!(msg.contains("ZYMBOL_SH"));
    }

    #[test]
    #[cfg(windows)]
    fn git_for_windows_candidates_end_in_sh_exe() {
        for candidate in git_for_windows_shells() {
            assert_eq!(candidate.file_name().unwrap(), "sh.exe");
        }
    }

    #[test]
    #[cfg(windows)]
    fn cmd_takes_slash_c_not_dash_c() {
        // The argument form is the whole difference between the two shells, and
        // getting it wrong would look exactly like the bug this replaces.
        let mut cmd = Command::new(r"C:\Windows\System32\cmd.exe");
        cmd.arg("/C").arg("echo hi");
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
        assert_eq!(args, vec!["/C", "echo hi"]);
    }

    #[test]
    fn a_shell_is_found_on_this_machine() {
        // Unix always has `sh`; Windows always has `cmd.exe` even with nothing else.
        assert!(shell().is_ok());
    }

    #[test]
    fn shell_directory_is_prepended_to_child_path() {
        // Git for Windows keeps sh.exe and the coreutils in the same directory, and
        // that directory is not on the Windows PATH. Without this the shell starts
        // and then cannot find `date`.
        let mut cmd = Command::new("sh");
        with_shell_utilities_on_path(&mut cmd, Path::new("/opt/toolchain/bin/sh"));

        let path = cmd
            .get_envs()
            .find(|(k, _)| *k == std::ffi::OsStr::new("PATH"))
            .and_then(|(_, v)| v)
            .expect("PATH should have been set");
        let first = std::env::split_paths(path).next().unwrap();
        assert_eq!(first, PathBuf::from("/opt/toolchain/bin"));
    }

    #[test]
    fn bare_shell_name_leaves_path_alone() {
        // The Unix case: `sh` has no directory, so there is nothing to prepend and
        // the child should inherit the PATH untouched.
        let mut cmd = Command::new("sh");
        with_shell_utilities_on_path(&mut cmd, Path::new("sh"));
        assert_eq!(cmd.get_envs().count(), 0);
    }
}
