//! The typed command-declaration surface: structural argv, typed check-time
//! interpolation, declared env, cwd, hardening and timeout.

mod cause;
mod check;
mod check_fills;
mod clauses;
mod decl;
mod derived;
mod emit;
mod error;
mod fill;
mod just_subset;
mod parse;
mod print;
pub mod recipe;
mod specimen;
mod words;

pub use cause::{CommandCause, CommandLayer};
pub use check::{CheckedCommand, check_command, check_commands};
pub use decl::{
    CommandDecl, CommandParam, CommandTimeout, CwdDecl, END_OF_OPTIONS_MARKER, EndOfOptionsClause,
    EnvBinding, HardeningDecl, HardeningLimit, HardeningPath, LeadingDashSpecimen, OperandDecl,
    OptionDecl, ProgramDecl, ProgramWord, TypedFill,
};
pub use derived::{DerivedShape, derived_shape_in};
pub use emit::{ArgumentValue, Argv, ParameterSet, emit};
pub use error::{
    CauseMisfiled, CommandCheckError, CommandComposeError, CommandEmitError, CommandFireError,
    CommandParseError, CommandRefusal, CommandSpecimenError,
};
pub use fill::{Fill, FillSegment};
pub use just_subset::{
    ABSORBED_PRODUCTIONS, AbsorbedProduction, REFUSED_FORMS, RefusedForm, SUITE_COMMIT,
    SUITE_LICENSE, SUITE_REPO,
};

pub(crate) use parse::parse_command_decl;
pub use print::{command_body_lines, command_header, print_command_decl};
pub(crate) use recipe::{parse_artifact_decl, parse_recipe_decl, parse_wall_decl};
pub use specimen::fire_specimen;
