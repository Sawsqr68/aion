//! The words the command-declaration surface adds to the language.
//!
//! Every one is a POSITIONAL word, not a reserved one, and that is a
//! measurement rather than a preference: `command` alone stands in 54 `.awl`
//! documents of this tree today as a harness-section field name, and `cwd`,
//! `path`, `program`, `env` and `operand` stand in others. Reserving any of
//! them would break documents that exist. So each is recognised in exactly
//! the one grammar position where nothing else may stand — the same
//! discipline the lexer already applies to `run`, `send`, `to`, `query`,
//! `answer` and `in`, and for the same stated reason.
//!
//! Naming them here rather than at each recognition site is what lets the
//! parser, the printer, the glossary and the language server read the same
//! words: a change to one is a change to all of them.

//! The words themselves are defined once, in the lexer's own inventory
//! (`crate::lexer`), because [`crate::POSITIONAL_WORDS`] is what the
//! glossary, the grammar table and the language server read. Re-stating them
//! here would be a second inventory that could fall behind the first. What
//! this module adds is the GROUPING the parser needs: which of them open a
//! clause, and which would open a form this cut refuses.

pub(super) use crate::lexer::{
    ADMITS_WORD, COMMAND_WORD, CWD_WORD, ENV_WORD, HARDENING_WORD, OPEN_FILES_WORD, OPERAND_WORD,
    OPTION_WORD, OWNER_WORD, PATH_WORD, PROGRAM_WORD, REFUSES_WORD, SPECIMEN_WORD,
};

/// Every word that may open a line in a command body, in the order the
/// language reads them.
pub const CLAUSE_WORDS: [&str; 8] = [
    PROGRAM_WORD,
    ADMITS_WORD,
    REFUSES_WORD,
    OPTION_WORD,
    OPERAND_WORD,
    ENV_WORD,
    CWD_WORD,
    HARDENING_WORD,
];

/// The words that would open a recipe, a dependency edge, or a multi-step
/// assembly, recognised ONLY so the refusal can name the form it refused.
///
/// This is the absence-of-escape discipline: a refused form is refused by
/// name with its own reason, never by falling through to "unknown clause".
pub const ASSEMBLY_WORDS: [(&str, &str); 6] = [
    ("recipe", "a recipe body"),
    ("depends", "a dependency edge"),
    ("needs", "a dependency edge"),
    ("after", "a dependency edge"),
    ("steps", "a multi-step assembly"),
    ("then", "a multi-step assembly"),
];
