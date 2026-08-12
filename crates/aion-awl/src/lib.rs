//! Front end for the AWL workflow language (rev-2 surface).
//!
//! The `.awl` document is the source of truth; the canonical workflow model
//! is its lossless parse. The lexer is hand-written so the parser can rely
//! on exact token spans, indentation tokens, and AWL's doc-line data tokens
//! (`//!`, `///`); the parser and canonical printer are one property:
//! `parse ∘ print = id`, `print ∘ parse ∘ print = print`.

mod ast;
mod checker;
mod compile;
mod emitter;
mod fold;
pub mod guide;
mod jsontext;
mod lexer;
// The AWL-BC bytecode MIR backend (BC-2). Exposed as `#[doc(hidden)] pub` — a
// tooling/test seam, NOT a supported public API — because the ratified type
// set (AWL-BC-IR.md Appendix A) includes the intentionally-unused `ResultTry`
// fallback and a not-yet-complete `lower`, which cannot coexist with a
// `pub(crate)` module under the workspace's `-D warnings` + no-`#[allow]`
// discipline (unconstructed private variants are `dead_code`). See
// `AWL-BC-IR.md` "BC-2 implementation status"; tightening to `pub(crate)` is a
// panel/operator decision once `lower` constructs the full op surface and the
// `ResultTry` marker is resolved.
mod harness_config;
#[doc(hidden)]
pub mod mir;
mod parser;
mod positions;
mod printer;
mod schema;
pub mod semantic;
mod spanned;
mod unlowered;
mod worker_document;
mod workspace_root;

pub use ast::{
    ActionBody, ActionDecl, AfterRef, AnswerStmt, Arg, BinaryOp, Binding, Call, CallStmt,
    ChildDecl, CollectStmt, CombinatorCall, CombinatorKind, CommandError, CommandReference,
    Comment, ConfigLine, ConfigValue, ConstDecl, DeliveryVerb, DistributeStmt, DocLine, Document,
    DocumentFamily, DurationLiteral, EnumVariant, Expr, FieldDecl, ForkHeader, ForkStmt, Guard,
    HARNESS_ARGS_FIELD, HARNESS_BINARY_FIELD, HARNESS_COMMAND_FIELD, HARNESS_CONCURRENCY_FIELD,
    HARNESS_CWD_FIELD, HARNESS_ENV_PASS_FIELD, HARNESS_EXIT_GRACE_FIELD, HARNESS_FIELDS,
    HARNESS_KIND_ACP, HARNESS_KIND_FIELD, HARNESS_KIND_NORN, HARNESS_PERMISSION_ALLOW_ONCE,
    HARNESS_PERMISSION_DENY, HARNESS_PERMISSION_FIELD, HARNESS_RECONNECT_INITIAL_BACKOFF_FIELD,
    HARNESS_RECONNECT_MAX_ATTEMPTS_FIELD, HARNESS_RECONNECT_MAX_BACKOFF_FIELD, HARNESS_WORD,
    HarnessDecl, HarnessInteger, HarnessSetting, HarnessText, HarnessWord, InputDecl,
    JSON_CAPTURE_WORD, JoinLine, Lead, LoopStmt, LoopTail, MaxVisits, OnFailure, OutcomeClause,
    OutcomeDecl, ParamDecl, PipeEnd, PipeStage, PipeStmt, PredicateKind, Quantifier, QueryDecl,
    RetrySpec, RouteDirection, RoutePayload, RouteStmt, RouteTarget, RunCapture, ScannedCommand,
    SendDecl, SendStmt, SignalDecl, SleepStmt, SpawnStmt, Statement, Step, SubflowDecl,
    SubflowOutcome, TEXT_CAPTURE_WORD, TypeBody, TypeDecl, TypeRef, WaitStmt, WorkerDecl,
    WorkflowTimeoutDecl, scan_command,
};
pub use checker::{CheckError, check, check_in};
pub use compile::{
    ActionImplementation, ActionRequirement, CompileError, CompiledActionBody, CompiledWorkflow,
    action_requirements, compile,
};
pub use emitter::{
    EmitError, EmittedArtifact, SynthesizedWorkflowEntry, emit, emit_artifact, emit_artifact_in,
    emit_in,
};
pub use harness_config::{
    CompiledHarness, CompiledHarnessKind, HarnessConfigError, HarnessPermission, compile_harness,
};
pub use lexer::{
    ANSWER_WORD, DurationUnit, IN_WORD, Keyword, LexError, POSITIONAL_WORDS, QUERY_WORD, RUN_WORD,
    SEND_WORD, Span, TO_WORD, Token, TokenKind, lex,
};
pub use parser::{ParseError, parse};
pub use positions::{
    Utf16Position, Utf16Range, byte_offset_at_utf16, utf16_position_at, utf16_range_for_span,
};
pub use printer::{expr_text, print};
pub use schema::{
    SchemaError, schema_for_outcomes, schema_for_outcomes_in, schema_for_params_in,
    schema_for_queries, schema_for_queries_in, schema_for_type, schema_for_type_in,
    schema_for_type_ref_in, schema_for_workflow, schema_for_workflow_in,
};
pub use spanned::Spanned;
pub use worker_document::{CompiledWorkerAction, CompiledWorkerDocument, compile_worker_document};
pub use workspace_root::{
    HarnessPathForm, HarnessPathRefusal, WORKSPACE_ROOT_PLACEHOLDER, harness_path_form,
};
