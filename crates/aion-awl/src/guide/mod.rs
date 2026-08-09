//! The language's glossary: one sentence per word, and where to read more.
//!
//! Three surfaces teach AWL — the language server's hover and completion, the
//! `aion awl guide` command, and the ops console's authoring help. Each was a
//! place the same sentence could have been written a fourth time, slightly
//! differently, and each would have gone on reading plausibly long after the
//! language moved. So none of them writes prose: they all render THIS, and
//! the full treatment stays in one document ([`REFERENCE_PATH`]) that every
//! entry cites by section.
//!
//! What lives here is deliberately only the pointer layer — the word, one
//! sentence, and a citation. What a construct MEANS in full belongs in the
//! reference, because a paragraph maintained in a Rust string literal is a
//! paragraph nobody reviews as documentation.

mod declarations;
mod entry;
mod flow;
mod lookup;
mod values;

pub use entry::{
    AUTHORING_PATH, COMMANDS_PATH, DECLARED_COMMANDS_PATH, GUIDE_PATH, GuideEntry, REFERENCE_PATH,
    Section, WORKERS_PATH, WordKind,
};
pub use lookup::{all, for_keyword, for_positional_word, for_word};
