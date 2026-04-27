//! Search intrinsics — find positions, counts, contains checks.

/// Count occurrences of `pat` char in `s` (no boxing per match).
#[inline]
pub fn count_char(s: &str, pat: char) -> i64 {
    s.chars().filter(|&c| c == pat).count() as i64
}

/// Count non-overlapping occurrences of string `pat` in `s`.
#[inline]
pub fn count_str(s: &str, pat: &str) -> i64 {
    if pat.is_empty() { return 0; }
    let mut count = 0i64;
    let mut start = 0;
    let bytes = s.as_bytes();
    let pat_bytes = pat.as_bytes();
    while start + pat_bytes.len() <= bytes.len() {
        if bytes[start..].starts_with(pat_bytes) {
            count += 1;
            start += pat_bytes.len();
        } else {
            start += 1;
        }
    }
    count
}

/// Collect all 1-based char indices where `pat` char is found.
pub fn find_positions_char(s: &str, pat: char) -> Vec<i64> {
    s.char_indices()
        .filter(|(_, c)| *c == pat)
        .enumerate()
        .map(|(i, _)| i as i64 + 1)
        .collect()
}

/// Collect all 1-based char indices where string `pat` starts.
pub fn find_positions_str(s: &str, pat: &str) -> Vec<i64> {
    if pat.is_empty() { return vec![]; }
    let chars: Vec<char> = s.chars().collect();
    let pat_chars: Vec<char> = pat.chars().collect();
    let mut result = Vec::new();
    let mut i = 0;
    while i + pat_chars.len() <= chars.len() {
        if chars[i..i + pat_chars.len()] == pat_chars[..] {
            result.push(i as i64 + 1);
            i += pat_chars.len();
        } else {
            i += 1;
        }
    }
    result
}
