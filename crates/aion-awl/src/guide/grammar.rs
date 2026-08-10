//! The grammar table: which words may stand where, and the form axis
//! derived from it.
//!
//! This is the language server's completion table, PROMOTED. It used to live
//! as a private `enum Context` inside `aion-awl-lsp`'s completion machinery,
//! which made it invisible to every other surface that wanted to say what
//! form a word is — the ops console's language panel above all. It lives
//! here now as an owned, testable surface: the LSP's completion delegates to
//! [`words_offered_at`] (the same delegation shape as the byte↔UTF-16 move
//! into `positions.rs`), and the guide derives each word's [`GrammarForm`]
//! from the same table through [`form_of`]. One table, two readers — the
//! panel and the completion list are structurally unable to disagree about
//! where a word may stand.
//!
//! The per-position word lists are curated on purpose: offering all 57
//! reserved words everywhere is a dictionary, not a grammar. Curation is
//! also exactly how a word goes missing, so `grammar_tests.rs` pins that
//! every word of the vocabulary is offered in at least one position and that
//! every derived form is honest against the table it came from.

use crate::lexer::{ANSWER_WORD, IN_WORD, Keyword, QUERY_WORD, RUN_WORD, SEND_WORD, TO_WORD};

/// A position in the grammar where the cursor — or a word — may stand.
///
/// These are the positions completion decides between from the text before
/// the cursor; the doc comment on each names the concrete syntax that puts
/// an author there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrammarPosition {
    /// Column zero: the document's own declarations.
    TopLevel,
    /// Indented under `workflow`: the header's clauses.
    WorkflowHeader,
    /// Indented under `worker`: its actions.
    WorkerBlock,
    /// Indented under `step` or `subflow`: a statement may start here.
    StepBody,
    /// Indented under `type`: a field declaration.
    TypeBody,
    /// After `route `: where control goes.
    RouteTarget,
    /// After `after `: an ordering dependency on another step.
    StepReference,
    /// After `wait ` or `send `: a signal.
    SignalReference,
    /// After `|>`: a pipe stage.
    PipeStage,
    /// After `:`: a type.
    TypeReference,
    /// After `when `: a condition.
    Condition,
    /// After `=` on a `const` line: the constant's value expression.
    ConstValue,
    /// After `=` on a `type` line: the declaration's door (`schema { … }`,
    /// `schema("…")`, or an enum body).
    TypeDoor,
    /// After `schema` in a value expression: what the schema derives from.
    SchemaSource,
    /// After `run` in a declared action body: the capture selector.
    RunCapture,
}

impl GrammarPosition {
    /// Every position, in the order the table reads them.
    pub const ALL: [Self; 15] = [
        Self::TopLevel,
        Self::WorkflowHeader,
        Self::WorkerBlock,
        Self::StepBody,
        Self::TypeBody,
        Self::RouteTarget,
        Self::StepReference,
        Self::SignalReference,
        Self::PipeStage,
        Self::TypeReference,
        Self::Condition,
        Self::ConstValue,
        Self::TypeDoor,
        Self::SchemaSource,
        Self::RunCapture,
    ];

    /// The position's stable identifier, as the guide's JSON carries it.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::TopLevel => "top-level",
            Self::WorkflowHeader => "workflow-header",
            Self::WorkerBlock => "worker-block",
            Self::StepBody => "step-body",
            Self::TypeBody => "type-body",
            Self::RouteTarget => "route-target",
            Self::StepReference => "step-reference",
            Self::SignalReference => "signal-reference",
            Self::PipeStage => "pipe-stage",
            Self::TypeReference => "type-reference",
            Self::Condition => "condition",
            Self::ConstValue => "const-value",
            Self::TypeDoor => "type-door",
            Self::SchemaSource => "schema-source",
            Self::RunCapture => "run-capture",
        }
    }

    /// The position said as a reader would: where the word stands.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::TopLevel => "at the start of a top-level line",
            Self::WorkflowHeader => "in a workflow header",
            Self::WorkerBlock => "in a worker block",
            Self::StepBody => "in a step or subflow body",
            Self::TypeBody => "in a type body",
            Self::RouteTarget => "after route, as its target",
            Self::StepReference => "after after, naming a step",
            Self::SignalReference => "after wait or send, naming a signal",
            Self::PipeStage => "after |>, as a pipe stage",
            Self::TypeReference => "after a colon, naming a type",
            Self::Condition => "in a when condition",
            Self::ConstValue => "after = in a const declaration",
            Self::TypeDoor => "after = in a type declaration",
            Self::SchemaSource => "after schema in a value expression",
            Self::RunCapture => "after run in a declared action body",
        }
    }
}

/// The reserved words that may stand in `position`.
///
/// Each list keeps the order completion offers the words in, so the LSP's
/// delegation reproduces its old output byte for byte.
#[must_use]
pub const fn keywords_offered_at(position: GrammarPosition) -> &'static [Keyword] {
    use Keyword as K;
    match position {
        // Exactly the words the parser dispatches on at the top level — the
        // document openers plus the declaration keywords. Pinned to the
        // parser itself by `tests/grammar_parser_gate.rs`.
        GrammarPosition::TopLevel => &[
            K::Workflow,
            K::Type,
            K::Worker,
            K::Child,
            K::Const,
            K::Subflow,
            K::Step,
        ],
        // The header-line openers the parser accepts, plus the words that
        // stand mid-line in a header declaration (`type`, `route`,
        // `success`, `failure` in an `outcome` line) — each mid-line word
        // is witnessed through the real checker by the same gate.
        GrammarPosition::WorkflowHeader => &[
            K::Input,
            K::Signal,
            K::Outcome,
            K::Timeout,
            K::Type,
            K::Route,
            K::Success,
            K::Failure,
        ],
        GrammarPosition::WorkerBlock => &[
            K::Action,
            K::Node,
            K::Timeout,
            K::Retry,
            K::Every,
            K::Backoff,
            K::Advisory,
            K::Agent,
        ],
        GrammarPosition::StepBody => &[
            K::Spawn,
            K::Wait,
            K::Sleep,
            K::Fork,
            K::Join,
            K::Loop,
            K::Counting,
            K::Until,
            K::Max,
            K::Visits,
            K::Sequential,
            K::Distribute,
            K::Sequence,
            K::Collect,
            K::Outcome,
            K::Route,
            K::After,
            K::On,
            K::Failure,
            K::Timeout,
            // An `outcome` line inside a step body carries its guard, so the
            // condition words start here too.
            K::When,
        ],
        // A type body's two slots — field name and field type — admit no
        // keyword at all (`parser/types.rs`), and a type reference after a
        // colon admits only a type identifier, a list, or `?`. Both offer
        // NOTHING: the words once filed here stand in the value positions
        // below, which is where the parser actually accepts them.
        GrammarPosition::TypeBody | GrammarPosition::TypeReference => &[],
        GrammarPosition::RouteTarget | GrammarPosition::StepReference => &[K::Success, K::Failure],
        GrammarPosition::SignalReference => &[K::Timeout],
        // The value stands, each witnessed through the real checker by
        // `tests/grammar_parser_gate.rs`: `const c = schema of T` and
        // `const c = json { … }` (const-value), `type T = schema { … }`
        // (type-door), `schema of T` (schema-source), `run json "…"`
        // (run-capture).
        GrammarPosition::ConstValue => &[K::Schema, K::Json],
        GrammarPosition::TypeDoor => &[K::Schema],
        GrammarPosition::SchemaSource => &[K::Of],
        GrammarPosition::RunCapture => &[K::Json],
        GrammarPosition::PipeStage => &[
            K::Filter,
            K::Sort,
            K::Map,
            K::Count,
            K::Any,
            K::All,
            K::Route,
        ],
        GrammarPosition::Condition => &[
            K::Is,
            K::Not,
            K::And,
            K::Or,
            K::Empty,
            K::Present,
            K::Absent,
            K::True,
            K::False,
            K::Otherwise,
            K::Any,
            K::All,
            K::Count,
        ],
    }
}

/// The non-reserved opener words that may stand in `position`.
///
/// The positional words are not in [`Keyword`] — deliberately, since
/// reserving them would break real documents — but an author still has to
/// type them, and a grammar table that omitted them would describe a
/// language smaller than the one the parser accepts.
#[must_use]
pub const fn positional_offered_at(position: GrammarPosition) -> &'static [&'static str] {
    // Every position spelled out — no catch-all — so a position added to the
    // language fails to compile here until someone decides its positional
    // words, instead of silently getting none.
    match position {
        // `send name: Type` and `query name: Type` are header declarations
        // (`parser/send.rs`, `parser/query.rs`).
        GrammarPosition::WorkflowHeader => &[QUERY_WORD, SEND_WORD],
        GrammarPosition::WorkerBlock => &[RUN_WORD],
        // `in` belongs here because `distribute <var> in <collection>` is a
        // step-body line; it is the only place an author types the word.
        GrammarPosition::StepBody => &[SEND_WORD, ANSWER_WORD, IN_WORD],
        GrammarPosition::SignalReference => &[TO_WORD],
        GrammarPosition::TopLevel
        | GrammarPosition::TypeBody
        | GrammarPosition::RouteTarget
        | GrammarPosition::StepReference
        | GrammarPosition::PipeStage
        | GrammarPosition::TypeReference
        | GrammarPosition::Condition
        | GrammarPosition::ConstValue
        | GrammarPosition::TypeDoor
        | GrammarPosition::SchemaSource
        | GrammarPosition::RunCapture => &[],
    }
}

/// Every word — reserved then positional, each in its curated order — that
/// may stand in `position`.
///
/// This is the exact sequence the language server's completion offers, and
/// the LSP's `keywords_for` delegates here verbatim.
#[must_use]
pub fn words_offered_at(position: GrammarPosition) -> Vec<&'static str> {
    let mut offered: Vec<&'static str> = keywords_offered_at(position)
        .iter()
        .copied()
        .map(Keyword::as_word)
        .collect();
    offered.extend(positional_offered_at(position));
    offered
}

/// Every position `word` may stand in, in the table's own order.
///
/// Empty when `word` is not a word of the language — or when a word of the
/// language is offered nowhere, which `grammar_tests.rs` pins can never be
/// true of the shipped table.
#[must_use]
pub fn positions_of(word: &str) -> Vec<GrammarPosition> {
    GrammarPosition::ALL
        .into_iter()
        .filter(|position| {
            keywords_offered_at(*position)
                .iter()
                .any(|keyword| keyword.as_word() == word)
                || positional_offered_at(*position).contains(&word)
        })
        .collect()
}

/// What form a word is: the grammar axis the language panel groups by.
///
/// Derived from [`positions_of`] — never hand-threaded onto an entry — so
/// the form a surface prints is a fact about the same table completion
/// offers words from. The derivation takes the FIRST claim in the order the
/// variants are documented below; a word standing in several positions is
/// filed under the outermost thing it can open, and its full position list
/// stays visible through [`positions_of`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrammarForm {
    /// May stand at the start of a top-level line, opening a declaration.
    Declaration,
    /// May stand in a step or subflow body (and nowhere more outer),
    /// opening or continuing a statement.
    Statement,
    /// Stands inside a value expression: opening a declaration's value
    /// after `=`, following `schema`, or naming `run`'s capture form.
    ValueWord,
    /// Stands in a workflow header or a worker block, qualifying the
    /// declaration above it.
    HeaderClause,
    /// Stands after `|>` as a stage of a value pipeline.
    PipelineStage,
    /// Stands inside a `when` condition.
    ConditionWord,
    /// Stands only where a statement names its target — a route's
    /// destination, an ordering dependency, or a signal's addressee.
    TargetWord,
}

impl GrammarForm {
    /// Every form, in the order a reader meets them in a document: the
    /// declarations, the clauses under them, the statements, then the
    /// in-expression vocabularies.
    pub const ALL: [Self; 7] = [
        Self::Declaration,
        Self::HeaderClause,
        Self::Statement,
        Self::PipelineStage,
        Self::ConditionWord,
        Self::ValueWord,
        Self::TargetWord,
    ];

    /// The form's stable identifier, as the guide's JSON carries it.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Declaration => "declaration",
            Self::Statement => "statement",
            Self::ValueWord => "value-word",
            Self::HeaderClause => "header-clause",
            Self::PipelineStage => "pipeline-stage",
            Self::ConditionWord => "condition-word",
            Self::TargetWord => "target-word",
        }
    }

    /// The form's name — singular, so the same string reads truly beside
    /// one word and, with its description, over a group of them.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Declaration => "Declaration",
            Self::Statement => "Step statement",
            Self::ValueWord => "Value word",
            Self::HeaderClause => "Header clause",
            Self::PipelineStage => "Pipeline stage",
            Self::ConditionWord => "Condition word",
            Self::TargetWord => "Target word",
        }
    }

    /// One sentence saying what standing in this form means.
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Declaration => {
                "Words that may open a declaration at the start of a top-level line."
            }
            Self::Statement => {
                "Words that open or continue a statement inside a step or subflow body."
            }
            Self::ValueWord => {
                "Words that stand inside a value expression: opening a declaration's value after =, following schema, or naming run's capture form."
            }
            Self::HeaderClause => {
                "Words that stand in a workflow header or a worker block, qualifying the declaration above them."
            }
            Self::PipelineStage => "Words that stand after |> as a stage of a value pipeline.",
            Self::ConditionWord => "Words that stand inside a when condition.",
            Self::TargetWord => {
                "Words that stand only where a statement names its target: a route's destination, an ordering dependency, or a signal's addressee."
            }
        }
    }
}

/// The form `word` is, derived from the positions the table offers it in.
///
/// `None` when the table offers the word nowhere — which for a word of the
/// language would be a completion defect, and is pinned impossible by
/// `every_word_of_the_vocabulary_stands_somewhere`.
#[must_use]
pub fn form_of(word: &str) -> Option<GrammarForm> {
    let positions = positions_of(word);
    let stands_in = |position: GrammarPosition| positions.contains(&position);
    if positions.is_empty() {
        return None;
    }
    if stands_in(GrammarPosition::TopLevel) {
        return Some(GrammarForm::Declaration);
    }
    if stands_in(GrammarPosition::StepBody) {
        return Some(GrammarForm::Statement);
    }
    if stands_in(GrammarPosition::ConstValue)
        || stands_in(GrammarPosition::TypeDoor)
        || stands_in(GrammarPosition::SchemaSource)
        || stands_in(GrammarPosition::RunCapture)
    {
        return Some(GrammarForm::ValueWord);
    }
    if stands_in(GrammarPosition::WorkflowHeader) || stands_in(GrammarPosition::WorkerBlock) {
        return Some(GrammarForm::HeaderClause);
    }
    if stands_in(GrammarPosition::PipeStage) {
        return Some(GrammarForm::PipelineStage);
    }
    if stands_in(GrammarPosition::Condition) {
        return Some(GrammarForm::ConditionWord);
    }
    // All that remains are the target positions: route-target,
    // step-reference, and signal-reference.
    Some(GrammarForm::TargetWord)
}

#[cfg(test)]
#[path = "grammar_tests.rs"]
mod grammar_tests;
