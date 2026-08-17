//! Namespace-mint routing over a REAL 3-node haematite cluster: the in-process
//! proof of the production start-path defect, and of its fix.
//!
//! ## The defect this pins
//!
//! An `aion start` mints its namespace on use. The registry record for a
//! namespace is keyed `n: || name`, so it hashes to a shard of its OWN —
//! `namespace_key("default")` lands on shard **1** of a 3-shard cluster
//! (`HaematiteStore::shard_for_namespace`, pinned in aion-store-haematite). Node
//! 0 of that cluster owns shard 0. Its quorum write for the namespace record is
//! therefore fenced by shard 1's serving owner, forever, on every attempt:
//! `NotOwner { shard: 1 }`, 262 times out of 262 on the record
//! (`docs/evidence/failover-start-path-investigation.md`).
//!
//! The workflow's own placement was never the problem — the R-1 remint already
//! puts the execution on a locally-owned shard. Two keys, two routings; only the
//! namespace half was unrouted.
//!
//! ## What this test asserts, in order, on ONE cluster
//!
//! 1. **Non-vacuity.** `default`'s registry shard is 1, and the node doing the
//!    minting (C) owns only shard 0 — so the mint is genuinely non-local and the
//!    fence is genuinely reachable. If either changes, the rest proves nothing.
//! 2. **The unrouted path is unchanged.** A minter with NO routing context — the
//!    single-node / non-clustered construction, byte-identical to before this
//!    lane — still refuses with the typed, retryable `NotOwner { shard: 1 }`.
//!    This is the production defect reproduced in-process, and it is the pin
//!    that off-cluster boots did not change behaviour.
//! 3. **The routed path lands the record.** The same mint, through a minter
//!    carrying the routing context a clustered boot wires, is executed by the
//!    namespace shard's OWNER and the record becomes readable on C's own
//!    replica — the whole read-modify-write having moved to the owner, never
//!    split across nodes.
//!
//! Plain `#[test]`: haematite's distribution coordinator blocks and refuses to
//! run from a thread with an entered tokio runtime, exactly as
//! `ss3_directory_failover_e2e.rs`. The async mint is driven through an explicit
//! `block_on` AFTER the cluster is stood up; the durable upsert itself hops to
//! `spawn_blocking` internally, so it is safe under the runtime.
//!
//! Behind `--features haematite-backend`.

#![cfg(feature = "haematite-backend")]

use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use aion_server::NamespaceMinter;
use aion_server::config::AutoCreate;
use aion_server::namespace::{
    ForwardMintError, MintCredentials, MintForwarder, MintOwner, MintShardOwners, NamespaceRouting,
    NamespaceShardResolver,
};
use aion_store::{NamespaceOrigin, NamespaceStore, StoreError};
use aion_store_haematite::HaematiteStore;
use haematite::db::respond_to_inbound_writes;
use haematite::sync::membership::WriteMembership;
use haematite::sync::{DistributionEndpoint, SyncNodeId};
use haematite::{Database, DatabaseConfig};

type TestResult = Result<(), Box<dyn Error>>;

const NODE_A: &str = "mintroute-a@127.0.0.1";
const NODE_B: &str = "mintroute-b@127.0.0.1";
const NODE_C: &str = "mintroute-c@127.0.0.1";

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
const OP_TIMEOUT: Duration = Duration::from_secs(5);

/// The namespace a zero-config `aion start` mints on use.
const DEFAULT_NAMESPACE: &str = "default";
/// The shard `namespace_key("default")` hashes to at `shard_count = 3`. Asserted
/// from the store rather than assumed, but named here because the whole defect
/// is that this is a CONSTANT and not a lottery.
const DEFAULT_NAMESPACE_SHARD: usize = 1;
/// The only shard the minting node (C) owns — deliberately not the namespace's.
const NODE_C_SHARD: usize = 0;

fn loopback() -> Result<SocketAddr, Box<dyn Error>> {
    Ok("127.0.0.1:0".parse()?)
}

fn config_for(path: &std::path::Path) -> DatabaseConfig {
    DatabaseConfig {
        data_dir: path.to_path_buf(),
        // Three shards: the production kill-9 cluster's shape, and the shape the
        // investigation measured `default → 1` against.
        shard_count: 3,
        executor_threads: None,
        distributed: None,
        // No byte bound, stated out loud: this fixture measures
        // replication/routing and never re-reads enough nodes for a ceiling
        // to bind, so a number here would be decoration. haematite has no
        // default for this; `Unlimited` is the pre-budget behaviour, named.
        node_cache_budget: Some(haematite::NodeCacheBudget::Unlimited),
    }
}

fn wait_until(timeout: Duration, mut predicate: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if predicate() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn membership(total_nodes: usize, send_targets: &[&str]) -> WriteMembership {
    WriteMembership {
        total_nodes,
        send_targets: send_targets
            .iter()
            .map(|name| SyncNodeId::from(*name))
            .collect(),
    }
}

/// One cluster node: a distributed `HaematiteStore` over a distribution-attached
/// `Database`, plus a background responder answering peers' replication/election.
struct Node {
    store: Arc<HaematiteStore>,
    event_store: Arc<haematite::EventStore>,
    addr: SocketAddr,
    name: &'static str,
    responder: Option<JoinHandle<()>>,
    running: Arc<std::sync::atomic::AtomicBool>,
}

impl Node {
    fn spawn(
        name: &'static str,
        dir: &std::path::Path,
        total_nodes: usize,
        send_targets: &[&str],
    ) -> Result<Self, Box<dyn Error>> {
        let endpoint = DistributionEndpoint::bind(name, loopback()?, 1, None)?;
        let addr = endpoint.local_addr();
        let database =
            Database::create(config_for(dir.join("db").as_path()))?.with_distribution(endpoint);
        let store = Arc::new(HaematiteStore::with_distribution(
            database,
            membership(total_nodes, send_targets),
            OP_TIMEOUT,
            name.to_owned(),
        ));
        let event_store = Arc::clone(store.event_store());

        let running = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let responder_store = Arc::clone(&event_store);
        let responder_running = Arc::clone(&running);
        let responder = std::thread::spawn(move || {
            while responder_running.load(std::sync::atomic::Ordering::Relaxed) {
                drop(respond_to_inbound_writes(
                    responder_store.database(),
                    Duration::from_millis(50),
                ));
            }
        });

        Ok(Self {
            store,
            event_store,
            addr,
            name,
            responder: Some(responder),
            running,
        })
    }

    fn database(&self) -> &Database {
        self.event_store.database()
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
        if let Some(handle) = self.responder.take() {
            drop(handle.join());
        }
    }
}

fn link(from: &Node, to: &Node) -> TestResult {
    let endpoint = from
        .database()
        .distribution()
        .ok_or("dialing node has no endpoint")?;
    endpoint.add_peer(to.name, to.addr);
    endpoint.connect(to.name)?;
    if !wait_until(HANDSHAKE_TIMEOUT, || endpoint.is_connected(to.name)) {
        return Err(format!("{} never registered a link to {}", from.name, to.name).into());
    }
    Ok(())
}

fn link_both(a: &Node, b: &Node) -> TestResult {
    link(a, b)?;
    link(b, a)?;
    Ok(())
}

/// The shard a `NotOwner` refusal names, or `None` when the error was not a
/// not-owner fence. Matched on the TYPED error, never on message text.
fn fenced_shard(error: &aion_server::ServerError) -> Option<usize> {
    match error {
        aion_server::ServerError::StoreBackend {
            source: StoreError::NotOwner { shard },
        } => Some(*shard),
        _ => None,
    }
}

/// The headline proof. See the module docs for the three ordered claims.
#[test]
fn unsteered_namespace_mint_reaches_the_registry_shards_owner() -> TestResult {
    let dir_a = private_tempdir()?;
    let dir_b = private_tempdir()?;
    let dir_c = private_tempdir()?;

    // A is the namespace shard's owner and replicates to {B, C}; C is the node a
    // client's unsteered `start` happens to reach.
    let node_a = Node::spawn(NODE_A, dir_a.path(), 3, &[NODE_B, NODE_C])?;
    let node_b = Node::spawn(NODE_B, dir_b.path(), 3, &[NODE_A, NODE_C])?;
    let node_c = Node::spawn(NODE_C, dir_c.path(), 3, &[NODE_A, NODE_B])?;
    link_both(&node_a, &node_b)?;
    link_both(&node_a, &node_c)?;
    link_both(&node_b, &node_c)?;

    node_a.store.set_owned_shards([DEFAULT_NAMESPACE_SHARD]);
    node_b.store.set_owned_shards([2]);
    node_c.store.set_owned_shards([NODE_C_SHARD]);

    // (1) NON-VACUITY. The default namespace's registry record does not live on
    // the shard the minting node owns — the whole premise of the defect.
    assert_eq!(
        node_c.store.shard_for_namespace(DEFAULT_NAMESPACE),
        DEFAULT_NAMESPACE_SHARD,
        "`default`'s registry shard must be {DEFAULT_NAMESPACE_SHARD} at shard_count 3"
    );
    assert_ne!(
        DEFAULT_NAMESPACE_SHARD, NODE_C_SHARD,
        "the minting node must NOT own the namespace's registry shard"
    );

    // A takes the fenced serve authority for the namespace's shard: from here on
    // any proposal from a node without that authority is fenced by the receiver.
    node_a.database().acquire_shard_and_serve(
        DEFAULT_NAMESPACE_SHARD,
        &membership(3, &[NODE_B, NODE_C]),
        OP_TIMEOUT,
    )?;

    let runtime = tokio::runtime::Runtime::new()?;

    // (2) THE UNROUTED PATH IS UNCHANGED — and is the production defect.
    let unrouted = NamespaceMinter::new(
        Arc::clone(&node_c.store) as Arc<dyn NamespaceStore>,
        AutoCreate::Open,
    );
    let refusal = runtime.block_on(async {
        unrouted
            .mint_or_gate(&[DEFAULT_NAMESPACE.to_owned()], NamespaceOrigin::StartMint)
            .await
    });
    let refusal = refusal.err().ok_or(
        "a minter with no routing context must still be fenced on a non-owned \
         registry shard — the off-cluster path must not have changed",
    )?;
    assert_eq!(
        fenced_shard(&refusal),
        Some(DEFAULT_NAMESPACE_SHARD),
        "expected the typed NotOwner {{ shard: {DEFAULT_NAMESPACE_SHARD} }} fence, got: {refusal}"
    );
    assert!(
        runtime
            .block_on(node_c.store.get_namespace(DEFAULT_NAMESPACE))?
            .is_none(),
        "the fenced mint must have created nothing anywhere"
    );

    // (3) THE ROUTED PATH LANDS THE RECORD. The same mint, from the same node,
    // through the routing context a clustered boot wires.
    let routed = routed_minter(&node_c, &node_a);
    runtime.block_on(async {
        routed
            .mint_or_gate(&[DEFAULT_NAMESPACE.to_owned()], NamespaceOrigin::StartMint)
            .await
    })?;

    // The owner executed the whole read-modify-write, and the record replicated
    // back to the initiator's own replica — readable from C without a forward.
    assert!(
        wait_until(OP_TIMEOUT, || {
            runtime
                .block_on(node_c.store.get_namespace(DEFAULT_NAMESPACE))
                .ok()
                .flatten()
                .is_some()
        }),
        "the routed mint's record must become readable on the initiating node's replica"
    );
    let record = runtime
        .block_on(node_a.store.get_namespace(DEFAULT_NAMESPACE))?
        .ok_or("the namespace shard's owner must hold the record it wrote")?;
    assert_eq!(record.name, DEFAULT_NAMESPACE);
    assert_eq!(record.origin, NamespaceOrigin::StartMint);

    // Idempotence survives the route: a second routed mint of the same namespace
    // converges on the one record rather than racing a duplicate into being.
    runtime.block_on(async {
        routed
            .mint_or_gate(&[DEFAULT_NAMESPACE.to_owned()], NamespaceOrigin::StartMint)
            .await
    })?;
    let after = runtime
        .block_on(node_a.store.get_namespace(DEFAULT_NAMESPACE))?
        .ok_or("the record must still exist after an idempotent re-mint")?;
    assert_eq!(after.created_at, record.created_at, "no second create");

    Ok(())
}

/// The minter a clustered boot builds on node C: the production
/// `NamespaceRouting` over C's own store as the namespace-shard resolver, a
/// directory that resolves the namespace's shard to A, and a forwarder that
/// hands the mint to A's OWN minter.
///
/// The forwarder here is in-process rather than gRPC deliberately, so this test
/// proves the ROUTING (decision → the owner executes the whole quorum write →
/// the record replicates back) without a tonic server in the way. The gRPC hop
/// itself — dial, verbatim caller metadata, hop stamp, the owner-side handler's
/// policy — is proved over a real tonic server in
/// `namespace_mint_forward_e2e.rs`. Together they cover the path end to end.
fn routed_minter(node_c: &Node, node_a: &Node) -> NamespaceMinter {
    let routing = NamespaceRouting::new(
        Arc::clone(&node_c.store) as Arc<dyn NamespaceShardResolver>,
        Arc::new(OwnerIsNodeA {
            shard: DEFAULT_NAMESPACE_SHARD,
            address: OWNER_ADDRESS,
        }),
        Arc::new(MintOnOwner {
            owner: NamespaceMinter::new(
                Arc::clone(&node_a.store) as Arc<dyn NamespaceStore>,
                AutoCreate::Open,
            ),
        }),
    );
    NamespaceMinter::new(
        Arc::clone(&node_c.store) as Arc<dyn NamespaceStore>,
        AutoCreate::Open,
    )
    .with_routing(routing)
}

/// The address the directory reports for node A. Never dialed — this test's
/// forwarder is in-process — but carried through the real decision path so the
/// route resolves `Remote` exactly as it does in production.
const OWNER_ADDRESS: SocketAddr =
    SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), 50051);

/// A directory that answers "node A owns the namespace's registry shard".
struct OwnerIsNodeA {
    shard: usize,
    address: SocketAddr,
}

impl MintShardOwners for OwnerIsNodeA {
    fn owner_of(&self, shard: usize) -> MintOwner {
        if shard == self.shard {
            MintOwner::Remote(self.address)
        } else {
            MintOwner::Local
        }
    }
}

/// A forwarder that hands the mint to the OWNER's own minter — what the
/// owner-side `MintNamespace` handler does once the gRPC hop has landed.
struct MintOnOwner {
    owner: NamespaceMinter,
}

#[async_trait::async_trait]
impl MintForwarder for MintOnOwner {
    async fn forward_mint(
        &self,
        _target: SocketAddr,
        _credentials: &MintCredentials,
        namespaces: &[String],
        origin: NamespaceOrigin,
    ) -> Result<(), ForwardMintError> {
        self.owner
            .mint_or_gate(namespaces, origin)
            .await
            .map_err(|error| ForwardMintError::Refused(error.to_wire_error()))
    }
}

/// Umask-independent private temporary directory: the server's private-root
/// validation requires sensitive roots to be `0700`, while
/// `tempfile::tempdir` inherits the process umask.
fn private_tempdir() -> std::io::Result<tempfile::TempDir> {
    let dir = tempfile::tempdir()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(dir)
}
