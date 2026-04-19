//! String transformation intrinsics — replace, trim, repeat, case.

/// Replace all occurrences of char `from` with `to`.
#[inline]
pub fn replace_char(s: &str, from: char, to: &str) -> String {
    s.replace(from, to)
}

/// Replace all occurrences of string `from` with `to`.
#[inline]
pub fn replace_str(s: &str, from: &str, to: &str) -> String {
    s.replace(from, to)
}

/// Replace first `n` occurrences of char `from` with `to`.
#[inline]
pub fn replace_n_char(s: &str, from: char, to: &str, n: i64) -> String {
    if n <= 0 { return s.to_string(); }
    let mut result = String::with_capacity(s.len());
    let mut remaining = n as usize;
    for c in s.chars() {
        if remaining > 0 && c == from {
            result.push_str(to);
            remaining -= 1;
        } else {
            result.push(c);
        }
    }
    result
}

/// Replace first `n` occurrences of string `from` with `to`.
pub fn replace_n_str(s: &str, from: &str, to: &str, n: i64) -> String {
    if n <= 0 || from.is_empty() { return s.to_string(); }
    let mut result = String::with_capacity(s.len());
    let mut remaining = n as usize;
    let mut start = 0;
    while let Some(pos) = s[start..].find(from) {
        result.push_str(&s[start..start + pos]);
        if remaining > 0 {
            result.push_str(to);
            remaining -= 1;
        } else {
            result.push_str(from);
        }
        start += pos + from.len();
        if remaining == 0 {
            break;
        }
    }
    result.push_str(&s[start..]);
    result
}

/// Repeat `s` exactly `n` times.
#[inline]
pub fn repeat(s: &str, n: i64) -> String {
    if n <= 0 { String::new() } else { s.repeat(n as usize) }
}

/// Trim leading and trailing whitespace.
#[inline]
pub fn trim(s: &str) -> &str {
    s.trim()
}
