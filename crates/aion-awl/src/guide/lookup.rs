//! The one place a word is joined to its meaning.
//!
//! [`for_keyword`] is an exhaustive match over [`Keyword`]. That is the whole
//! anti-drift mechanism and it is deliberately not a lookup table: a keyword
//! added to the language fails to compile here until someone writes what it
//! means, rather than going quietly undocumented in the editor, the CLI, and
//! the console at once.
//!
//! The positional words are not a Rust enum — reserving them would break
//! documents that exist — so their arm is a string match, pinned by
//! `every_positional_word_has_an_entry` against the lexer's own
//! [`POSITIONAL_WORDS`] inventory.

use crate::lexer::{
    ADMITS_WORD, ALLOWS_WORD, ANSWER_WORD, ARTIFACT_WORD, CAPTURES_WORD, COMMAND_WORD, CWD_WORD,
    DERIVES_WORD, DIGEST_WORD, ENV_WORD, EXCLUDES_WORD, EXPECTING_WORD, EXTRACT_WORD, FIRES_WORD,
    HARDENING_WORD, IN_WORD, Keyword, OPEN_FILES_WORD, OPERAND_WORD, OPTION_WORD, OWNER_WORD,
    PARAMETER_WORD, PATH_WORD, POSITIONAL_WORDS, PROGRAM_WORD, QUERY_WORD, RECIPE_WORD,
    REFUSES_WORD, REQUIRES_WORD, RESTORES_WORD, RUN_WORD, RUNS_WORD, SEND_WORD, SPECIMEN_WORD,
    TO_WORD, WALL_WORD,
};

use super::commands as cmd;
use super::declarations as decl;
use super::entry::GuideEntry;
use super::flow;
use super::recipes as rec;
use super::values;

/// What the language reference says about one reserved word.
#[must_use]
pub const fn for_keyword(keyword: Keyword) -> GuideEntry {
    match keyword {
        Keyword::Workflow => decl::WORKFLOW,
        Keyword::Input => decl::INPUT,
        Keyword::Signal => decl::SIGNAL,
        Keyword::Outcome => flow::OUTCOME,
        Keyword::Type => decl::TYPE,
        Keyword::Schema => decl::SCHEMA,
        Keyword::Worker => decl::WORKER,
        Keyword::Action => decl::ACTION,
        Keyword::Child => decl::CHILD,
        Keyword::Step => flow::STEP,
        Keyword::After => flow::AFTER,
        Keyword::Fork => flow::FORK,
        Keyword::Join => flow::JOIN,
        Keyword::Loop => flow::LOOP,
        Keyword::Counting => flow::COUNTING,
        Keyword::Until => flow::UNTIL,
        Keyword::Max => flow::MAX,
        Keyword::Sequential => flow::SEQUENTIAL,
        Keyword::Spawn => decl::SPAWN,
        Keyword::Wait => flow::WAIT,
        Keyword::Sleep => flow::SLEEP,
        Keyword::Timeout => decl::TIMEOUT,
        Keyword::Retry => decl::RETRY,
        Keyword::Fallback => decl::FALLBACK,
        Keyword::Advisory => decl::ADVISORY,
        Keyword::Agent => decl::AGENT,
        Keyword::Every => decl::EVERY,
        Keyword::Backoff => decl::BACKOFF,
        Keyword::Node => decl::NODE,
        Keyword::On => flow::ON,
        Keyword::Failure => flow::FAILURE,
        Keyword::When => flow::WHEN,
        Keyword::Otherwise => flow::OTHERWISE,
        Keyword::Route => flow::ROUTE,
        Keyword::Success => flow::SUCCESS,
        Keyword::Filter => values::FILTER,
        Keyword::Map => values::MAP,
        Keyword::Sort => values::SORT,
        Keyword::Count => values::COUNT,
        Keyword::Any => values::ANY,
        Keyword::All => values::ALL,
        Keyword::Is => values::IS,
        Keyword::Empty => values::EMPTY,
        Keyword::Present => values::PRESENT,
        Keyword::Absent => values::ABSENT,
        Keyword::Not => values::NOT,
        Keyword::And => values::AND,
        Keyword::Or => values::OR,
        Keyword::True => decl::TRUE,
        Keyword::False => decl::FALSE,
        Keyword::Const => decl::CONST,
        Keyword::Json => decl::JSON,
        Keyword::Of => decl::OF,
        Keyword::Subflow => decl::SUBFLOW,
        Keyword::Distribute => flow::DISTRIBUTE,
        Keyword::Sequence => flow::SEQUENCE,
        Keyword::Collect => flow::COLLECT,
        Keyword::Visits => flow::VISITS,
    }
}

/// What the language reference says about one positional word.
///
/// Positional words are recognised by the parser in exactly one grammar
/// position each, so this answers what the word means THERE. It says nothing
/// about an identifier that merely happens to spell the same letters —
/// `action send(to: String, …)` is a parameter named `to`, and no surface
/// should claim otherwise.
#[must_use]
pub fn for_positional_word(word: &str) -> Option<GuideEntry> {
    match word {
        SEND_WORD => Some(flow::SEND),
        TO_WORD => Some(flow::TO),
        QUERY_WORD => Some(flow::QUERY),
        ANSWER_WORD => Some(flow::ANSWER),
        RUN_WORD => Some(decl::RUN),
        IN_WORD => Some(flow::IN),
        COMMAND_WORD => Some(cmd::COMMAND),
        PROGRAM_WORD => Some(cmd::PROGRAM),
        ADMITS_WORD => Some(cmd::ADMITS),
        REFUSES_WORD => Some(cmd::REFUSES),
        SPECIMEN_WORD => Some(cmd::SPECIMEN),
        OPTION_WORD => Some(cmd::OPTION),
        OPERAND_WORD => Some(cmd::OPERAND),
        ENV_WORD => Some(cmd::ENV),
        CWD_WORD => Some(cmd::CWD),
        HARDENING_WORD => Some(cmd::HARDENING),
        PATH_WORD => Some(cmd::PATH),
        OPEN_FILES_WORD => Some(cmd::OPEN_FILES),
        OWNER_WORD => Some(cmd::OWNER),
        RECIPE_WORD => Some(rec::RECIPE),
        PARAMETER_WORD => Some(rec::PARAMETER),
        DERIVES_WORD => Some(rec::DERIVES),
        EXTRACT_WORD => Some(rec::EXTRACT),
        WALL_WORD => Some(rec::WALL),
        ALLOWS_WORD => Some(rec::ALLOWS),
        EXCLUDES_WORD => Some(rec::EXCLUDES),
        ARTIFACT_WORD => Some(rec::ARTIFACT),
        DIGEST_WORD => Some(rec::DIGEST),
        RESTORES_WORD => Some(rec::RESTORES),
        RUNS_WORD => Some(rec::RUNS),
        CAPTURES_WORD => Some(rec::CAPTURES),
        FIRES_WORD => Some(rec::FIRES),
        EXPECTING_WORD => Some(rec::EXPECTING),
        REQUIRES_WORD => Some(rec::REQUIRES),
        _ => None,
    }
}

/// What the language reference says about any word of the language.
#[must_use]
pub fn for_word(word: &str) -> Option<GuideEntry> {
    match Keyword::from_word(word) {
        Some(keyword) => Some(for_keyword(keyword)),
        None => for_positional_word(word),
    }
}

/// Every entry, reserved words in the language's declaration order followed
/// by the positional words in the lexer's own order.
///
/// Both orders come from `aion-awl`'s own inventories rather than from a
/// hand-kept list here, so a word added to the language appears in every
/// surface that renders this without anyone remembering to add it.
#[must_use]
pub fn all() -> Vec<GuideEntry> {
    let mut entries: Vec<GuideEntry> = Keyword::ALL.into_iter().map(for_keyword).collect();
    entries.extend(POSITIONAL_WORDS.into_iter().filter_map(for_positional_word));
    entries
}

#[cfg(test)]
#[path = "guide_tests.rs"]
mod guide_tests;
