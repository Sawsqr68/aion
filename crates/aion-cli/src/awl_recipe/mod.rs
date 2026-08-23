//! The EXECUTING VERB of the recipe surface: `aion awl recipe`.
//!
//! Declaration, check and projection stay in the language front
//! (`crates/aion-awl`), which opens no file, spawns no process and speaks no
//! transport. This is where a recipe is actually RUN — the item-5 seam kept
//! at the crate boundary rather than by convention, and the ratified
//! placement (fold B) the flight's report measures the front's dependency
//! delta against.
//!
//! What lives here is exactly the two ports the front declares and cannot
//! implement: [`tree::WorkingTree`] reads bytes out of a real working tree,
//! and [`firer::ProcessFirer`] runs a declared artifact and reads the record
//! its output names. Everything else — the census comparison, the typed
//! stage compare, the receipt, every refusal — is the front's, consumed.

mod args;
mod firer;
mod run;
mod tree;

pub(crate) use args::RecipeCommand;
pub(crate) use run::run;
