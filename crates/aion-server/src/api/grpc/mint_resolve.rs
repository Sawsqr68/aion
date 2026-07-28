//! The owner side of namespace-mint routing: serving a `MintNamespace` that a
//! peer forwarded here because THIS node owns the namespace's registry shard.
//!
//! Peeled out of `grpc/mod.rs` into a sibling with a private `impl` block, the
//! same AO-007 500-code-line discipline `routing_resolve.rs` follows — the trait
//! method in `mod.rs` is a one-line delegation, so the 900-line service adapter
//! does not grow by a whole handler.

use tonic::{Request, Response, Status};

use aion_proto::WireError;
use aion_proto::generated;

use super::WorkflowGrpcService;
use super::status::status_from_wire_error;
use crate::namespace::decode_mint_origin;

impl WorkflowGrpcService {
    /// Mint (or gate) an already-authorized namespace set on THIS node.
    ///
    /// A node whose registry shard for a namespace is foreign forwards the mint
    /// here rather than proposing a write it can never win. What arrives is an
    /// ordinary client request, not a privileged internal one:
    ///
    /// 1. the caller is resolved from the forwarded metadata exactly as on every
    ///    other RPC (`caller_from_metadata`), so a forwarded mint authenticates
    ///    as the ORIGINAL caller, never as "a peer node";
    /// 2. EVERY namespace in the set is authorized against that caller through
    ///    the same guard the worker-registration seam uses, so this RPC can
    ///    never mint a namespace its caller could not mint locally;
    /// 3. the mint itself runs through this node's ORDINARY
    ///    [`NamespaceMinter::mint_or_gate`](crate::NamespaceMinter::mint_or_gate)
    ///    — never a raw store write — so an `auto_create = closed` deployment
    ///    refuses a forwarded mint with exactly the refusal it gives a local
    ///    one. A forwarded mint is precisely as privileged as a local one.
    ///
    /// The minter used here carries NO routing: a mint that already travelled
    /// does not travel again. If this node's ownership of the shard has moved on
    /// since the initiator resolved it, the local quorum write is fenced and the
    /// typed `NotOwner` returns to the initiator, which surfaces it to the
    /// client. No re-forward chains and no internal retry loops — the client
    /// owns retry policy.
    ///
    /// An empty namespace set or an unspecified provenance is refused as invalid
    /// input rather than accepted as a no-op or guessed at.
    pub(super) async fn mint_namespace_here(
        &self,
        request: Request<generated::MintNamespaceRequest>,
    ) -> Result<Response<generated::MintNamespaceResponse>, Status> {
        let caller = self.caller(&request).await?;
        let inner = request.into_inner();
        let origin = decode_mint_origin(inner.origin).ok_or_else(|| {
            status_from_wire_error(WireError::invalid_input(
                "mint origin is unspecified or unknown",
            ))
        })?;
        if inner.namespaces.is_empty() {
            return Err(status_from_wire_error(WireError::invalid_input(
                "mint request named no namespaces",
            )));
        }
        let namespaces = self
            .state
            .namespace_guard()
            .scope_worker_namespaces(&caller, &inner.namespaces)
            .map_err(|error| status_from_wire_error(error.to_wire_error()))?;
        self.state
            .namespace_minter()
            .without_routing()
            .mint_or_gate(&namespaces, origin)
            .await
            .map_err(|error| status_from_wire_error(error.to_wire_error()))?;
        Ok(Response::new(generated::MintNamespaceResponse {}))
    }
}
