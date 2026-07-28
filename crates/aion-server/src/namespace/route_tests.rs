//! Unit tests for the namespace-mint routing decision and its failure surface,
//! with no cluster and no transport: the decision is a pure function of the
//! shard resolver and the owner directory, so it is proved as one.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::Mutex;

use async_trait::async_trait;

use aion_proto::{WireError, WireErrorCode};
use aion_store::NamespaceOrigin;

use super::{
    ForwardMintError, MintCredentials, MintForwarder, MintOwner, MintRoute, MintShardOwners,
    NamespaceRouting, NamespaceShardResolver,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const OWNER: &str = "127.0.0.1:50051";

/// A shard resolver with a fixed name→shard table; any unlisted name is 0.
struct FixedShards(HashMap<String, usize>);

impl NamespaceShardResolver for FixedShards {
    fn shard_for_namespace(&self, name: &str) -> usize {
        self.0.get(name).copied().unwrap_or(0)
    }
}

/// A directory that answers one shard one way and everything else `Local`.
struct FixedOwners {
    shard: usize,
    owner: MintOwner,
}

impl MintShardOwners for FixedOwners {
    fn owner_of(&self, shard: usize) -> MintOwner {
        if shard == self.shard {
            self.owner.clone()
        } else {
            MintOwner::Local
        }
    }
}

/// Records what it was asked to forward and answers with a scripted outcome.
struct ScriptedForwarder {
    outcome: Mutex<Option<Result<(), ForwardMintError>>>,
    seen: Mutex<Vec<(SocketAddr, Vec<String>, NamespaceOrigin)>>,
}

impl ScriptedForwarder {
    fn new(outcome: Result<(), ForwardMintError>) -> Arc<Self> {
        Arc::new(Self {
            outcome: Mutex::new(Some(outcome)),
            seen: Mutex::new(Vec::new()),
        })
    }
}

#[async_trait]
impl MintForwarder for ScriptedForwarder {
    async fn forward_mint(
        &self,
        target: SocketAddr,
        _credentials: &MintCredentials,
        namespaces: &[String],
        origin: NamespaceOrigin,
    ) -> Result<(), ForwardMintError> {
        if let Ok(mut seen) = self.seen.lock() {
            seen.push((target, namespaces.to_vec(), origin));
        }
        match self.outcome.lock() {
            Ok(mut outcome) => outcome.take().unwrap_or(Ok(())),
            Err(_) => Err(ForwardMintError::Unreachable("poisoned script".to_owned())),
        }
    }
}

fn routing(
    shards: HashMap<String, usize>,
    owners: FixedOwners,
    forwarder: Arc<ScriptedForwarder>,
) -> NamespaceRouting {
    NamespaceRouting::new(Arc::new(FixedShards(shards)), Arc::new(owners), forwarder)
}

fn shard_table(entries: &[(&str, usize)]) -> HashMap<String, usize> {
    entries
        .iter()
        .map(|(name, shard)| ((*name).to_owned(), *shard))
        .collect()
}

/// A namespace whose registry shard this node owns routes Local — the ordinary
/// quorum write, no forward.
#[test]
fn locally_owned_registry_shard_routes_local() {
    let routing = routing(
        shard_table(&[("default", 1)]),
        FixedOwners {
            shard: 1,
            owner: MintOwner::Local,
        },
        ScriptedForwarder::new(Ok(())),
    );
    assert_eq!(routing.route_for("default"), MintRoute::Local { shard: 1 });
}

/// A confidently-remote, dialable owner routes Remote, carrying BOTH the shard
/// (so a refusal can name it) and the address to dial.
#[test]
fn remote_forwardable_owner_routes_remote() -> TestResult {
    let target: SocketAddr = OWNER.parse()?;
    let routing = routing(
        shard_table(&[("default", 1)]),
        FixedOwners {
            shard: 1,
            owner: MintOwner::Remote(target),
        },
        ScriptedForwarder::new(Ok(())),
    );
    assert_eq!(
        routing.route_for("default"),
        MintRoute::Remote { shard: 1, target }
    );
    Ok(())
}

/// An owner that is not known with confidence routes Unknown, so the caller
/// attempts LOCALLY and the receiver fence — not the directory — decides. The
/// directory is a hint; it is never trusted over the fence.
#[test]
fn unknown_owner_routes_unknown_so_the_fence_decides() {
    let routing = routing(
        shard_table(&[("default", 1)]),
        FixedOwners {
            shard: 1,
            owner: MintOwner::Unknown,
        },
        ScriptedForwarder::new(Ok(())),
    );
    assert_eq!(
        routing.route_for("default"),
        MintRoute::Unknown { shard: 1 }
    );
}

/// Namespaces are routed independently: two names on different registry shards
/// get different decisions from the same routing context.
#[test]
fn namespaces_route_independently_by_their_own_shard() -> TestResult {
    let target: SocketAddr = OWNER.parse()?;
    let routing = routing(
        shard_table(&[("default", 1), ("orders", 2)]),
        FixedOwners {
            shard: 1,
            owner: MintOwner::Remote(target),
        },
        ScriptedForwarder::new(Ok(())),
    );
    assert_eq!(
        routing.route_for("default"),
        MintRoute::Remote { shard: 1, target }
    );
    assert_eq!(routing.route_for("orders"), MintRoute::Local { shard: 2 });
    Ok(())
}

/// A successful forward carries the namespace and the origin verbatim to the
/// owner, so the owner records the provenance the initiator would have.
#[tokio::test]
async fn a_forwarded_mint_carries_the_namespace_and_origin() -> TestResult {
    let target: SocketAddr = OWNER.parse()?;
    let forwarder = ScriptedForwarder::new(Ok(()));
    let routing = routing(
        shard_table(&[("default", 1)]),
        FixedOwners {
            shard: 1,
            owner: MintOwner::Remote(target),
        },
        Arc::clone(&forwarder),
    );

    routing
        .forward("default", target, 1, NamespaceOrigin::StartMint)
        .await?;

    let seen = forwarder
        .seen
        .lock()
        .map_err(|_| "forward log poisoned")?
        .clone();
    assert_eq!(
        seen,
        vec![(
            target,
            vec!["default".to_owned()],
            NamespaceOrigin::StartMint
        )]
    );
    Ok(())
}

/// The owner ANSWERED and refused: its typed refusal is relayed verbatim, so an
/// `auto_create = closed` denial on the owner is a namespace denial to the
/// original caller — not a routing error, and not a fabricated success.
#[tokio::test]
async fn an_owners_refusal_is_relayed_verbatim() -> TestResult {
    let target: SocketAddr = OWNER.parse()?;
    let refusal = WireError::namespace_denied("namespace tenant-x does not exist");
    let routing = routing(
        shard_table(&[("tenant-x", 1)]),
        FixedOwners {
            shard: 1,
            owner: MintOwner::Remote(target),
        },
        ScriptedForwarder::new(Err(ForwardMintError::Refused(refusal))),
    );

    let error = routing
        .forward("tenant-x", target, 1, NamespaceOrigin::StartMint)
        .await
        .err()
        .ok_or("the owner's refusal must not be swallowed")?;
    let wire = error.to_wire_error();
    assert_eq!(wire.code, WireErrorCode::NamespaceDenied);
    assert_eq!(wire.message, "namespace tenant-x does not exist");
    Ok(())
}

/// A forward that reached no decision surfaces ONE truthful typed refusal
/// naming the namespace and its shard — the retryable `not_owner` code, so a
/// routing-aware client re-resolves. Nothing retries internally.
#[tokio::test]
async fn an_unreachable_owner_surfaces_one_truthful_not_owner() -> TestResult {
    let target: SocketAddr = OWNER.parse()?;
    let routing = routing(
        shard_table(&[("default", 1)]),
        FixedOwners {
            shard: 1,
            owner: MintOwner::Remote(target),
        },
        ScriptedForwarder::new(Err(ForwardMintError::Unreachable(
            "forward dial failed".to_owned(),
        ))),
    );

    let error = routing
        .forward("default", target, 1, NamespaceOrigin::StartMint)
        .await
        .err()
        .ok_or("an unreachable owner must not be reported as a mint")?;
    let wire = error.to_wire_error();
    assert_eq!(wire.code, WireErrorCode::NotOwner);
    assert!(
        wire.message.contains("namespace `default`") && wire.message.contains("shard 1"),
        "the refusal must name the namespace and its shard: {}",
        wire.message
    );
    Ok(())
}

/// The credential carrier never prints its values: a bearer token must not
/// reach a log through a `Debug` derive.
#[test]
fn credentials_debug_redacts_values() -> TestResult {
    let mut metadata = tonic::metadata::MetadataMap::new();
    metadata.insert("authorization", "Bearer super-secret".parse()?);
    let credentials = MintCredentials::from_grpc_metadata(&metadata);
    let rendered = format!("{credentials:?}");
    assert!(!rendered.contains("super-secret"), "rendered: {rendered}");
    assert!(rendered.contains("entries: 1"), "rendered: {rendered}");
    Ok(())
}
