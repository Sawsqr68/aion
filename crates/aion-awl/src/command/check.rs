//! The check layer: what a parsed declaration must SAY to be sound.
//!
//! Parsing established that the bytes are a command declaration. This
//! establishes that the declaration is one a machine could act on: it names a
//! program, it states its end-of-options property exactly once, a refusing
//! invocation carries the specimen that proves its protection, every
//! interpolation names a parameter that exists and has the right type, no
//! hand literal stands where a derivation belongs, and the cwd reaches for
//! nothing but the one admitted placeholder.
//!
//! The product is a [`CheckedCommand`]: a declaration whose invariants are
//! resolved once, so the emitter does not re-derive them and cannot disagree
//! with the checker about what was decided.

use std::collections::BTreeMap;

use crate::Span;
use crate::workspace_root::WORKSPACE_ROOT_PLACEHOLDER;

use super::cause::CommandCause;
use super::check_fills::{check_fills, check_literal_derivations};
use super::decl::{CommandDecl, HardeningDecl, LeadingDashSpecimen, ProgramDecl};
use super::error::CommandRefusal;
use super::words::{
    ADMITS_WORD, CWD_WORD, HARDENING_WORD, OPEN_FILES_WORD, PATH_WORD, PROGRAM_WORD, REFUSES_WORD,
    SPECIMEN_WORD,
};

/// A declaration every check-layer rule has passed, with its invariants
/// resolved exactly once.
#[derive(Debug, Clone, Copy)]
pub struct CheckedCommand<'a> {
    decl: &'a CommandDecl,
    program: &'a ProgramDecl,
    admits: bool,
    specimen: Option<&'a LeadingDashSpecimen>,
    hardening: Option<&'a HardeningDecl>,
}

impl<'a> CheckedCommand<'a> {
    /// The declaration this was checked from.
    #[must_use]
    pub const fn decl(self) -> &'a CommandDecl {
        self.decl
    }

    /// The invocation: program plus subcommand words.
    #[must_use]
    pub const fn program(self) -> &'a ProgramDecl {
        self.program
    }

    /// Whether the invocation admits the `--` end-of-options marker.
    ///
    /// Resolved here and read by the emitter, so the two cannot disagree
    /// about a fact the author stated once.
    #[must_use]
    pub const fn admits_end_of_options(self) -> bool {
        self.admits
    }

    /// The leading-dash refusal specimen a refusing invocation carries.
    #[must_use]
    pub const fn specimen(self) -> Option<&'a LeadingDashSpecimen> {
        self.specimen
    }

    /// The one hardening block, if the command declares one.
    #[must_use]
    pub const fn hardening(self) -> Option<&'a HardeningDecl> {
        self.hardening
    }
}

/// Check every command declaration of a document, including the rules that
/// are about the SET of them rather than about any one.
#[must_use]
pub fn check_commands(commands: &[CommandDecl]) -> Vec<CommandRefusal> {
    let mut refusals = Vec::new();
    let mut seen: BTreeMap<&str, Span> = BTreeMap::new();
    for command in commands {
        if let Some(first) = seen.insert(command.name.as_str(), command.name_span) {
            refusals.push(CommandRefusal::refuse(
                CommandCause::CommandRestated,
                command.name_span,
                &command.name,
                format!(
                    "command `{}` is declared twice in this document; the first stands at line \
                     {}, and two declarations of one name are two answers to which command runs",
                    command.name, first.line
                ),
            ));
        }
        if let Err(mut command_refusals) = check_command(command) {
            refusals.append(&mut command_refusals);
        }
    }
    refusals
}

/// Check one declaration, producing every refusal it earns.
///
/// Every rule runs — the function does not stop at the first refusal —
/// because an author fixing one diagnostic at a time is an author running the
/// checker five times to learn five things it already knew.
///
/// # Errors
///
/// Returns every check-layer refusal the declaration earns.
pub fn check_command(decl: &CommandDecl) -> Result<CheckedCommand<'_>, Vec<CommandRefusal>> {
    let mut refusals = Vec::new();
    let program = check_program(decl, &mut refusals);
    let (admits, specimen) = check_end_of_options(decl, &mut refusals);
    let hardening = check_hardening(decl, &mut refusals);
    check_parameters(decl, &mut refusals);
    check_names_are_stated_once(decl, &mut refusals);
    check_fills(decl, &mut refusals);
    check_cwd(decl, &mut refusals);
    check_timeout(decl, &mut refusals);

    match (program, admits) {
        (Some(program), Some(admits)) if refusals.is_empty() => Ok(CheckedCommand {
            decl,
            program,
            admits,
            specimen,
            hardening,
        }),
        _ => {
            if refusals.is_empty() {
                refusals.push(CommandRefusal::refuse(
                    CommandCause::ProgramUnstated,
                    decl.span,
                    &decl.name,
                    format!("command `{}` did not resolve to an invocation", decl.name),
                ));
            }
            Err(refusals)
        }
    }
}

/// The declaration must name a program.
fn check_program<'a>(
    decl: &'a CommandDecl,
    refusals: &mut Vec<CommandRefusal>,
) -> Option<&'a ProgramDecl> {
    if decl.program.is_none() {
        refusals.push(CommandRefusal::refuse(
            CommandCause::ProgramUnstated,
            decl.span,
            &decl.name,
            format!(
                "command `{}` carries no `{PROGRAM_WORD}` clause, so nothing says what runs",
                decl.name
            ),
        ));
    }
    decl.program.as_ref()
}

/// The end-of-options property: stated exactly once, and a refusing
/// invocation carries its mandatory specimen.
fn check_end_of_options<'a>(
    decl: &'a CommandDecl,
    refusals: &mut Vec<CommandRefusal>,
) -> (Option<bool>, Option<&'a LeadingDashSpecimen>) {
    let admits = match decl.end_of_options.as_slice() {
        [] => {
            refusals.push(CommandRefusal::refuse(
                CommandCause::EndOfOptionsUnstated,
                decl.span,
                &decl.name,
                format!(
                    "command `{}` states neither `{ADMITS_WORD} \"--\"` nor `{REFUSES_WORD} \
                     \"--\"`. Whether an invocation admits the end-of-options marker is a \
                     declared, author-stated, per-invocation fact and is never inferred — `git \
                     tag` admits it while `git diff` would read a `--` as the start of a \
                     pathspec. No command may sit in an unstated middle",
                    decl.name
                ),
            ));
            None
        }
        [only] => Some(only.admits),
        [first, second, ..] => {
            refusals.push(CommandRefusal::refuse(
                CommandCause::EndOfOptionsRestated,
                second.span,
                if second.admits {
                    ADMITS_WORD
                } else {
                    REFUSES_WORD
                },
                format!(
                    "command `{}` states its end-of-options property again at line {}, having \
                     already stated it at line {}; an invocation either admits `--` or refuses \
                     it, and two statements are two answers",
                    decl.name, second.span.line, first.span.line
                ),
            ));
            None
        }
    };

    let specimen = decl.specimen.as_ref();
    match (admits, specimen) {
        (Some(false), None) => {
            let at = decl
                .end_of_options
                .first()
                .map_or(decl.span, |clause| clause.span);
            refusals.push(CommandRefusal::refuse(
                CommandCause::LeadingDashSpecimenMissing,
                at,
                REFUSES_WORD,
                format!(
                    "command `{}` refuses `--`, so a leading-dash operand would be read as an \
                     option and nothing would stop it. Write `{SPECIMEN_WORD} \"-x\"` naming \
                     bytes this command must refuse. The specimen is MANDATORY: what protects a \
                     refusing invocation is tested, never assumed",
                    decl.name
                ),
            ));
        }
        (Some(true), Some(specimen)) => {
            refusals.push(CommandRefusal::refuse(
                CommandCause::SpecimenWithoutRefusal,
                specimen.span,
                &specimen.bytes,
                format!(
                    "command `{}` admits `--`, so the emitter places the marker unconditionally \
                     and the leading-dash family closes by construction; a specimen here proves \
                     nothing and nothing keeps it honest",
                    decl.name
                ),
            ));
        }
        (_, Some(specimen)) if !specimen.bytes.starts_with('-') => {
            refusals.push(CommandRefusal::refuse(
                CommandCause::LeadingDashSpecimenNotLeading,
                specimen.bytes_span,
                &specimen.bytes,
                format!(
                    "command `{}`'s specimen does not begin with `-`, so firing it would prove \
                     nothing about the family it stands for; a leading-dash refusal specimen \
                     names bytes that LEAD with a dash",
                    decl.name
                ),
            ));
        }
        _ => {}
    }
    (admits, specimen)
}

/// Hardening has one home per scope, and a block that states nothing hardens
/// nothing.
fn check_hardening<'a>(
    decl: &'a CommandDecl,
    refusals: &mut Vec<CommandRefusal>,
) -> Option<&'a HardeningDecl> {
    if let [first, second, ..] = decl.hardening.as_slice() {
        refusals.push(CommandRefusal::refuse(
            CommandCause::HardeningRestated,
            second.span,
            HARDENING_WORD,
            format!(
                "command `{}` states `{HARDENING_WORD}` again at line {}, having already stated \
                 it at line {}. Hardening is stated once on the executing declaration scope and \
                 inherited; two statements are two answers to one question and nothing says \
                 which one runs",
                decl.name, second.span.line, first.span.line
            ),
        ));
    }
    let block = decl.hardening.first();
    if let Some(block) = block
        && !block.states_something()
    {
        refusals.push(CommandRefusal::refuse(
            CommandCause::HardeningEmpty,
            block.span,
            HARDENING_WORD,
            format!(
                "command `{}`'s `{HARDENING_WORD}` block states nothing, so it hardens \
                     nothing; declare a `{PATH_WORD}`, an `{OPEN_FILES_WORD}`, or remove the \
                     block — an empty block and an absent one are different claims and must not \
                     read alike",
                decl.name
            ),
        ));
    }
    block
}

/// A parameter's default may reference only parameters declared before it,
/// which is the absorbed Just rule made typed.
fn check_parameters(decl: &CommandDecl, refusals: &mut Vec<CommandRefusal>) {
    for (index, parameter) in decl.parameters.iter().enumerate() {
        let Some(default) = &parameter.default else {
            continue;
        };
        for (name, span) in default.holes() {
            let known = decl.parameters[..index]
                .iter()
                .any(|earlier| earlier.name == name);
            if !known {
                refusals.push(CommandRefusal::refuse(
                    CommandCause::ParameterUndeclared,
                    span,
                    &default.source,
                    format!(
                        "`{{{{{name}}}}}` in `{}`'s default names no parameter declared before \
                         it; a default may reference only the parameters to its left, because a \
                         default that reached forward would depend on a value not yet bound",
                        parameter.name
                    ),
                ));
            }
        }
        check_literal_derivations(default, refusals, &parameter.name);
    }
}

/// Every name a declaration binds is bound once.
fn check_names_are_stated_once(decl: &CommandDecl, refusals: &mut Vec<CommandRefusal>) {
    restated(
        decl.parameters
            .iter()
            .map(|item| (item.name.as_str(), item.name_span)),
        CommandCause::ParameterRestated,
        "parameter",
        &decl.name,
        refusals,
    );
    restated(
        decl.operands
            .iter()
            .map(|item| (item.name.as_str(), item.name_span)),
        CommandCause::OperandRestated,
        "operand",
        &decl.name,
        refusals,
    );
    restated(
        decl.options
            .iter()
            .map(|item| (item.flag.as_str(), item.flag_span)),
        CommandCause::OptionRestated,
        "option",
        &decl.name,
        refusals,
    );
    restated(
        decl.env
            .iter()
            .map(|item| (item.name.as_str(), item.name_span)),
        CommandCause::EnvRestated,
        "environment binding",
        &decl.name,
        refusals,
    );
}

/// Refuse a name bound twice, naming where it was bound first.
fn restated<'a>(
    items: impl Iterator<Item = (&'a str, Span)>,
    cause: CommandCause,
    what: &str,
    command: &str,
    refusals: &mut Vec<CommandRefusal>,
) {
    let mut seen: BTreeMap<&str, Span> = BTreeMap::new();
    for (name, span) in items {
        if let Some(first) = seen.insert(name, span) {
            refusals.push(CommandRefusal::refuse(
                cause,
                span,
                name,
                format!(
                    "command `{command}` declares the {what} `{name}` again, having already \
                     declared it at line {}; one name, one binding",
                    first.line
                ),
            ));
        }
    }
}

/// `{workspace_root}` is the ONE admitted placeholder. The closed-delta-set
/// fence, mechanical: any other brace placeholder is refused, and this cut
/// adds no new venue-binding holes.
fn check_cwd(decl: &CommandDecl, refusals: &mut Vec<CommandRefusal>) {
    let Some(cwd) = &decl.cwd else {
        return;
    };
    let mut rest = cwd.value.as_str();
    while let Some(open) = rest.find('{') {
        let after = &rest[open..];
        let Some(close) = after.find('}') else {
            refusals.push(CommandRefusal::refuse(
                CommandCause::CwdPlaceholderUnknown,
                cwd.value_span,
                &cwd.value,
                format!(
                    "command `{}`'s `{CWD_WORD}` opens a `{{` that never closes; the one \
                     placeholder a working directory admits is `{WORKSPACE_ROOT_PLACEHOLDER}`",
                    decl.name
                ),
            ));
            return;
        };
        let placeholder = &after[..=close];
        if placeholder != WORKSPACE_ROOT_PLACEHOLDER {
            refusals.push(CommandRefusal::refuse(
                CommandCause::CwdPlaceholderUnknown,
                cwd.value_span,
                placeholder,
                format!(
                    "command `{}`'s `{CWD_WORD}` reaches for `{placeholder}`. \
                     `{WORKSPACE_ROOT_PLACEHOLDER}` is the ONE placeholder a working directory \
                     admits, and this surface adds no new venue-binding holes: binding a command \
                     to a venue is the deploy surface's act, not a declaration's",
                    decl.name
                ),
            ));
        }
        rest = &after[close + 1..];
    }
}

/// A declared timeout names an owner. Absence of the whole clause is legal;
/// an unowned presence is not.
fn check_timeout(decl: &CommandDecl, refusals: &mut Vec<CommandRefusal>) {
    let Some(timeout) = &decl.timeout else {
        return;
    };
    if timeout.owner.trim().is_empty() {
        refusals.push(CommandRefusal::refuse(
            CommandCause::TimeoutOwnerEmpty,
            timeout.owner_span,
            &timeout.owner,
            format!(
                "command `{}` declares a timeout whose owner names nobody; a ceiling nobody owns \
                 is a number nobody chose, which is the shape an invented default takes",
                decl.name
            ),
        ));
    }
}
