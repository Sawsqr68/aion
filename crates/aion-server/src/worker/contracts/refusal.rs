//! Rendering the operator-facing worker-admission refusal.
//!
//! A refused worker retries forever and looks merely asleep. Everything an
//! operator needs to end that must therefore be IN the refusal text, because
//! the refusal is all either side gets: the server logs it at WARN and the
//! worker prints the identical string back. On 2026-07-30 the text named the
//! package, action and field but not WHY that package was still being held
//! against the worker, nor what to do about it — and the answer (a stale,
//! non-routed version nothing could reach) took an hour to find.
//!
//! On 2026-07-30 it then failed a second time, differently: a worker advertising
//! 8 activity-type NAMES and 0 typed CONTRACTS was refused with action
//! `assistant` reported `<missing>` — in a log record that listed `assistant`
//! among the advertised types on the same line. Both statements were true of
//! DIFFERENT sets and the record said which of neither, so the operator could
//! not resolve it. [`advertisement_summary`] is that reconciliation: the refusal
//! now states both advertised counts, says which set admission compared, and
//! names the actions that carry a name but no contract.
//!
//! The rendered form, verbatim from `a_worker_that_cannot_serve_the_routed_
//! version_is_still_refused` (hashes shortened here only for legibility):
//!
//! ```text
//! WORKER_CONTRACT_MISMATCH: worker build `stale-worker` queue `payments`
//! refused. checked 1 reachable deployed package version (1 routes new starts);
//! ignored 1 unreachable version that nothing can dispatch under: `0c578458…`
//! (workflow `admission_drift`). worker advertised 1 activity-type name and 1
//! typed contract, and admission compares CONTRACTS. this connection advertises
//! no node, so it is held only to unpinned actions. 1 disagreement. [1/1]
//! package `854bace6…`
//! (workflow `admission_drift`, held because it currently routes new starts)
//! action `charge` field `input_schema.properties.amount.type` expected
//! "string" but worker advertised "integer". REMEDY: rebuild and redeploy this
//! worker from the same source as the deployed package(s) named above, so it
//! advertises their action shapes; a version that routes new starts cannot be
//! unloaded — supersede it by deploying a newer version of that workflow, or
//! re-point the route at another loaded version with POST /deploy/route; the
//! ignored version(s) can be cleared with POST /deploy/unload
//! {"workflow_type":"admission_drift","content_hash":"0c578458…"} — that is
//! hygiene only and will NOT change this refusal
//! ```

use std::collections::BTreeSet;

use aion::{AdmissionReason, QueueAdmission, ReasonCensus};

use super::{Disagreement, WorkerAdvertisement};

/// Renders the full diagnosis: what was checked, what disagreed, what to do.
pub(super) fn render_mismatch(
    admission: &QueueAdmission,
    node: Option<&str>,
    advertised: WorkerAdvertisement<'_>,
    disagreements: &[Disagreement],
) -> String {
    let mut sections = vec![
        checked_summary(admission),
        advertisement_summary(advertised),
    ];
    if let Some(node) = node {
        sections.push(format!(
            "this connection advertises node `{node}`, so it is held only to the actions a \
             dispatch on that node can reach"
        ));
    } else {
        sections.push(
            "this connection advertises no node, so it is held only to unpinned actions".to_owned(),
        );
    }
    sections.push(pluralized(disagreements.len(), "disagreement"));
    let total = disagreements.len();
    for (index, disagreement) in disagreements.iter().enumerate() {
        sections.push(render_disagreement(index + 1, total, disagreement));
    }
    sections.push(remedy(admission, advertised, disagreements));
    sections.join(". ")
}

/// What the worker announced, in both forms, and the gap between them.
///
/// This clause exists because the two forms are routinely confused, including by
/// this system's own logs: a worker announces activity-type NAMES (what the
/// dispatcher selects on) and typed CONTRACTS (what admission compares), and a
/// name with no contract behind it is selectable but not admissible. A refusal
/// that reports an action `<missing>` while the worker plainly advertises its
/// name is not wrong — it is answering about the other set — but it is
/// unresolvable by the person reading it unless the refusal says so. So it says
/// so, and it names the actions in the gap.
fn advertisement_summary(advertised: WorkerAdvertisement<'_>) -> String {
    let mut clauses = vec![format!(
        "worker advertised {} and {}, and admission compares CONTRACTS",
        pluralized(advertised.activity_types.len(), "activity-type name"),
        pluralized(advertised.contracts.len(), "typed contract"),
    )];
    let gap = advertised.names_without_contracts();
    if !gap.is_empty() {
        clauses.push(format!(
            "{} advertised by name with NO contract: {} — a name makes a worker \
             selectable for dispatch, a contract makes it admissible",
            pluralized(gap.len(), "action"),
            backticked(&gap),
        ));
    }
    clauses.join("; ")
}

/// The header: what admission held the worker to, and what it deliberately did
/// not. Naming the ignored versions is not decoration — an operator staring at
/// a queue with five deployed versions needs to know which ones were in play.
fn checked_summary(admission: &QueueAdmission) -> String {
    let census = admission.reason_census();
    let mut clauses = vec![format!(
        "checked {} ({})",
        pluralized(
            admission.required.len(),
            "reachable deployed package version"
        ),
        census_clause(census)
    )];
    if !admission.unreachable.is_empty() {
        let ignored = admission
            .unreachable
            .iter()
            .map(|contract| {
                format!(
                    "`{}` ({})",
                    contract.package_version,
                    workflow_clause(&contract.workflow_types)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        clauses.push(format!(
            "ignored {} that nothing can dispatch under: {ignored}",
            pluralized(admission.unreachable.len(), "unreachable version")
        ));
    }
    clauses.join("; ")
}

fn census_clause(census: ReasonCensus) -> String {
    let mut clauses = Vec::new();
    if census.route_active > 0 {
        clauses.push(format!("{} routes new starts", census.route_active));
    }
    if census.live_workflow > 0 {
        clauses.push(format!("{} has a live workflow run", census.live_workflow));
    }
    if census.start_in_flight > 0 {
        clauses.push(format!(
            "{} has a workflow start in flight",
            census.start_in_flight
        ));
    }
    if clauses.is_empty() {
        // Reachable-but-empty cannot produce a disagreement, so this arm is
        // only ever hit by a caller rendering an empty admission. Say so
        // plainly rather than emitting an empty parenthesis.
        return "none reachable".to_owned();
    }
    clauses.join(", ")
}

fn render_disagreement(index: usize, total: usize, disagreement: &Disagreement) -> String {
    format!(
        "[{index}/{total}] package `{}` ({}, held because {}) action `{}` field `{}` expected {} but worker advertised {}",
        disagreement.diff.package_version,
        workflow_clause(&disagreement.workflow_types),
        disagreement.reason.explanation(),
        disagreement.diff.action,
        disagreement.diff.field,
        rendered_value(disagreement.diff.expected.as_ref()),
        rendered_value(disagreement.diff.advertised.as_ref()),
    )
}

/// The operator's next move, chosen by what the disagreeing versions ARE.
///
/// Unloading is deliberately NOT offered for any version named in a
/// disagreement. The three reasons a version is demanded are exactly the three
/// reasons an unload refuses it — route-active becomes
/// `EngineError::RouteActive`, a live run becomes `PinHolder::LiveRun`, and an
/// in-flight start becomes `PinHolder::InFlightStart` — so telling an operator
/// under pressure to unload it would send them straight into a second refusal.
/// What CAN be unloaded is the ignored set, and the message says plainly that
/// doing so will not change this refusal.
fn remedy(
    admission: &QueueAdmission,
    advertised: WorkerAdvertisement<'_>,
    disagreements: &[Disagreement],
) -> String {
    let mut clauses = vec![
        "REMEDY: rebuild and redeploy this worker from the same source as the deployed \
         package(s) named above, so it advertises their action shapes"
            .to_owned(),
    ];
    let gap = advertised.names_without_contracts();
    if !gap.is_empty() {
        // Naming the mechanism, not one SDK's method name: every worker SDK has
        // to solve this, and the schemas are declared in the package the queue
        // is serving, so they are always knowable.
        clauses.push(format!(
            "the {} listed above with no contract must announce an input and output schema \
             at registration, not just a name — the shapes are the ones the deployed \
             package declares",
            pluralized(gap.len(), "action")
        ));
    }
    let reasons = disagreements
        .iter()
        .map(|disagreement| disagreement.reason)
        .collect::<BTreeSet<_>>();
    if reasons.contains(&AdmissionReason::RouteActive) {
        clauses.push(
            "a version that routes new starts cannot be unloaded — supersede it by deploying a \
             newer version of that workflow, or re-point the route at another loaded version with \
             POST /deploy/route"
                .to_owned(),
        );
    }
    if reasons.contains(&AdmissionReason::LiveWorkflow)
        || reasons.contains(&AdmissionReason::StartInFlight)
    {
        clauses.push(
            "a version a live run is pinned to cannot be unloaded either — it stops binding \
             workers only once that run reaches a terminal state"
                .to_owned(),
        );
    }
    if !admission.unreachable.is_empty() {
        let bodies = admission
            .unreachable
            .iter()
            .map(|contract| {
                unload_body(
                    &contract.package_version.to_string(),
                    &contract.workflow_types,
                )
            })
            .collect::<Vec<_>>()
            .join(" ");
        clauses.push(format!(
            "the ignored version(s) can be cleared with POST /deploy/unload {bodies} — that is \
             hygiene only and will NOT change this refusal"
        ));
    }
    clauses.join("; ")
}

/// The exact request body for the ONLY removal path that exists — there is no
/// `aion undeploy` verb, and an operator who has to work that out under
/// pressure is an operator the message failed.
fn unload_body(version: &str, workflow_types: &[String]) -> String {
    let workflow_type = workflow_types
        .first()
        .map_or("<workflow_type>", String::as_str);
    format!(r#"{{"workflow_type":"{workflow_type}","content_hash":"{version}"}}"#)
}

fn workflow_clause(workflow_types: &[String]) -> String {
    match workflow_types {
        [] => "no workflow type".to_owned(),
        [single] => format!("workflow `{single}`"),
        many => format!(
            "workflows {}",
            many.iter()
                .map(|name| format!("`{name}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn backticked(names: &[String]) -> String {
    names
        .iter()
        .map(|name| format!("`{name}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn pluralized(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("{count} {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

fn rendered_value(value: Option<&serde_json::Value>) -> String {
    value.map_or_else(|| "<missing>".to_owned(), serde_json::Value::to_string)
}
