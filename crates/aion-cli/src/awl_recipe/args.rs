//! The `aion awl recipe` subcommands and their arguments.

use std::path::PathBuf;

use clap::Subcommand;

/// The `aion awl recipe` subcommands: compose a declared assembly, or fire
/// it.
#[derive(Debug, Subcommand)]
pub(crate) enum RecipeCommand {
    /// Compose a declared recipe against the values a dispatch supplies,
    /// WITHOUT running anything.
    ///
    /// Every slot the recipe declares is filled from `--fill` or from its own
    /// declared default; a slot nothing filled, or one still carrying a
    /// skeleton's `<<PLACEHOLDER>>` or `__SENTINEL__`, is refused BY NAME and
    /// the exit code is non-zero. A `re_derive` slot is NOT filled here and
    /// must not be: it is read from its declared source at the FIRE minute,
    /// because the compose minute and the fire minute drift.
    ///
    /// Prints what resolved and what is deferred. Nothing is measured and
    /// nothing runs.
    Compose {
        /// Path to the `.awl` document.
        file: PathBuf,
        /// The declared recipe to compose.
        #[arg(long)]
        name: String,
        /// One `slot=value` binding. Repeat for each slot.
        #[arg(long = "fill", value_name = "SLOT=VALUE")]
        fills: Vec<String>,
    },
    /// FIRE a declared recipe against the working tree.
    ///
    /// The order is the front's and is not this command's to reorder: the
    /// receipt opens first and names the tree it measured, then every
    /// declared at-fire precondition is checked against `--met`, then the
    /// reference census measures every referenced artifact's deployed bytes
    /// against its declared digest, then the `re_derive` slots are read from
    /// their sources, then the steps run in dependency order.
    ///
    /// A refused fire STILL prints its receipt, because a stage nobody can
    /// audit is the failure receipt-first ordering exists to prevent.
    Fire {
        /// Path to the `.awl` document.
        file: PathBuf,
        /// The declared recipe to fire.
        #[arg(long)]
        name: String,
        /// One `slot=value` binding. Repeat for each slot.
        #[arg(long = "fill", value_name = "SLOT=VALUE")]
        fills: Vec<String>,
        /// One declared at-fire precondition this operator answers MET.
        /// Repeat for each. A precondition the recipe declares and this flag
        /// does not name is refused: an unanswered precondition is not a met
        /// one.
        #[arg(long = "met", value_name = "PRECONDITION")]
        met: Vec<String>,
        /// The root the artifact addresses resolve against; omitted, the
        /// document's own directory.
        #[arg(long)]
        root: Option<PathBuf>,
        /// Write the generated receipt here as well as to stdout.
        #[arg(long)]
        receipt: Option<PathBuf>,
    },
}
