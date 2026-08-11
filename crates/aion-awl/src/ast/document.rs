use crate::Span;

use super::expr::{DurationLiteral, Expr};
use super::harness::HarnessDecl;
use super::steps::Step;
use super::trivia::{Comment, DocLine, Lead};

/// Which family a `.awl` document belongs to.
///
/// The two families are told apart by the file's own opening declaration —
/// `workflow <name>` or `worker <queue>` — and nothing else. One file is one
/// family: a document either describes a flow (its inputs, outcomes and
/// steps) or serves a queue (its actions and their bodies), and the checker
/// refuses the other family's declarations by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentFamily {
    /// `workflow <name>` — a flow: typed start contract, outcomes, steps, and
    /// the worker blocks it REQUIRES.
    Workflow,
    /// `worker <queue>` — an implementation: the actions one task queue
    /// serves, each carrying the body that runs it.
    Worker,
}

impl DocumentFamily {
    /// The opening keyword that declares this family.
    #[must_use]
    pub const fn keyword(self) -> &'static str {
        match self {
            Self::Workflow => "workflow",
            Self::Worker => "worker",
        }
    }
}

/// Parsed representation of a complete rev-2 document, of either family.
///
/// One struct for both families on purpose: a worker document's types,
/// actions, bodies and trivia are the same constructs a workflow document's
/// `worker` blocks carry, so parsing, checking and printing share one
/// implementation and cannot drift. [`Document::family`] records which
/// opening declaration the file carried, and the family-conditional rules
/// live in one checker pass rather than in a second parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    /// Span covering the whole source document.
    pub span: Span,
    /// Which family the file's opening declaration put it in.
    pub family: DocumentFamily,
    /// The `//!` narration lines at the top of the document (one or more).
    pub narration: Vec<DocLine>,
    /// Own-line comments between the narration and the `workflow` line.
    pub lead: Vec<Lead>,
    /// The name declared after the opening keyword: the workflow name in a
    /// [`DocumentFamily::Workflow`] document, the task queue in a
    /// [`DocumentFamily::Worker`] one.
    pub name: String,
    /// Source span of that name.
    pub name_span: Span,
    /// Same-line comment on the opening declaration's line.
    pub trailing: Option<Comment>,
    /// Optional document-level workflow timeout.
    pub timeout: Option<WorkflowTimeoutDecl>,
    /// `input` declarations in source order.
    pub inputs: Vec<InputDecl>,
    /// `signal` declarations in source order.
    pub signals: Vec<SignalDecl>,
    /// `send` declarations in source order — the OUTBOUND signal surface.
    pub sends: Vec<SendDecl>,
    /// `query` declarations in source order — the READ-ONLY surface an
    /// operator may ask of a running instance.
    pub queries: Vec<QueryDecl>,
    /// Workflow `outcome` declarations in source order (at least one).
    pub outcomes: Vec<OutcomeDecl>,
    /// `const` declarations in source order.
    pub consts: Vec<ConstDecl>,
    /// `type` declarations in source order.
    pub types: Vec<TypeDecl>,
    /// `worker` blocks in source order.
    pub workers: Vec<WorkerDecl>,
    /// `child` workflow declarations in source order.
    pub children: Vec<ChildDecl>,
    /// `subflow` declarations in source order.
    pub subflows: Vec<SubflowDecl>,
    /// `step` declarations in source order.
    pub steps: Vec<Step>,
    /// Trailing trivia at the very end of the document.
    pub epilogue: Vec<Lead>,
}

/// One `timeout <duration>` declaration in the workflow header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowTimeoutDecl {
    /// Span covering the declaration line.
    pub span: Span,
    /// Leading trivia before the declaration.
    pub lead: Vec<Lead>,
    /// Same-line trailing comment.
    pub trailing: Option<Comment>,
    /// Duration literal, using the shared AWL duration syntax.
    pub duration: DurationLiteral,
    /// Whether the source duration carried a leading minus sign.
    pub negative: bool,
}

/// One `input name: Type` declaration in the workflow header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputDecl {
    /// Span covering the declaration line.
    pub span: Span,
    /// Leading trivia before the declaration.
    pub lead: Vec<Lead>,
    /// Same-line trailing comment.
    pub trailing: Option<Comment>,
    /// Input name.
    pub name: String,
    /// Source span of the input name.
    pub name_span: Span,
    /// Declared contract type.
    pub ty: TypeRef,
}

/// One `signal name: Type` declaration in the workflow header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalDecl {
    /// Span covering the declaration line.
    pub span: Span,
    /// Leading trivia before the declaration.
    pub lead: Vec<Lead>,
    /// Same-line trailing comment.
    pub trailing: Option<Comment>,
    /// Signal name.
    pub name: String,
    /// Source span of the signal name.
    pub name_span: Span,
    /// Declared payload type.
    pub ty: TypeRef,
}

/// One `send name: Type` declaration in the workflow header: a signal this
/// workflow SENDS to another workflow.
///
/// Separate from [`SignalDecl`] because the two are different directions of
/// a workflow's surface and only the inbound one is a contract the engine
/// can route to this run. Keeping them apart is also what lets a relay
/// declare `signal cancel: Reason` and `send cancel: Reason` in one file
/// without either declaration lying about the other.
///
/// The declaration exists to close the language's one silent hole: the
/// engine routes signals BY NAME, so a send whose name no receiver awaits
/// succeeds and does nothing. AWL cannot verify the far side (the target is
/// a runtime id — see the `send` statement), but it CAN refuse a send whose
/// name and payload shape the document never stated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendDecl {
    /// Span covering the declaration line.
    pub span: Span,
    /// Leading trivia before the declaration.
    pub lead: Vec<Lead>,
    /// Same-line trailing comment.
    pub trailing: Option<Comment>,
    /// Outbound signal name — the name the engine routes on.
    pub name: String,
    /// Source span of the signal name.
    pub name_span: Span,
    /// Declared payload type.
    pub ty: TypeRef,
}

/// One `query name: Type` declaration in the workflow header: a question an
/// operator may ask a RUNNING instance of this workflow, answered from live
/// in-memory state without appending an event.
///
/// The declared type is the query's ANSWER type — what a caller receives —
/// which is why this is a separate declaration from [`SignalDecl`] and
/// [`SendDecl`] rather than a third direction of one: a signal's type is what
/// crosses the wire INTO or OUT OF the run as an effect, and a query's is what
/// the run reports about itself when asked.
///
/// A declaration alone answers nothing. The value is registered by the
/// `answer` statement, and a declared query that no `answer` ever registers is
/// a check error — an operator querying such a name would get `UnknownQuery`
/// from the engine, which reads as "this workflow has no such query" when the
/// truth is "the author declared it and never set it".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryDecl {
    /// Span covering the declaration line.
    pub span: Span,
    /// Leading trivia before the declaration.
    pub lead: Vec<Lead>,
    /// Same-line trailing comment.
    pub trailing: Option<Comment>,
    /// Query name — the name an operator dispatches on.
    pub name: String,
    /// Source span of the query name.
    pub name_span: Span,
    /// Declared ANSWER type.
    pub ty: TypeRef,
}

/// Engine terminal status a workflow outcome maps to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteDirection {
    /// `route success` — the run completes.
    Success,
    /// `route failure` — the run fails.
    Failure,
}

/// One `outcome name: type T, route success|failure` header declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomeDecl {
    /// Span covering the declaration line.
    pub span: Span,
    /// Leading trivia before the declaration.
    pub lead: Vec<Lead>,
    /// Same-line trailing comment.
    pub trailing: Option<Comment>,
    /// Outcome name.
    pub name: String,
    /// Source span of the outcome name.
    pub name_span: Span,
    /// Payload type the outcome carries.
    pub ty: TypeRef,
    /// Terminal status the outcome maps to.
    pub direction: RouteDirection,
}

/// One document-level `const name = <value>` declaration. The value is
/// restricted to compile-time forms — literals (including raw strings and
/// `json { … }` bodies), `schema of Type`, list literals of these, `+`
/// concatenations of these, and references to other consts — and is folded
/// before lowering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstDecl {
    /// Span from the `const` keyword through the value expression.
    pub span: Span,
    /// Leading trivia before the declaration.
    pub lead: Vec<Lead>,
    /// `///` doc lines attached to the declaration.
    pub docs: Vec<DocLine>,
    /// Same-line trailing comment (after the value's last line).
    pub trailing: Option<Comment>,
    /// Declared const name.
    pub name: String,
    /// Source span of the const name.
    pub name_span: Span,
    /// The compile-time value expression.
    pub value: Expr,
}

/// Type expression used by declarations, fields, and parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeRef {
    /// Reference to a named type (builtin or declared).
    Named {
        /// Source span of the type name.
        span: Span,
        /// Referenced type name.
        name: String,
    },
    /// List type `[T]`.
    List {
        /// Source span from `[` through `]`.
        span: Span,
        /// Element type.
        inner: Box<TypeRef>,
    },
    /// Optional type `T?` — the value may be absent (never null).
    Optional {
        /// Source span from the inner type through `?`.
        span: Span,
        /// Wrapped type.
        inner: Box<TypeRef>,
    },
}

/// A `type` declaration through any of the three doors, or an enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeDecl {
    /// Span covering the declaration (header line for multi-line bodies).
    pub span: Span,
    /// Leading trivia before the declaration.
    pub lead: Vec<Lead>,
    /// `///` doc lines attached to the declaration.
    pub docs: Vec<DocLine>,
    /// Same-line trailing comment.
    pub trailing: Option<Comment>,
    /// Declared type name.
    pub name: String,
    /// Source span of the type name.
    pub name_span: Span,
    /// The declaration body.
    pub body: TypeBody,
}

/// Body of a `type` declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeBody {
    /// Shorthand record: `type T { field: Type, … }`.
    Record {
        /// Field declarations in source order.
        fields: Vec<FieldDecl>,
    },
    /// Payload-less enum: `type T = A | B | C`.
    Enum {
        /// Variants in source order.
        variants: Vec<EnumVariant>,
    },
    /// Inline raw JSON Schema: `type T = schema { … }`, body verbatim
    /// including the enclosing braces.
    SchemaInline {
        /// The raw schema text, byte-for-byte as authored.
        body: String,
        /// Source span covering the braced body.
        body_span: Span,
    },
    /// File import: `type T = schema("path")`.
    SchemaImport {
        /// Imported schema path as written.
        path: String,
        /// Source span of the path literal.
        path_span: Span,
    },
}

/// One field inside a shorthand record type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldDecl {
    /// Span covering the field declaration.
    pub span: Span,
    /// Leading trivia before the field (multi-line bodies only).
    pub lead: Vec<Lead>,
    /// `///` doc lines attached to the field.
    pub docs: Vec<DocLine>,
    /// Same-line trailing comment.
    pub trailing: Option<Comment>,
    /// Field name.
    pub name: String,
    /// Source span of the field name.
    pub name_span: Span,
    /// Declared field type.
    pub ty: TypeRef,
}

/// One bare variant of a payload-less enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumVariant {
    /// Source span of the variant name.
    pub span: Span,
    /// Variant name.
    pub name: String,
}

/// A `worker` block: the task queue and the actions required on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerDecl {
    /// Span covering the `worker` header line.
    pub span: Span,
    /// Leading trivia before the block.
    pub lead: Vec<Lead>,
    /// `///` doc lines attached to the block.
    pub docs: Vec<DocLine>,
    /// Same-line trailing comment on the header.
    pub trailing: Option<Comment>,
    /// Worker (task queue) name.
    pub name: String,
    /// Source span of the worker name.
    pub name_span: Span,
    /// The launch declaration for an agent worker on this queue, when the
    /// document carries one.
    ///
    /// `None` is a first-class, permanent case: a queue served by a worker SDK
    /// build, by `aion worker shell`, or by declared `run` bodies has no agent
    /// harness to launch and nothing to declare. A section that IS present is
    /// the complete launch truth — see [`HarnessDecl`].
    pub harness: Option<HarnessDecl>,
    /// Action requirements declared on this worker (one or more).
    pub actions: Vec<ActionDecl>,
}

/// One `action name(params…) -> Type` requirement inside a worker block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionDecl {
    /// Span covering the action header line.
    pub span: Span,
    /// Leading trivia before the declaration.
    pub lead: Vec<Lead>,
    /// `///` doc lines attached to the action.
    pub docs: Vec<DocLine>,
    /// Same-line trailing comment.
    pub trailing: Option<Comment>,
    /// Action name.
    pub name: String,
    /// Source span of the action name.
    pub name_span: Span,
    /// Typed parameters in source order.
    pub params: Vec<ParamDecl>,
    /// Declared result type.
    pub returns: TypeRef,
    /// Optional per-action config line (`node …, timeout …, retry …`).
    pub config: Option<ConfigLine>,
    /// The declarative implementation, when the document carries one.
    ///
    /// `None` is a first-class, permanent case: the action is a pure
    /// REQUIREMENT on the queue, satisfied by an out-of-band worker build
    /// (the Rust, Python or TypeScript worker SDKs). A body makes the
    /// document itself the implementation.
    pub body: Option<ActionBody>,
}

/// What a `run` body's result is, and therefore what the action's declared
/// return type has to be.
///
/// The three forms are not three vocabularies: they are the one question a
/// command's caller always has to answer — what of the process do I want —
/// written down instead of assumed. [`RunCapture::Outcome`] is the form the
/// language has always had; the other two exist because a worker document is
/// an implementation of a typed action, and an action returning `Person`
/// cannot be served by a body that can only ever hand back an exit code and
/// two streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunCapture {
    /// Bare `run "…"` — the result is the command's outcome record
    /// (`exit_code`, `stdout`, `stderr`), and a non-zero exit is a value, not
    /// a failure. The only form a WORKFLOW document may declare, because the
    /// server's declared-command dispatcher produces exactly this record.
    Outcome,
    /// `run text "…"` — trimmed stdout becomes a JSON string. A non-zero exit
    /// is a retryable failure carrying stderr.
    Text,
    /// `run json "…"` — trimmed stdout is parsed as JSON and becomes the
    /// result. A non-zero exit is a retryable failure carrying stderr;
    /// stdout that is not valid JSON is a terminal one.
    Json,
}

impl RunCapture {
    /// The word that selects this capture after `run`, or `None` for the
    /// bare outcome form which has no word.
    #[must_use]
    pub const fn word(self) -> Option<&'static str> {
        match self {
            Self::Outcome => None,
            Self::Text => Some(TEXT_CAPTURE_WORD),
            Self::Json => Some(JSON_CAPTURE_WORD),
        }
    }
}

/// The word selecting [`RunCapture::Text`] after `run`.
pub const TEXT_CAPTURE_WORD: &str = "text";
/// The word selecting [`RunCapture::Json`] after `run`.
pub const JSON_CAPTURE_WORD: &str = "json";

/// The declarative implementation of an action, written under its
/// declaration.
///
/// An enum because the language will grow further body kinds; today the one
/// kind is a shell command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionBody {
    /// `run [text|json] "<command>"` — one shell command line. The command
    /// may reference the action's declared parameters with `$name` /
    /// `${name}`; `$$` is a literal dollar sign. See
    /// [`super::segment_command`].
    Run {
        /// Span from the `run` keyword through the command literal.
        span: Span,
        /// Leading trivia before the body line.
        lead: Vec<Lead>,
        /// Same-line trailing comment.
        trailing: Option<Comment>,
        /// What the body's result is taken from.
        capture: RunCapture,
        /// Source span of the capture word, `None` for the bare outcome
        /// form. Carried so a refusal can anchor on the word itself.
        capture_span: Option<Span>,
        /// The command text exactly as authored, after string-escape
        /// processing and before `$` interpolation is interpreted.
        command: String,
        /// Source span of the command string literal, quotes included.
        command_span: Span,
    },
}

/// One typed parameter of an action or child declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamDecl {
    /// Span covering the parameter.
    pub span: Span,
    /// Parameter name.
    pub name: String,
    /// Source span of the parameter name.
    pub name_span: Span,
    /// Declared parameter type.
    pub ty: TypeRef,
}

/// An indented config line: `advisory, node <name>, timeout <duration>,
/// retry …`.
///
/// Appears under an action declaration, or under a call statement as a
/// call-site override (the checker rules on which keys may pin where).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigLine {
    /// Span covering the config line.
    pub span: Span,
    /// Leading trivia before the config line.
    pub lead: Vec<Lead>,
    /// Same-line trailing comment.
    pub trailing: Option<Comment>,
    /// The `advisory` class marker, carrying its own source span.
    ///
    /// Declaration-only, like `retry`: an action IS advisory or is not, and
    /// a call site cannot make one call of it side-channel. Present means
    /// the action is a side channel — its failure warns on the run and
    /// never faults the calling step (RUNTIME-OPERATIONS.md R5).
    pub advisory: Option<Span>,
    /// The `agent` marker, carrying its own source span.
    ///
    /// Declaration-only, like `advisory`: an action IS an agent seam or is
    /// not. It says the action's one `String` parameter carries a prompt and
    /// its result is the canonical agent-outcome record — a declared record
    /// of exactly `text: String` and `stop_reason: String` — which is the
    /// whole of what an adapter needs to know to hand the call to a harness
    /// instead of running it as an ordinary activity.
    ///
    /// It exists because that fact was previously carried in a Rust constant
    /// listing action names by hand (`AGENT_ACTIVITY_TYPES`), so the language
    /// could declare an agent call and the worker could not know it had. A
    /// list of names in another language is not a contract; a marker on the
    /// declaration is.
    pub agent: Option<Span>,
    /// Optional `node <name>` routing key.
    pub node: Option<ConfigValue>,
    /// Optional `timeout <duration>` key.
    pub timeout: Option<DurationLiteral>,
    /// Optional retry policy.
    pub retry: Option<RetrySpec>,
}

/// A named config value (the `node` selector).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigValue {
    /// Source span of the value.
    pub span: Span,
    /// The configured name.
    pub name: String,
}

/// Retry policy on an action config line.
///
/// `retry N` declares N FURTHER attempts AFTER the first — N+1 total
/// executions. The lowerings translate `count` into the SDK's total-attempt
/// budget (`emitter::stmts::total_attempts` and its direct-lowering twin);
/// `count` itself is always the number the author wrote, and it is what
/// `RetryContract` commits to package identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetrySpec {
    /// `retry N every D` — constant interval.
    Every {
        /// Span covering the complete retry clause.
        span: Span,
        /// Number of further attempts after the first.
        count: u64,
        /// Constant delay between attempts.
        every: DurationLiteral,
    },
    /// `retry N backoff Dmin..Dmax` — bounded backoff.
    Backoff {
        /// Span covering the complete retry clause.
        span: Span,
        /// Number of further attempts after the first.
        count: u64,
        /// Minimum backoff delay.
        min: DurationLiteral,
        /// Maximum backoff delay.
        max: DurationLiteral,
    },
}

/// A `subflow name(params…)` declaration: a named flow container with the
/// same anatomy as a workflow — typed inputs (the parameters), exactly one
/// success outcome, and its own steps. Used, it is one step; it compiles
/// inline and captures nothing from the enclosing flow (parameters only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubflowDecl {
    /// Span covering the `subflow` header line.
    pub span: Span,
    /// Leading trivia before the declaration.
    pub lead: Vec<Lead>,
    /// `///` doc lines attached to the declaration.
    pub docs: Vec<DocLine>,
    /// Same-line trailing comment on the header.
    pub trailing: Option<Comment>,
    /// Subflow name.
    pub name: String,
    /// Source span of the subflow name.
    pub name_span: Span,
    /// Typed parameters in source order.
    pub params: Vec<ParamDecl>,
    /// The single success outcome the subflow's steps route to.
    pub outcome: SubflowOutcome,
    /// `step` declarations in source order.
    pub steps: Vec<Step>,
}

/// The one `outcome name: type T` declaration of a subflow. Unlike a
/// workflow outcome it carries no `route success|failure` direction: the
/// invocation site binds the payload and the enclosing flow routes on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubflowOutcome {
    /// Span covering the declaration line.
    pub span: Span,
    /// Leading trivia before the declaration.
    pub lead: Vec<Lead>,
    /// Same-line trailing comment.
    pub trailing: Option<Comment>,
    /// Outcome name.
    pub name: String,
    /// Source span of the outcome name.
    pub name_span: Span,
    /// Payload type the outcome carries.
    pub ty: TypeRef,
}

/// A `child name(params…) -> Type` workflow delegation declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildDecl {
    /// Span covering the declaration line.
    pub span: Span,
    /// Leading trivia before the declaration.
    pub lead: Vec<Lead>,
    /// `///` doc lines attached to the declaration.
    pub docs: Vec<DocLine>,
    /// Same-line trailing comment.
    pub trailing: Option<Comment>,
    /// Child workflow name.
    pub name: String,
    /// Source span of the child name.
    pub name_span: Span,
    /// Typed parameters in source order.
    pub params: Vec<ParamDecl>,
    /// Declared result type.
    pub returns: TypeRef,
}
