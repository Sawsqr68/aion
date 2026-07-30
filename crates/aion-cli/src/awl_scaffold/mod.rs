//! The `aion awl scaffold` surface: a worker crate generated from a `.awl`
//! document's compiled contract.

mod command;
mod paths;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

pub(crate) use command::run;
