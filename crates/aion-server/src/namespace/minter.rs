//! Transport-agnostic minted-on-use namespace hook (Control-Plane Phase 1).
//!
//! The single, shared implementation of the auto-create policy: given an
//! already-authorized namespace set, durably mint each unseen namespace
//! ([`AutoCreate::Open`]) or gate it ([`AutoCreate::Closed`]). Both mint
//! choke-points reuse this one type so the policy can never diverge:
//!
//! - the worker-registration seam ([`crate::worker::registry::ConnectedWorkerRegistry`],
//!   the primary minter — S5), and
//! - the workflow-start seam ([`crate::api::handlers::start_with_placement`], the
//!   safety net for a client that starts before any worker registers — S6).
//!
//! In every case the mint runs strictly AFTER the caller's authorization, so it
//! can only ever record a namespace the caller is already permitted to use — the
//! mint is auth-scoped by construction (CVE-2025-14986: open minting and
//! namespace isolation only coexist when minting is auth-gated).

use std::sync::Arc;

use aion_core::{ClusterEvent, NamespacePlacementWire};
use aion_store::{MintOutcome, NamespaceOrigin, NamespacePlacement, NamespaceStore};

use super::route::{MintCredentials, MintRoute, NamespaceRouting};
use crate::cluster_publisher::ClusterEventPublisher;
use crate::config::AutoCreate;
use crate::error::ServerError;

/// Minted-on-use hook pairing the durable namespace registry with its
/// [`AutoCreate`] policy.
///
/// Holds an `Arc<dyn NamespaceStore>` and the policy, so it is cheap to clone
/// and share between the worker-registration and workflow-start mint seams. The
/// hook is the *only* place the auto-create policy is implemented; both seams
/// call [`NamespaceMinter::mint_or_gate`], so the behaviour can never diverge
/// across transports or call sites.
#[derive(Clone)]
pub struct NamespaceMinter {
    store: Arc<dyn NamespaceStore>,
    policy: AutoCreate,
    /// Optional ops-console push channel (WS3). When present, a genuinely-new
    /// namespace (a [`MintOutcome::Created`] edge) emits a durable
    /// [`ClusterEvent::NamespaceCreated`] delta on the SAME deploy-scoped
    /// channel that already carries the worker/peer/shard topology deltas — so
    /// the live namespace panel appends each namespace exactly once with no
    /// refresh. `None` keeps every existing construction (and every test) silent,
    /// exactly like the registry's other optional seams.
    cluster_publisher: Option<ClusterEventPublisher>,
    /// Optional namespace-mint routing context. `None` on every single-node /
    /// non-clustered boot, where the mint is always local and the minter behaves
    /// byte-for-byte as it did before routing existed. On a clustered boot it
    /// carries the three handles that decide, per namespace, whether the durable
    /// write executes here or on the registry shard's owner.
    routing: Option<NamespaceRouting>,
}

impl std::fmt::Debug for NamespaceMinter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NamespaceMinter")
            .field("policy", &self.policy)
            .field("cluster_publisher", &self.cluster_publisher.is_some())
            .field("routing", &self.routing.is_some())
            .finish_non_exhaustive()
    }
}

impl NamespaceMinter {
    /// Build a minter over a durable namespace store and an auto-create policy.
    #[must_use]
    pub fn new(store: Arc<dyn NamespaceStore>, policy: AutoCreate) -> Self {
        Self {
            store,
            policy,
            cluster_publisher: None,
            routing: None,
        }
    }

    /// Attach the namespace-mint routing context a clustered boot builds, so a
    /// namespace whose registry shard this node does not own is minted by the
    /// node that does instead of being fenced forever.
    ///
    /// Pure builder addition: without it the minter mints locally exactly as
    /// before, which is what every single-node boot and every unit test does.
    #[must_use]
    pub fn with_routing(mut self, routing: NamespaceRouting) -> Self {
        self.routing = Some(routing);
        self
    }

    /// Drop the routing context, pinning every mint to THIS node.
    ///
    /// Used by the owner-side `MintNamespace` handler: a mint that already
    /// travelled must not travel again, so an arrival whose shard has moved
    /// on is answered with the local fence's typed refusal rather than a
    /// re-forward chain. The refusal returns to the initiator, which surfaces
    /// it; nothing retries internally.
    #[must_use]
    pub fn without_routing(mut self) -> Self {
        self.routing = None;
        self
    }

    /// Carry the inbound request's caller credentials onto any forwarded mint,
    /// so the owning node authorizes the caller exactly as this node did.
    ///
    /// A no-op with no routing context attached (nothing can be forwarded).
    #[must_use]
    pub fn with_caller_credentials(mut self, credentials: MintCredentials) -> Self {
        self.routing = self
            .routing
            .map(|routing| routing.with_credentials(credentials));
        self
    }

    /// Attach the WS3 cluster-event publisher so a first mint pushes a live
    /// `namespace created` delta to the ops console (Control-Plane Phase 1, S8).
    ///
    /// Pure builder addition: without it the minter behaves exactly as before
    /// (durable record + the `tracing` audit event only). The publisher is the
    /// deployment-global cluster channel — the same one the worker registry and
    /// supervisor emit on — so the delta reuses the existing browser push path
    /// rather than inventing a parallel channel.
    #[must_use]
    pub fn with_cluster_publisher(mut self, publisher: ClusterEventPublisher) -> Self {
        self.cluster_publisher = Some(publisher);
        self
    }

    /// The auto-create policy this minter applies.
    #[must_use]
    pub fn policy(&self) -> AutoCreate {
        self.policy
    }

    /// Apply the minted-on-use policy to an already-authorized namespace set.
    ///
    /// The caller MUST have authorized every namespace in `namespaces` before
    /// calling this — the mint is auth-scoped by construction, never a path to
    /// create a namespace the caller cannot use.
    ///
    /// - [`AutoCreate::Open`]: each namespace is durably upserted via
    ///   [`NamespaceStore::register_namespace`] with the given `origin`; a
    ///   [`MintOutcome::Created`] (first mint) emits a loud structured `tracing`
    ///   event — the Phase-1 "namespace created" signal (the socket-delta
    ///   surfacing lands in a later slice). A [`MintOutcome::AlreadyExisted`] is
    ///   silent (idempotent re-reference), so no duplicate row and no second
    ///   "created" event ever appear.
    /// - [`AutoCreate::Closed`]: a namespace with no registry row is rejected
    ///   with a namespace-denied error; nothing is created.
    ///
    /// A [`aion_store::StoreError::NotOwner`] from a quorum mint (this node is
    /// not the namespace shard's owner) propagates unchanged through `?` as
    /// [`ServerError::StoreBackend`], which surfaces as the typed, *retryable*
    /// `NotOwner` wire code — never a silent success.
    ///
    /// **Closed-policy existence check (Phase 1).** Existence is probed by
    /// registry-row presence ([`NamespaceStore::get_namespace`]). In a fresh
    /// Phase-1 deployment every used namespace already has a row minted on first
    /// register/start, so a missing row correctly means "never referenced".
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::StoreBackend`] if a durable upsert/lookup fails
    /// (including a retryable `NotOwner` fence), or [`ServerError::Namespace`]
    /// when `closed` rejects an unknown namespace.
    pub async fn mint_or_gate(
        &self,
        namespaces: &[String],
        origin: NamespaceOrigin,
    ) -> Result<(), ServerError> {
        for namespace in namespaces {
            match self.policy {
                AutoCreate::Open => self.mint(namespace, origin).await?,
                AutoCreate::Closed => {
                    if self.store.get_namespace(namespace).await?.is_none() {
                        return Err(ServerError::namespace_denied(format!(
                            "namespace {namespace} does not exist and auto_create is closed"
                        )));
                    }
                }
            }
        }
        Ok(())
    }

    /// Durably mint ONE namespace under the `open` policy, on the node that owns
    /// its registry record.
    ///
    /// With no routing context (every single-node / non-clustered boot) this is
    /// byte-for-byte the local upsert it always was. With one, the namespace's
    /// registry shard decides: this node writes when it owns the shard, or when
    /// ownership is not known with confidence (the receiver fence — not the
    /// directory — is the authority, so an uncertain answer is resolved by
    /// attempting and being told the truth). Only a confidently-remote,
    /// dialable owner is forwarded to, and then the WHOLE read-modify-write
    /// happens there.
    ///
    /// The `Created` edge is therefore decided wherever the write executes: a
    /// forwarded mint's `NamespaceCreated` delta fires on the OWNER's publisher,
    /// which is the deployment-global channel the ops console already listens
    /// on. That is the one observable difference a forwarded mint makes.
    async fn mint(&self, namespace: &str, origin: NamespaceOrigin) -> Result<(), ServerError> {
        if let Some(routing) = &self.routing {
            if let MintRoute::Remote { shard, target } = routing.route_for(namespace) {
                return routing.forward(namespace, target, shard, origin).await;
            }
        }
        if self.store.register_namespace(namespace, origin).await? == MintOutcome::Created {
            self.announce_created(namespace, origin).await?;
        }
        Ok(())
    }

    /// Explicit operator create (`POST /namespaces`, S7) routed through the SAME
    /// `MintOutcome::Created` choke-point so the live "namespace created" delta
    /// fires once for an operator-minted namespace exactly as it does for a
    /// worker- or start-minted one.
    ///
    /// Unlike [`NamespaceMinter::mint_or_gate`] this never gates on the
    /// [`AutoCreate::Closed`] policy: an explicit operator create is the
    /// documented escape hatch that brings a namespace into being in a
    /// locked-down deployment. The caller MUST have authorized `name` first (the
    /// HTTP handler runs the grant check), so the create is auth-scoped by
    /// construction.
    ///
    /// Returns the [`MintOutcome`] so the handler can report created-vs-existing
    /// to the operator. Idempotent: a re-create observes `AlreadyExisted` and
    /// emits no second delta.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::StoreBackend`] if the durable upsert/lookup fails
    /// (including a retryable `NotOwner` fence).
    pub async fn create_explicit(&self, name: &str) -> Result<MintOutcome, ServerError> {
        let outcome = self
            .store
            .register_namespace(name, NamespaceOrigin::Explicit)
            .await?;
        if outcome == MintOutcome::Created {
            self.announce_created(name, NamespaceOrigin::Explicit)
                .await?;
        }
        Ok(outcome)
    }

    /// Set an existing namespace's durable placement directive and emit the
    /// placement-changed socket delta (Control-Plane Phase 2, P2-P2).
    ///
    /// The caller MUST have authorized `name` first (the HTTP handler runs the
    /// SAME grant check `POST /namespaces` does), so the placement change is
    /// auth-scoped by construction — a caller can never place a namespace it
    /// cannot access. The durable write is the idempotent quorum value-CAS update
    /// of the record's `placement` field
    /// ([`NamespaceStore::set_namespace_placement`]): re-applying the same
    /// placement is a successful no-op.
    ///
    /// Returns `true` when the placement was durably set, or `false` when no
    /// registry row exists for `name` (placement targets an already-minted
    /// namespace, so the handler surfaces a not-found rather than minting here).
    /// The placement-changed delta fires only on a real set (never on the
    /// not-found path), mirroring the `Created`-edge discipline of
    /// [`Self::announce_created`].
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::StoreBackend`] if the durable update fails
    /// (including a retryable `NotOwner` fence).
    pub async fn set_placement(
        &self,
        name: &str,
        placement: NamespacePlacement,
    ) -> Result<bool, ServerError> {
        if self
            .store
            .set_namespace_placement(name, placement.clone())
            .await?
            .is_none()
        {
            return Ok(false);
        }
        self.announce_placement_changed(name, &placement);
        Ok(true)
    }

    /// Read a namespace's durable placement directive, for the worker-admission
    /// gate (Control-Plane Phase 2, P2-I1). Reads the SAME registry record
    /// [`Self::set_placement`] writes — the single source of truth — so admission
    /// and dispatch can never disagree on a namespace's placement.
    ///
    /// An absent registry row means no placement applies:
    /// [`NamespacePlacement::Unplaced`] (any worker), so a namespace that has not
    /// yet been minted never gates a registration.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::StoreBackend`] if the durable lookup fails (including
    /// a retryable `NotOwner` fence).
    pub async fn placement_of(&self, name: &str) -> Result<NamespacePlacement, ServerError> {
        Ok(self
            .store
            .get_namespace(name)
            .await?
            .map(|record| record.placement)
            .unwrap_or_default())
    }

    /// Emit the audit event AND (when a publisher is attached) the durable
    /// `namespace placement changed` socket delta after a placement update.
    ///
    /// Fires only on a real durable set (the `set_placement` not-found path
    /// returns before reaching here). Without a publisher attached it is the audit
    /// `tracing` event only, exactly like [`Self::announce_created`].
    fn announce_placement_changed(&self, name: &str, placement: &NamespacePlacement) {
        let wire = placement_to_wire(placement);
        tracing::info!(
            namespace = %name,
            placement_kind = %wire.kind,
            "namespace placement changed"
        );
        let Some(publisher) = &self.cluster_publisher else {
            return;
        };
        let name = name.to_owned();
        drop(
            publisher.emit(move |meta| ClusterEvent::NamespacePlacementChanged {
                meta,
                name,
                placement: wire,
            }),
        );
    }

    /// Emit the loud audit event AND (when a publisher is attached) the durable
    /// `namespace created` socket delta for a genuinely-new namespace.
    ///
    /// Called ONLY on the `MintOutcome::Created` edge, so it fires exactly once
    /// per genuinely-new namespace and never on an idempotent re-reference. The
    /// delta's `created_at` is read back from the durable record so the console's
    /// created column is the registry's authoritative instant, not a re-stamp at
    /// emit time; if the record cannot be re-read (a racer deprecated it, or a
    /// quorum hiccup) the audit event still fires and the delta is skipped rather
    /// than carrying a fabricated timestamp.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::StoreBackend`] only if the read-back lookup fails
    /// at the backend; an absent record (already reconciled away) is not an
    /// error — the audit event has already fired.
    async fn announce_created(
        &self,
        name: &str,
        origin: NamespaceOrigin,
    ) -> Result<(), ServerError> {
        tracing::info!(
            namespace = %name,
            origin = origin_label(origin),
            "namespace created"
        );
        let Some(publisher) = &self.cluster_publisher else {
            return Ok(());
        };
        let Some(record) = self.store.get_namespace(name).await? else {
            return Ok(());
        };
        let name = record.name;
        let created_at = record.created_at;
        let label = origin_label(record.origin).to_owned();
        drop(publisher.emit(move |meta| ClusterEvent::NamespaceCreated {
            meta,
            name,
            created_at,
            origin: label,
        }));
        Ok(())
    }
}

/// Project a durable [`NamespacePlacement`] onto its stable wire form for the
/// cluster socket delta: a `snake_case` `kind` tag plus the (possibly empty)
/// node-label set. `Unplaced` carries an empty `nodes`; `Prefer`/`Pinned` carry
/// their deterministically-ordered label set. Kept here (not in the leaf
/// `aion-core` crate) because only the server depends on `aion-store`'s enum.
fn placement_to_wire(placement: &NamespacePlacement) -> NamespacePlacementWire {
    match placement {
        NamespacePlacement::Unplaced => NamespacePlacementWire {
            kind: "unplaced".to_owned(),
            nodes: Vec::new(),
        },
        NamespacePlacement::Prefer { nodes } => NamespacePlacementWire {
            kind: "prefer".to_owned(),
            nodes: nodes.iter().cloned().collect(),
        },
        NamespacePlacement::Pinned { nodes } => NamespacePlacementWire {
            kind: "pinned".to_owned(),
            nodes: nodes.iter().cloned().collect(),
        },
    }
}

/// Stable `snake_case` label for the "namespace created" audit event, so the log
/// field stays the operational identifier (`worker_mint` / `start_mint` /
/// `explicit` / `inferred_from_state`) regardless of the enum's `Debug` form.
const fn origin_label(origin: NamespaceOrigin) -> &'static str {
    match origin {
        NamespaceOrigin::WorkerMint => "worker_mint",
        NamespaceOrigin::StartMint => "start_mint",
        NamespaceOrigin::Explicit => "explicit",
        NamespaceOrigin::InferredFromState => "inferred_from_state",
    }
}

/// Unit tests live in a sibling file so this module stays under the 500-line
/// law with the test bodies intact rather than thinned.
#[cfg(test)]
#[path = "minter_tests.rs"]
mod tests;
