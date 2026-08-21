//! The kill-9 failover gate's cluster TOML: one `aion server` config per node,
//! wired into an N-node haematite cluster with the liminal outbox transport.
//!
//! Split out of the gate itself so the scenario file stays readable: this module
//! answers only "what does node `index`'s config say", including which peer each
//! node is allowed to adopt shards from.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Error type used across the OS-process harness and its tests (the same alias
/// `common/osproc.rs` exports; this module is included alongside it — including
/// `osproc` again here would compile a SECOND copy of it into the test binary).
type TestError = Box<dyn std::error::Error>;

/// The node index that is killed (it always owns shard 0 and hosts the worker's
/// first connection).
const OWNER_INDEX: usize = 0;
/// The node index designated as the SOLE adopter of the killed owner's shard.
/// Pinning the adopter (rather than letting both survivors race the shard
/// election) makes the single worker's redial deterministic: it migrates to its
/// next candidate, which is this node, and lands on the genuine new owner. Every
/// node still lists every peer for the distribution mesh + quorum; only the
/// killed owner's `owned_shards` is declared exclusively here.
const ADOPTER_INDEX: usize = 1;

/// Emit one node's TOML config: an N-node haematite cluster with the liminal
/// outbox transport. Node `index` owns shard `index`. The killed owner
/// ([`OWNER_INDEX`]) is declared as an adoptable peer ONLY in the designated
/// adopter's config ([`ADOPTER_INDEX`]); every other node lists it as a peer for
/// the mesh/quorum but with no owned shards, so exactly one survivor adopts it.
pub fn write_node_config(
    dir: &Path,
    index: usize,
    package: &Path,
    ports: &[(u16, u16, u16)],
    bind_ports: &[u16],
) -> Result<PathBuf, TestError> {
    let (http_port, grpc_port, liminal_port) = ports[index];
    let data_dir = dir.join(format!("data{index}"));
    let bind_port = bind_ports[index];

    let mut members = String::new();
    for node in 0..ports.len() {
        let _ = write!(members, "\"node-{node}@127.0.0.1\", ");
    }
    let members = members.trim_end_matches(", ");

    let mut peers = String::new();
    for (peer, &(_, peer_grpc, _)) in ports.iter().enumerate() {
        if peer == index {
            continue;
        }
        // The killed owner's shard is adopted ONLY by the designated adopter, so
        // the redial lands deterministically on the genuine new owner. Other
        // peers keep their own shard as their declared adoptable set.
        let owned = if peer == OWNER_INDEX && index != ADOPTER_INDEX {
            String::new()
        } else {
            peer.to_string()
        };
        let _ = write!(
            peers,
            "\n[[store.cluster.peers]]\nname = \"node-{peer}@127.0.0.1\"\n\
             address = \"127.0.0.1:{peer_bind}\"\n\
             grpc_address = \"127.0.0.1:{peer_grpc}\"\n\
             owned_shards = [{owned}]\n",
            peer_bind = bind_ports[peer],
        );
    }

    let config = format!(
        r#"workflow_packages = ["{package}"]

[server]
listen_address = "127.0.0.1:{http_port}"
grpc_address = "127.0.0.1:{grpc_port}"

[store]
backend = "haematite"
# Required, no defaults. Unlimited: this failover gate measures kill-9 shard
# adoption, never caching; patience/cadence mirror the estate's other e2e nodes.
node_cache_budget = "unlimited"
lock_acquisition_patience_ms = 250
lock_acquisition_retry_cadence_ms = 5
data_dir = "{data_dir}"
shard_count = {node_count}
owned_shards = [{index}]

[store.cluster]
node_id = "node-{index}@127.0.0.1"
bind_address = "127.0.0.1:{bind_port}"
members = [{members}]
failover_poll_interval_ms = 500
failover_confirmations = 3
{peers}
[runtime]
query_timeout_ms = 10000

[namespaces]
default = "default"

[websocket]
event_broadcast_capacity = 1024
cluster_broadcast_capacity = 64

[observability]
# Required, no default: the transcript drain's flush policy.
max_batch_events = 64
max_batch_hold_ms = 0

[outbox]
enabled = true
poll_interval_ms = 20
batch_size = 16
max_attempts = 5
backoff_base_ms = 50
backoff_multiplier = 2
backoff_max_ms = 1000
transport = "liminal"
liminal_listen_address = "127.0.0.1:{liminal_port}"
"#,
        package = package.display(),
        data_dir = data_dir.display(),
        node_count = ports.len(),
    );
    let path = dir.join(format!("node{index}.toml"));
    std::fs::write(&path, config)?;
    Ok(path)
}
