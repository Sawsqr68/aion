//! Namespace-mint routing: send the MINT to the namespace registry shard's
//! owner, never the start.
//!
//! A namespace's registry record is keyed `n: || name`, so it hashes to a shard
//! of its own — unrelated to the shard any workflow started in that namespace
//! lands on. Two keys, two routings. The workflow half was already routed (the
//! R-1 unsteered remint places the execution on a locally-owned shard); the
//! namespace half was not, so on a multi-shard cluster every unsteered `start`
//! on a node that did not happen to own `shard_for_namespace(name)` was fenced
//! by the quorum CAS, forever, with a constant `NotOwner { shard }`
//! (`docs/evidence/failover-start-path-investigation.md`).
//!
//! Routing the whole START instead was considered and rejected: the caller's
//! workflow-shard steering and the namespace shard hash DIFFERENT keys and
//! routinely disagree, so start-forwarding re-couples two independent placement
//! decisions and can bounce off the forward hop cap.
//!
//! ## The seam
//!
//! [`NamespaceRouting`] is the one optional dependency
//! [`NamespaceMinter`](super::NamespaceMinter) gains. It is `None` on every
//! single-node / non-clustered boot, where the minter behaves byte-for-byte as
//! it did before this module existed. It holds three handles:
//!
//! - a [`NamespaceShardResolver`] — which registry shard a name lives on;
//! - a [`MintShardOwners`] directory — who owns that shard right now;
//! - a [`MintForwarder`] — how to ship an already-authorized mint there.
//!
//! Each is a narrow trait rather than a concrete type, so this module compiles
//! and is testable without the distributed backend; the production
//! implementations (haematite's store, the R-2 `StaticShardDirectory`, the R-3
//! gRPC forwarder) are adapted at the bottom of this file behind
//! `feature = "haematite-backend"`.
//!
//! ## What the route preserves
//!
//! The forward moves ONLY which process initiates the write. The ENTIRE
//! read-modify-write — the `database.get`, the `Hash::of` expected value, and
//! the `replicate_write` proposal — executes on the node that runs it. Reading
//! locally and proposing remotely would introduce a cross-node TOCTOU and is
//! deliberately not what this does. The epoch fence stays receiver-side and
//! unchanged; forwarding to the owner simply supplies the stamp receivers
//! accept, which is the whole fix.
//!
//! The directory is a liveness-gated HINT, never the authority: an owner that
//! cannot be resolved with confidence resolves [`MintOwner::Unknown`] and the
//! mint is attempted locally so the receiver fence — the real enforcement —
//! answers. The directory is never trusted over the fence.

use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;

use aion_proto::WireError;
use aion_proto::generated;
use aion_store::NamespaceOrigin;

use crate::error::ServerError;

/// Which distribution shard a namespace's registry record lives on.
///
/// Implemented by the distributed store; a narrow trait so the routing decision
/// is unit-testable without standing up a cluster.
pub trait NamespaceShardResolver: Send + Sync {
    /// The registry shard `name`'s record hashes to.
    fn shard_for_namespace(&self, name: &str) -> usize;
}

/// The mint's view of who owns a shard right now.
///
/// Deliberately coarser than the routing edge's `OwnerView`: a mint can only do
/// two things with the answer — forward to a dialable address, or attempt the
/// write locally and let the fence rule. An owner that is known but carries no
/// forward address is therefore [`Self::Unknown`] here: there is nothing to dial,
/// so the truthful outcome is the local attempt's fence refusal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MintOwner {
    /// This node owns the shard: take the ordinary local quorum write.
    Local,
    /// Another node owns it and is reachable for forwarding at this address.
    Remote(SocketAddr),
    /// Ownership is not known with confidence (no record, owner believed down,
    /// or no forward address): attempt locally and let the fence be the
    /// authority.
    Unknown,
}

/// Resolves the current owner of a distribution shard for the mint.
pub trait MintShardOwners: Send + Sync {
    /// Who owns `shard`, as far as the mint can act on it.
    fn owner_of(&self, shard: usize) -> MintOwner;
}

/// The decision for one namespace's mint, carrying the shard so a refusal can
/// name it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MintRoute {
    /// Mint here — this node owns the registry shard.
    Local {
        /// The namespace's registry shard.
        shard: usize,
    },
    /// Forward the mint to the owner at `target`.
    Remote {
        /// The namespace's registry shard.
        shard: usize,
        /// The owner's gRPC address.
        target: SocketAddr,
    },
    /// Attempt here and let the receiver fence answer.
    Unknown {
        /// The namespace's registry shard.
        shard: usize,
    },
}

/// The already-authorized caller's inbound request credentials, carried verbatim
/// onto a forwarded mint so the OWNING node authorizes the caller exactly as
/// this node did — the same discipline the R-3 request forwarder applies to a
/// forwarded signal/query/cancel.
///
/// Empty for mint seams that have no inbound request metadata to copy (the HTTP
/// start path and the worker-registration seam). A forwarded mint carrying no
/// credentials authorizes on the owner exactly as an unauthenticated request
/// would: the operator identity when the deployment runs with auth off, and a
/// refusal when it does not. That is a truthful refusal, never a silent bypass.
#[derive(Clone, Default)]
pub struct MintCredentials {
    metadata: tonic::metadata::MetadataMap,
}

impl MintCredentials {
    /// Copy an inbound gRPC request's caller metadata.
    #[must_use]
    pub fn from_grpc_metadata(metadata: &tonic::metadata::MetadataMap) -> Self {
        Self {
            metadata: metadata.clone(),
        }
    }

    /// The metadata to stamp onto an outbound forward.
    #[must_use]
    pub fn to_grpc_metadata(&self) -> tonic::metadata::MetadataMap {
        self.metadata.clone()
    }
}

/// Prints how many credential entries are carried, never their values: a bearer
/// token must not reach a log through a `Debug` derive.
impl std::fmt::Debug for MintCredentials {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MintCredentials")
            .field("entries", &self.metadata.len())
            .finish()
    }
}

/// Why a forwarded mint did not succeed.
///
/// The two arms have opposite meanings and opposite surfacings, so they are
/// distinguished structurally (does the reply carry the server's typed wire
/// error?) and never by matching on message text.
#[derive(Debug)]
pub enum ForwardMintError {
    /// The owner ANSWERED and refused. Its typed refusal is relayed to the
    /// caller verbatim — an `auto_create = closed` denial forwarded back is a
    /// namespace denial here too.
    Refused(WireError),
    /// The forward never reached a decision: the dial failed, the transport
    /// broke, or the reply did not match the request. The caller is told the
    /// truth (the namespace's shard is owned elsewhere and was not reachable)
    /// and owns the retry policy; nothing retries internally.
    Unreachable(String),
}

/// Ships an already-authorized mint to the namespace shard's owning node.
#[async_trait]
pub trait MintForwarder: Send + Sync {
    /// Forward the mint of `namespaces` (with `origin`) to the owner at
    /// `target`, copying `credentials` so the owner authorizes identically.
    ///
    /// # Errors
    ///
    /// Returns [`ForwardMintError::Refused`] with the owner's typed refusal, or
    /// [`ForwardMintError::Unreachable`] when no decision was reached.
    async fn forward_mint(
        &self,
        target: SocketAddr,
        credentials: &MintCredentials,
        namespaces: &[String],
        origin: NamespaceOrigin,
    ) -> Result<(), ForwardMintError>;
}

/// The routing context a clustered boot threads into every
/// [`NamespaceMinter`](super::NamespaceMinter).
#[derive(Clone)]
pub struct NamespaceRouting {
    shards: Arc<dyn NamespaceShardResolver>,
    owners: Arc<dyn MintShardOwners>,
    forwarder: Arc<dyn MintForwarder>,
    credentials: MintCredentials,
}

impl std::fmt::Debug for NamespaceRouting {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NamespaceRouting")
            .field("credentials", &self.credentials)
            .finish_non_exhaustive()
    }
}

impl NamespaceRouting {
    /// Build the routing context over its three handles, with no caller
    /// credentials yet.
    #[must_use]
    pub fn new(
        shards: Arc<dyn NamespaceShardResolver>,
        owners: Arc<dyn MintShardOwners>,
        forwarder: Arc<dyn MintForwarder>,
    ) -> Self {
        Self {
            shards,
            owners,
            forwarder,
            credentials: MintCredentials::default(),
        }
    }

    /// Attach the inbound request's caller credentials for this request's mints.
    #[must_use]
    pub fn with_credentials(mut self, credentials: MintCredentials) -> Self {
        self.credentials = credentials;
        self
    }

    /// Where `name`'s mint must execute.
    ///
    /// Decision order: resolve the name's registry shard, then ask the directory
    /// who owns it. `Local` and `Unknown` both execute here — the second
    /// deliberately, so the receiver fence (the enforcement) answers rather than
    /// the directory (a hint).
    #[must_use]
    pub fn route_for(&self, name: &str) -> MintRoute {
        let shard = self.shards.shard_for_namespace(name);
        match self.owners.owner_of(shard) {
            MintOwner::Local => MintRoute::Local { shard },
            MintOwner::Remote(target) => MintRoute::Remote { shard, target },
            MintOwner::Unknown => MintRoute::Unknown { shard },
        }
    }

    /// Forward `name`'s mint to `target`, surfacing exactly one truthful typed
    /// refusal on failure.
    ///
    /// # Errors
    ///
    /// Returns the owner's refusal verbatim when it answered, or a typed
    /// `not_owner` naming the namespace and its shard when the forward reached
    /// no decision. Never retries internally — the client owns retry policy.
    pub async fn forward(
        &self,
        name: &str,
        target: SocketAddr,
        shard: usize,
        origin: NamespaceOrigin,
    ) -> Result<(), ServerError> {
        let namespaces = [name.to_owned()];
        match self
            .forwarder
            .forward_mint(target, &self.credentials, &namespaces, origin)
            .await
        {
            Ok(()) => Ok(()),
            Err(ForwardMintError::Refused(wire)) => Err(ServerError::Wire { wire }),
            Err(ForwardMintError::Unreachable(detail)) => Err(ServerError::Wire {
                wire: WireError::not_owner(format!(
                    "namespace `{name}` is registered on shard {shard}, owned by \
                         cluster node {target}, which did not answer the mint: {detail}"
                ))
                .with_error_type("NotOwner"),
            }),
        }
    }
}

/// The production namespace-shard resolver: the distributed haematite store
/// hashes the registry key itself.
#[cfg(feature = "haematite-backend")]
impl NamespaceShardResolver for aion_store_haematite::HaematiteStore {
    fn shard_for_namespace(&self, name: &str) -> usize {
        Self::shard_for_namespace(self, name)
    }
}

/// The production owner directory: the R-2 static directory with its SS-3
/// quorum-replicated shard-owner overlay and peer-liveness gate — the SAME
/// resolver the request-routing edge consults, so the mint and a signal for the
/// same shard can never disagree about who owns it.
#[cfg(feature = "haematite-backend")]
impl MintShardOwners for crate::routing::StaticShardDirectory {
    fn owner_of(&self, shard: usize) -> MintOwner {
        use crate::routing::{OwnerView, ShardDirectory};
        match ShardDirectory::owner_of(self, shard) {
            OwnerView::Local => MintOwner::Local,
            // A known owner with no declared gRPC address has nothing to dial:
            // the truthful outcome is the local attempt's fence, not a forward.
            OwnerView::Remote(node) => node.grpc_addr.map_or(MintOwner::Unknown, MintOwner::Remote),
            OwnerView::Unknown => MintOwner::Unknown,
        }
    }
}

/// The wire code for a mint origin, so the owner records the same provenance the
/// initiating node would have recorded.
#[must_use]
pub const fn encode_mint_origin(origin: NamespaceOrigin) -> i32 {
    let code = match origin {
        NamespaceOrigin::WorkerMint => generated::NamespaceMintOrigin::Worker,
        NamespaceOrigin::StartMint => generated::NamespaceMintOrigin::Start,
        NamespaceOrigin::Explicit => generated::NamespaceMintOrigin::Explicit,
        NamespaceOrigin::InferredFromState => generated::NamespaceMintOrigin::InferredFromState,
    };
    code as i32
}

/// Decode a wire mint origin. `None` for the unspecified/unknown code, which the
/// owner-side handler refuses as invalid input rather than guessing a
/// provenance.
#[must_use]
pub fn decode_mint_origin(code: i32) -> Option<NamespaceOrigin> {
    match generated::NamespaceMintOrigin::try_from(code).ok()? {
        generated::NamespaceMintOrigin::Unspecified => None,
        generated::NamespaceMintOrigin::Worker => Some(NamespaceOrigin::WorkerMint),
        generated::NamespaceMintOrigin::Start => Some(NamespaceOrigin::StartMint),
        generated::NamespaceMintOrigin::Explicit => Some(NamespaceOrigin::Explicit),
        generated::NamespaceMintOrigin::InferredFromState => {
            Some(NamespaceOrigin::InferredFromState)
        }
    }
}

#[cfg(test)]
#[path = "route_tests.rs"]
mod tests;
