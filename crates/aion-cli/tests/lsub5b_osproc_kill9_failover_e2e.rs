//! LSUB-5-B kill-9 reconnect-to-survivor failover (G-1, #112): a REAL
//! multi-OS-process hard-kill liminal-PUSH failover.
//!
//! ## CURRENT STATUS — GREEN, first reached 2026-07-28
//!
//! For its whole existence before that date this gate was RED and had never
//! once fired the `kill -9` it exists to exercise: seven specimens across two
//! machines, zero passes, every one of them dead in SETUP on the unsteered
//! start below. Two independent blockers sat one behind the other, and BOTH had
//! to be cleared before the kill could happen at all:
//!
//! 1. **The namespace fence — production, landed on `main` at `7bd0dbb0`.** An
//!    `aion start` mints its namespace on use; `namespace_key("default")` lands
//!    on shard 1 of this 3-shard cluster, node 0 owns only shard 0, and the
//!    quorum write was fenced — so every start against node 0 returned the SAME
//!    `NotOwner { shard: 1 }`, forever. Diagnosed at the bytes in
//!    `docs/evidence/failover-start-path-investigation.md`; the namespace-mint
//!    routing lane now routes the mint to its owning node.
//! 2. **The fixture's missing queue declaration — this gate's own asset.** With
//!    the fence gone the start reached the ENGINE and was refused
//!    `NO_QUEUE_DECLARATION` 172 times out of 172: the harness-built package
//!    declared its activities with no task queue, which the queue-service
//!    taxonomy refuses because an unserved queue would otherwise wait silently
//!    forever. That refusal was EARNED — the fixture predated the taxonomy and
//!    was never trued. `common/osproc.rs` now declares the four `fan:N`
//!    activities on `default`, the queue this gate's worker actually polls.
//!
//! The `RefusalTally` on the start loop is the instrument that named both
//! blockers, one after the other. It stays.
//!
//! ## What it proves
//!
//! Each deployed `aion server` hosts its OWN liminal listener backed by its OWN
//! per-process `ConnectedWorkerRegistry`. G-1 gave the worker a STATIC
//! candidate-address list and a redial-on-drop loop
//! (`aion_worker::serve_with_redial`) that re-runs the in-band
//! `WorkerRegister`/`Ack`, re-registering in a survivor's registry, so a worker
//! can migrate when the owner of its shard is `kill -9`'d.
//!
//! This test exercises that capability over REAL processes, no in-process
//! shortcuts (the in-process LSUB-5 capstone only achieves liminal failover by
//! SHARING one listener+registry across both nodes — which does not exist across
//! OS processes):
//!
//!   1. boot a 3-node haematite cluster of REAL `aion server` processes (quorum
//!      needs >= 3: a 2-node survivor can never re-elect), each hosting its own
//!      liminal listener with `outbox.transport = liminal`; node i owns shard i;
//!   2. start ONE OS-process liminal worker with ALL THREE liminal addresses as
//!      candidates — it dials node 0 first and registers there;
//!   3. start the `collect_four` fan-out workflow so it lands on shard 0 (node
//!      0's shard); node 0's outbox dispatcher pushes fan-out activities to the
//!      worker over the liminal connection (a dispatch in flight);
//!   4. `kill -9` node 0 (the owner of shard 0 and the worker's connected
//!      server). The worker's connection drops; it redials the NEXT candidate
//!      and RE-REGISTERS in a survivor's registry;
//!   5. the survivors' `ClusterSupervisor`s auto-adopt shard 0 (shipped library
//!      code), and a survivor's outbox dispatcher re-dispatches the remaining
//!      fan-out rows to the re-registered worker;
//!   6. ASSERT on real observables read over a SURVIVOR's gRPC (`aion describe`):
//!      the workflow reaches `Completed`, and its history has EXACTLY ONE
//!      `ActivityCompleted` terminal per ordinal (4 total) — exactly-once, the
//!      `record_fan_out_completion` dedup absorbing the owner's lost wave plus
//!      the survivor's redelivery. No sleeps gate the assertions.
//!
//! All failover intelligence lives in shipped library code (the SS-5b
//! `ClusterSupervisor` + `Engine::adopt_shards` + the worker's `serve_with_redial`
//! redial loop); this test only boots processes, drives start/describe over the
//! CLI, `kill -9`s, and polls real observables.
//!
//! ## Running
//!
//! Runtime-gated, never `#[ignore]`d (slow, spawns 4 OS processes + a 3-node cluster):
//! ```text
//! AION_RUN_COSTLY_TESTS=1 cargo test -p aion-cli \
//!   --features haematite-backend,liminal-transport \
//!   --test lsub5b_osproc_kill9_failover_e2e -- --nocapture
//! ```

#![cfg(all(unix, feature = "haematite-backend", feature = "liminal-transport"))]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::Duration;

use serde_json::Value;

#[path = "common/aion_cli.rs"]
mod aion_cli;
#[path = "common/cluster_config.rs"]
mod cluster_config;
#[path = "common/costly_gate.rs"]
mod costly_gate;
#[path = "common/osproc.rs"]
mod osproc;

use aion_cli::{RefusalTally, check_exactly_once, describe, status_of, try_start};
use cluster_config::write_node_config;
use osproc::{
    TestError, build_worker_binary, log_redirect, log_tail, preserve_on_failure, reap,
    reserve_port, wait_for_liveness, wait_until, write_package_archive,
};

/// Whether `path` exists and its contents contain `needle` (a server log line).
fn file_contains(path: &Path, needle: &str) -> bool {
    std::fs::read_to_string(path).is_ok_and(|contents| contents.contains(needle))
}

/// Number of nodes: haematite `quorum_size(3) = 2`, so a 3-node cluster losing
/// one leaves a 2-node majority that CAN re-elect and adopt the dead shard. A
/// 2-node cluster could not (a lone survivor never forms a majority).
const NODE_COUNT: usize = 3;
/// The library log line a survivor's `ClusterSupervisor::tick` emits on
/// auto-adoption — the load-bearing proof the LIBRARY did the failover.
const ADOPT_LINE: &str = "adopted a downed peer's shards (SS-5b auto-failover)";
/// Stagger between node boots: sidesteps the documented beamr simultaneous-
/// connect boot race (a benign `SimultaneousAbort` mis-mapped to fatal). Pure
/// launch ordering; no failover logic.
const BOOT_STAGGER: Duration = Duration::from_secs(3);
/// Deadline for the cluster to fully boot.
const CLUSTER_BOOT_DEADLINE: Duration = Duration::from_secs(40);
/// Deadline for the post-kill failover to complete the workflow.
const FAILOVER_DEADLINE: Duration = Duration::from_secs(90);
/// Deadline for a `start` to land on node 0's shard (it is retried until it does).
const START_DEADLINE: Duration = Duration::from_secs(30);
/// Per-activity delay forced into the worker so the fan-out stays in flight long
/// enough for the kill to interrupt it (4 ordinals served serially through the
/// single push serve loop, so the total in-flight window is ~4x this).
const FAN_ACTIVITY_DELAY_MS: &str = "1500";

/// A booted node: its child process, ports, and log path.
struct Node {
    child: Child,
    http_port: u16,
    grpc_port: u16,
    liminal_port: u16,
    log_path: PathBuf,
}

/// Spawn one `aion server` OS process from `config`, logging to `log_path`.
fn boot_node(config: &Path, log_path: &Path, ports: (u16, u16, u16)) -> Result<Node, TestError> {
    let (stdout, stderr) = log_redirect(log_path)?;
    let child = Command::new(env!("CARGO_BIN_EXE_aion"))
        .args(["server", "--config", &config.to_string_lossy()])
        .env(
            "AION_HOME",
            std::env::temp_dir().join(format!("aion-e2e-home-{}", std::process::id())),
        )
        .env("RUST_LOG", "info")
        .stdout(stdout)
        .stderr(stderr)
        .spawn()?;
    Ok(Node {
        child,
        http_port: ports.0,
        grpc_port: ports.1,
        liminal_port: ports.2,
        log_path: log_path.to_path_buf(),
    })
}

#[test]
fn osproc_kill9_liminal_failover_completes_exactly_once() -> Result<(), TestError> {
    // Runtime-gated, never `#[ignore]`d. This is the CROSS-NODE failover
    // proof — one of the two tests that establish the durability claim — so
    // a run that skips it must say so rather than leave a tally that reads
    // identically to a run that proved it.
    if costly_gate::skip_unless_opted_in(
        "a kill -9 of a node in a 3-node cluster failing over and completing the work EXACTLY ONCE",
    ) {
        return Ok(());
    }
    let temp = tempfile::tempdir()?;
    run_kill9_failover(temp.path())
        .map_err(|error| preserve_on_failure(temp.path(), "lsub5b-kill9-failover", &error))
}

/// The scenario proper, run inside `dir`. Split out from the `#[test]` so EVERY
/// failure — including a boot death before the cluster is even up — flows
/// through the caller's log preservation before the tempdir is deleted.
fn run_kill9_failover(dir: &Path) -> Result<(), TestError> {
    let package = write_package_archive(dir)?;
    let worker_binary = build_worker_binary()?;

    // Reserve EVERY port up front — including each node's cluster distribution
    // bind port — so configs can cross-reference peers and nothing collides with
    // a port the host already owns. The distribution ports used to be hardcoded
    // `7000 + index`, and 127.0.0.1:7000 is macOS AirPlay Receiver's port: a peer
    // dialing node 0's distribution address before node 0 had bound it reached
    // AirTunes, spoke beamr at an HTTP server, and died with "beamr distribution
    // connection could not be established". Nothing in this gate may assume a
    // fixed port is free.
    let mut ports: Vec<(u16, u16, u16)> = Vec::with_capacity(NODE_COUNT);
    let mut bind_ports: Vec<u16> = Vec::with_capacity(NODE_COUNT);
    for _ in 0..NODE_COUNT {
        ports.push((reserve_port()?, reserve_port()?, reserve_port()?));
        bind_ports.push(reserve_port()?);
    }

    let mut nodes = boot_cluster(dir, &package, &ports, &bind_ports)?;

    let worker = match spawn_worker(dir, &worker_binary, &nodes) {
        Ok(worker) => worker,
        Err(error) => {
            reap_all(nodes);
            return Err(error);
        }
    };

    let result = drive_failover(&mut nodes, &ports);
    reap(worker);
    reap_all(nodes);
    result
}

/// Boot the N-node cluster, wait for every node's liveness probe, and confirm
/// each survivor commissioned its cluster supervisor (the auto-adoption driver).
/// Reaps any already-booted nodes on failure so a partial boot never leaks.
fn boot_cluster(
    dir: &Path,
    package: &Path,
    ports: &[(u16, u16, u16)],
    bind_ports: &[u16],
) -> Result<Vec<Node>, TestError> {
    let mut nodes: Vec<Node> = Vec::with_capacity(ports.len());
    for index in 0..ports.len() {
        let config = match write_node_config(dir, index, package, ports, bind_ports) {
            Ok(config) => config,
            Err(error) => {
                reap_all(nodes);
                return Err(error);
            }
        };
        let log_path = dir.join(format!("node{index}.log"));
        match boot_node(&config, &log_path, ports[index]) {
            Ok(node) => nodes.push(node),
            Err(error) => {
                reap_all(nodes);
                return Err(error);
            }
        }
        if index + 1 < ports.len() {
            std::thread::sleep(BOOT_STAGGER);
        }
    }

    for index in 0..nodes.len() {
        let http_port = nodes[index].http_port;
        let log_path = nodes[index].log_path.clone();
        if let Err(error) = wait_for_liveness(&mut nodes[index].child, http_port, &log_path) {
            reap_all(nodes);
            return Err(format!("node {index} failed to boot: {error}").into());
        }
    }
    let supervisors_up = wait_until(CLUSTER_BOOT_DEADLINE, || nodes.iter().all(commissioned));
    if supervisors_up {
        return Ok(nodes);
    }
    let mut still_waiting = String::new();
    for node in nodes.iter().filter(|node| !commissioned(node)) {
        let _ = write!(
            still_waiting,
            "\n{}:\n{}",
            node.log_path.display(),
            log_tail(&node.log_path)
        );
    }
    reap_all(nodes);
    Err(format!(
        "cluster supervisors did not commission within {CLUSTER_BOOT_DEADLINE:?}; \
         still waiting on:{still_waiting}"
    )
    .into())
}

/// Whether `node`'s log carries its cluster supervisor's commissioning line.
fn commissioned(node: &Node) -> bool {
    file_contains(&node.log_path, "commissioned")
}

/// Spawn ONE worker with ALL nodes' liminal addresses as candidates and wait for
/// its first registration (the readiness file). It dials node 0 first; on a drop
/// it migrates to the next candidate, re-registering there.
fn spawn_worker(dir: &Path, worker_binary: &Path, nodes: &[Node]) -> Result<Child, TestError> {
    let ready_file = dir.join("worker.ready");
    let mut worker_args: Vec<String> = Vec::new();
    for node in nodes {
        worker_args.push("--address".to_owned());
        worker_args.push(format!("127.0.0.1:{}", node.liminal_port));
    }
    worker_args.push("--identity".to_owned());
    worker_args.push("kill9-failover-worker".to_owned());
    worker_args.push("--ready-file".to_owned());
    worker_args.push(ready_file.to_string_lossy().into_owned());
    let worker_log_path = dir.join("worker.log");
    let (worker_log, worker_log_err) = log_redirect(&worker_log_path)?;
    let worker = Command::new(worker_binary)
        .args(&worker_args)
        .env("RUST_LOG", "info")
        // Slow each fan-out activity so the fan-out cannot finish before the
        // owner is killed — the survivor must re-dispatch the pending ordinals to
        // the redialed worker. Without this the worker might serve all 4 before
        // the kill and the failover would not be genuinely exercised.
        .env("LIMINAL_FAN_DELAY_MS", FAN_ACTIVITY_DELAY_MS)
        .stdout(worker_log)
        .stderr(worker_log_err)
        .spawn()?;
    if wait_until(FAILOVER_DEADLINE, || ready_file.exists()) {
        Ok(worker)
    } else {
        reap(worker);
        Err(format!(
            "worker never registered against node 0's liminal listener within \
             {FAILOVER_DEADLINE:?}; tail of {}:\n{}",
            worker_log_path.display(),
            log_tail(&worker_log_path),
        )
        .into())
    }
}

/// Drive the in-flight start, the kill-9, and the survivor-side exactly-once
/// assertion. `nodes[0]` is the owner that gets killed; the remainder survive.
fn drive_failover(nodes: &mut Vec<Node>, ports: &[(u16, u16, u16)]) -> Result<(), TestError> {
    // Start collect_four so it lands on shard 0 (node 0's shard).
    //
    // This loop's premise — "an unsteered start to node 0 lands on a locally-owned
    // shard or is fenced; retry until it lands" — held two constant refusals for
    // this gate's whole red history, and neither was a lottery the deadline could
    // win (see the module header and
    // docs/evidence/failover-start-path-investigation.md): the namespace-registry
    // fence, until production learned to route the mint to its owning node, and
    // then the fixture's undeclared task queue, until the archive this harness
    // writes declared one. Both are gone; the retry is now what it always claimed
    // to be — tolerance for a cluster that is still settling, not a wait on a
    // refusal that can never change.
    let node0_grpc = format!("http://127.0.0.1:{}", ports[0].1);
    let mut workflow_id = String::new();
    // Tally EVERY refusal, not just the last: one refusal repeated N times is a
    // deterministic routing decision that retrying can never change, while a
    // spread is a race. The two have opposite fixes.
    let mut refusals = RefusalTally::default();
    let landed = wait_until(START_DEADLINE, || match try_start(&node0_grpc) {
        Ok(id) => {
            workflow_id = id;
            true
        }
        Err(refusal) => {
            refusals.record(refusal);
            false
        }
    });
    if !landed {
        return Err(format!(
            "collect_four never landed on node 0's shard within {START_DEADLINE:?} against \
             {node0_grpc}; what it was told: {}",
            refusals.report()
        )
        .into());
    }

    // Confirm the fan-out is genuinely in flight (Running, NOT yet Completed)
    // before the kill, so the kill interrupts a real in-flight dispatch — the
    // per-activity delay keeps the fan-out from finishing first. If it raced to
    // Completed before we could observe Running, the failover was not exercised,
    // so that is a test failure, not a silent pass.
    let mut last_status = String::new();
    let in_flight = wait_until(FAILOVER_DEADLINE, || {
        let description = describe(&node0_grpc, &workflow_id);
        match description.as_ref().and_then(status_of) {
            Some(status) => {
                status.clone_into(&mut last_status);
                status == "Running"
            }
            None => false,
        }
    });
    if !in_flight {
        return Err(format!(
            "collect_four never observed Running on node 0 before the kill \
             (last status: {last_status:?}); the kill would not have interrupted an \
             in-flight dispatch"
        )
        .into());
    }

    // kill -9 node 0: the owner of shard 0 and the worker's connected server.
    let owner = nodes.remove(0);
    let owner_pid = owner.child.id();
    let _: Result<std::process::ExitStatus, std::io::Error> = Command::new("kill")
        .args(["-9", &owner_pid.to_string()])
        .status();
    reap(owner.child);

    // A survivor's ClusterSupervisor must auto-adopt shard 0 (library failover).
    let adopted = wait_until(FAILOVER_DEADLINE, || {
        nodes
            .iter()
            .any(|node| file_contains(&node.log_path, ADOPT_LINE))
    });
    if !adopted {
        return Err("no survivor logged the SS-5b auto-adoption after the kill".into());
    }

    // THE PROOF: read over a SURVIVOR's gRPC. The fan-out must reach Completed
    // (the worker redialed a survivor, re-registered, and the survivor
    // re-dispatched the remaining ordinals) with EXACTLY ONE ActivityCompleted
    // terminal per ordinal — exactly-once across the kill.
    let survivor_grpc: Vec<String> = nodes
        .iter()
        .map(|node| format!("http://127.0.0.1:{}", node.grpc_port))
        .collect();
    let mut final_description: Option<Value> = None;
    let completed = wait_until(FAILOVER_DEADLINE, || {
        for endpoint in &survivor_grpc {
            if let Some(description) = describe(endpoint, &workflow_id)
                && status_of(&description) == Some("Completed")
            {
                final_description = Some(description);
                return true;
            }
        }
        false
    });

    if completed {
        check_exactly_once(final_description.as_ref())
    } else {
        Err("fan-out did not reach Completed on any survivor after the kill-9 failover".into())
    }
}

/// Reap every node so a failed assertion never leaks a cluster.
fn reap_all(nodes: Vec<Node>) {
    for node in nodes {
        reap(node.child);
    }
}
