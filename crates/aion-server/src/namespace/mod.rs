//! Namespace authorization and routing support.

/// Namespace guard rules for workflow operations and subscriptions.
pub mod guard;
/// The production gRPC mint forwarder.
#[cfg(feature = "haematite-backend")]
pub mod mint_forward;
/// Transport-agnostic minted-on-use namespace hook (Control-Plane Phase 1).
pub mod minter;
/// Caller identity resolution and scoped-engine selection.
pub mod resolver;
/// Namespace-mint routing: the mint travels to its registry shard's owner.
pub mod route;
/// Durable schedule→namespace ownership sources.
pub mod schedule_source;

pub use guard::{
    NamespaceGuard, NamespaceOperation, ScheduleTarget, SubscriptionScope, WorkflowTarget,
};
#[cfg(feature = "haematite-backend")]
pub use mint_forward::GrpcMintForwarder;
pub use minter::NamespaceMinter;
pub use resolver::{
    CallerIdentity, DISPLAY_NAME_ATTRIBUTE, NAMESPACE_ATTRIBUTE, NamespaceResolver, ScopedEngine,
    StaticWorkflowNamespaces, TASK_QUEUE_ATTRIBUTE, WorkflowAttribution, WorkflowNamespaceSource,
};
pub use route::{
    ForwardMintError, MintCredentials, MintForwarder, MintOwner, MintRoute, MintShardOwners,
    NamespaceRouting, NamespaceShardResolver, decode_mint_origin, encode_mint_origin,
};
pub use schedule_source::{ScheduleNamespaceSource, StaticScheduleNamespaces};
