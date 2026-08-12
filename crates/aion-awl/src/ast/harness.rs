//! The `harness` section of a `worker` block: an agent worker's launch truth.
//!
//! # Why the settings live in the document
//!
//! An agent worker's launch was hand-typed at a shell — a dozen flags, retyped on
//! every box, drifting between a runbook and the command somebody actually ran. The
//! document already declares the queue and the actions; the `harness` section declares
//! the rest, so ONE reviewed, deployed artifact says what gets launched and how.
//!
//! # Why every setting is an `Option` here and required in the checker
//!
//! The parser's job is to record what was written, losslessly, on real spans. Which
//! settings a section MUST carry depends on the `kind` it declares — an ACP document
//! needs a `command`, a Norn document has no use for one — and that is a semantic rule,
//! so it lives in the checker where a refusal can name the field and point at the
//! section that omitted it. A parser that demanded fields would have to guess the kind
//! before reading it.
//!
//! Nothing in this module supplies a value. A setting that is absent is absent: no
//! default is manufactured here, in the checker, in the compiler, or at launch.

use crate::Span;
use crate::ast::{Comment, DocLine, DurationLiteral, Lead};

/// The field word for the harness kind selector.
pub const HARNESS_KIND_FIELD: &str = "kind";
/// The field word for the maximum activities served at once.
pub const HARNESS_CONCURRENCY_FIELD: &str = "concurrency";
/// The field word for the delay before the first reconnect attempt.
pub const HARNESS_RECONNECT_INITIAL_BACKOFF_FIELD: &str = "reconnect_initial_backoff";
/// The field word for the reconnect backoff cap.
pub const HARNESS_RECONNECT_MAX_BACKOFF_FIELD: &str = "reconnect_max_backoff";
/// The field word for the cumulative session-drop budget.
pub const HARNESS_RECONNECT_MAX_ATTEMPTS_FIELD: &str = "reconnect_max_attempts";
/// The field word for the environment pass-through allow-list.
pub const HARNESS_ENV_PASS_FIELD: &str = "env_pass";
/// The field word for the ACP agent executable.
pub const HARNESS_COMMAND_FIELD: &str = "command";
/// The field word for the agent's working directory.
pub const HARNESS_CWD_FIELD: &str = "cwd";
/// The word that opens a `cwd` BINDING: `cwd from <parameter>`.
pub const HARNESS_CWD_FROM_WORD: &str = "from";
/// The field word for the ACP permission policy.
pub const HARNESS_PERMISSION_FIELD: &str = "permission";
/// The field word for the ACP shutdown grace.
pub const HARNESS_EXIT_GRACE_FIELD: &str = "exit_grace";
/// The field word for the extra arguments passed to the launched agent.
pub const HARNESS_ARGS_FIELD: &str = "args";
/// The field word for the Norn binary location.
pub const HARNESS_BINARY_FIELD: &str = "binary";

/// The word that opens the section.
pub const HARNESS_WORD: &str = "harness";

/// Every field word the section admits, in canonical print order.
///
/// Two duties, one table. The parser refuses an unknown field by naming this list, and
/// the printer emits the section in exactly this ORDER — enforced by
/// `printer::harness::tests`, because the printer cannot walk an array of twelve
/// differently-typed values without becoming a pile of closures. A field added to the
/// grammar with no print position fails that pin rather than printing nothing and
/// vanishing from a deployed document on `aion awl fmt`.
pub const HARNESS_FIELDS: [&str; 12] = [
    HARNESS_KIND_FIELD,
    HARNESS_CONCURRENCY_FIELD,
    HARNESS_RECONNECT_INITIAL_BACKOFF_FIELD,
    HARNESS_RECONNECT_MAX_BACKOFF_FIELD,
    HARNESS_RECONNECT_MAX_ATTEMPTS_FIELD,
    HARNESS_ENV_PASS_FIELD,
    HARNESS_COMMAND_FIELD,
    HARNESS_ARGS_FIELD,
    HARNESS_CWD_FIELD,
    HARNESS_PERMISSION_FIELD,
    HARNESS_EXIT_GRACE_FIELD,
    HARNESS_BINARY_FIELD,
];

/// The `acp` harness kind, as `kind` spells it.
pub const HARNESS_KIND_ACP: &str = "acp";
/// The `norn` harness kind, as `kind` spells it.
pub const HARNESS_KIND_NORN: &str = "norn";

/// The `permission` policy that refuses every gated action.
///
/// Stated here, beside the kind words, so the checker that ACCEPTS a spelling and the
/// compiler that ACTS on it read one table. A policy accepted by one and unrecognised by
/// the other would be a security decision lost between two files.
pub const HARNESS_PERMISSION_DENY: &str = "deny";
/// The `permission` policy that approves one request, once.
pub const HARNESS_PERMISSION_ALLOW_ONCE: &str = "allow-once";

/// Where one attempt's working directory comes from.
///
/// Two forms, because the tree an agent stands in is a property of the WORK and only
/// sometimes a property of the worker. A document is written before any work exists, so
/// a document that can only name a literal path can only ever be right for a worker that
/// serves exactly one tree. That form survives here, narrowed to that honest meaning.
///
/// The other form binds the directory to an action PARAMETER, so the run that knows which
/// tree it concerns is the thing that supplies it. Nothing is inferred and nobody is
/// asked: the checker proves the named parameter exists on every action the harness
/// serves, and the worker reads it out of the dispatch.
///
/// What does NOT change is the hazard both forms exist to close: an agent must never
/// inherit whatever directory its worker process happened to be launched in. A bound
/// value that is missing, relative, or not a directory is a terminal failure, exactly as
/// an unlaunchable literal already was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HarnessCwd {
    /// A path written into the document, absolute or `{workspace_root}`-leading.
    Literal(HarnessText),
    /// The name of an action parameter every action this harness serves declares.
    Bound(HarnessWord),
}

impl HarnessCwd {
    /// The span of whichever form was written.
    ///
    /// A refusal anchors on the value the author typed, and the two forms carry their
    /// spans on different types — so the span is asked for HERE rather than matched on at
    /// every site that reports, which is how one form ends up anchoring on the section
    /// while the other anchors on the value.
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            Self::Literal(text) => text.span,
            Self::Bound(word) => word.span,
        }
    }

    /// The literal path, when this is the literal form.
    #[must_use]
    pub fn literal(&self) -> Option<&HarnessText> {
        match self {
            Self::Literal(text) => Some(text),
            Self::Bound(_) => None,
        }
    }

    /// The bound parameter's name, when this is the binding form.
    #[must_use]
    pub fn bound(&self) -> Option<&HarnessWord> {
        match self {
            Self::Bound(word) => Some(word),
            Self::Literal(_) => None,
        }
    }
}

/// The `harness` section declared inside one `worker` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessDecl {
    /// Span covering the `harness` word.
    pub span: Span,
    /// Leading trivia before the section.
    pub lead: Vec<Lead>,
    /// `///` doc lines attached to the section.
    pub docs: Vec<DocLine>,
    /// Same-line trailing comment on the `harness` line.
    pub trailing: Option<Comment>,
    /// Which harness this worker launches (`acp` or `norn`).
    pub kind: Option<HarnessSetting<HarnessWord>>,
    /// Maximum activities served at once.
    pub concurrency: Option<HarnessSetting<HarnessInteger>>,
    /// Delay before the first reconnect attempt.
    pub reconnect_initial_backoff: Option<HarnessSetting<DurationLiteral>>,
    /// Reconnect backoff cap.
    pub reconnect_max_backoff: Option<HarnessSetting<DurationLiteral>>,
    /// Cumulative session-drop budget.
    pub reconnect_max_attempts: Option<HarnessSetting<HarnessInteger>>,
    /// The environment variable NAMES carried into the harness child.
    pub env_pass: Option<HarnessSetting<Vec<HarnessText>>>,
    /// The ACP agent executable, an absolute path.
    pub command: Option<HarnessSetting<HarnessText>>,
    /// Extra arguments handed to the launched agent, in order.
    pub args: Option<HarnessSetting<Vec<HarnessText>>>,
    /// Where each attempt's working directory comes from: a literal, or a run-supplied
    /// action parameter.
    pub cwd: Option<HarnessSetting<HarnessCwd>>,
    /// How the ACP adapter answers `session/request_permission`.
    pub permission: Option<HarnessSetting<HarnessText>>,
    /// How long the ACP agent may take to exit once its stdin closes.
    pub exit_grace: Option<HarnessSetting<DurationLiteral>>,
    /// The `norn` binary to launch, when it is not the adapter's own default.
    pub binary: Option<HarnessSetting<HarnessText>>,
}

impl HarnessDecl {
    /// The declared kind's word, when the section names one.
    #[must_use]
    pub fn kind_word(&self) -> Option<&str> {
        self.kind.as_ref().map(|kind| kind.value.word.as_str())
    }

    /// The action parameter this section takes each attempt's working directory from,
    /// when it takes it from one at all.
    ///
    /// Read straight off the syntax, because the pass that decides an `agent` action's
    /// shape runs before the harness pass and still has to know this: an agent action
    /// carrying the directory declares one parameter more than one that does not. Asking
    /// the declaration itself keeps that a single fact rather than a table built early and
    /// a match written late, which is how the two come to disagree.
    #[must_use]
    pub fn bound_cwd(&self) -> Option<&str> {
        self.cwd
            .as_ref()
            .and_then(|setting| setting.value.bound())
            .map(|word| word.word.as_str())
    }
}

/// One setting line inside the section, with the trivia that surrounds it.
///
/// Each line carries its own trivia because each line IS one: a comment written above
/// `command` belongs to `command`, and a printer that pooled them at the section would
/// re-emit them somewhere nobody put them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessSetting<V> {
    /// Span covering the field word.
    pub span: Span,
    /// Leading trivia before the setting line.
    pub lead: Vec<Lead>,
    /// `///` doc lines attached to the setting.
    pub docs: Vec<DocLine>,
    /// Same-line trailing comment.
    pub trailing: Option<Comment>,
    /// The declared value.
    pub value: V,
}

/// A bare word value (`kind acp`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessWord {
    /// Span covering the word.
    pub span: Span,
    /// The word, verbatim.
    pub word: String,
}

/// A whole-number value (`concurrency 4`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessInteger {
    /// Span covering the literal.
    pub span: Span,
    /// The declared value.
    pub value: u64,
}

/// A quoted string value (`command "/usr/local/bin/claude-code-acp"`).
///
/// Paths, arguments and variable names are quoted rather than written bare because
/// each of them legitimately carries characters no identifier may — a leading `-` on an
/// agent argument, a `/` in a path, a `.` in a variable name — and a grammar that
/// admitted them bare would have to guess where the value ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessText {
    /// Span covering the literal.
    pub span: Span,
    /// The text after escape processing.
    pub value: String,
}
