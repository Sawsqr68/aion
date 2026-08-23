mod cursor;
mod error;
mod scanner;
mod tokens;

#[cfg(test)]
mod grammar_vocabulary_tests;
#[cfg(test)]
mod tokens_tests;

pub use error::LexError;
pub use scanner::lex;
pub use tokens::{
    ADMITS_WORD, ANSWER_WORD, COMMAND_WORD, CWD_WORD, DurationUnit, ENV_WORD, HARDENING_WORD,
    IN_WORD, Keyword, OPEN_FILES_WORD, OPERAND_WORD, OPTION_WORD, OWNER_WORD, PATH_WORD,
    POSITIONAL_WORDS, PROGRAM_WORD, QUERY_WORD, REFUSES_WORD, RUN_WORD, SEND_WORD, SPECIMEN_WORD,
    Span, TO_WORD, Token, TokenKind,
};
