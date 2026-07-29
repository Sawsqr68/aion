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
    ActionBody, ActionDecl, ChildDecl, ConfigLine, ConfigValue, ConstDecl, Document, EnumVariant,
    FieldDecl, InputDecl, OutcomeDecl, ParamDecl, RetrySpec, RouteDirection, SignalDecl,
    SubflowDecl, SubflowOutcome, TypeBody, TypeDecl, TypeRef, WorkerDecl, WorkflowTimeoutDecl,
};
pub(crate) use expr::join_span;
pub use expr::{Arg, BinaryOp, DurationLiteral, Expr, PredicateKind, Quantifier};
pub use steps::{
    AfterRef, Binding, Call, CallStmt, CollectStmt, CombinatorCall, CombinatorKind, DeliveryVerb,
    DistributeStmt, ForkHeader, ForkStmt, Guard, JoinLine, LoopStmt, LoopTail, MaxVisits,
    OnFailure, OutcomeClause, PipeEnd, PipeStage, PipeStmt, RoutePayload, RouteStmt, RouteTarget,
    SleepStmt, SpawnStmt, Statement, Step, WaitStmt,
};
pub(crate) use trivia::doc_text;
pub use trivia::{Comment, DocLine, Lead};
