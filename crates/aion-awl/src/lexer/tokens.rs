/// A byte and display-position range in the source document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Span {
    /// Zero-based byte offset where the span starts.
    pub start: usize,
    /// Zero-based byte offset just after the span ends.
    pub end: usize,
    /// One-based source line where the span starts.
    pub line: usize,
    /// One-based source column where the span starts, counted in characters
    /// (not bytes), so diagnostics stay editor-correct after multibyte
    /// content earlier on the same line.
    pub column: usize,
}

impl Span {
    pub(super) const fn new(start: usize, end: usize, line: usize, column: usize) -> Self {
        Self {
            start,
            end,
            line,
            column,
        }
    }
}

/// One lexical token with its source span.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    /// The token's lexical kind and value, when the token carries one.
    pub kind: TokenKind,
    /// The token's source span.
    pub span: Span,
}

impl Token {
    pub(super) const fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// The word that opens a signal `send`, positionally.
pub const SEND_WORD: &str = "send";
/// The word that names a `send`'s target workflow, positionally.
pub const TO_WORD: &str = "to";
/// The word that opens a `query` declaration, positionally.
pub const QUERY_WORD: &str = "query";
/// The word that opens an `answer` statement, positionally.
pub const ANSWER_WORD: &str = "answer";
/// The word that opens a declarative action body, positionally.
pub const RUN_WORD: &str = "run";
/// The word that separates a per-item region's binding from its collection,
/// positionally.
pub const IN_WORD: &str = "in";

/// The word that opens a `command` declaration, positionally.
pub const COMMAND_WORD: &str = "command";
/// The word that names a command's invocation, positionally.
pub const PROGRAM_WORD: &str = "program";
/// The word declaring that an invocation admits the `--` end-of-options
/// marker, positionally.
pub const ADMITS_WORD: &str = "admits";
/// The word declaring that an invocation refuses the `--` end-of-options
/// marker, positionally.
pub const REFUSES_WORD: &str = "refuses";
/// The word carrying a refusing invocation's mandatory leading-dash refusal
/// specimen, positionally.
pub const SPECIMEN_WORD: &str = "specimen";
/// The word declaring a command's named option, positionally.
pub const OPTION_WORD: &str = "option";
/// The word declaring a command's positional operand, positionally.
pub const OPERAND_WORD: &str = "operand";
/// The word declaring a command's environment binding, positionally.
pub const ENV_WORD: &str = "env";
/// The word declaring a command's working directory, positionally.
pub const CWD_WORD: &str = "cwd";
/// The word opening a command's PATH/ulimit hardening block, positionally.
pub const HARDENING_WORD: &str = "hardening";
/// The word naming the PATH inside a hardening block, positionally.
pub const PATH_WORD: &str = "path";
/// The word naming the open-file ceiling inside a hardening block,
/// positionally.
pub const OPEN_FILES_WORD: &str = "open_files";
/// The word naming who owns a declared timeout, positionally.
pub const OWNER_WORD: &str = "owner";

/// The word that opens a `recipe` declaration, positionally.
pub const RECIPE_WORD: &str = "recipe";
/// The word declaring one of a recipe's typed parameters, positionally.
pub const PARAMETER_WORD: &str = "parameter";
/// The word naming a re-derive parameter's source address, positionally.
pub const DERIVES_WORD: &str = "derives";
/// The word naming what a derivation takes out of its source, positionally.
pub const EXTRACT_WORD: &str = "extract";
/// The word that opens a `wall` declaration, positionally.
pub const WALL_WORD: &str = "wall";
/// The word naming one path pattern a wall admits, positionally.
pub const ALLOWS_WORD: &str = "allows";
/// The word naming bytes that must not appear — a wall's negative
/// vocabulary, an artifact's unfilled-slot sentinel — positionally.
pub const EXCLUDES_WORD: &str = "excludes";
/// The word that opens an `artifact` declaration, positionally.
pub const ARTIFACT_WORD: &str = "artifact";
/// The word naming a declared artifact's content digest, positionally.
pub const DIGEST_WORD: &str = "digest";
/// The word naming the path a restore artifact must restore byte-exactly,
/// positionally.
pub const RESTORES_WORD: &str = "restores";
/// The word naming the declared command a recipe step runs, positionally.
pub const RUNS_WORD: &str = "runs";
/// The word naming the typed result a recipe step captures, positionally.
pub const CAPTURES_WORD: &str = "captures";
/// The word naming the declared artifact a recipe step fires, positionally.
pub const FIRES_WORD: &str = "fires";
/// The word naming a fired specimen's known answer, positionally.
pub const EXPECTING_WORD: &str = "expecting";
/// The word naming one at-fire precondition of a recipe step, positionally.
pub const REQUIRES_WORD: &str = "requires";

/// Every word AWL reads as an opener WITHOUT reserving it.
///
/// These are not in [`Keyword`] on purpose, and that is not an oversight: the
/// corpus already carries `action send(to: String, …)`, `step run`, `wait
/// decision -> answer` and `action parse_output(…, query: String)`, so
/// reserving any of them would break documents that exist today. Each is
/// instead recognised in the one grammar position where nothing else may
/// stand.
///
/// What they lacked was a shared NAME. Every recognition site compared against
/// a bare literal, so a consumer outside the parser — the language server's
/// completion provider is the first — had to re-type the vocabulary and hope
/// it still matched. Naming them here makes the parser and its consumers read
/// the same words: a change to one is a change to both.
///
/// The command-declaration surface added thirteen of them at once, and every
/// one is positional for the SAME measured reason rather than by preference:
/// `command` stands in 54 `.awl` documents of this tree today as a
/// harness-section field name, and `cwd`, `path`, `program`, `env`, `option`,
/// `operand` and `refuses` stand in others. Reserving any of them would break
/// documents that exist.
///
/// The recipe surface added fifteen more on the same measured ground, counted
/// over the discovered `**/*.awl` corpus of this tree: `runs` stands in 36
/// documents, `parameter` in 20, `requires` in 12, `artifact` in 8, `derives`
/// in 6, `wall`, `digest` and `fires` in 3 each, `recipe` in 2, and `allows`,
/// `captures` and `extract` in 1 each. Only `excludes`, `restores` and
/// `expecting` stand nowhere, and reserving three words while recognising
/// twelve positionally would be two grammars for one vocabulary.
pub const POSITIONAL_WORDS: [&str; 34] = [
    SEND_WORD,
    TO_WORD,
    QUERY_WORD,
    ANSWER_WORD,
    RUN_WORD,
    IN_WORD,
    COMMAND_WORD,
    PROGRAM_WORD,
    ADMITS_WORD,
    REFUSES_WORD,
    SPECIMEN_WORD,
    OPTION_WORD,
    OPERAND_WORD,
    ENV_WORD,
    CWD_WORD,
    HARDENING_WORD,
    PATH_WORD,
    OPEN_FILES_WORD,
    OWNER_WORD,
    RECIPE_WORD,
    PARAMETER_WORD,
    DERIVES_WORD,
    EXTRACT_WORD,
    WALL_WORD,
    ALLOWS_WORD,
    EXCLUDES_WORD,
    ARTIFACT_WORD,
    DIGEST_WORD,
    RESTORES_WORD,
    RUNS_WORD,
    CAPTURES_WORD,
    FIRES_WORD,
    EXPECTING_WORD,
    REQUIRES_WORD,
];

/// AWL keyword tokens.
///
/// This is the complete reserved inventory from the AWL-2 spec plus the
/// flow-vocabulary rev-3 words (`subflow`, `distribute`, `sequence`,
/// `collect`, `visits`). The declarative action-body word `run` is
/// deliberately ABSENT: it is recognised positionally, as a plain
/// identifier inside an action's block, so that `step run`, `input run` and
/// every other existing use of the word keeps working unchanged. The signal
/// sender's `send` and `to` are absent for the same reason and are read the
/// same way (`parser/send.rs`): the corpus carries `action send(to: String,
/// …)` and `step send` today, and reserving either word would break them.
/// The read-only surface's `query` and `answer` are absent for the same
/// reason and read the same way (`parser/query.rs`): the corpus carries
/// `action parse_output(…, query: String)` and `wait decision -> answer`
/// today.
/// Words that
/// were keywords in AWL-0/1 but are gone from rev-2 (`about`, `do`, `as`,
/// `each`, `repeat`, `finish`, `match`, `case`, `parallel`, `race`,
/// `output`, `error`, `up`, `to`, `in`, `order`, `queue`, `fail`) lex as
/// plain identifiers; the parser rejects them with targeted migration
/// diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keyword {
    /// `workflow`.
    Workflow,
    /// `input`.
    Input,
    /// `signal`.
    Signal,
    /// `outcome`.
    Outcome,
    /// `type`.
    Type,
    /// `schema`.
    Schema,
    /// `worker`.
    Worker,
    /// `action`.
    Action,
    /// `child`.
    Child,
    /// `step`.
    Step,
    /// `after`.
    After,
    /// `fork`.
    Fork,
    /// `join`.
    Join,
    /// `loop`.
    Loop,
    /// `counting`.
    Counting,
    /// `until`.
    Until,
    /// `max`.
    Max,
    /// `sequential`.
    Sequential,
    /// `spawn`.
    Spawn,
    /// `wait`.
    Wait,
    /// `sleep`.
    Sleep,
    /// `timeout`.
    Timeout,
    /// `retry`.
    Retry,
    /// `fallback`.
    Fallback,
    /// `advisory`.
    Advisory,
    /// `agent` — the action's single String input is an agent prompt and its
    /// result is the canonical agent-outcome record (`text: String`,
    /// `stop_reason: String`).
    Agent,
    /// `every`.
    Every,
    /// `backoff`.
    Backoff,
    /// `node`.
    Node,
    /// `on`.
    On,
    /// `failure`.
    Failure,
    /// `when`.
    When,
    /// `otherwise`.
    Otherwise,
    /// `route`.
    Route,
    /// `success`.
    Success,
    /// `filter`.
    Filter,
    /// `map`.
    Map,
    /// `sort`.
    Sort,
    /// `count`.
    Count,
    /// `any`.
    Any,
    /// `all`.
    All,
    /// `is`.
    Is,
    /// `empty`.
    Empty,
    /// `present`.
    Present,
    /// `absent`.
    Absent,
    /// `not`.
    Not,
    /// `and`.
    And,
    /// `or`.
    Or,
    /// `true`.
    True,
    /// `false`.
    False,
    /// `const`.
    Const,
    /// `json`.
    Json,
    /// `of`.
    Of,
    /// `subflow`.
    Subflow,
    /// `distribute`.
    Distribute,
    /// `sequence`.
    Sequence,
    /// `collect`.
    Collect,
    /// `visits`.
    Visits,
}

impl Keyword {
    /// Every reserved word in the language, in declaration order.
    ///
    /// Rust cannot enumerate an enum's variants, so a consumer that needs the
    /// whole vocabulary — the language server's completion provider is the
    /// first — would otherwise hand-copy it and fall silently behind the day a
    /// keyword is added. `keyword_inventory_covers_every_variant` pins this
    /// list against a count derived from [`Keyword::as_word`]'s own source, so
    /// a new variant fails a test here rather than going quietly missing
    /// downstream.
    pub const ALL: [Self; 58] = [
        Self::Workflow,
        Self::Input,
        Self::Signal,
        Self::Outcome,
        Self::Type,
        Self::Schema,
        Self::Worker,
        Self::Action,
        Self::Child,
        Self::Step,
        Self::After,
        Self::Fork,
        Self::Join,
        Self::Loop,
        Self::Counting,
        Self::Until,
        Self::Max,
        Self::Sequential,
        Self::Spawn,
        Self::Wait,
        Self::Sleep,
        Self::Timeout,
        Self::Retry,
        Self::Fallback,
        Self::Advisory,
        Self::Agent,
        Self::Every,
        Self::Backoff,
        Self::Node,
        Self::On,
        Self::Failure,
        Self::When,
        Self::Otherwise,
        Self::Route,
        Self::Success,
        Self::Filter,
        Self::Map,
        Self::Sort,
        Self::Count,
        Self::Any,
        Self::All,
        Self::Is,
        Self::Empty,
        Self::Present,
        Self::Absent,
        Self::Not,
        Self::And,
        Self::Or,
        Self::True,
        Self::False,
        Self::Const,
        Self::Json,
        Self::Of,
        Self::Subflow,
        Self::Distribute,
        Self::Sequence,
        Self::Collect,
        Self::Visits,
    ];

    /// The reserved word this keyword lexes from.
    #[must_use]
    pub const fn as_word(self) -> &'static str {
        match self {
            Self::Workflow => "workflow",
            Self::Input => "input",
            Self::Signal => "signal",
            Self::Outcome => "outcome",
            Self::Type => "type",
            Self::Schema => "schema",
            Self::Worker => "worker",
            Self::Action => "action",
            Self::Child => "child",
            Self::Step => "step",
            Self::After => "after",
            Self::Fork => "fork",
            Self::Join => "join",
            Self::Loop => "loop",
            Self::Counting => "counting",
            Self::Until => "until",
            Self::Max => "max",
            Self::Sequential => "sequential",
            Self::Spawn => "spawn",
            Self::Wait => "wait",
            Self::Sleep => "sleep",
            Self::Timeout => "timeout",
            Self::Retry => "retry",
            Self::Fallback => "fallback",
            Self::Advisory => "advisory",
            Self::Agent => "agent",
            Self::Every => "every",
            Self::Backoff => "backoff",
            Self::Node => "node",
            Self::On => "on",
            Self::Failure => "failure",
            Self::When => "when",
            Self::Otherwise => "otherwise",
            Self::Route => "route",
            Self::Success => "success",
            Self::Filter => "filter",
            Self::Map => "map",
            Self::Sort => "sort",
            Self::Count => "count",
            Self::Any => "any",
            Self::All => "all",
            Self::Is => "is",
            Self::Empty => "empty",
            Self::Present => "present",
            Self::Absent => "absent",
            Self::Not => "not",
            Self::And => "and",
            Self::Or => "or",
            Self::True => "true",
            Self::False => "false",
            Self::Const => "const",
            Self::Json => "json",
            Self::Of => "of",
            Self::Subflow => "subflow",
            Self::Distribute => "distribute",
            Self::Sequence => "sequence",
            Self::Collect => "collect",
            Self::Visits => "visits",
        }
    }

    /// Look up the keyword for a lexed `snake_case` word, if it is reserved.
    #[must_use]
    pub fn from_word(text: &str) -> Option<Self> {
        match text {
            "workflow" => Some(Self::Workflow),
            "input" => Some(Self::Input),
            "signal" => Some(Self::Signal),
            "outcome" => Some(Self::Outcome),
            "type" => Some(Self::Type),
            "schema" => Some(Self::Schema),
            "worker" => Some(Self::Worker),
            "action" => Some(Self::Action),
            "child" => Some(Self::Child),
            "step" => Some(Self::Step),
            "after" => Some(Self::After),
            "fork" => Some(Self::Fork),
            "join" => Some(Self::Join),
            "loop" => Some(Self::Loop),
            "counting" => Some(Self::Counting),
            "until" => Some(Self::Until),
            "max" => Some(Self::Max),
            "sequential" => Some(Self::Sequential),
            "spawn" => Some(Self::Spawn),
            "wait" => Some(Self::Wait),
            "sleep" => Some(Self::Sleep),
            "timeout" => Some(Self::Timeout),
            "retry" => Some(Self::Retry),
            "fallback" => Some(Self::Fallback),
            "advisory" => Some(Self::Advisory),
            "agent" => Some(Self::Agent),
            "every" => Some(Self::Every),
            "backoff" => Some(Self::Backoff),
            "node" => Some(Self::Node),
            "on" => Some(Self::On),
            "failure" => Some(Self::Failure),
            "when" => Some(Self::When),
            "otherwise" => Some(Self::Otherwise),
            "route" => Some(Self::Route),
            "success" => Some(Self::Success),
            "filter" => Some(Self::Filter),
            "map" => Some(Self::Map),
            "sort" => Some(Self::Sort),
            "count" => Some(Self::Count),
            "any" => Some(Self::Any),
            "all" => Some(Self::All),
            "is" => Some(Self::Is),
            "empty" => Some(Self::Empty),
            "present" => Some(Self::Present),
            "absent" => Some(Self::Absent),
            "not" => Some(Self::Not),
            "and" => Some(Self::And),
            "or" => Some(Self::Or),
            "true" => Some(Self::True),
            "false" => Some(Self::False),
            "const" => Some(Self::Const),
            "json" => Some(Self::Json),
            "of" => Some(Self::Of),
            "subflow" => Some(Self::Subflow),
            "distribute" => Some(Self::Distribute),
            "sequence" => Some(Self::Sequence),
            "collect" => Some(Self::Collect),
            "visits" => Some(Self::Visits),
            _ => None,
        }
    }
}

/// A duration unit suffix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurationUnit {
    /// Seconds, `s`.
    Seconds,
    /// Minutes, `m`.
    Minutes,
    /// Hours, `h`.
    Hours,
    /// Days, `d`.
    Days,
}

/// AWL rev-2 token kinds.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    /// A reserved keyword.
    Keyword(Keyword),
    /// A `snake_case` identifier.
    Identifier(String),
    /// A `TitleCase` type or constructor identifier.
    TypeIdentifier(String),
    /// A `.field` accessor: a dot immediately followed by a `snake_case`
    /// field name (`workspace.branch`, `filter(.blocking)`). The payload is
    /// the field name without the dot.
    FieldAccessor(String),
    /// A string literal after escape processing.
    String(String),
    /// A triple-quoted `"""…"""` raw string literal: the payload is the text
    /// between the delimiters, byte-for-byte — newlines literal, zero escape
    /// processing — so printing re-emits the literal verbatim. The body ends
    /// at the first `"""` after the opener, so a raw string cannot contain
    /// three consecutive double quotes.
    RawString(String),
    /// An integer literal.
    Integer(u64),
    /// A floating-point literal, holding the exact source lexeme (e.g.
    /// `"1.0"`, `"0.5"`) so printing can round-trip it byte-for-byte instead
    /// of reformatting through an `f64`.
    Float(String),
    /// An integer duration literal with a unit suffix.
    Duration {
        /// The integer value before the unit suffix.
        magnitude: u64,
        /// The parsed duration unit suffix.
        unit: DurationUnit,
    },
    /// `(`.
    LeftParen,
    /// `)`.
    RightParen,
    /// `{`.
    LeftBrace,
    /// `}`.
    RightBrace,
    /// `[`.
    LeftBracket,
    /// `]`.
    RightBracket,
    /// `:`.
    Colon,
    /// `,`.
    Comma,
    /// `->`.
    Arrow,
    /// `|>`.
    Pipe,
    /// `|` (enum variant separator).
    Bar,
    /// `?` (postfix type optionality).
    Question,
    /// `=` (loop seed and `type X = …` binder).
    Equal,
    /// `..` (backoff duration range).
    DotDot,
    /// `+` (string concatenation).
    Plus,
    /// `-` (accepted before a workflow timeout for checker diagnostics).
    Minus,
    /// `==`.
    EqualEqual,
    /// `!=`.
    BangEqual,
    /// `<`.
    Less,
    /// `<=`.
    LessEqual,
    /// `>`.
    Greater,
    /// `>=`.
    GreaterEqual,
    /// A significant line break after a non-blank source line.
    Newline,
    /// A `//!` workflow-narration doc line: DATA, not trivia. The payload is
    /// the text after `//!`, verbatim (leading space preserved), so the
    /// printer can round-trip the line byte-for-byte.
    DocHeader(String),
    /// A `///` declaration doc line: DATA, not trivia. The payload is the
    /// text after `///`, verbatim (leading space preserved).
    DocLine(String),
    /// A `//` source comment without the marker or leading single space.
    Comment(String),
    /// The verbatim body of an inline `schema { … }` type door, including the
    /// enclosing braces. The lexer captures the brace-balanced region raw
    /// (string-aware, so braces inside JSON strings do not count) and never
    /// tokenizes it: legal JSON Schema — negative numbers, exponent literals,
    /// `\uXXXX` and `\/` string escapes, any indentation — passes through
    /// byte-for-byte for the parser to validate as JSON and the printer to
    /// re-emit losslessly ("paste an existing JSON Schema verbatim").
    SchemaBody(String),
    /// The verbatim body of a `json { … }` literal, including the enclosing
    /// braces. Captured exactly like [`TokenKind::SchemaBody`]: the lexer
    /// consumes the brace-balanced region raw (string-aware, so braces inside
    /// JSON strings do not count) and never tokenizes it; the checker
    /// validates it as JSON and the printer re-emits it byte-for-byte.
    JsonBody(String),
    /// Increase in two-space indentation level.
    Indent,
    /// Decrease in two-space indentation level.
    Dedent,
}
