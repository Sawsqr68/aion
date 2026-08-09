//! The canonical workflow model: the lossless parse of a rev-2 document.

mod command;
mod document;
mod expr;
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
pub use steps::{
    AfterRef, AnswerStmt, Binding, Call, CallStmt, CollectStmt, CombinatorCall, CombinatorKind,
    DeliveryVerb, DistributeStmt, ForkHeader, ForkStmt, Guard, JoinLine, LoopStmt, LoopTail,
    MaxVisits, OnFailure, OutcomeClause, PipeEnd, PipeStage, PipeStmt, RoutePayload, RouteStmt,
    RouteTarget, SendStmt, SleepStmt, SpawnStmt, Statement, Step, WaitStmt,
};
pub(crate) use trivia::doc_text;
pub use trivia::{Comment, DocLine, Lead};
