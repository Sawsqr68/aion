//! The words the recipe surface adds to the language, and the grouping the
//! parser reads them by.
//!
//! Every one is a POSITIONAL word, and that is a measurement rather than a
//! preference. Counted over the discovered `**/*.awl` corpus of this tree:
//! `runs` stands in 36 documents, `parameter` in 20, `requires` in 12,
//! `artifact` in 8, `derives` in 6, `wall`, `digest` and `fires` in 3 each,
//! `recipe` in 2, and `allows`, `captures` and `extract` in 1 each.
//! Reserving any of those would break documents that exist. The three that
//! stand nowhere today — `excludes`, `restores`, `expecting` — are read the
//! same way regardless: reserving three words while recognising twelve
//! positionally would be two grammars for one vocabulary.
//!
//! The words themselves are defined once, in the lexer's own inventory,
//! because [`crate::POSITIONAL_WORDS`] is what the glossary, the grammar
//! table and the language server read. What this module adds is the
//! GROUPING: which words open a clause of which body.

pub(super) use crate::lexer::{
    ALLOWS_WORD, ARTIFACT_WORD, CAPTURES_WORD, COMMAND_WORD, DERIVES_WORD, DIGEST_WORD,
    EXCLUDES_WORD, EXPECTING_WORD, EXTRACT_WORD, FIRES_WORD, PARAMETER_WORD, RECIPE_WORD,
    REQUIRES_WORD, RESTORES_WORD, RUNS_WORD, WALL_WORD,
};

/// Every word that may open a line in a recipe body, in the order the
/// language reads them. `step` is a reserved word and opens the third.
pub const RECIPE_CLAUSE_WORDS: [&str; 1] = [PARAMETER_WORD];

/// Every word that may open a line in a recipe STEP body, in order. `after`
/// is a reserved word and opens the dependency edge — reused rather than
/// re-spelled, because `after` already means "an ordering dependency on
/// another step" everywhere else in this language, and minting a synonym
/// would give one meaning two words.
pub const STEP_CLAUSE_WORDS: [&str; 4] = [RUNS_WORD, FIRES_WORD, CAPTURES_WORD, REQUIRES_WORD];

/// Every word that may open a line in a wall body, in order.
pub const WALL_CLAUSE_WORDS: [&str; 2] = [ALLOWS_WORD, EXCLUDES_WORD];

/// Every word that may open a line in an artifact body, in order.
pub const ARTIFACT_CLAUSE_WORDS: [&str; 3] = [DIGEST_WORD, RESTORES_WORD, EXCLUDES_WORD];

/// The admitted clause set of one body, rendered for a diagnostic.
pub(super) fn admitted(words: &[&str], extra: &[&str]) -> String {
    let mut rendered: Vec<String> = words.iter().map(|word| format!("`{word}`")).collect();
    rendered.extend(extra.iter().map(|word| format!("`{word}`")));
    rendered.join(", ")
}
