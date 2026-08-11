//! The server half of the update check: watching the check's own dispatches
//! complete, and recording what they learned.
//!
//! The check RUNS as any declared body runs — the engine schedules the
//! activity, [`crate::worker::DeclaredCommandDispatcher`] executes the
//! declared `curl` at the server, and the output streams onto the transcript.
//! Nothing in that spine knows what an update check is. This decorator sits
//! immediately above it and adds the one thing the spine cannot: when a
//! dispatch of [`FETCH_ACTION`] on [`UPDATE_CHECK_QUEUE`] completes, parse
//! the fetched index body and record the greatest INSTALLABLE version (not
//! yanked, not a prerelease — `super::index` states the rule) into
//! [`UpdateStatusState`] for `GET /update-status` to serve.
//!
//! # Only the genuine command's output is recorded
//!
//! Names are not identity: any deployed package may declare an action called
//! `fetch_crate_index` on a queue called `update_check`, and its runs would
//! then execute THAT package's body. Before recording, the observer resolves
//! the dispatching run's own body through the same [`DeclaredBodySource`] the
//! executor resolves it through, and records only when it is verbatim
//! [`FETCH_COMMAND`]. A look-alike's output is passed through untouched (the
//! run is not interfered with) and loudly NOT recorded.
//!
//! ## The two resolutions are a known, accepted race (reviewed, #189 r1 m1)
//!
//! The observer resolves the body immediately before the executor resolves it
//! again, from the same source and the same run identity. Both normally read
//! the dispatching run's PINNED package version, so they agree. The window
//! where they can differ: the run's registry handle vanishes or its pinned
//! version is retired BETWEEN the two calls, dropping the second resolution
//! to the queue-wide reading — and queue-wide disagreement refuses the
//! dispatch (`Ambiguous`), so the only reachable divergence is a genuine
//! version retired mid-dispatch while a look-alike is the sole remaining
//! declarer. In that shape the executor runs the look-alike's command while
//! the observer's verdict — formed microseconds earlier against the genuine
//! pinned body — says "record", so ONE look-alike output could be recorded.
//! Exploiting it requires deploy-and-retire authority (an operator lying to
//! themselves) plus winning a race the width of a registry read, and every
//! parse guard still applies to whatever was recorded. Accepted as reviewed;
//! closing it outright means the EXECUTOR reporting the body it actually ran
//! alongside the result — a `DeclaredCommandDispatcher` contract change for
//! the desk, not something to absorb silently here.
//!
//! # Observation never fails the dispatch
//!
//! The activity's recorded result is replay-authoritative and belongs to the
//! workflow. Whatever happens here — an unparseable body, a refused
//! verification — the result is returned unchanged; the failure costs a log
//! line and the recording, never the run.

use std::sync::Arc;

use aion::{ActivityDispatch, ActivityDispatcher};
use aion_package::ActionBodyContract;
use serde::Deserialize;

use super::document::{FETCH_ACTION, FETCH_COMMAND, UPDATE_CHECK_QUEUE};
use super::index::latest_published;
use super::status::{LastCheck, UpdateStatusState};
use crate::worker::{DeclaredBodyLookup, DeclaredBodySource, DispatchingRun};

/// The one field of the declared-body executor's encoded result this
/// observation needs: the command's whole stdout — the fetched index body.
#[derive(Debug, Deserialize)]
struct CommandOutput {
    /// The executed command's stdout, complete.
    stdout: String,
}

/// Dispatcher decorator that records completed update checks.
///
/// Wraps the declared-body executor. Every dispatch that is not the check's
/// own `(queue, action)` address is delegated untouched, without so much as a
/// body lookup.
pub struct UpdateCheckObserver {
    inner: Arc<dyn ActivityDispatcher>,
    bodies: DeclaredBodySource,
    status: UpdateStatusState,
}

impl UpdateCheckObserver {
    /// Wrap `inner`, verifying candidate dispatches through `bodies` and
    /// recording completed checks into `status`.
    #[must_use]
    pub const fn new(
        inner: Arc<dyn ActivityDispatcher>,
        bodies: DeclaredBodySource,
        status: UpdateStatusState,
    ) -> Self {
        Self {
            inner,
            bodies,
            status,
        }
    }

    /// Whether the dispatching run's own resolved body is the genuine check
    /// command. Resolved through the SAME source, with the SAME run identity,
    /// the executor resolves through one call later.
    fn is_genuine_check(&self, request: &ActivityDispatch) -> bool {
        let run = DispatchingRun {
            workflow_id: &request.workflow_id,
            run_id: &request.run_id,
        };
        match self
            .bodies
            .body_for(&request.task_queue, &request.name, run)
        {
            DeclaredBodyLookup::Declared(ActionBodyContract::Run { command }) => {
                if command == FETCH_COMMAND {
                    true
                } else {
                    tracing::warn!(
                        operation = "update_check.observe",
                        workflow_id = %request.workflow_id,
                        run_id = %request.run_id,
                        resolved_command = %command,
                        "an action named like the update check resolved to a DIFFERENT declared \
                         command; its result will pass through but will not be recorded as an \
                         update check"
                    );
                    false
                }
            }
            other => {
                tracing::warn!(
                    operation = "update_check.observe",
                    workflow_id = %request.workflow_id,
                    run_id = %request.run_id,
                    lookup = ?other,
                    "a dispatch on the update check's address did not resolve to a declared \
                     body; nothing will be recorded from it"
                );
                false
            }
        }
    }

    /// Parse a completed genuine check's encoded result and record it.
    fn record_completed_check(&self, encoded: &str) {
        let output: CommandOutput = match serde_json::from_str(encoded) {
            Ok(output) => output,
            Err(error) => {
                tracing::error!(
                    operation = "update_check.observe",
                    %error,
                    "a completed update check's result did not decode as a declared-command \
                     outcome; the check recorded nothing"
                );
                return;
            }
        };
        match latest_published(&output.stdout) {
            Ok(version) => {
                let check = LastCheck {
                    latest_known: version.to_string(),
                    checked_at: chrono::Utc::now(),
                };
                tracing::info!(
                    operation = "update_check.observe",
                    latest_known = %check.latest_known,
                    "update check completed; the latest published aion-cli version was recorded"
                );
                self.status.record(check);
            }
            Err(error) => {
                tracing::error!(
                    operation = "update_check.observe",
                    %error,
                    "a completed update check fetched a body the index parser refused; the \
                     check recorded nothing — read the run's transcript for the raw answer"
                );
            }
        }
    }
}

impl std::fmt::Debug for UpdateCheckObserver {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UpdateCheckObserver")
            .finish_non_exhaustive()
    }
}

impl ActivityDispatcher for UpdateCheckObserver {
    fn dispatch(&self, request: ActivityDispatch) -> Result<String, String> {
        if request.task_queue != UPDATE_CHECK_QUEUE || request.name != FETCH_ACTION {
            return self.inner.dispatch(request);
        }
        // Verified against the request BEFORE it moves into the executor —
        // the same instant the executor itself resolves the body, so the two
        // reads cannot straddle a redeploy any wider than the executor's own.
        let genuine = self.is_genuine_check(&request);
        let result = self.inner.dispatch(request);
        if genuine && let Ok(encoded) = &result {
            self.record_completed_check(encoded);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};

    use aion::{ActivityDispatch, ActivityDispatcher};
    use aion_core::{ActivityId, RunId, WorkflowId};
    use aion_package::ActionBodyContract;

    use super::super::document::{FETCH_ACTION, FETCH_COMMAND, UPDATE_CHECK_QUEUE};
    use super::super::status::UpdateStatusState;
    use super::UpdateCheckObserver;
    use crate::worker::{DeclaredBodies, DeclaredBodyLookup, DeclaredBodySource, DispatchingRun};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    struct RecordingInner {
        reached: Arc<Mutex<Vec<String>>>,
        reply: Result<String, String>,
    }

    impl ActivityDispatcher for RecordingInner {
        fn dispatch(&self, request: ActivityDispatch) -> Result<String, String> {
            match self.reached.lock() {
                Ok(mut names) => names.push(request.name),
                Err(poisoned) => poisoned.into_inner().push(request.name),
            }
            self.reply.clone()
        }
    }

    /// A body reader that records whether it was consulted at all.
    struct CountingBodies {
        lookup: DeclaredBodyLookup,
        consulted: Arc<Mutex<usize>>,
    }

    impl DeclaredBodies for CountingBodies {
        fn body_for(
            &self,
            _task_queue: &str,
            _action: &str,
            _run: DispatchingRun<'_>,
        ) -> DeclaredBodyLookup {
            match self.consulted.lock() {
                Ok(mut count) => *count += 1,
                Err(poisoned) => *poisoned.into_inner() += 1,
            }
            self.lookup.clone()
        }
    }

    fn request(task_queue: &str, name: &str) -> ActivityDispatch {
        ActivityDispatch {
            namespace: "default".to_owned(),
            task_queue: task_queue.to_owned(),
            node: None,
            workflow_id: WorkflowId::new_v4(),
            run_id: RunId::new_v4(),
            activity_id: ActivityId::from_sequence_position(1),
            name: name.to_owned(),
            input: "{}".to_owned(),
            config: "{}".to_owned(),
            attempt: 1,
            labels: BTreeMap::new(),
            advisory: false,
        }
    }

    /// The declared-body executor's encoded result carrying `stdout`.
    fn encoded_result(stdout: &str) -> Result<String, String> {
        serde_json::to_string(&serde_json::json!({
            "exit_code": 0,
            "stdout": stdout,
            "stderr": "",
        }))
        .map_err(|error| error.to_string())
    }

    /// One assembled observer with handles onto everything it touched: the
    /// status slot it may record into, how many body lookups it made, and
    /// which dispatches reached the inner path.
    struct Harness {
        observer: UpdateCheckObserver,
        status: UpdateStatusState,
        consulted: Arc<Mutex<usize>>,
        reached: Arc<Mutex<Vec<String>>>,
    }

    impl Harness {
        fn lookups(&self) -> usize {
            match self.consulted.lock() {
                Ok(count) => *count,
                Err(poisoned) => *poisoned.into_inner(),
            }
        }

        fn reached_names(&self) -> Vec<String> {
            match self.reached.lock() {
                Ok(names) => names.clone(),
                Err(poisoned) => poisoned.into_inner().clone(),
            }
        }
    }

    fn observer(lookup: DeclaredBodyLookup, reply: Result<String, String>) -> Harness {
        let reached = Arc::new(Mutex::new(Vec::new()));
        let consulted = Arc::new(Mutex::new(0));
        let bodies = DeclaredBodySource::default();
        bodies.install(Arc::new(CountingBodies {
            lookup,
            consulted: Arc::clone(&consulted),
        }));
        let status = UpdateStatusState::default();
        let observer = UpdateCheckObserver::new(
            Arc::new(RecordingInner {
                reached: Arc::clone(&reached),
                reply,
            }),
            bodies,
            status.clone(),
        );
        Harness {
            observer,
            status,
            consulted,
            reached,
        }
    }

    fn genuine_lookup() -> DeclaredBodyLookup {
        DeclaredBodyLookup::Declared(ActionBodyContract::Run {
            command: FETCH_COMMAND.to_owned(),
        })
    }

    const INDEX_BODY: &str = concat!(
        "{\"name\":\"aion-cli\",\"vers\":\"0.13.7\",\"yanked\":false}\n",
        "{\"name\":\"aion-cli\",\"vers\":\"0.13.2\",\"yanked\":false}\n",
    );

    #[test]
    fn a_completed_genuine_check_records_the_parsed_version() -> TestResult {
        let harness = observer(genuine_lookup(), encoded_result(INDEX_BODY));
        let before = chrono::Utc::now();
        let result = harness
            .observer
            .dispatch(request(UPDATE_CHECK_QUEUE, FETCH_ACTION));
        assert!(result.is_ok(), "the dispatch result must pass through");

        let check = harness
            .status
            .last()
            .ok_or("a completed check must be recorded")?;
        assert_eq!(check.latest_known, "0.13.7");
        assert!(
            check.checked_at >= before && check.checked_at <= chrono::Utc::now(),
            "checked_at must be the observation moment"
        );
        Ok(())
    }

    /// Names are not identity: the same address resolving to a DIFFERENT
    /// command passes through but records nothing.
    #[test]
    fn a_look_alike_body_is_never_recorded() {
        let lookalike = DeclaredBodyLookup::Declared(ActionBodyContract::Run {
            command: "echo {\"name\":\"aion-cli\",\"vers\":\"9.9.9\",\"yanked\":false}".to_owned(),
        });
        let fake_index = "{\"name\":\"aion-cli\",\"vers\":\"9.9.9\",\"yanked\":false}\n";
        let harness = observer(lookalike, encoded_result(fake_index));
        let result = harness
            .observer
            .dispatch(request(UPDATE_CHECK_QUEUE, FETCH_ACTION));
        assert!(result.is_ok(), "the look-alike's own result is untouched");
        assert_eq!(
            harness.status.last(),
            None,
            "a look-alike must record nothing"
        );
        assert_eq!(
            harness.reached_names(),
            vec![FETCH_ACTION.to_owned()],
            "the run itself must not be interfered with"
        );
    }

    #[test]
    fn a_failed_dispatch_records_nothing() {
        let harness = observer(
            genuine_lookup(),
            Err("retryable:curl: (6) could not resolve host".to_owned()),
        );
        let result = harness
            .observer
            .dispatch(request(UPDATE_CHECK_QUEUE, FETCH_ACTION));
        assert!(result.is_err(), "the failure must pass through");
        assert_eq!(harness.status.last(), None);
    }

    /// An unparseable fetched body refuses the RECORDING, never the run.
    #[test]
    fn an_unparseable_body_records_nothing_and_passes_the_result_through() -> TestResult {
        let harness = observer(
            genuine_lookup(),
            encoded_result("<html>rate limited</html>"),
        );
        let result = harness
            .observer
            .dispatch(request(UPDATE_CHECK_QUEUE, FETCH_ACTION));
        let encoded = result.map_err(|error| format!("the result must pass through: {error}"))?;
        let decoded: serde_json::Value = serde_json::from_str(&encoded)?;
        assert_eq!(decoded["stdout"], "<html>rate limited</html>");
        assert_eq!(harness.status.last(), None);
        Ok(())
    }

    /// Every other dispatch is delegated without even a body lookup.
    #[test]
    fn unrelated_dispatches_pass_through_without_a_body_lookup() {
        let harness = observer(genuine_lookup(), Ok("\"worker-served\"".to_owned()));
        for (queue, action) in [
            ("general", "send_invoice"),
            (UPDATE_CHECK_QUEUE, "some_other_action"),
            ("some_other_queue", FETCH_ACTION),
        ] {
            let result = harness.observer.dispatch(request(queue, action));
            assert_eq!(result, Ok("\"worker-served\"".to_owned()));
        }
        assert_eq!(
            harness.lookups(),
            0,
            "no unrelated dispatch may cost a body lookup"
        );
        assert_eq!(harness.status.last(), None);
        assert_eq!(
            harness.reached_names().len(),
            3,
            "every dispatch must reach the inner path"
        );
    }
}
