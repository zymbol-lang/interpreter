//! The type symbols — what `#?` answers with, and how the engines name a type.
//!
//! Ten spellings, each `##` followed by one character, and the character is the
//! one that most evokes the type: `#` a number, `.` a decimal point, `"` a
//! string, `'` a character, `?` a truth value, and for the collections their own
//! delimiter.
//!
//! ```text
//! ###   Int          ##]   array — every element the same type
//! ##.   Float        ##[   list  — an array whose elements are NOT all one type
//! ##"   String       ##)   positional tuple      (1, 2)
//! ##'   Char         ##(   dictionary            #(a: 1)
//! ##?   Bool         ##_   Unit
//! ```
//!
//! The rule for the collections: **the unmarked one takes the closing delimiter
//! and the marked one takes the opening delimiter.** `[…]` is `##]` and `#[…]`
//! is `##[`; `(…)` is `##)` and `#(…)` is `##(`. It is the literal's own mark
//! with a `#` in front, which is what the mark already meant.
//!
//! # Why this file exists at all
//!
//! Ten sites across two Rust engines each wrote the table out by hand, and two
//! of them were the same twenty lines of the register VM copied twice. That is
//! how `##)` came to mean both the tuple and the dictionary long after the two
//! stopped being the same thing.
//!
//! # `##(` is a type and `##[` is a reading
//!
//! A dictionary really is a different type from a tuple — the value carries its
//! keys, one takes `d["k"]$~ v` and the other answers *tuples are immutable* —
//! so `##(` is what a dictionary is, everywhere a type is named, error messages
//! included.
//!
//! `##[` is not a type. `#[…]` and `[…]` are the same type, decided deliberately
//! so `json::decode`'s heterogeneous array had somewhere to land, and the mark
//! on the literal is a compile-time declaration that leaves no trace: `[1, 2]`
//! and `#[1, 2]` are equal, and `#[1, "dos"]$-[2]` is a plain array of one Int.
//! So `##[` is computed from **what the value holds when asked**, not from how it
//! was written — which is also the question a caller actually has. It is
//! therefore [`symbol`]'s answer only, and never [`base_symbol`]'s: a
//! destructuring error says "expected an array" whatever the elements are.

/// Signed integer.
pub const INT: &str = "###";
/// Floating point.
pub const FLOAT: &str = "##.";
/// String.
pub const STRING: &str = "##\"";
/// Single character.
pub const CHAR: &str = "##'";
/// Truth value.
pub const BOOL: &str = "##?";
/// An array — with every element of the same type, when [`symbol`] answers it.
pub const ARRAY: &str = "##]";
/// An array holding more than one type. See the module note: a reading, not a type.
pub const LIST: &str = "##[";
/// The positional tuple, `(1, 2)`.
pub const TUPLE: &str = "##)";
/// The dictionary, `#(a: 1)` — including the empty one, `#()`.
pub const DICT: &str = "##(";
/// Unit, and what an undefined name answers.
pub const UNIT: &str = "##_";
/// A named function.
pub const FUNCTION: &str = "##()";
/// A lambda.
pub const LAMBDA: &str = "##->";

/// [`ARRAY`] when the elements are all one type, [`LIST`] when they are not.
///
/// Takes the elements' **base** symbols, so an array of arrays is uniform
/// whatever those inner arrays hold: this describes one level, and two arrays
/// are the same type as each other. An empty array is uniform — there is
/// nothing in it to disagree.
pub fn array_symbol<'a>(mut element_base_symbols: impl Iterator<Item = &'a str>) -> &'static str {
    match element_base_symbols.next() {
        None => ARRAY,
        Some(first) => {
            if element_base_symbols.all(|s| s == first) {
                ARRAY
            } else {
                LIST
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_array_is_uniform() {
        assert_eq!(array_symbol([].into_iter()), ARRAY);
    }

    #[test]
    fn one_element_cannot_disagree_with_itself() {
        assert_eq!(array_symbol([INT].into_iter()), ARRAY);
    }

    #[test]
    fn all_the_same_is_an_array_and_anything_else_is_a_list() {
        assert_eq!(array_symbol([INT, INT, INT].into_iter()), ARRAY);
        assert_eq!(array_symbol([INT, STRING].into_iter()), LIST);
        assert_eq!(array_symbol([FLOAT, INT].into_iter()), LIST);
        // A tuple and a dictionary are now different types, so an array holding
        // one of each is a list — which it was not while `##)` meant both.
        assert_eq!(array_symbol([TUPLE, DICT].into_iter()), LIST);
        assert_eq!(array_symbol([DICT, DICT].into_iter()), ARRAY);
    }

    #[test]
    fn every_symbol_is_distinct() {
        let all = [
            INT, FLOAT, STRING, CHAR, BOOL, ARRAY, LIST, TUPLE, DICT, UNIT, FUNCTION, LAMBDA,
        ];
        for (i, a) in all.iter().enumerate() {
            assert!(
                !all[..i].contains(a),
                "{a} is listed twice — two types cannot share a symbol"
            );
        }
    }
}
