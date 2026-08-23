//! The shape of a declared command: a typed list, never a string a shell
//! re-parses.
//!
//! # Why there is no command string anywhere in this module
//!
//! The defect family this surface closes is the one where a command is
//! carried as text and re-split by whatever runs it. A value containing a
//! space becomes two arguments; a value beginning with `-` becomes an option;
//! a value containing a quote becomes anything at all. Every one of those is
//! a property of the RE-SPLIT, not of the command, so the cure is to never
//! produce a string that could be re-split: a declaration names its program,
//! its options and its operands as separate typed things, and the emitter
//! produces a `Vec<String>` of argv elements that no one parses again.
//!
//! # The end-of-options property is DECLARED
//!
//! [`EndOfOptions`] is stated by the author, per invocation, and is never
//! inferred from the program's name. Inference is not available in principle:
//! whether `--` separates options from operands is a fact about a specific
//! invocation of a specific program, and the same program answers differently
//! for different subcommands. `git tag -- <name> <sha>` is the cure for the
//! leading-dash family; `git diff -- <path>` separates revisions from PATHS,
//! so writing `git diff -- BASE..HEAD` silently reinterprets a revision range
//! as a pathspec and changes what the command measures. One `git` cannot be
//! both, which is why the declared unit is the INVOCATION — program plus its
//! subcommand words — and not the bare program.
//!
//! # Program words are literals, never fills
//!
//! [`ProgramDecl`] carries plain string words with no interpolation. This
//! mirrors the rule the existing declared-body scanner already enforces
//! (`crate::ast::command`, which refuses a parameter reference anywhere in
//! the program word) and for the same reason: a caller-supplied value must
//! never influence which program executes.

use crate::Span;
use crate::ast::{Comment, DocLine, DurationLiteral, Lead, TypeRef};

use super::fill::Fill;

/// One declared command: a name, typed parameters, and the invocation those
/// parameters fill.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandDecl {
    /// Span covering the declaration's header line.
    pub span: Span,
    /// Leading trivia before the declaration.
    pub lead: Vec<Lead>,
    /// `///` documentation lines attached to the declaration.
    pub docs: Vec<DocLine>,
    /// Same-line comment on the header line.
    pub trailing: Option<Comment>,
    /// The command's name.
    pub name: String,
    /// Source span of the name.
    pub name_span: Span,
    /// Declared parameters, in source order. These are the ONLY names a fill
    /// may interpolate.
    pub parameters: Vec<CommandParam>,
    /// The invocation: program plus subcommand words.
    ///
    /// `None` when the declaration carried no `program` clause. The AST is
    /// the faithful parse product; that a declaration MUST name a program is
    /// a check-layer ruling, not something the grammar can put right.
    pub program: Option<ProgramDecl>,
    /// The `admits`/`refuses` clauses in source order.
    ///
    /// A vector, not an `Option`, because "stated twice" is a thing an author
    /// can write and the checker must be able to SEE — collapsing it here
    /// would make the second statement invisible to the diagnostic that
    /// refuses it.
    pub end_of_options: Vec<EndOfOptionsClause>,
    /// The leading-dash refusal specimen, if one was written.
    pub specimen: Option<LeadingDashSpecimen>,
    /// Declared options, in source order — emitted before the operands.
    pub options: Vec<OptionDecl>,
    /// Declared operands, in source order — emitted after the options.
    pub operands: Vec<OperandDecl>,
    /// Declared environment bindings, in source order.
    pub env: Vec<EnvBinding>,
    /// The declared working directory, if the command declares one.
    pub cwd: Option<CwdDecl>,
    /// The `hardening` blocks in source order. One home per scope is a
    /// check-layer ruling, so a second block is carried here and refused
    /// there.
    pub hardening: Vec<HardeningDecl>,
    /// The declared timeout, if the command declares one.
    ///
    /// ABSENCE IS A LEGAL DECLARED STATE. A command with no timeout emits no
    /// timeout and defers to the engine's own behaviour; this surface invents
    /// no ceiling and no default.
    pub timeout: Option<CommandTimeout>,
}

/// One declared parameter: a name, a type, and optionally a default.
///
/// Defaults are the absorbed Just shape (`recipe param="value"`), typed here
/// because AWL's parameters are typed and Just's are not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandParam {
    /// Span covering the parameter.
    pub span: Span,
    /// Parameter name.
    pub name: String,
    /// Source span of the name.
    pub name_span: Span,
    /// Declared type.
    pub ty: TypeRef,
    /// The default this parameter takes when a parameter set omits it.
    pub default: Option<Fill>,
}

/// The invocation: the program and the subcommand words that qualify it.
///
/// `program "git" "diff"` declares the invocation `git diff`, which is the
/// unit the end-of-options property is a fact about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramDecl {
    /// Span covering the clause.
    pub span: Span,
    /// The literal words, in order. Never fills.
    pub words: Vec<ProgramWord>,
}

/// One literal word of an invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramWord {
    /// Source span of the word's literal.
    pub span: Span,
    /// The word itself.
    pub text: String,
}

/// The end-of-options marker the language knows. The bytes are the contract.
pub const END_OF_OPTIONS_MARKER: &str = "--";

/// One `admits "--"` or `refuses "--"` statement, as written.
///
/// The end-of-options property is DECLARED, author-stated, and per
/// invocation. It is never inferred, and inference is not available in
/// principle — see this module's own note on `git tag` and `git diff`.
///
/// Two faces, two disciplines. An admitting invocation gets `--` placed
/// between its options and its operands UNCONDITIONALLY, so the leading-dash
/// family closes by construction rather than by review. A refusing invocation
/// must carry a leading-dash refusal specimen, so that what protects it is
/// tested rather than assumed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndOfOptionsClause {
    /// Span covering the clause.
    pub span: Span,
    /// `true` for `admits "--"`, `false` for `refuses "--"`.
    pub admits: bool,
}

/// The mandatory specimen a refusing invocation carries: bytes beginning with
/// `-` that the surface must refuse in an operand position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeadingDashSpecimen {
    /// Span covering the `specimen` clause.
    pub span: Span,
    /// The specimen bytes, verbatim.
    pub bytes: String,
    /// Source span of the specimen literal.
    pub bytes_span: Span,
}

/// A named option: its flag, and the typed value it carries when it has one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionDecl {
    /// Span covering the clause.
    pub span: Span,
    /// The flag as it appears in the argv, verbatim (`--name-only`).
    pub flag: String,
    /// Source span of the flag literal.
    pub flag_span: Span,
    /// The typed value, when the option takes one. A bare flag has none and
    /// emits exactly one argv element.
    pub value: Option<TypedFill>,
}

/// A positional operand: a name for diagnostics, a type, and its fill.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperandDecl {
    /// Span covering the clause.
    pub span: Span,
    /// The operand's name. Diagnostics name it; the argv does not carry it.
    pub name: String,
    /// Source span of the name.
    pub name_span: Span,
    /// The typed value.
    pub value: TypedFill,
}

/// A typed fill: the declared type and the template that produces the value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedFill {
    /// The declared type of the value.
    pub ty: TypeRef,
    /// The fill that produces it.
    pub fill: Fill,
}

/// One environment binding, structural: a name and a value.
///
/// Never a `K=V` string prefix. The prefix form is unparseable, not linted —
/// the grammar has no production that puts an `=` inside a value.
///
/// Environment values are strings and carry no type annotation, because the
/// operating system's answer is a string whatever AWL thinks. A fill that
/// resolves to a list has no single environment rendering and is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvBinding {
    /// Span covering the clause.
    pub span: Span,
    /// The variable name.
    pub name: String,
    /// Source span of the name.
    pub name_span: Span,
    /// The fill that produces the value.
    pub fill: Fill,
}

/// The declared working directory.
///
/// `{workspace_root}` is the ONE admitted placeholder, consumed from the
/// existing rule (`crate::workspace_root`) rather than extended. Any other
/// brace placeholder is a check error: the closed-delta-set fence, mechanical.
/// This cut adds no new venue-binding holes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CwdDecl {
    /// Span covering the clause.
    pub span: Span,
    /// The declared path, verbatim.
    pub value: String,
    /// Source span of the path literal.
    pub value_span: Span,
}

/// The PATH/ulimit hardening, stated once on the executing declaration scope.
///
/// One home per scope: a second `hardening` block is a check error, because
/// two statements of the same hardening are two answers to one question and
/// nothing says which one runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HardeningDecl {
    /// Span covering the block's opening line.
    pub span: Span,
    /// The declared PATH, if stated.
    pub path: Option<HardeningPath>,
    /// The declared open-file ceiling, if stated.
    pub open_files: Option<HardeningLimit>,
}

impl HardeningDecl {
    /// Whether the block states anything at all.
    ///
    /// A block that states nothing hardens nothing, and is refused rather
    /// than silently treated as absent — the two are different claims.
    #[must_use]
    pub const fn states_something(&self) -> bool {
        self.path.is_some() || self.open_files.is_some()
    }
}

/// The PATH the command runs under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HardeningPath {
    /// Span covering the clause.
    pub span: Span,
    /// The fill that produces the PATH.
    pub fill: Fill,
}

/// The open-file ceiling the command runs under (`ulimit -n`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HardeningLimit {
    /// Span covering the clause.
    pub span: Span,
    /// The declared ceiling.
    pub value: u64,
    /// Source span of the number.
    pub value_span: Span,
}

/// A declared timeout and the owner accountable for the number.
///
/// The owner is not decoration: a timeout without one is a number nobody
/// chose, and the estate's no-assumed-defaults law makes an unowned ceiling
/// indistinguishable from an invented one. ABSENCE of the whole clause is a
/// legal declared state; an unowned presence is not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandTimeout {
    /// Span covering the clause.
    pub span: Span,
    /// The declared duration.
    pub duration: DurationLiteral,
    /// Who owns this number.
    pub owner: String,
    /// Source span of the owner literal.
    pub owner_span: Span,
}
