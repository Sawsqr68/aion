//! An optional end-to-end run against a REAL ACP agent binary.
//!
//! The in-crate protocol tests (`src/wire_tests.rs`) drive a real [`aion_integrations::jsonrpc`]
//! connection against a scripted fake agent, which is the strongest shape available without a live
//! binary. This test adds the one thing that cannot fake: a real agent process, its real
//! `initialize` advertisement, and its real streamed turn.
//!
//! It is **runtime-gated, never `#[ignore]`d**: no agent binary is universally available, so with
//! nothing configured the test emits a skip line and passes. Configure it with:
//!
//! ```text
//! AION_ACP_AGENT=npx \
//! AION_ACP_AGENT_ARGS='@google/gemini-cli --acp' \
//! AION_ACP_CWD=/absolute/path/to/a/workspace \
//! cargo test -p aion-integration-acp --test acp_live_agent
//! ```
//!
//! `AION_ACP_AGENT` is the executable, `AION_ACP_AGENT_ARGS` its whitespace-separated arguments
//! (there is no standard ACP launch flag, so they must be given), and `AION_ACP_CWD` the absolute
//! session root — ACP requires an absolute `cwd`, and this test will not invent one.
//!
//! `AION_ACP_EXIT_GRACE_SECS` overrides how long the agent may take to exit once its stdin closes.
//! Aimed at an agent that **never** exits, with a grace short enough to observe, it makes this test
//! the proof that the grace-elapsed kill path works: the terminal result is bounded by the grace, so
//! an adapter that waited on a deaf agent instead of killing it would fail here rather than hang a
//! worker in production.
//!
//! # Reproducing the two proofs without a third-party agent
//!
//! `tests/fixtures/` carries two conforming ACP agents, so both findings this test exists for stay
//! **repeatable** rather than proven once. They need only a Python 3 interpreter (stdlib alone, 3.9
//! and later), which is why they are fixtures behind the same runtime gate rather than a dependency
//! of the default suite.
//!
//! Both commands below name the interpreter by **absolute path** rather than as `python3`. That is
//! deliberate: `AION_ACP_AGENT` is spawned as given, so a broken or wrongly-architected `python3`
//! earlier on `PATH` fails the spawn and the failure reads as an adapter fault. Substitute whichever
//! interpreter your box actually runs.
//!
//! `persistent_agent.py` answers the turn, prints a non-JSON `Debugger listening on ws://…` banner
//! on stdout mid-turn the way a node-launched agent does, and then **keeps running** — the exact
//! shape that hung every driver-run turn before the pump dropped its transcript sender at the
//! terminal Response:
//!
//! ```text
//! AION_ACP_AGENT=/usr/bin/python3 \
//! AION_ACP_AGENT_ARGS=crates/aion-integration-acp/tests/fixtures/persistent_agent.py \
//! AION_ACP_CWD="$PWD" \
//! cargo test -p aion-integration-acp --test acp_live_agent
//! ```
//!
//! `deaf_agent.py` answers the turn and then **ignores stdin EOF**, so only the grace-elapsed kill
//! can end it. Point the test at it with a short grace and the run completes at the grace boundary
//! instead of hanging:
//!
//! ```text
//! AION_ACP_AGENT=/usr/bin/python3 \
//! AION_ACP_AGENT_ARGS=crates/aion-integration-acp/tests/fixtures/deaf_agent.py \
//! AION_ACP_CWD="$PWD" AION_ACP_EXIT_GRACE_SECS=2 \
//! cargo test -p aion-integration-acp --test acp_live_agent
//! ```

use std::time::Duration;

use aion_integration_acp::{AcpHarness, PermissionPolicy};
use aion_integrations::{
    ActivityEventKind, ActivityId, AgentHarness, AgentRunSpec, AgentSession, ContentType, Payload,
    RunId, WorkflowId,
};
use futures::StreamExt;

/// The executable to launch as the ACP agent.
const AGENT_ENV: &str = "AION_ACP_AGENT";
/// Its whitespace-separated arguments.
const ARGS_ENV: &str = "AION_ACP_AGENT_ARGS";
/// The absolute session working directory.
const CWD_ENV: &str = "AION_ACP_CWD";

/// How long the whole live turn may take. A real agent calls a model, so this is generous; it exists
/// only so a wedged agent fails the test instead of hanging the suite forever.
const TURN_TIMEOUT: Duration = Duration::from_secs(300);

/// The prompt the live agent is asked to answer: deterministic, cheap, and needing no tools.
const PROMPT: &str = "Reply with exactly the word: ablative";

/// How long the live agent may take to exit on its own once its stdin closes, before it is killed.
///
/// A real value an operator could choose, not a default the harness supplies: the harness has none.
const EXIT_GRACE: Duration = Duration::from_secs(5);

/// The env var overriding [`EXIT_GRACE`], in whole seconds.
///
/// It exists so the **grace-elapsed** path is reachable: an agent that ignores stdin EOF must be
/// killed rather than waited on forever, and proving that needs a grace short enough to observe.
const EXIT_GRACE_ENV: &str = "AION_ACP_EXIT_GRACE_SECS";

/// How much longer than the grace the terminal result may take to arrive.
///
/// The guarantee under test is that a deaf agent cannot hang the worker, so the bound is a function
/// of the configured grace rather than a flat timeout: with an agent that never exits, `wait_result`
/// must still return once the grace elapses and the process is killed.
const SHUTDOWN_MARGIN: Duration = Duration::from_secs(20);

/// The configured exit grace: [`EXIT_GRACE_ENV`] when it names whole seconds, else [`EXIT_GRACE`].
///
/// A value that is present but unreadable is an operator error, not a reason to fall back silently.
fn exit_grace() -> Result<Duration, String> {
    match std::env::var(EXIT_GRACE_ENV) {
        Err(_) => Ok(EXIT_GRACE),
        Ok(raw) if raw.trim().is_empty() => Ok(EXIT_GRACE),
        Ok(raw) => raw
            .trim()
            .parse::<u64>()
            .map(Duration::from_secs)
            .map_err(|source| {
                format!("{EXIT_GRACE_ENV} must be whole seconds, got `{raw}`: {source}")
            }),
    }
}

type TestResult = Result<(), String>;

/// The configured agent command, arguments, and session root — or `None` when unconfigured.
fn configured() -> Option<(String, Vec<String>, String)> {
    let agent = std::env::var(AGENT_ENV).ok()?;
    let cwd = std::env::var(CWD_ENV).ok()?;
    let args = std::env::var(ARGS_ENV)
        .unwrap_or_default()
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    Some((agent, args, cwd))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_real_acp_agent_completes_a_turn() -> TestResult {
    let Some((agent, args, cwd)) = configured() else {
        // Runtime-gated, not `#[ignore]`d: with no agent configured there is nothing to prove and
        // nothing to fail.
        tracing::info!(
            "skipping the live ACP test: set {AGENT_ENV} (and {CWD_ENV}, {ARGS_ENV}) to run it"
        );
        return Ok(());
    };
    if !std::path::Path::new(&cwd).is_absolute() {
        return Err(format!("{CWD_ENV} must be an absolute path, got `{cwd}`"));
    }

    let grace = exit_grace()?;

    // `Deny` deliberately: a live test must not let a real agent take a gated action on a real disk.
    let harness = AcpHarness::new(agent.clone(), args, cwd, PermissionPolicy::Deny, grace);
    let spec = AgentRunSpec::new(
        WorkflowId::new_v4(),
        RunId::new_v4(),
        ActivityId::from_sequence_position(1),
        1,
        "acp-live",
        Payload::new(
            ContentType::Json,
            serde_json::to_vec(PROMPT)
                .map_err(|source| format!("the prompt must encode: {source}"))?,
        ),
    );

    let mut session = harness
        .start(spec)
        .await
        .map_err(|error| format!("failed to start `{agent}` as an ACP agent: {error}"))?;

    // The negotiated set comes from a real handshake against a real binary.
    assert!(
        !session.capabilities().is_empty(),
        "a negotiated ACP session advertises cancel"
    );

    // Drain the live transcript to END-OF-STREAM, exactly as the platform's agent driver does: it
    // pumps `events.next()` and takes the result only when the stream yields `None`. A real ACP agent
    // is a persistent stdio server that does NOT exit after answering, so this is the shape that
    // proves a real turn can finish — breaking out at the Stop event would pass against an adapter
    // that never closes the stream and hangs every driver-run turn forever.
    let mut events = session.events();
    let mut saw_event = false;
    let mut saw_stop = false;
    let drained = tokio::time::timeout(TURN_TIMEOUT, async {
        while let Some(event) = events.next().await {
            saw_event = true;
            if matches!(event.kind, ActivityEventKind::Stop { .. }) {
                saw_stop = true;
            }
        }
    })
    .await;
    drained.map_err(|_elapsed| {
        "the live transcript never ended: the adapter must close the event stream once the turn is \
         answered, without waiting for the agent process to exit"
            .to_owned()
    })?;
    assert!(saw_event, "at least one live ActivityEvent must arrive");
    assert!(saw_stop, "the turn must emit a terminal Stop event");
    drop(events);

    // Bounded by the GRACE, not by the turn timeout: `wait_result` shuts the agent down on its way
    // out, so an agent that ignores stdin EOF is only survivable if the grace elapses and the
    // process is killed. Point this at an agent that never exits and the bound is the assertion.
    let payload = tokio::time::timeout(grace + SHUTDOWN_MARGIN, session.wait_result())
        .await
        .map_err(|_elapsed| {
            format!(
                "the terminal result did not arrive within the {}s exit grace plus {}s: an agent \
                 that ignores stdin EOF must be killed once the grace elapses, never waited on",
                grace.as_secs(),
                SHUTDOWN_MARGIN.as_secs()
            )
        })?
        .map_err(|error| format!("the live turn did not complete: {error}"))?;
    let decoded = payload
        .to_json()
        .map_err(|error| format!("the terminal payload must be JSON: {error}"))?;
    assert_eq!(
        decoded["stopReason"],
        serde_json::json!("end_turn"),
        "a completed turn reports end_turn: {decoded}"
    );
    let text = decoded["text"]
        .as_str()
        .ok_or_else(|| format!("the payload carries the agent's text: {decoded}"))?;
    assert!(
        !text.is_empty(),
        "the agent's answer must reach the terminal payload: {decoded}"
    );
    Ok(())
}
