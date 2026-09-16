//! How a diagnostic names the type of a value: in words, as the analyzer does.
//!
//! `#?` answers with symbols (`typesym`), which is what a program asks with. A
//! message is read by a person, and names the type the way `zymbol check` names
//! it — `Int`, `[Int]`, `(Int, String)`, `#(k: Int)` — so the same value reads
//! the same in the analyzer's warning and in the runtime error that follows it.
//! Decided 2026-09-16 (GLB-033 and step 3.5 of the diagnostics plan):
//!
//! ```text
//! Int  Float  String  Char  Bool  Unit  Error
//! [Int]           an array, named by what its elements are
//! [Any]           …when they are not all one type
//! [?]             …when it is empty: nothing says what it holds
//! [[Int]]         nested, one level at a time
//! (Int, String)   a positional tuple, element by element
//! #(k: Int)       a dictionary, as it is written
//! Function        a function or a lambda; its parameter types are not known
//!                 at run time, and the name does not pretend otherwise
//! ```
//!
//! The engines only map their own value representation onto these; the rules
//! for the collections live here, once, so the tree-walker and the VM cannot
//! name the same array differently.

pub const INT: &str = "Int";
pub const FLOAT: &str = "Float";
pub const STRING: &str = "String";
pub const CHAR: &str = "Char";
pub const BOOL: &str = "Bool";
pub const UNIT: &str = "Unit";
pub const ERROR: &str = "Error";
pub const FUNCTION: &str = "Function";

/// `[T]` when every element names the same type, `[Any]` when they do not, and
/// `[?]` for an empty array.
pub fn array<I: IntoIterator<Item = String>>(element_names: I) -> String {
    let mut names = element_names.into_iter();
    let Some(first) = names.next() else { return "[?]".to_string() };
    if names.all(|n| n == first) {
        format!("[{}]", first)
    } else {
        "[Any]".to_string()
    }
}

/// `(A, B)` — a positional tuple, element by element.
pub fn tuple<I: IntoIterator<Item = String>>(element_names: I) -> String {
    format!("({})", element_names.into_iter().collect::<Vec<_>>().join(", "))
}

/// `#(k: T, j: U)` — a dictionary, as it is written.
pub fn dict<'a, I: IntoIterator<Item = (&'a str, String)>>(fields: I) -> String {
    let parts: Vec<String> = fields.into_iter().map(|(k, t)| format!("{}: {}", k, t)).collect();
    format!("#({})", parts.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collections() {
        assert_eq!(array(vec!["Int".into(), "Int".into()]), "[Int]");
        assert_eq!(array(vec!["Int".into(), "String".into()]), "[Any]");
        assert_eq!(array(Vec::<String>::new()), "[?]");
        assert_eq!(array(vec!["[Int]".into(), "[Int]".into()]), "[[Int]]");
        assert_eq!(array(vec!["[Int]".into(), "[?]".into()]), "[Any]");
        assert_eq!(tuple(vec!["Int".into(), "String".into()]), "(Int, String)");
        assert_eq!(dict(vec![("k", "Int".to_string()), ("j", "[Int]".to_string())]), "#(k: Int, j: [Int])");
    }
}
