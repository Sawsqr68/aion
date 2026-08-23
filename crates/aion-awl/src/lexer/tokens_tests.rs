//! Pins on the vocabulary inventories every consumer outside the lexer reads.

use super::tokens::{
    ADMITS_WORD, ANSWER_WORD, COMMAND_WORD, CWD_WORD, ENV_WORD, HARDENING_WORD, IN_WORD, Keyword,
    OPEN_FILES_WORD, OPERAND_WORD, OPTION_WORD, OWNER_WORD, PATH_WORD, POSITIONAL_WORDS,
    PROGRAM_WORD, QUERY_WORD, REFUSES_WORD, RUN_WORD, SEND_WORD, SPECIMEN_WORD, TO_WORD,
};

/// The window of `tokens.rs` holding `as_word`'s match arms: one arm per
/// keyword, and the only place in the file where the arms can be counted
/// without writing the count down a second time.
fn as_word_arm_count() -> usize {
    let source = include_str!("tokens.rs");
    let after = source
        .split_once("pub const fn as_word")
        .map(|(_, rest)| rest)
        .unwrap_or_default();
    let body = after
        .split_once("pub fn from_word")
        .map(|(body, _)| body)
        .unwrap_or_default();
    body.matches("Self::").count()
}

/// [`Keyword::ALL`] is the whole language, not most of it.
///
/// Rust cannot enumerate an enum's variants, so `ALL` is written by hand and a
/// hand-written list of 57 things silently loses an entry. The count is
/// therefore DERIVED from `as_word`'s own source rather than restated: adding a
/// variant adds an arm there (the compiler forces it — the match is
/// exhaustive) and this test fails until `ALL` is extended too.
///
/// Assert the invariant, not the enumeration: comparing against a literal 57
/// would only pin what was true the day it was typed.
#[test]
fn keyword_inventory_covers_every_variant() {
    let arms = as_word_arm_count();
    assert!(
        arms > 50,
        "the control failed: the arm count is derived by reading this crate's own source, and a \
         parse that found {arms} arms is not reading `as_word` at all — the comparison below \
         would pass vacuously"
    );
    assert_eq!(
        Keyword::ALL.len(),
        arms,
        "`Keyword::ALL` lists {} of the language's {arms} reserved words. A keyword missing from \
         it does not fail to compile — it goes quietly absent from every consumer that reads the \
         vocabulary, starting with the language server's completions.",
        Keyword::ALL.len()
    );
}

/// Every entry in the inventory is a distinct, real word.
///
/// A duplicated entry would keep the count right while losing a keyword, which
/// is exactly the failure the count is meant to catch.
#[test]
fn every_keyword_in_the_inventory_round_trips_and_is_distinct() {
    let mut seen = std::collections::BTreeSet::new();
    for keyword in Keyword::ALL {
        let word = keyword.as_word();
        assert_eq!(
            Keyword::from_word(word),
            Some(keyword),
            "`{word}` must lex back to the keyword it was written from"
        );
        assert!(
            seen.insert(word),
            "`{word}` appears twice in `Keyword::ALL`, which keeps the count right while losing a \
             keyword"
        );
    }
    assert_eq!(seen.len(), Keyword::ALL.len());
}

/// The positional words are NOT reserved, and must stay that way.
///
/// This is the load-bearing property of that list, not an incidental one: the
/// corpus carries `action send(to: String, …)`, `step run` and `wait decision
/// -> answer`. If one of these ever became a keyword, those documents would
/// stop parsing — so the inventory that names them for outside consumers also
/// pins that naming them did not reserve them.
#[test]
fn the_positional_words_are_named_without_being_reserved() {
    for word in POSITIONAL_WORDS {
        assert_eq!(
            Keyword::from_word(word),
            None,
            "`{word}` is read positionally and must never lex as a keyword — reserving it breaks \
             documents that exist today"
        );
    }
    assert_eq!(
        POSITIONAL_WORDS,
        [
            SEND_WORD,
            TO_WORD,
            QUERY_WORD,
            ANSWER_WORD,
            RUN_WORD,
            IN_WORD,
            COMMAND_WORD,
            PROGRAM_WORD,
            ADMITS_WORD,
            REFUSES_WORD,
            SPECIMEN_WORD,
            OPTION_WORD,
            OPERAND_WORD,
            ENV_WORD,
            CWD_WORD,
            HARDENING_WORD,
            PATH_WORD,
            OPEN_FILES_WORD,
            OWNER_WORD
        ],
        "the inventory must hold every named positional word, or a consumer reading it offers \
         fewer words than the parser accepts"
    );
}
