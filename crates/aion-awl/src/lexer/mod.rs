mod cursor;
mod error;
mod scanner;
mod tokens;

#[cfg(test)]
mod tokens_tests;

pub use error::LexError;
pub use scanner::lex;
pub use tokens::{
    ANSWER_WORD, DurationUnit, IN_WORD, Keyword, POSITIONAL_WORDS, QUERY_WORD, RUN_WORD, SEND_WORD,
    Span, TO_WORD, Token, TokenKind,
};
