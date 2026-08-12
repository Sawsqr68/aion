//! The canonical workflow model: the lossless parse of a rev-2 document.

mod command;
mod document;
mod expr;
mod harness;
mod steps;
mod trivia;

#[cfg(test)]
mod command_tests;

pub use command::{CommandError, CommandReference, ScannedCommand, scan_command};
pub use document::{
    ActionBody, ActionDecl, ChildDecl, ConfigLine, ConfigValue, ConstDecl, Document,
    DocumentFamily, EnumVariant, FieldDecl, InputDecl, JSON_CAPTURE_WORD, OutcomeDecl, ParamDecl,
    QueryDecl, RetrySpec, RouteDirection, RunCapture, SendDecl, SignalDecl, SubflowDecl,
    SubflowOutcome, TEXT_CAPTURE_WORD, TypeBody, TypeDecl, TypeRef, WorkerDecl,
    WorkflowTimeoutDecl,
};
pub use expr::{Arg, BinaryOp, DurationLiteral, Expr, PredicateKind, Quantifier};
pub(crate) use expr::{access_path, chain_operands, field_path, join_span};
pub use harness::{
    HARNESS_ARGS_FIELD, HARNESS_BINARY_FIELD, HARNESS_COMMAND_FIELD, HARNESS_CONCURRENCY_FIELD,
    HARNESS_CWD_FIELD, HARNESS_CWD_FROM_WORD, HARNESS_ENV_PASS_FIELD, HARNESS_EXIT_GRACE_FIELD,
    HARNESS_FIELDS, HARNESS_KIND_ACP, HARNESS_KIND_FIELD, HARNESS_KIND_NORN,
    HARNESS_PERMISSION_ALLOW_ONCE, HARNESS_PERMISSION_DENY, HARNESS_PERMISSION_FIELD,
    HARNESS_RECONNECT_INITIAL_BACKOFF_FIELD, HARNESS_RECONNECT_MAX_ATTEMPTS_FIELD,
    HARNESS_RECONNECT_MAX_BACKOFF_FIELD, HARNESS_WORD, HarnessCwd, HarnessDecl, HarnessInteger,
    HarnessSetting, HarnessText, HarnessWord,
};
pub use steps::{
    AfterRef, AnswerStmt, Binding, Call, CallStmt, CollectStmt, CombinatorCall, CombinatorKind,
    DeliveryVerb, DistributeStmt, ForkHeader, ForkStmt, Guard, JoinLine, LoopStmt, LoopTail,
    MaxVisits, OnFailure, OutcomeClause, PipeEnd, PipeStage, PipeStmt, RoutePayload, RouteStmt,
    RouteTarget, SendStmt, SleepStmt, SpawnStmt, Statement, Step, WaitStmt,
};
pub(crate) use trivia::doc_text;
pub use trivia::{Comment, DocLine, Lead};
