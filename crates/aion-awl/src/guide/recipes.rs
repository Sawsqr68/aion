//! The recipe cluster: the fifteen words a declared assembly is written with.
//!
//! Every one is a POSITIONAL word, measured rather than preferred: `runs`
//! stands in 36 `.awl` documents of this tree today, `parameter` in 20,
//! `requires` in 12, `artifact` in 8, `derives` in 6, and `wall`, `digest`,
//! `fires`, `recipe`, `allows`, `captures` and `extract` in others.
//! Reserving any of them would break documents that exist, so each is
//! recognised in the one grammar position where nothing else may stand.
//!
//! Each entry cites §18 of the language reference, which is where the
//! surface is explained in full; `tests/guide_reference_gate.rs` refuses any
//! entry whose cited section does not exist or does not use its word.

use super::entry::{GuideEntry, RECIPE_DECLARATIONS, WordKind};
use super::examples;

/// `recipe`.
pub(crate) const RECIPE: GuideEntry = GuideEntry::new(
    "recipe",
    WordKind::Positional,
    "Declares an assembly act: typed parameters with declared kinds, and the ordered steps those \
     parameters feed.",
    RECIPE_DECLARATIONS,
    &[examples::RECIPE_STEPS, examples::RECIPE_PARAMETERS],
);

/// `parameter`.
pub(crate) const PARAMETER: GuideEntry = GuideEntry::new(
    "parameter",
    WordKind::Positional,
    "Declares one of a recipe's typed parameters and its kind — `skeleton`, `per_item`, `both`, \
     or `re_derive`.",
    RECIPE_DECLARATIONS,
    &[examples::RECIPE_PARAMETERS],
);

/// `derives`.
pub(crate) const DERIVES: GuideEntry = GuideEntry::new(
    "derives",
    WordKind::Positional,
    "Names the source address a `re_derive` parameter is read from at the fire minute, never at \
     the compose minute.",
    RECIPE_DECLARATIONS,
    &[examples::RECIPE_PARAMETERS],
);

/// `extract`.
pub(crate) const EXTRACT: GuideEntry = GuideEntry::new(
    "extract",
    WordKind::Positional,
    "Names what a derivation takes out of its source — `line <n>` or `whole`, and nothing an \
     evaluator would be needed for.",
    RECIPE_DECLARATIONS,
    &[examples::RECIPE_PARAMETERS],
);

/// `wall`.
pub(crate) const WALL: GuideEntry = GuideEntry::new(
    "wall",
    WordKind::Positional,
    "Declares a wall once, so its prompt rendering and its mechanical enforcement come from one \
     source and cannot disagree.",
    RECIPE_DECLARATIONS,
    &[examples::ASSEMBLY_SOURCES],
);

/// `allows`.
pub(crate) const ALLOWS: GuideEntry = GuideEntry::new(
    "allows",
    WordKind::Positional,
    "Declares one path pattern a wall admits; the patterns are what both of the wall's renderings \
     are projected from.",
    RECIPE_DECLARATIONS,
    &[examples::ASSEMBLY_SOURCES],
);

/// `excludes`.
pub(crate) const EXCLUDES: GuideEntry = GuideEntry::new(
    "excludes",
    WordKind::Positional,
    "Declares bytes that must not appear — a term a round measured out of a wall, or the sentinel \
     that says an artifact was never filled.",
    RECIPE_DECLARATIONS,
    &[examples::ASSEMBLY_SOURCES],
);

/// `artifact`.
pub(crate) const ARTIFACT: GuideEntry = GuideEntry::new(
    "artifact",
    WordKind::Positional,
    "Declares something a recipe reaches for outside the document, by the address its bytes live \
     at, so the reference census can measure it.",
    RECIPE_DECLARATIONS,
    &[examples::ASSEMBLY_SOURCES],
);

/// `digest`.
pub(crate) const DIGEST: GuideEntry = GuideEntry::new(
    "digest",
    WordKind::Positional,
    "Declares an artifact's content digest, which the fire step measures the deployed bytes \
     against before anything runs.",
    RECIPE_DECLARATIONS,
    &[examples::ASSEMBLY_SOURCES],
);

/// `restores`.
pub(crate) const RESTORES: GuideEntry = GuideEntry::new(
    "restores",
    WordKind::Positional,
    "Declares the path a fired artifact must restore byte-exactly; the restoration is \
     byte-compared and refuses if inexact.",
    RECIPE_DECLARATIONS,
    &[examples::RECIPE_STEPS],
);

/// `runs`.
pub(crate) const RUNS: GuideEntry = GuideEntry::new(
    "runs",
    WordKind::Positional,
    "Names the declared command a recipe step runs; a step never carries a shell string, which is \
     unparseable rather than discouraged.",
    RECIPE_DECLARATIONS,
    &[examples::RECIPE_STEPS],
);

/// `captures`.
pub(crate) const CAPTURES: GuideEntry = GuideEntry::new(
    "captures",
    WordKind::Positional,
    "Declares the typed result a step reads a declared artifact into.",
    RECIPE_DECLARATIONS,
    &[examples::RECIPE_STEPS],
);

/// `fires`.
pub(crate) const FIRES: GuideEntry = GuideEntry::new(
    "fires",
    WordKind::Positional,
    "Names the declared demonstration artifact a step fires, so a known-answer red is executed \
     rather than remembered.",
    RECIPE_DECLARATIONS,
    &[examples::RECIPE_STEPS],
);

/// `expecting`.
pub(crate) const EXPECTING: GuideEntry = GuideEntry::new(
    "expecting",
    WordKind::Positional,
    "Names the known answer a fired artifact is compared against, typed; a mismatch refuses the \
     stage with both sides' bytes.",
    RECIPE_DECLARATIONS,
    &[examples::RECIPE_STEPS],
);

/// `requires`.
pub(crate) const REQUIRES: GuideEntry = GuideEntry::new(
    "requires",
    WordKind::Positional,
    "Declares one at-fire precondition of a step; a precondition nobody answered is not a met one \
     and stops the fire.",
    RECIPE_DECLARATIONS,
    &[examples::RECIPE_STEPS],
);
