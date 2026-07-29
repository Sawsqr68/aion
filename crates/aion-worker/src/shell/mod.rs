//! Declared commands as activities — an action that runs without a Rust crate.

pub mod action;
pub mod registry;
pub mod template;

pub use action::{FailureMode, ShellAction, ShellOutcome};
pub use registry::{DeclaredArguments, DeclaredCommands};
pub use template::{CommandTemplate, SubstitutionError, TemplateError};
