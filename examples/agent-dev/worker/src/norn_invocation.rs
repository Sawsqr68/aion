//! The norn command-line surface this worker drives, and the operator advice
//! attached to the one norn refusal an operator cannot otherwise get past.
//!
//! The argv lives here rather than inline in the composition root so it can be
//! asserted directly. A flag that is only ever observed through a live agent
//! run is untested in the case that matters — which is exactly how
//! `--account` came to be missing from the harness invocation once already
//! (see `main.rs`).

/// Norn's flag for resuming a session whose exact provider transcript can no
/// longer be replayed.
pub const ALLOW_DEGRADED_SESSION_FLAG: &str = "--allow-degraded-session";

/// Substrings identifying norn's degraded-session refusal. Both must appear:
/// the condition (`fresh-epoch projection`) and the demand (`explicit resume
/// approval is required`). Requiring both keeps the advice off unrelated
/// storage errors that happen to mention an epoch.
const DEGRADED_SESSION_MARKERS: [&str; 2] = [
    "fresh-epoch projection",
    "explicit resume approval is required",
];

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
/// `allow_degraded_session` appends [`ALLOW_DEGRADED_SESSION_FLAG`]. It is an
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

/// Whether `detail` is norn's degraded-session refusal.
#[must_use]
pub fn is_degraded_session_refusal(detail: &str) -> bool {
    DEGRADED_SESSION_MARKERS
        .iter()
        .all(|marker| detail.contains(marker))
}

/// The operator advice for a failed agent round, or `None` when the failure is
/// not the degraded-session refusal or the opt-in was already given.
///
/// A durable session can park for weeks — one real run parked 28 days and the
/// engine resumed it perfectly — and in that time the agent provider can move
/// to a new epoch. When it has, norn refuses to resume without explicit
/// approval, and the operator sees a raw storage string with no route out.
/// This is the route out, in the same style as the `--account` refusal: name
/// the flag, and state the tradeoff in one line so the decision is informed.
///
/// The advice is withheld once the opt-in has been given: at that point norn
/// has refused for some other reason and pointing at a flag already in the
/// argv would send the operator in a circle.
#[must_use]
pub fn degraded_session_advice(detail: &str, allow_degraded_session: bool) -> Option<String> {
    if allow_degraded_session || !is_degraded_session_refusal(detail) {
        return None;
    }
    Some(format!(
        "{detail}\n\nOPERATOR ACTION: this session outlived the agent provider's \
         epoch, so norn will not resume it without approval. Restart this worker \
         with --norn-allow-degraded-session to allow it. The tradeoff: the session \
         resumes from a fresh provider epoch because its exact provider transcript \
         cannot be replayed, so provider-side fidelity is reduced — the conversation \
         itself is retained. Without the flag the round fails on every retry and the \
         workflow fails with it."
    ))
}

#[cfg(test)]
#[path = "norn_invocation_tests.rs"]
mod tests;
