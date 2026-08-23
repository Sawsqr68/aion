//! The recipe surface: assembly acts with typed dependencies, parameter
//! kinds with declared derivations, mechanical specimen firing, walls
//! projected from one declaration, receipts generated rather than typed, and
//! referenced artifacts census-checked at fire time.

mod census;
mod check;
mod compare;
mod compose;
mod decl;
mod derivation;
mod fire;
mod graph;
mod parse;
mod print;
mod project;
mod receipt;
mod record;
mod sweep;
mod words;

pub use census::{
    ArtifactSource, CensusRow, SourceUnreadable, digest_of, measure, referenced_artifacts,
};
pub use check::{DISPATCH_DECLARED, check_assembly, is_unfilled_sentinel, unbound_bytes};
pub use compare::{
    ComparisonParts, StageComparison, VerdictRecord, VerdictRecordError, compare, source_text,
};
pub use compose::{Composed, Composition, compose};
pub use decl::{
    ArtifactDecl, Derivation, ParamLiteral, ParameterKind, RecipeDecl, RecipeParam, RecipeStep,
    StepBody, StepReference, WallDecl,
};
pub use derivation::{Extraction, parse_extraction, resolve};
pub use fire::{CommandRunner, Fired, Ports, Preconditions, SpecimenFirer, fire};
pub use graph::{DependencyCycle, topological_order};
pub use print::{
    artifact_body_lines, artifact_header, print_artifact_decl, print_recipe_decl, print_wall_decl,
    recipe_body_lines, recipe_header, wall_body_lines, wall_header,
};
pub use project::{WallDiff, WallProjection, diff, project};
pub use receipt::{MeasuredTree, Receipt, ReceiptError, StepOutcome, claim};
pub use record::record_of;
pub use sweep::{SourceKind, SweepRecord, SweptInput, sweep_by_claim};
pub use words::{ARTIFACT_CLAUSE_WORDS, RECIPE_CLAUSE_WORDS, STEP_CLAUSE_WORDS, WALL_CLAUSE_WORDS};

pub(crate) use parse::{parse_artifact_decl, parse_recipe_decl, parse_wall_decl};
