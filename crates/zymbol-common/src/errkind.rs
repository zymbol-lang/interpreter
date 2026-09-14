//! Which `##` family a runtime error belongs to, read from its message.
//!
//! A `:! ##Div` filters by kind, so the kind of an error is part of the
//! language: the same failure has to land in the same family in every engine,
//! or a catch that works in one swallows nothing — or everything — in another.
//!
//! The register VM classified by the Rust variant of its error and the
//! tree-walker by the words of the message, and the two agreed only where a
//! variant happened to exist. A native called with the wrong type raises a plain
//! message in both, so the tree-walker said `##Type` and the VM said `##_`, and
//! `!? { m::sqrt("a") } :! ##Type { }` caught it in one engine and not in the
//! other (GLB-010). One rule, here, used by both for every error that does not
//! carry a kind of its own.
//!
//! It is a rule about WORDS, which is fragile, and it is written down so that
//! the fragility is in one place: a message that starts to mention "index" moves
//! family. The order matters and is the tree-walker's.

/// The family of an error that carries only a message: `"Range"`, `"Key"`,
/// `"Index"`, `"Type"`, `"Div"`, `"Parse"`, or `"_"` for none of them.
pub fn error_kind_of_message(message: &str) -> &'static str {
    let m = message.to_lowercase();
    // Before the rest: an integer that left its range is a ##Range whatever
    // else the message happens to mention.
    if m.contains("overflow") || m.contains("out of range") {
        "Range"
    // Before the index branch: a missing key is a ##Key even though the reader
    // reached it through the index syntax `d["k"]` (decision 10).
    } else if m.contains("no key") {
        "Key"
    } else if m.contains("index") || m.contains("out of bounds") {
        "Index"
    } else if m.contains("type") {
        "Type"
    } else if m.contains("division") || m.contains("divide by zero") || m.contains("modulo") {
        "Div"
    } else if m.contains("parse") {
        "Parse"
    } else {
        "_"
    }
}

#[cfg(test)]
mod tests {
    use super::error_kind_of_message as kind;

    #[test]
    fn each_family_by_its_words() {
        assert_eq!(kind("integer overflow: 9 * 9"), "Range");
        assert_eq!(kind("no key 'x' in dictionary — it is empty"), "Key");
        assert_eq!(kind("array index out of bounds: index 9"), "Index");
        assert_eq!(kind("mat::sqrt: incompatible argument type(s) [String]"), "Type");
        assert_eq!(kind("division by zero"), "Div");
        assert_eq!(kind("modulo by zero"), "Div");
        assert_eq!(kind("cannot parse '12x'"), "Parse");
        assert_eq!(kind("something else went wrong"), "_");
    }

    #[test]
    fn range_wins_over_index_and_key_over_index() {
        assert_eq!(kind("index out of range"), "Range");
        assert_eq!(kind("no key 'i' — index it by key"), "Key");
    }
}
