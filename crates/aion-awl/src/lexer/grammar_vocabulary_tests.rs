//! The editor grammar and this lexer must agree about AWL's vocabulary.
//!
//! AWL's words are defined twice: `Keyword::ALL` here, and `const keywords` in
//! `tools/tree-sitter-awl/grammar.js`. The second one is not decoration — it is
//! compiled to wasm, vendored into the ops console at
//! `apps/aion-ops-console/src/features/authoring/assets/awl.wasm`, and run
//! under `web-tree-sitter` to colour the operator's own authoring surface. The
//! same grammar backs `editors/zed-awl` and `editors/nvim-awl`.
//!
//! Two independent definitions of one invariant, with nothing comparing them,
//! drift silently — each copy looks right on its own, so no test ever fails.
//! On 2026-07-31 the grammar was two words behind: `agent` (checker-enforced,
//! read off the document by the worker) and `advisory` both rendered as
//! ordinary identifiers in every editor. This is the door that makes the next
//! one loud.
//!
//! This gate reads the SOURCE grammar, so it says nothing about whether the
//! vendored wasm was rebuilt from it. That half is held by
//! `apps/aion-ops-console/src/features/authoring/lib/highlight-assets.test.ts`,
//! which loads the compiled artefact and checks it against this same array.
//! Both are needed: this one catches a missing word, that one catches a
//! missing rebuild.

use super::Keyword;
use crate::lexer::POSITIONAL_WORDS;

/// The grammar, read at compile time so this cannot pass against a stale copy.
const GRAMMAR: &str = include_str!("../../../../tools/tree-sitter-awl/grammar.js");

/// The `in` exception, and the only word the grammar may colour without this
/// lexer reserving it.
///
/// `in` is a POSITIONAL WORD: the parser accepts it in `fork item in items`
/// and it stays legal as an ordinary name everywhere else. The grammar is a
/// highlighting grammar, too coarse to tell those two uses apart, so listing a
/// word colours it EVERYWHERE. For `in` that costs nothing, because nobody
/// names a binding `in`.
///
/// Its five siblings — `send`, `to`, `query`, `answer`, `run` — are ordinary
/// English words that authors DO bind, and they are deliberately absent.
/// `wait approval timeout 30s -> answer` binds a result called `answer`;
/// adding those five turns three grammar corpus fixtures red, each because a
/// real binding suddenly lexes as syntax. A plain statement opener is a
/// cosmetic loss; a tinted binding is the editor lying about the author's code.
const COLOURED_POSITIONAL_WORDS: [&str; 1] = ["in"];

/// Reserved words the grammar colours through a MORE specific node than
/// `keyword`, paired with the rule that must still cover them.
///
/// `true` and `false` are `Keyword` variants here, but the grammar gives them
/// `boolean: _ => choice('true','false')` so an editor can render them as
/// constants rather than as syntax. That is a refinement, not drift, and the
/// reserved-word gate would otherwise report it as a defect.
///
/// The rule name is carried so the exemption has to keep earning itself: if
/// the `boolean` rule is ever deleted or renamed, these words stop being
/// covered by anything and `the_specifically_typed_words_are_still_covered`
/// fires rather than the exemption silently hiding a real gap.
const SPECIFICALLY_TYPED_WORDS: [(&str, &str); 2] = [("true", "boolean"), ("false", "boolean")];

/// Every quoted word inside the grammar's `const keywords = [ ... ];`.
///
/// Extraction starts AFTER the opening bracket, so the explanatory comment
/// above it — which names words in backticks — cannot leak into the result.
fn grammar_keywords() -> Vec<String> {
    let after = GRAMMAR
        .split_once("const keywords = [")
        .map_or("", |(_, rest)| rest);
    let body = after.split_once("];").map_or("", |(body, _)| body);
    body.split('\'')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect()
}

/// Every reserved word this lexer knows is coloured by the editor grammar.
///
/// This is the direction that matters most: a keyword the compiler enforces
/// but the editor shows as a plain name teaches the author that the word means
/// nothing.
#[test]
fn every_reserved_word_is_known_to_the_editor_grammar() {
    let grammar = grammar_keywords();
    assert!(
        grammar.len() > 50,
        "the grammar keyword array did not extract — {} words found, so every assertion below \
         would pass vacuously",
        grammar.len()
    );

    let missing: Vec<&str> = Keyword::ALL
        .iter()
        .map(|keyword| keyword.as_word())
        .filter(|word| !grammar.iter().any(|listed| listed == word))
        .filter(|word| {
            !SPECIFICALLY_TYPED_WORDS
                .iter()
                .any(|(typed, _)| typed == word)
        })
        .collect();
    assert!(
        missing.is_empty(),
        "these words are reserved by the AWL lexer but absent from \
         tools/tree-sitter-awl/grammar.js, so they render as ordinary identifiers in the ops \
         console and in every editor: {missing:?}\nAdd them to `const keywords`, re-run \
         `tree-sitter generate && tree-sitter test`, rebuild the wasm with `tree-sitter build \
         --wasm`, and re-vendor it to \
         apps/aion-ops-console/src/features/authoring/assets/awl.wasm"
    );
}

/// The grammar colours nothing this lexer cannot account for.
///
/// Without this half, the gate would be satisfied by a grammar that listed
/// every English word. It also forces a deliberate decision the next time
/// somebody wants a positional word tinted, rather than a quiet addition.
#[test]
fn the_grammar_colours_nothing_the_lexer_cannot_account_for() {
    let reserved: Vec<&str> = Keyword::ALL
        .iter()
        .map(|keyword| keyword.as_word())
        .collect();
    let unexplained: Vec<String> = grammar_keywords()
        .into_iter()
        .filter(|word| !reserved.contains(&word.as_str()))
        .filter(|word| !COLOURED_POSITIONAL_WORDS.contains(&word.as_str()))
        .collect();
    assert!(
        unexplained.is_empty(),
        "tools/tree-sitter-awl/grammar.js colours these words, but the AWL lexer neither reserves \
         them nor lists them as a deliberate positional-word exception: {unexplained:?}\nEither \
         they are stale and should go, or the exception is intended and belongs in \
         COLOURED_POSITIONAL_WORDS with the reason written down"
    );
}

/// A word exempted for having a more specific node still HAS that node.
///
/// Without this, `SPECIFICALLY_TYPED_WORDS` would be a way to silence the
/// reserved-word gate by writing a word down, which is the opposite of what it
/// is for.
#[test]
fn the_specifically_typed_words_are_still_covered() {
    for (word, rule) in SPECIFICALLY_TYPED_WORDS {
        let Some((_, rest)) = GRAMMAR.split_once(&format!("{rule}:")) else {
            unreachable!(
                "the grammar has no `{rule}:` rule, so `{word}` is exempted from the \
                 reserved-word gate while nothing in the grammar covers it"
            )
        };
        let body = &rest[..rest.len().min(200)];
        assert!(
            body.contains(&format!("'{word}'")),
            "the grammar's `{rule}` rule no longer matches `{word}`, so that word is now \
             exempted from the reserved-word gate while nothing colours it: {body}"
        );
    }
}

/// The `in` exception is a real positional word, not a typo that outlived its
/// author.
#[test]
fn the_coloured_exception_is_a_genuine_positional_word() {
    for word in COLOURED_POSITIONAL_WORDS {
        assert!(
            POSITIONAL_WORDS.contains(&word),
            "`{word}` is listed as a coloured positional word but is not one of AWL's positional \
             words, so the exception is protecting something that does not exist"
        );
        assert_eq!(
            Keyword::from_word(word),
            None,
            "`{word}` is a RESERVED keyword, so it needs no exception — remove it from \
             COLOURED_POSITIONAL_WORDS and let the reserved-word gate cover it"
        );
    }
}

/// The positional words the grammar deliberately leaves plain stay plain.
///
/// Stated as an assertion rather than a comment because the reason is
/// empirical: each of these is a plausible binding name, and colouring them
/// broke real fixtures. A future addition should have to delete this test and
/// justify it, not merely edit a list.
#[test]
fn the_bindable_positional_words_are_deliberately_left_plain() {
    let grammar = grammar_keywords();
    let wrongly_coloured: Vec<&&str> = POSITIONAL_WORDS
        .iter()
        .filter(|word| !COLOURED_POSITIONAL_WORDS.contains(*word))
        .filter(|word| grammar.iter().any(|listed| listed == *word))
        .collect();
    assert!(
        wrongly_coloured.is_empty(),
        "these positional words are now coloured by the grammar: {wrongly_coloured:?}\nThey are \
         ordinary names an author may bind — `wait approval timeout 30s -> answer` binds \
         `answer` — and this grammar cannot tell a statement opener from a binding, so colouring \
         them tints the author's own code"
    );
    assert!(
        POSITIONAL_WORDS.len() > COLOURED_POSITIONAL_WORDS.len(),
        "this test proves nothing if every positional word is an exception"
    );
}
