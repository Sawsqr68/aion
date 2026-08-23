//! The command-declaration cluster: the thirteen words a declared command is
//! written with.
//!
//! Every one is a POSITIONAL word. That is a measurement, not a preference —
//! `command` alone stands in 54 `.awl` documents of this tree today as a
//! harness-section field name, and `cwd`, `path`, `program`, `env`, `option`,
//! `operand` and `refuses` stand in others. Reserving any of them would break
//! documents that exist, so each is recognised in the one grammar position
//! where nothing else may stand.
//!
//! Each entry cites §17 of the language reference, which is where the surface
//! is explained in full; `tests/guide_reference_gate.rs` refuses any entry
//! whose cited section does not exist or does not use its word.

use super::entry::{COMMAND_DECLARATIONS, GuideEntry, WordKind};
use super::examples;

/// `command`.
pub(crate) const COMMAND: GuideEntry = GuideEntry::new(
    "command",
    WordKind::Positional,
    "Declares a command as a typed list — program, options, operands — never a string a shell \
     re-parses.",
    COMMAND_DECLARATIONS,
    &[examples::ADMITTING_COMMAND, examples::REFUSING_COMMAND],
);

/// `program`.
pub(crate) const PROGRAM: GuideEntry = GuideEntry::new(
    "program",
    WordKind::Positional,
    "Names the invocation — the program and the subcommand words that qualify it — as literals a \
     caller can never influence.",
    COMMAND_DECLARATIONS,
    &[examples::ADMITTING_COMMAND],
);

/// `admits`.
pub(crate) const ADMITS: GuideEntry = GuideEntry::new(
    "admits",
    WordKind::Positional,
    "Declares that this invocation takes the `--` end-of-options marker, so the emitter places it \
     before the operands unconditionally.",
    COMMAND_DECLARATIONS,
    &[examples::ADMITTING_COMMAND],
);

/// `refuses`.
pub(crate) const REFUSES: GuideEntry = GuideEntry::new(
    "refuses",
    WordKind::Positional,
    "Declares that this invocation has no `--` end-of-options marker, which makes a leading-dash \
     refusal specimen mandatory.",
    COMMAND_DECLARATIONS,
    &[examples::REFUSING_COMMAND],
);

/// `specimen`.
pub(crate) const SPECIMEN: GuideEntry = GuideEntry::new(
    "specimen",
    WordKind::Positional,
    "Names bytes beginning with `-` that a refusing invocation must refuse; the checker fires it \
     rather than taking the claim on trust.",
    COMMAND_DECLARATIONS,
    &[examples::REFUSING_COMMAND],
);

/// `option`.
pub(crate) const OPTION: GuideEntry = GuideEntry::new(
    "option",
    WordKind::Positional,
    "Declares one named option by its exact flag, with an optional typed value emitted as a \
     separate argv element.",
    COMMAND_DECLARATIONS,
    &[examples::REFUSING_COMMAND],
);

/// `operand`.
pub(crate) const OPERAND: GuideEntry = GuideEntry::new(
    "operand",
    WordKind::Positional,
    "Declares one positional operand with a name, a type, and the fill that produces it.",
    COMMAND_DECLARATIONS,
    &[examples::ADMITTING_COMMAND],
);

/// `env`.
pub(crate) const ENV: GuideEntry = GuideEntry::new(
    "env",
    WordKind::Positional,
    "Binds one environment variable structurally, as a name and a value, never as a `K=V` string \
     a shell would split.",
    COMMAND_DECLARATIONS,
    &[examples::REFUSING_COMMAND],
);

/// `cwd`.
pub(crate) const CWD: GuideEntry = GuideEntry::new(
    "cwd",
    WordKind::Positional,
    "Declares the working directory; `{workspace_root}` is the one placeholder it admits.",
    COMMAND_DECLARATIONS,
    &[examples::REFUSING_COMMAND],
);

/// `hardening`.
pub(crate) const HARDENING: GuideEntry = GuideEntry::new(
    "hardening",
    WordKind::Positional,
    "Opens the PATH and open-file block, stated once per scope and inherited.",
    COMMAND_DECLARATIONS,
    &[examples::REFUSING_COMMAND],
);

/// `path`.
pub(crate) const PATH: GuideEntry = GuideEntry::new(
    "path",
    WordKind::Positional,
    "Declares, inside a `hardening` block, the PATH the command runs under.",
    COMMAND_DECLARATIONS,
    &[examples::REFUSING_COMMAND],
);

/// `open_files`.
pub(crate) const OPEN_FILES: GuideEntry = GuideEntry::new(
    "open_files",
    WordKind::Positional,
    "Declares, inside a `hardening` block, the open-file ceiling the command runs under.",
    COMMAND_DECLARATIONS,
    &[examples::REFUSING_COMMAND],
);

/// `owner`.
pub(crate) const OWNER: GuideEntry = GuideEntry::new(
    "owner",
    WordKind::Positional,
    "Names who owns a command's declared timeout; a ceiling nobody owns is a number nobody chose.",
    COMMAND_DECLARATIONS,
    &[examples::REFUSING_COMMAND],
);
