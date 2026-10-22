//! The effect surface of a program, and what `zymbol run --deny` refuses (G1).
//!
//! `zymbol-design/PREMISES.md` AGT-4: every way a program reaches outside itself has its
//! own mark or its own import, and the list is closed — `std/io`, `std/net`, `std/db`,
//! `std/time`, `std/random`, `<\ … \>`, `</ … />`, `<<` and `><`. AGT-3: a `<#` path and a
//! `</ … />` path are literals and there is no `eval`. Together they make the surface of a
//! program knowable **before it runs**: reading the entry file and following `<#` and
//! `</ />` reaches every line that can run, and every gate in those lines is a token or an
//! import. That is what lets `--deny` refuse a program before any of it has run, instead
//! of stopping it half way — after it has written a file and before it reaches the network.
//!
//! The walk is the packaging closure's (`closure.rs`), with two differences a refusal
//! needs and packaging does not: it follows an absolute or `~/` import (packaging skips
//! it, W001, because it cannot ship it), and it resolves a `</ />` again when the module
//! holding it is reached from a second script directory (packaging resolves it once, W004).
//! Whatever the walk cannot read — a file that is missing or does not parse, a `</ />`
//! target that does not exist — is reported as unverified, and `--deny` refuses that too:
//! it cannot say what an unread file would do.

use std::path::{Path, PathBuf};

/// One gate of AGT-4's list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Effect {
    Io,
    Net,
    Db,
    Time,
    Random,
    Shell,
    Sub,
    Stdin,
    Argv,
}

impl Effect {
    pub const ALL: [Effect; 9] = [
        Effect::Io,
        Effect::Net,
        Effect::Db,
        Effect::Time,
        Effect::Random,
        Effect::Shell,
        Effect::Sub,
        Effect::Stdin,
        Effect::Argv,
    ];

    /// The name `--deny` takes.
    pub fn name(self) -> &'static str {
        match self {
            Effect::Io => "io",
            Effect::Net => "net",
            Effect::Db => "db",
            Effect::Time => "time",
            Effect::Random => "random",
            Effect::Shell => "shell",
            Effect::Sub => "sub",
            Effect::Stdin => "stdin",
            Effect::Argv => "argv",
        }
    }

    /// How it is written in a program.
    pub fn mark(self) -> &'static str {
        match self {
            Effect::Io => "<# std/io",
            Effect::Net => "<# std/net",
            Effect::Db => "<# std/db",
            Effect::Time => "<# std/time",
            Effect::Random => "<# std/random",
            Effect::Shell => "<\\ … \\>",
            Effect::Sub => "</ … />",
            Effect::Stdin => "<<",
            Effect::Argv => "><",
        }
    }

    pub fn from_name(name: &str) -> Option<Effect> {
        Effect::ALL.into_iter().find(|e| e.name() == name)
    }

    /// The gate a `std/…` import opens, if it opens one: `std/math`, `std/json` and
    /// `std/term` compute and reach nothing.
    pub(crate) fn of_stdlib(path: &str) -> Option<Effect> {
        match path {
            "std/io" => Some(Effect::Io),
            "std/net" => Some(Effect::Net),
            "std/db" => Some(Effect::Db),
            "std/time" => Some(Effect::Time),
            "std/random" => Some(Effect::Random),
            _ => None,
        }
    }
}

/// `--deny io,net`: the names, checked. An unknown name is an error that lists the
/// known ones, so a typo never silently denies nothing.
pub fn parse_effects(names: &[String]) -> Result<Vec<Effect>, String> {
    let mut out = Vec::new();
    for raw in names {
        for name in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            match Effect::from_name(name) {
                Some(e) => {
                    if !out.contains(&e) {
                        out.push(e)
                    }
                }
                None => {
                    let known: Vec<&str> = Effect::ALL.iter().map(|e| e.name()).collect();
                    return Err(format!("unknown effect '{}' — the effects are {}", name, known.join(", ")));
                }
            }
        }
    }
    out.sort();
    Ok(out)
}

/// One use of a gate: which, in which file, on which line.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct EffectUse {
    pub file: PathBuf,
    pub line: u32,
    pub effect: Effect,
}

/// The surface of a program: every use of a gate in every file it can reach, and every
/// file the walk could not read, with why.
#[derive(Debug, Clone, Default)]
pub struct EffectReport {
    pub uses: Vec<EffectUse>,
    pub unverified: Vec<(PathBuf, String)>,
}

impl EffectReport {
    /// The uses `deny` refuses.
    pub fn denied<'a>(&'a self, deny: &'a [Effect]) -> impl Iterator<Item = &'a EffectUse> + 'a {
        self.uses.iter().filter(move |u| deny.contains(&u.effect))
    }
}

/// The effect surface of the program whose entry file is `entry`.
pub fn effects_of(entry: &Path) -> std::io::Result<EffectReport> {
    crate::closure::walk_for_effects(entry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(dir: &Path, rel: &str, src: &str) {
        let p = dir.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, src).unwrap();
    }

    fn surface(entry: &Path) -> Vec<(String, u32, &'static str)> {
        let r = effects_of(entry).unwrap();
        assert!(r.unverified.is_empty(), "unverified: {:?}", r.unverified);
        let base = entry.parent().unwrap();
        r.uses
            .iter()
            .map(|u| {
                let f = u.file.strip_prefix(base).unwrap_or(&u.file).to_string_lossy().replace('\\', "/");
                (f, u.line, u.effect.name())
            })
            .collect()
    }

    #[test]
    fn every_gate_of_agt4_is_seen_with_its_line() {
        let d = tempfile::tempdir().unwrap();
        write(
            d.path(),
            "main.zy",
            "<# std/io => io\n<# std/net => net\n<# std/db => db\n<# std/time => t\n<# std/random => r\n<# std/math => m\n<# std/json => j\nx = <\\ \"echo\" \\>\n<< nombre\n>< args\nr = </ ./sub.zy />\n",
        );
        write(d.path(), "sub.zy", ">> 1 ¶\n");
        let s = surface(&d.path().join("main.zy"));
        let names: Vec<_> = s.iter().map(|(f, l, e)| format!("{f}:{l} {e}")).collect();
        assert_eq!(
            names,
            [
                "main.zy:1 io",
                "main.zy:2 net",
                "main.zy:3 db",
                "main.zy:4 time",
                "main.zy:5 random",
                "main.zy:8 shell",
                "main.zy:9 stdin",
                "main.zy:10 argv",
                "main.zy:11 sub"
            ]
        );
    }

    #[test]
    fn a_gate_in_an_imported_module_or_a_sub_script_is_the_programs() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "main.zy", "<# ./lib/clima => clima\n>> clima::hoy() ¶\nr = </ ./tarea.zy />\n");
        write(
            d.path(),
            "lib/clima.zy",
            "# clima {\n    <# std/net => net\n    #> { hoy }\n    hoy() { <~ net::get(\"https://x\") }\n}\n",
        );
        write(d.path(), "tarea.zy", "x = <\\ \"date\" \\>\n");
        let s = surface(&d.path().join("main.zy"));
        assert!(s.contains(&("lib/clima.zy".into(), 2, "net")), "{s:?}");
        assert!(s.contains(&("tarea.zy".into(), 1, "shell")), "{s:?}");
        assert!(s.contains(&("main.zy".into(), 3, "sub")), "{s:?}");
    }

    #[test]
    fn an_absolute_import_is_followed_although_packaging_skips_it() {
        // An absolute import path is made of identifiers, and `tempdir()` names its
        // directory `.tmpXXXX`: this one is named so that it is one.
        let d = tempfile::Builder::new().prefix("zyfx").rand_bytes(8).tempdir_in("/tmp").unwrap();
        write(d.path(), "lejos/m.zy", "# m {\n    <# std/io => io\n    #> { f }\n    f() { <~ 1 }\n}\n");
        let abs = d.path().join("lejos/m").to_string_lossy().replace('\\', "/");
        assert!(
            abs.split('/').skip(1).all(|c| c.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_')),
            "the path must be identifiers: {abs}"
        );
        write(d.path(), "main.zy", &format!("<# {abs} => m\n>> m::f() ¶\n"));
        let s = surface(&d.path().join("main.zy"));
        assert!(s.iter().any(|(_, l, e)| *l == 2 && *e == "io"), "{s:?}");
    }

    #[test]
    fn a_sub_script_reached_from_two_script_dirs_is_resolved_from_both() {
        // `</ ./x.zy />` resolves against the running script's directory. `m` is imported
        // by `main.zy` and by `otro/s.zy`, so its `</ />` names two files: both are read.
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "main.zy", "<# ./m => m\n>> m::f() ¶\nr = </ ./otro/s.zy />\n");
        write(d.path(), "m.zy", "# m {\n    #> { f }\n    f() { <~ </ ./x.zy /> }\n}\n");
        write(d.path(), "x.zy", ">> 1 ¶\n");
        write(d.path(), "otro/s.zy", "<# ../m => m\n>> m::f() ¶\n");
        write(d.path(), "otro/x.zy", "<# std/net => net\n>> 2 ¶\n");
        let s = surface(&d.path().join("main.zy"));
        assert!(s.contains(&("otro/x.zy".into(), 1, "net")), "{s:?}");
    }

    #[test]
    fn what_cannot_be_read_is_unverified_not_clean() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "main.zy", "<# ./roto => r\nz = </ ./falta.zy />\n");
        write(d.path(), "roto.zy", "# r {\n    #> { f }\n    f( { \n}\n");
        let r = effects_of(&d.path().join("main.zy")).unwrap();
        let why: Vec<String> = r.unverified.iter().map(|(_, w)| w.clone()).collect();
        assert_eq!(r.unverified.len(), 2, "{why:?}");
    }

    #[test]
    fn deny_names_are_checked() {
        assert_eq!(parse_effects(&["net,shell".into()]).unwrap(), vec![Effect::Net, Effect::Shell]);
        assert_eq!(parse_effects(&["io".into(), "io".into()]).unwrap(), vec![Effect::Io]);
        let e = parse_effects(&["nett".into()]).unwrap_err();
        assert!(e.contains("unknown effect 'nett'") && e.contains("io, net, db"), "{e}");
    }
}
