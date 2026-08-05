//! Which retained version's body a declared-command dispatch runs.
//!
//! Content-hash namespacing keeps every deployed version of a document alive at
//! once, and a run is pinned to exactly one of them for its whole life — that is
//! the mechanism that lets a workflow sleep for three months and wake up as
//! itself. So the question "what command does this action run?" has a single
//! correct answer, and it is not a property of the queue: **it is the body the
//! DISPATCHING RUN's own package version declares.**
//!
//! Resolution used to scan every retained version on the task queue and refuse
//! the dispatch when two of them declared different bodies. That reading makes
//! a second deploy fatal to work already in flight: a run started under version
//! A, still executing, fails terminally at its next bodied activity because
//! version B — which that run will never execute — was deployed with an edited
//! command. Nothing about the run changed. Under the correct reading there is
//! no conflict to refuse: A's runs use A's body, B's runs use B's.
//!
//! When the run's version cannot be named — its handle is gone from the
//! registry, or the registry could not be read — the queue-wide reading is used
//! instead, ambiguity refusal and all. Without the run's identity, choosing a
//! body IS the guess the refusal exists to prevent.

use aion::DeployedWorkerContract;
use aion_package::{ActionBodyContract, ContentHash};

use super::declared_body::DeclaredBodyLookup;
use super::declared_body_ambiguity::DeclaringVersion;

/// Resolve the body `action` runs, given every retained contract on the queue
/// and — when it could be named — the package version the dispatching run is
/// pinned to.
///
/// With `run_version` known, that version's contract is the SOLE authority: if
/// it declares a body, that body runs; if it does not, the action belongs to an
/// out-of-band worker and the dispatch is delegated. Reading a body off any
/// other version would run a command the run's own package never declared.
#[must_use]
pub fn select_declared_body(
    contracts: &[DeployedWorkerContract],
    action: &str,
    run_version: Option<&ContentHash>,
) -> DeclaredBodyLookup {
    match run_version {
        Some(version) => body_of_version(contracts, action, version),
        None => body_across_queue(contracts, action),
    }
}

/// The body `version` itself declares for `action`, or
/// [`DeclaredBodyLookup::None`] when it declares none — including when that
/// version does not serve this queue at all.
fn body_of_version(
    contracts: &[DeployedWorkerContract],
    action: &str,
    version: &ContentHash,
) -> DeclaredBodyLookup {
    for deployed in contracts {
        if &deployed.package_version != version {
            continue;
        }
        for declared in &deployed.contract.actions {
            if declared.name == action
                && let Some(body) = &declared.body
            {
                return DeclaredBodyLookup::Declared(body.clone());
            }
        }
    }
    DeclaredBodyLookup::None
}

/// The queue-wide reading, used only when the run's version is unknown.
///
/// Identical bodies across versions collapse to one; different bodies refuse,
/// carrying the declaring versions so the refusal can name what to retire.
fn body_across_queue(contracts: &[DeployedWorkerContract], action: &str) -> DeclaredBodyLookup {
    let mut bodies: Vec<ActionBodyContract> = Vec::new();
    let mut declaring: Vec<DeclaringVersion> = Vec::new();
    for deployed in contracts {
        for declared in &deployed.contract.actions {
            if declared.name == action
                && let Some(body) = &declared.body
            {
                let index = bodies
                    .iter()
                    .position(|seen| seen == body)
                    .unwrap_or_else(|| {
                        bodies.push(body.clone());
                        bodies.len() - 1
                    });
                declaring.push(DeclaringVersion {
                    content_hash: deployed.package_version.to_string(),
                    workflow_types: deployed.workflow_types.clone(),
                    route_active: deployed.route_active,
                    body: index,
                });
                // One record per retained version. A contract names each action
                // once, and stopping here keeps the version count in the refusal
                // equal to the number of versions an operator would act on.
                break;
            }
        }
    }
    match bodies.len() {
        0 => DeclaredBodyLookup::None,
        1 => match bodies.pop() {
            Some(body) => DeclaredBodyLookup::Declared(body),
            // Unreachable by the length check; classified rather than panicked
            // so a refactor cannot turn this into an abort.
            None => DeclaredBodyLookup::None,
        },
        _ => DeclaredBodyLookup::Ambiguous { declaring },
    }
}

#[cfg(test)]
mod tests {
    use aion::DeployedWorkerContract;
    use aion_package::{ActionBodyContract, ActionContract, ContentHash, WorkerContract};

    use super::super::declared_body::DeclaredBodyLookup;
    use super::select_declared_body;

    fn version(byte: u8) -> ContentHash {
        ContentHash::from_bytes([byte; 32])
    }

    /// One retained version declaring `action` with `command`, or with no body
    /// at all when `command` is `None`.
    fn contract(
        hash: u8,
        workflow_type: &str,
        route_active: bool,
        action: &str,
        command: Option<&str>,
    ) -> DeployedWorkerContract {
        DeployedWorkerContract {
            package_version: version(hash),
            contract: WorkerContract {
                task_queue: "local".to_owned(),
                actions: vec![ActionContract {
                    name: action.to_owned(),
                    input_schema: serde_json::json!({"type": "object"}),
                    output_schema: serde_json::json!({"type": "object"}),
                    node: None,
                    timeout: None,
                    retry: None,
                    advisory: false,
                    agent: false,
                    body: command.map(|command| ActionBodyContract::Run {
                        command: command.to_owned(),
                    }),
                }],
            },
            workflow_types: vec![workflow_type.to_owned()],
            route_active,
        }
    }

    fn command_of(lookup: &DeclaredBodyLookup) -> Option<&str> {
        match lookup {
            DeclaredBodyLookup::Declared(ActionBodyContract::Run { command }) => Some(command),
            _ => None,
        }
    }

    /// The defect this module exists to remove: a run pinned to the OLD version
    /// keeps running the old version's command after a new one is deployed. The
    /// queue-wide reading refused the dispatch outright.
    #[test]
    fn an_in_flight_run_keeps_its_own_versions_body_after_a_redeploy() {
        let retained = [
            contract(0x0a, "sweep", false, "collect", Some("echo old")),
            contract(0x0b, "sweep", true, "collect", Some("echo new")),
        ];

        let old_run = select_declared_body(&retained, "collect", Some(&version(0x0a)));
        assert_eq!(
            command_of(&old_run),
            Some("echo old"),
            "the run pinned to the superseded version must keep its own body: {old_run:?}"
        );

        let new_run = select_declared_body(&retained, "collect", Some(&version(0x0b)));
        assert_eq!(
            command_of(&new_run),
            Some("echo new"),
            "a run on the route-active version gets the new body: {new_run:?}"
        );

        // The control: the SAME retained set, read without the run's identity,
        // is the refusal this fix removes. If this ever stops refusing, the two
        // assertions above stop proving anything about the fix.
        let blind = select_declared_body(&retained, "collect", None);
        assert!(
            matches!(blind, DeclaredBodyLookup::Ambiguous { .. }),
            "without the run's version the queue-wide reading must still refuse: {blind:?}"
        );
    }

    /// A run's own version is the sole authority. Where it declares no body,
    /// the action is a requirement on a worker — even though another retained
    /// version happens to declare one.
    #[test]
    fn a_version_without_a_body_delegates_rather_than_borrowing_one() {
        let retained = [
            contract(0x0a, "sweep", false, "collect", None),
            contract(0x0b, "sweep", true, "collect", Some("echo new")),
        ];
        let lookup = select_declared_body(&retained, "collect", Some(&version(0x0a)));
        assert!(
            matches!(lookup, DeclaredBodyLookup::None),
            "a run whose package declares no body must delegate, not borrow: {lookup:?}"
        );
    }

    /// A version that does not serve this queue at all is not a body source
    /// either — the same rule, reached by a different route.
    #[test]
    fn a_version_absent_from_the_queue_declares_nothing_here() {
        let retained = [contract(0x0b, "sweep", true, "collect", Some("echo new"))];
        let lookup = select_declared_body(&retained, "collect", Some(&version(0x0c)));
        assert!(
            matches!(lookup, DeclaredBodyLookup::None),
            "an unrelated version cannot pick up this queue's body: {lookup:?}"
        );
    }

    /// Agreement is not ambiguity. Two versions declaring the SAME command
    /// resolve without the run's identity, exactly as before.
    #[test]
    fn identical_bodies_across_versions_still_collapse_to_one() {
        let retained = [
            contract(0x0a, "sweep", false, "collect", Some("echo same")),
            contract(0x0b, "sweep", true, "collect", Some("echo same")),
        ];
        let lookup = select_declared_body(&retained, "collect", None);
        assert_eq!(command_of(&lookup), Some("echo same"), "{lookup:?}");
    }

    /// An action nothing declares a body for is delegated under either reading.
    #[test]
    fn a_bodiless_action_is_none_with_or_without_the_run_version() {
        let retained = [contract(0x0a, "sweep", true, "collect", None)];
        assert!(matches!(
            select_declared_body(&retained, "collect", None),
            DeclaredBodyLookup::None
        ));
        assert!(matches!(
            select_declared_body(&retained, "collect", Some(&version(0x0a))),
            DeclaredBodyLookup::None
        ));
    }

    /// The refusal the blind reading produces still carries what an operator
    /// needs — the fix narrows when it fires, it does not hollow it out.
    #[test]
    fn the_blind_refusal_still_names_every_declaring_version() -> Result<(), String> {
        let retained = [
            contract(0x0a, "sweep", false, "collect", Some("echo old")),
            contract(0x0b, "sweep", true, "collect", Some("echo new")),
        ];
        let lookup = select_declared_body(&retained, "collect", None);
        let DeclaredBodyLookup::Ambiguous { declaring } = lookup else {
            return Err(format!(
                "different bodies must refuse when read blind: {lookup:?}"
            ));
        };
        assert_eq!(declaring.len(), 2, "{declaring:?}");
        assert_eq!(declaring[0].content_hash, version(0x0a).to_string());
        assert!(!declaring[0].route_active);
        assert!(declaring[1].route_active);
        assert_ne!(
            declaring[0].body, declaring[1].body,
            "the two versions must be recorded as carrying DIFFERENT bodies"
        );
        Ok(())
    }
}
