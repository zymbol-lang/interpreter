//! What `<<|` and `<<|?` hand a program for a key: one character, and the
//! modifiers held with it.
//!
//! Every engine and the `--keys` script go through [`key`], so a terminal, a
//! browser and a key script cannot disagree about what a key is. Each engine
//! only translates its own event (crossterm's `KeyEvent`, the browser's
//! `KeyboardEvent`) into a [`Code`] and a [`Mods`].
//!
//! - **The keys that are not text arrive as glyphs** (D-13, 2026-11-22), the way
//!   the arrows always did: `⇱` Home, `⇲` End, `⇞` Page Up, `⇟` Page Down,
//!   `⌦` Delete, `⇤` Shift+Tab, `①`–`⑫` F1–F12. They used to arrive as `'\0'`,
//!   all of them, which is the same value `<<|?` gives when no key was pressed
//!   (DIS-ZYE-001).
//! - **The modifiers are a number**, read with `<<| k, m` (D-14): Shift = 1,
//!   Alt = 2, Ctrl = 4, summed. Ctrl+letter still arrives as its control
//!   character, with `m = 4`.
//! - **Shift is not reported on text** (D-15): `'A'` already says Shift, and
//!   what the platforms report for it differs — a terminal marks every capital
//!   as shifted, Caps Lock included, and the Windows console marks the Shift
//!   key. Ctrl+Alt on a printable character is AltGr, which is how Windows and
//!   browsers report it, and gives `m = 0`.

/// `m`'s bits.
pub const SHIFT: i64 = 1;
pub const ALT: i64 = 2;
pub const CTRL: i64 = 4;

pub const UP: char = '\u{2191}'; // ↑
pub const DOWN: char = '\u{2193}'; // ↓
pub const LEFT: char = '\u{2190}'; // ←
pub const RIGHT: char = '\u{2192}'; // →
pub const HOME: char = '\u{21F1}'; // ⇱
pub const END: char = '\u{21F2}'; // ⇲
pub const PAGE_UP: char = '\u{21DE}'; // ⇞
pub const PAGE_DOWN: char = '\u{21DF}'; // ⇟
pub const DELETE: char = '\u{2326}'; // ⌦
pub const BACK_TAB: char = '\u{21E4}'; // ⇤
/// F1; F2…F12 follow it: `①`–`⑫`.
pub const F1: char = '\u{2460}';

/// A key, as an engine's event names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Code {
    Char(char),
    Up,
    Down,
    Left,
    Right,
    Enter,
    Esc,
    Tab,
    BackTab,
    Backspace,
    Home,
    End,
    PageUp,
    PageDown,
    Delete,
    /// F1–F12; any other number arrives as `'\0'`.
    F(u8),
    /// A key the language does not name (Insert, media keys, a lone Shift):
    /// `'\0'`, as before.
    Other,
}

/// The modifiers held with a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Mods {
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
}

/// The character and the modifier number a program receives for a key.
pub fn key(code: Code, mods: Mods) -> (char, i64) {
    let alt = if mods.alt { ALT } else { 0 };
    let ctrl = if mods.ctrl { CTRL } else { 0 };
    let shift = if mods.shift { SHIFT } else { 0 };
    match code {
        Code::Char(c) => {
            // Ctrl+letter is a control character, and that is what the
            // terminal puts on the wire: Ctrl+A is 0x01, Ctrl+S is 0x13
            // (BUG-ZYB-006: it used to arrive as the letter).
            if mods.ctrl && c.is_ascii_alphabetic() {
                let ch = (c.to_ascii_lowercase() as u8 - b'a' + 1) as char;
                return (ch, ctrl | alt);
            }
            // AltGr: Ctrl+Alt on a printable character (D-15).
            if mods.ctrl && mods.alt && !c.is_control() {
                return (c, 0);
            }
            // Shift is in the character already (D-15).
            (c, ctrl | alt)
        }
        // Shift+Tab is Back Tab, whoever reports it: crossterm names it
        // `BackTab`, a browser says Tab with Shift held.
        Code::Tab if mods.shift => (BACK_TAB, SHIFT | ctrl | alt),
        Code::BackTab => (BACK_TAB, SHIFT | ctrl | alt),
        Code::Other => ('\0', 0),
        Code::F(n) if !(1..=12).contains(&n) => ('\0', 0),
        named => {
            let ch = match named {
                Code::Up => UP,
                Code::Down => DOWN,
                Code::Left => LEFT,
                Code::Right => RIGHT,
                Code::Enter => '\n',
                Code::Esc => '\x1B',
                // Tab and Backspace used to fall through to `'\0'` together,
                // so a form could not tell a jump between fields from an
                // erase.
                Code::Tab => '\t',
                Code::Backspace => '\x7F', // DEL, which is what a terminal sends
                Code::Home => HOME,
                Code::End => END,
                Code::PageUp => PAGE_UP,
                Code::PageDown => PAGE_DOWN,
                Code::Delete => DELETE,
                Code::F(n) => char::from_u32(F1 as u32 + n as u32 - 1).unwrap_or('\0'),
                Code::Char(_) | Code::BackTab | Code::Other => unreachable!(),
            };
            (ch, shift | ctrl | alt)
        }
    }
}

/// The key a `--keys` script names: `UP`, `HOME`, `F5`, `BACKTAB`, `SPACE`, a
/// single character. Modifier prefixes are read by the script, not here.
pub fn named(name: &str) -> Option<Code> {
    Some(match name {
        "SPACE" => Code::Char(' '),
        "ENTER" => Code::Enter,
        "ESC" => Code::Esc,
        "TAB" => Code::Tab,
        "BACKTAB" => Code::BackTab,
        "BACKSPACE" => Code::Backspace,
        "UP" => Code::Up,
        "DOWN" => Code::Down,
        "LEFT" => Code::Left,
        "RIGHT" => Code::Right,
        "HOME" => Code::Home,
        "END" => Code::End,
        "PGUP" => Code::PageUp,
        "PGDN" => Code::PageDown,
        "DEL" => Code::Delete,
        _ if name.len() > 1 && name.starts_with('F') => {
            let n: u8 = name[1..].parse().ok()?;
            if !(1..=12).contains(&n) {
                return None;
            }
            Code::F(n)
        }
        _ => {
            let mut chars = name.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => Code::Char(c),
                _ => return None,
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: Mods = Mods { shift: false, alt: false, ctrl: false };
    const SH: Mods = Mods { shift: true, alt: false, ctrl: false };
    const AL: Mods = Mods { shift: false, alt: true, ctrl: false };
    const CT: Mods = Mods { shift: false, alt: false, ctrl: true };
    const ALTGR: Mods = Mods { shift: false, alt: true, ctrl: true };

    #[test]
    fn glyphs_for_the_keys_that_are_not_text() {
        assert_eq!(key(Code::Home, NONE), ('⇱', 0));
        assert_eq!(key(Code::End, NONE), ('⇲', 0));
        assert_eq!(key(Code::PageUp, NONE), ('⇞', 0));
        assert_eq!(key(Code::PageDown, NONE), ('⇟', 0));
        assert_eq!(key(Code::Delete, NONE), ('⌦', 0));
        assert_eq!(key(Code::F(1), NONE), ('①', 0));
        assert_eq!(key(Code::F(12), NONE), ('⑫', 0));
        assert_eq!(key(Code::F(13), NONE), ('\0', 0));
        assert_eq!(key(Code::Other, SH), ('\0', 0));
    }

    #[test]
    fn shift_is_reported_on_named_keys_only() {
        assert_eq!(key(Code::Right, SH), ('→', SHIFT));
        assert_eq!(key(Code::Char('A'), SH), ('A', 0));
        assert_eq!(key(Code::Char('!'), SH), ('!', 0));
        assert_eq!(key(Code::Char(' '), SH), (' ', 0));
    }

    #[test]
    fn back_tab_carries_shift_however_it_is_reported() {
        assert_eq!(key(Code::BackTab, NONE), ('⇤', SHIFT));
        assert_eq!(key(Code::BackTab, SH), ('⇤', SHIFT));
        assert_eq!(key(Code::Tab, SH), ('⇤', SHIFT));
        assert_eq!(key(Code::Tab, NONE), ('\t', 0));
    }

    #[test]
    fn ctrl_letter_is_its_control_character_with_ctrl_set() {
        assert_eq!(key(Code::Char('s'), CT), ('\x13', CTRL));
        assert_eq!(key(Code::Char('S'), Mods { shift: true, ctrl: true, alt: false }), ('\x13', CTRL));
        assert_eq!(key(Code::Char('a'), ALTGR), ('\x01', CTRL | ALT));
        assert_eq!(key(Code::Left, CT), ('←', CTRL));
    }

    #[test]
    fn alt_and_altgr() {
        assert_eq!(key(Code::Char('x'), AL), ('x', ALT));
        assert_eq!(key(Code::Char('@'), ALTGR), ('@', 0));
        assert_eq!(key(Code::Up, ALTGR), ('↑', CTRL | ALT));
    }

    #[test]
    fn script_names() {
        assert_eq!(named("PGDN"), Some(Code::PageDown));
        assert_eq!(named("F10"), Some(Code::F(10)));
        assert_eq!(named("F13"), None);
        assert_eq!(named("F"), Some(Code::Char('F')));
        assert_eq!(named("ñ"), Some(Code::Char('ñ')));
        assert_eq!(named("hola"), None);
    }
}
