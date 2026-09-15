//! The names a `{…}` interpolation reads, as the analysis passes see them.
//!
//! One scanner for every pass that needs it: `variable_analysis` counts these
//! names as uses and `type_check` refuses the ones that name nothing. Two copies
//! of an identifier rule drift apart, and when they did (HLZ-KL-001) a valid name
//! was reported as unused.

/// Every name interpolated in the text of a `Literal::InterpolatedString`, in
/// order. Escaped braces were already turned into `\x01`/`\x02` by the lexer,
/// so any `{` here opens an interpolation. The identifier rule is the lexer's.
pub(crate) fn interpolated_names(s: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_var = false;
    let mut var_name = String::new();
    for ch in s.chars() {
        if ch == '{' && !in_var {
            in_var = true;
            var_name.clear();
        } else if ch == '}' && in_var {
            in_var = false;
            if !var_name.is_empty() {
                names.push(std::mem::take(&mut var_name));
            }
        } else if in_var {
            // A narrower rule than the lexer's reported "unused variable" for
            // names the interpolation does resolve — pIqaD and emoji
            // identifiers, which are not is_alphanumeric.
            let ok = if var_name.is_empty() {
                zymbol_lexer::Lexer::is_ident_start(ch)
            } else {
                zymbol_lexer::Lexer::is_ident_continue(ch)
            };
            if ok {
                var_name.push(ch);
            } else {
                // Non-identifier char inside {…} — not a variable reference
                in_var = false;
            }
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::interpolated_names;

    #[test]
    fn names_in_order_and_not_the_rest() {
        assert_eq!(interpolated_names("a {x} b {整} {y z}"), vec!["x", "整"]);
        assert!(interpolated_names("\u{1}x\u{2}").is_empty());
    }
}
