//! The norn command-line surface this worker drives.
//!
//! The argv lives here rather than inline in the composition root so it can be
//! asserted directly. A flag that is only ever observed through a live agent
//! run is untested in the case that matters — which is exactly how
//! `--account` came to be missing from the harness invocation once already
//! (see `main.rs`).
//!
//! The degraded-session recognition and operator advice that used to live
//! here were promoted into `aion_integration_norn::advice` (so the built-in
//! `aion worker agent` verb teaches the same way); this worker now composes
//! [`aion_integration_norn::AdvisingHarness`] with its own opt-in spelling,
//! [`DEGRADED_SESSION_OPT_IN`].

use aion_integration_norn::ALLOW_DEGRADED_SESSION_FLAG;

/// THIS worker's spelling of the degraded-resume opt-in — the flag the
/// operator restarts `agent-dev-worker` with. It is what the promoted advice
/// names in the failure detail, and what `main.rs` parses; one constant so
/// the advice can never name a flag the parser does not accept.
pub const DEGRADED_SESSION_OPT_IN: &str = "--norn-allow-degraded-session";

/// Build the ordered norn argument list for one composed harness.
///
/// `run_clone` is the per-run workspace template — it is passed BOTH as
/// `--workspace-root` (which confines the file tools) and as `-C` (which sets
/// the tool-execution cwd); without the latter the agent's shell commands run
/// in the worker's own directory, outside the clone.
///
/// `--session-id` plus `--resume-if-exists` is what makes a multi-round
/// session one continuous conversation, and it is also what makes
/// `norn_account` mandatory: norn refuses to resume an existing OAuth session
/// without an account alias.
///
/// `norn_model` is an operator pass-through with NO default of this worker's:
/// `Some` appends `--model <value>` and `None` defers to norn's own model
/// selection. Which model answers a session is the operator's call, and
/// inventing one here would silently override every deployment that never
/// asked.
///
/// `allow_degraded_session` appends
/// [`ALLOW_DEGRADED_SESSION_FLAG`]. It is an
/// explicit operator opt-in and has NO default: resuming degraded means the
/// exact provider transcript could not be replayed, which is a real fidelity
/// tradeoff and not this worker's to assume on an operator's behalf.
#[must_use]
pub fn norn_args(
    run_clone: &str,
    norn_account: &str,
    norn_model: Option<&str>,
    allow_degraded_session: bool,
) -> Vec<String> {
    let mut args = vec![
        "--workspace-root".to_owned(),
        run_clone.to_owned(),
        "-C".to_owned(),
        run_clone.to_owned(),
        "--session-id".to_owned(),
        "{workflow_id}-{activity_type}".to_owned(),
        "--resume-if-exists".to_owned(),
        "--account".to_owned(),
        norn_account.to_owned(),
        "--fast".to_owned(),
    ];
    if let Some(model) = norn_model {
        args.push("--model".to_owned());
        args.push(model.to_owned());
    }
    if allow_degraded_session {
        args.push(ALLOW_DEGRADED_SESSION_FLAG.to_owned());
    }
    args
}

#[cfg(test)]
#[path = "norn_invocation_tests.rs"]
mod tests;
