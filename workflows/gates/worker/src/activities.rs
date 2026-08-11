//! Typed implementations of the eight `lane_gates` activities.
//!
//! Every activity delegates to the committed `workflows/gates/gate-runner.zsh`
//! — the ONE place the gate commands live — derived from the measured
//! workspace itself (`<workspace>/workflows/gates/gate-runner.zsh`), never
//! accepted as a free input: the manifest names that script by path and
//! sha256, so the record and the execution point at the same bytes.
//!
//! The runner is spawned as `zsh -f <runner> …` with the environment cleared
//! to the operator-named PATH alone. `-f` suppresses zsh startup files
//! (`~/.zshenv` included), so no host dotfile can prepend directories or
//! export credentials into the gate environment underneath the record — the
//! r1 review measured exactly that happening without it.
//!
//! The runner's contract splits verdict from instrument: a RED GATE is data
//! (the runner exits 0 and its stdout is a `GateResult` carrying the gate
//! command's real exit code and coverage-floor verdict), while a BROKEN
//! MEASUREMENT (the runner exiting non-zero: absent tool, unenterable
//! workspace, command-not-found, unwritable log) is a terminal
//! [`ActivityFailure`] — an unmeasured gate is never reported as a result, in
//! either direction.
//!
//! CANCELLATION IS REAL: every spawn goes through the SDK's
//! [`run_cancellable_command`], which puts the runner (and every gate process
//! under it) in a fresh process group and, when the activity is cancelled,
//! kills and reaps the whole group before reporting. A cancelled gate is a
//! CANCELLED MEASUREMENT — a terminal failure naming itself — never a result.

use std::process::Output;

use aion_worker::{
    ActivityContext, ActivityFailure, CancellableCommandOutput, CommandOutputObserver,
    CommandStream, run_cancellable_command,
};
use serde::{Deserialize, Serialize};
use tokio::process::Command;

/// The operator-named execution environment every runner invocation gets.
#[derive(Clone, Debug)]
pub struct RunnerEnvironment {
    /// The PATH the gates resolve their tools from — named by the operator at
    /// worker start (`--path`), recorded into the manifest by the runner.
    pub tool_path: String,
}

/// Shared input shape for every gate action.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GateInput {
    /// Workspace directory the gate measures; the runner script is
    /// `<workspace>/workflows/gates/gate-runner.zsh` by construction.
    pub workspace: String,
    /// The running workflow's durable id — names the retained-log directory
    /// `<workspace>/gate-logs/gates-runs/<workflow_id>/`. It is the WORKFLOW
    /// id, not the run id: the document passes `workflow.id`, and the
    /// manifest derivation rebuilds the same path from the same value.
    pub workflow_id: String,
}

/// One gate's bounded, factual result — the shape the workflow routes on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GateResult {
    /// Stable human-readable gate name.
    pub gate: String,
    /// Real gate-command exit code (128+signal for a signal-killed process).
    pub exit_code: i32,
    /// True exactly when the gate command exited zero AND the leg's coverage
    /// floor held.
    pub passed: bool,
    /// At most the final 40 lines of combined output, with any coverage-floor
    /// verdict at its head; the full log is retained on the measuring box.
    pub output_tail: String,
}

/// Input for the manifest-derivation action.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ManifestInput {
    /// The running workflow's durable id — what the generator describes.
    pub workflow_id: String,
    /// gRPC endpoint the generator dials for `aion describe`.
    pub server_grpc: String,
    /// HTTP base the generator reads `GET /build` from.
    pub server_http: String,
    /// Where the manifest artifact is written.
    pub manifest_path: String,
    /// How many gate legs the workflow dispatched — the derivation refuses a
    /// history holding any other number.
    ///
    /// The schema override advertises the bare `{"type":"integer"}` the AWL
    /// contract's `Int` derives: input compatibility is contravariant, so a
    /// schemars `format`-stamped advertisement would be refused at admission
    /// as narrower than what the workflow may send.
    #[schemars(schema_with = "bare_integer")]
    pub expected_legs: i32,
    /// The measured checkout, recorded into the manifest by commit and full
    /// porcelain status.
    pub workspace: String,
}

/// The manifest generator's receipt, recorded into history.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ManifestReceipt {
    /// Where the manifest landed.
    pub path: String,
    /// The run id the manifest was derived for, read from the run's own history.
    pub run_id: String,
    /// sha256 over the gate output tails, in dispatch order.
    pub tails_digest: String,
    /// How many gate legs the derivation found recorded.
    pub legs_recorded: i32,
}

/// Bare `{"type":"integer"}`, exactly what the AWL contract derives for `Int`.
fn bare_integer(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({"type": "integer"})
}

/// The runner script's location inside the measured workspace.
fn runner_path(workspace: &str) -> String {
    format!("{workspace}/workflows/gates/gate-runner.zsh")
}

/// Streams every runner output line into the worker's own log at debug level,
/// so a long gate is observable from the worker without touching the record.
struct TracingObserver;

impl CommandOutputObserver for TracingObserver {
    fn on_line(&self, stream: CommandStream, line: &str) {
        tracing::debug!(stream = stream.name(), line, "gate runner output");
    }
}

/// Spawn the runner under process-group containment with the cleared,
/// operator-named environment, honouring cancellation.
///
/// # Errors
///
/// Terminal [`ActivityFailure`] when the runner cannot be spawned or
/// contained, when the activity is cancelled (the whole process group is
/// killed and reaped first), or when the runner itself exits non-zero — a
/// broken measurement, per its contract. Never a fabricated result.
async fn spawn_runner(
    environment: &RunnerEnvironment,
    workspace: &str,
    arguments: &[&str],
    action: &str,
    context: &ActivityContext,
) -> Result<Output, ActivityFailure> {
    let runner = runner_path(workspace);
    let mut command = Command::new("zsh");
    command
        .arg("-f")
        .arg(&runner)
        .args(arguments)
        .env_clear()
        .env("PATH", &environment.tool_path)
        .stdin(std::process::Stdio::null());

    let outcome = run_cancellable_command(command, context.cancelled(), &TracingObserver)
        .await
        .map_err(|error| {
            ActivityFailure::terminal(format!(
                "action `{action}`: the gate runner could not be run under containment \
                 (`zsh -f {runner}`): {error}"
            ))
        })?;
    let output = match outcome {
        CancellableCommandOutput::Completed(output) => output,
        CancellableCommandOutput::Cancelled => {
            return Err(ActivityFailure::terminal(format!(
                "action `{action}`: CANCELLED MEASUREMENT — the activity was cancelled, the \
                 runner's whole process group was killed and confirmed gone, and no result is \
                 reported"
            )));
        }
    };
    if !output.status.success() {
        return Err(ActivityFailure::terminal(format!(
            "action `{action}`: the gate runner itself failed (status {status}) — the \
             measurement is broken and no result is reported. runner stderr:\n{stderr}",
            status = output.status,
            stderr = String::from_utf8_lossy(&output.stderr),
        )));
    }
    Ok(output)
}

/// Decode the runner's stdout as the declared result type.
fn decode<T: serde::de::DeserializeOwned>(
    action: &str,
    output: &Output,
) -> Result<T, ActivityFailure> {
    serde_json::from_slice(&output.stdout).map_err(|error| {
        ActivityFailure::terminal(format!(
            "action `{action}`: the gate runner printed output that is not the declared result \
             type: {error}. stdout was:\n{stdout}",
            stdout = String::from_utf8_lossy(&output.stdout),
        ))
    })
}

/// Run one gate subcommand of the runner.
///
/// # Errors
///
/// See [`spawn_runner`]; additionally terminal when the output does not
/// decode as one `GateResult`.
pub async fn run_gate(
    environment: &RunnerEnvironment,
    subcommand: &str,
    input: GateInput,
    context: &ActivityContext,
) -> Result<GateResult, ActivityFailure> {
    let GateInput {
        workspace,
        workflow_id,
    } = input;
    let output = spawn_runner(
        environment,
        &workspace,
        &[subcommand, workspace.as_str(), workflow_id.as_str()],
        subcommand,
        context,
    )
    .await?;
    decode(subcommand, &output)
}

/// Derive the committed manifest from the run's own recorded history.
///
/// # Errors
///
/// See [`spawn_runner`]; additionally terminal when the runner refuses the
/// derivation (leg-count mismatch, unreachable server, unreadable workspace)
/// or the output does not decode as one `ManifestReceipt`.
pub async fn write_manifest(
    environment: &RunnerEnvironment,
    input: ManifestInput,
    context: &ActivityContext,
) -> Result<ManifestReceipt, ActivityFailure> {
    let ManifestInput {
        workflow_id,
        server_grpc,
        server_http,
        manifest_path,
        expected_legs,
        workspace,
    } = input;
    let expected_legs = expected_legs.to_string();
    let output = spawn_runner(
        environment,
        &workspace,
        &[
            "manifest",
            workflow_id.as_str(),
            server_grpc.as_str(),
            server_http.as_str(),
            manifest_path.as_str(),
            expected_legs.as_str(),
            workspace.as_str(),
        ],
        "write_manifest",
        context,
    )
    .await?;
    decode("write_manifest", &output)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    use aion_worker::ActivityContext;

    use super::{GateInput, RunnerEnvironment, run_gate};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    /// The committed runner script, two directories up from this crate.
    fn committed_runner() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../gate-runner.zsh")
    }

    /// A context whose cancellation the test drives by hand.
    fn test_context() -> (ActivityContext, aion_worker::ActivityCancellationHandle) {
        ActivityContext::new(
            aion_core::WorkflowId::new(uuid::Uuid::new_v4()),
            aion_core::RunId::new(uuid::Uuid::new_v4()),
            aion_core::ActivityId::from_sequence_position(0),
            1,
        )
    }

    /// The ambient PATH, for tests that need the real toolchain.
    fn ambient_environment() -> RunnerEnvironment {
        RunnerEnvironment {
            tool_path: std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".to_owned()),
        }
    }

    /// A fixture workspace carrying the COMMITTED runner at the derived
    /// location, so the activities' `<workspace>/workflows/gates/…` path
    /// resolves against real bytes.
    ///
    /// It is a real git repository with a real commit, because every gate leg
    /// now captures the HEAD it is about to measure (M2) and a workspace whose
    /// commit cannot be read is a broken measurement by design.
    fn fixture_workspace(
        misformatted: bool,
    ) -> Result<tempfile::TempDir, Box<dyn std::error::Error>> {
        let fixture = tempfile::tempdir()?;
        let gates_dir = fixture.path().join("workflows/gates");
        fs::create_dir_all(&gates_dir)?;
        fs::copy(committed_runner(), gates_dir.join("gate-runner.zsh"))?;
        fs::create_dir(fixture.path().join("src"))?;
        fs::write(
            fixture.path().join("Cargo.toml"),
            "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[workspace]\n",
        )?;
        let body = if misformatted {
            "pub fn red( ) ->   bool { true }\n"
        } else {
            "pub fn green() -> bool {\n    true\n}\n"
        };
        fs::write(fixture.path().join("src/lib.rs"), body)?;
        git_init_fixture(&fixture.path().to_string_lossy())?;
        Ok(fixture)
    }

    /// Write a stand-in program and make it runnable.
    fn write_executable(path: &Path, body: &str) -> TestResult {
        fs::write(path, body)?;
        let mut permissions = fs::metadata(path)?.permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
        fs::set_permissions(path, permissions)?;
        Ok(())
    }

    /// The fixture repository's HEAD — what a leg's identity capture records.
    fn fixture_head(workspace: &str) -> Result<String, Box<dyn std::error::Error>> {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(workspace)
            .args(["rev-parse", "HEAD"])
            .output()?;
        if !output.status.success() {
            return Err("git rev-parse failed in the fixture".into());
        }
        Ok(String::from_utf8(output.stdout)?.trim().to_owned())
    }

    /// sha256 of a file, computed by the test's own invocation — never read
    /// back out of the artifact under assertion.
    fn sha256_of(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
        let output = std::process::Command::new("shasum")
            .args(["-a", "256", "--"])
            .arg(path)
            .output()?;
        if !output.status.success() {
            return Err("shasum failed".into());
        }
        let text = String::from_utf8(output.stdout)?;
        let digest = text
            .split_whitespace()
            .next()
            .ok_or("shasum printed no digest")?
            .to_owned();
        Ok(format!("sha256:{digest}"))
    }

    /// Stand in for the per-leg identity observations a real run's gate legs
    /// write before they measure anything, so the manifest derivation can be
    /// exercised without running seven cargo gates.
    fn write_identity_observations(
        workspace: &Path,
        workflow_id: &str,
        observations: &[(&str, &str, &str)],
    ) -> TestResult {
        let dir = workspace
            .join("gate-logs/gates-runs")
            .join(workflow_id)
            .join("identity");
        fs::create_dir_all(&dir)?;
        for (gate, head_commit, runner_sha256) in observations {
            let body = serde_json::json!({
                "gate": gate,
                "observed_at": "2026-08-11T00:00:00+1000",
                "head_commit": head_commit,
                "runner_path": workspace.join("workflows/gates/gate-runner.zsh").display().to_string(),
                "runner_sha256": runner_sha256,
            });
            fs::write(dir.join(format!("{gate}.json")), serde_json::to_vec(&body)?)?;
        }
        Ok(())
    }

    #[tokio::test]
    async fn a_red_gate_is_data_not_a_failure() -> TestResult {
        let fixture = fixture_workspace(true)?;
        let (context, _handle) = test_context();
        let result = run_gate(
            &ambient_environment(),
            "fmt",
            GateInput {
                workspace: fixture.path().to_string_lossy().into_owned(),
                workflow_id: "test-red-arm".to_owned(),
            },
            &context,
        )
        .await
        .map_err(|failure| format!("measurement broke: {failure:?}"))?;
        assert_eq!(result.gate, "fmt");
        assert_ne!(result.exit_code, 0, "misformatted fixture must be red");
        assert!(!result.passed);
        // M7: the full log is retained at the derived per-run home.
        let log = fixture
            .path()
            .join("gate-logs/gates-runs/test-red-arm/fmt.log");
        assert!(log.is_file(), "the full leg log must be retained");
        // M2: the leg captured the tree and the instrument it measured,
        // BEFORE it measured them.
        let observation = fixture
            .path()
            .join("gate-logs/gates-runs/test-red-arm/identity/fmt.json");
        let observed: serde_json::Value = serde_json::from_slice(&fs::read(&observation)?)?;
        let workspace = fixture.path().to_string_lossy().into_owned();
        assert_eq!(
            observed["head_commit"],
            serde_json::Value::String(fixture_head(&workspace)?),
            "the leg records the commit it measured"
        );
        assert_eq!(
            observed["runner_sha256"],
            serde_json::Value::String(sha256_of(Path::new(&super::runner_path(&workspace)))?),
            "the leg records the instrument that ran it"
        );
        Ok(())
    }

    /// M-r2-4: a cargo whose `fmt` subcommand is not installed is a BROKEN
    /// MEASUREMENT, not a red gate. `cargo <missing-subcommand>` exits **101**
    /// — the same code a genuine `-D warnings` clippy finding uses — so the
    /// 127 guard cannot see it and only an explicit `--version` probe can.
    /// Without this, a host without rustfmt or clippy writes a red leg
    /// indistinguishable from a real finding into the record.
    #[tokio::test]
    async fn a_missing_cargo_subcommand_is_a_broken_measurement_not_a_red_gate() -> TestResult {
        let fixture = fixture_workspace(false)?;
        let workspace = fixture.path().to_string_lossy().into_owned();
        let ambient = std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".to_owned());

        // A cargo that exists and answers `--version` but has no `fmt`
        // subcommand — byte-for-byte how a toolchain without rustfmt behaves.
        let without_rustfmt = fixture.path().join("cargo-without-rustfmt");
        fs::create_dir(&without_rustfmt)?;
        write_executable(
            &without_rustfmt.join("cargo"),
            "#!/bin/zsh\n\
             if [[ \"$1\" == --version ]]; then print -r -- 'cargo 1.0.0 (stub)'; exit 0; fi\n\
             print -u2 -r -- \"error: no such command: \\`$1\\`\"\n\
             exit 101\n",
        )?;
        let (context, _handle) = test_context();
        let result = run_gate(
            &RunnerEnvironment {
                tool_path: format!("{}:{ambient}", without_rustfmt.display()),
            },
            "fmt",
            GateInput {
                workspace: workspace.clone(),
                workflow_id: "test-no-rustfmt".to_owned(),
            },
            &context,
        )
        .await;
        let failure = match result {
            Err(failure) => format!("{failure:?}"),
            Ok(result) => {
                return Err(format!(
                    "a cargo without rustfmt must never yield a gate result, got: {result:?}"
                )
                .into());
            }
        };
        assert!(
            failure.contains("rustfmt"),
            "the failure must name the absent component: {failure}"
        );

        // Vacuity control: the SAME stub with the subcommand present yields a
        // gate RESULT again, red, carrying the command's real exit code — so
        // the refusal above is about the missing subcommand and not about the
        // stub being a stub.
        let with_rustfmt = fixture.path().join("cargo-with-rustfmt");
        fs::create_dir(&with_rustfmt)?;
        write_executable(
            &with_rustfmt.join("cargo"),
            "#!/bin/zsh\n\
             if [[ \"$1\" == --version ]]; then print -r -- 'cargo 1.0.0 (stub)'; exit 0; fi\n\
             if [[ \"$1\" == fmt && \"$2\" == --version ]]; then print -r -- 'rustfmt 1.0.0 (stub)'; exit 0; fi\n\
             print -r -- 'Diff in stub'\n\
             exit 1\n",
        )?;
        let (context, _handle) = test_context();
        let result = run_gate(
            &RunnerEnvironment {
                tool_path: format!("{}:{ambient}", with_rustfmt.display()),
            },
            "fmt",
            GateInput {
                workspace,
                workflow_id: "test-with-rustfmt".to_owned(),
            },
            &context,
        )
        .await
        .map_err(|failure| format!("the control must measure, not break: {failure:?}"))?;
        assert_eq!(result.exit_code, 1);
        assert!(!result.passed);
        Ok(())
    }

    /// B1, inverted from r1: a workspace that does not exist is a BROKEN
    /// MEASUREMENT — a terminal failure — never a gate result. The old test
    /// locked the opposite in.
    #[tokio::test]
    async fn a_missing_workspace_is_a_broken_measurement_not_a_result() -> TestResult {
        let fixture = tempfile::tempdir()?;
        let missing = fixture.path().join("missing");
        let (context, _handle) = test_context();
        let result = run_gate(
            &ambient_environment(),
            "fmt",
            GateInput {
                workspace: missing.to_string_lossy().into_owned(),
                workflow_id: "test-missing-ws".to_owned(),
            },
            &context,
        )
        .await;
        assert!(
            result.is_err(),
            "a workspace the measurement cannot enter must never yield a gate result, got: {result:?}"
        );
        assert!(
            !missing.exists(),
            "probing a missing workspace must not create it"
        );
        Ok(())
    }

    /// B1: a gate whose own tool is absent from the operator-named PATH is a
    /// BROKEN MEASUREMENT, not a red result. `/usr/bin:/bin` genuinely lacks
    /// cargo, and `zsh -f` keeps it that way (no `~/.zshenv` re-widening —
    /// the exact false-green fixture the r1 review caught).
    #[tokio::test]
    async fn an_absent_gate_tool_is_a_broken_measurement_not_a_result() -> TestResult {
        let fixture = fixture_workspace(false)?;
        let bare = RunnerEnvironment {
            tool_path: "/usr/bin:/bin".to_owned(),
        };
        let (context, _handle) = test_context();
        let result = run_gate(
            &bare,
            "fmt",
            GateInput {
                workspace: fixture.path().to_string_lossy().into_owned(),
                workflow_id: "test-no-cargo".to_owned(),
            },
            &context,
        )
        .await;
        let failure = match result {
            Err(failure) => format!("{failure:?}"),
            Ok(result) => {
                return Err(format!(
                    "an absent cargo must never yield a gate result, got: {result:?}"
                )
                .into());
            }
        };
        assert!(
            failure.contains("cargo"),
            "the failure names the absent tool: {failure}"
        );
        Ok(())
    }

    /// r2 minors 2 and 3: a coverage floor that cannot be EVALUATED is a
    /// BROKEN MEASUREMENT — a terminal failure — not a red leg. "We could not
    /// verify this leg" must never be recorded as "this leg failed", which
    /// attributes the fault to the code under test; and the error path that
    /// carries it (`floor="$($floor_fn "$log")" || return 70`) was dead code
    /// while both floor functions returned zero unconditionally.
    ///
    /// Driven with a stub `cargo` that emits a properly paired log and a stub
    /// `awk` that cannot run — the scan failing, with everything else honest.
    #[tokio::test]
    async fn a_coverage_floor_that_cannot_be_evaluated_is_a_broken_measurement() -> TestResult {
        let fixture = fixture_workspace(false)?;
        let workspace = fixture.path().to_string_lossy().into_owned();
        let ambient = std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".to_owned());
        let paired_log = "#!/bin/zsh\n\
             if [[ \"$1\" == --version ]]; then print -r -- 'cargo 1.0.0 (stub)'; exit 0; fi\n\
             print -r -- '     Running unittests src/lib.rs (target/debug/deps/fixture)'\n\
             print -r -- 'running 1 test'\n\
             print -r -- 'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out'\n\
             exit 0\n";

        let broken = fixture.path().join("stub-broken-awk");
        fs::create_dir(&broken)?;
        write_executable(&broken.join("cargo"), paired_log)?;
        write_executable(
            &broken.join("awk"),
            "#!/bin/zsh\nprint -u2 -r -- 'awk: cannot open the log'\nexit 1\n",
        )?;
        let (context, _handle) = test_context();
        let result = run_gate(
            &RunnerEnvironment {
                tool_path: format!("{}:{ambient}", broken.display()),
            },
            "test",
            GateInput {
                workspace: workspace.clone(),
                workflow_id: "test-broken-floor".to_owned(),
            },
            &context,
        )
        .await;
        let failure = match result {
            Err(failure) => format!("{failure:?}"),
            Ok(result) => {
                return Err(format!(
                    "an unevaluable coverage floor must never yield a gate result, got: {result:?}"
                )
                .into());
            }
        };
        assert!(
            failure.contains("coverage"),
            "the failure must name the unverifiable coverage: {failure}"
        );

        // Control: the same stub cargo with a WORKING awk yields a gate
        // result again — so the refusal above is about the broken scan and
        // not about the stub.
        let working = fixture.path().join("stub-working-awk");
        fs::create_dir(&working)?;
        write_executable(&working.join("cargo"), paired_log)?;
        let (context, _handle) = test_context();
        let result = run_gate(
            &RunnerEnvironment {
                tool_path: format!("{}:{ambient}", working.display()),
            },
            "test",
            GateInput {
                workspace,
                workflow_id: "test-working-floor".to_owned(),
            },
            &context,
        )
        .await
        .map_err(|failure| format!("the control must measure, not break: {failure:?}"))?;
        assert_eq!(result.exit_code, 0);
        assert!(
            result.passed,
            "a paired, non-empty run passes the floor: {result:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn a_broken_runner_is_a_terminal_failure_not_a_result() -> TestResult {
        // A workspace with NO runner script at the derived location: the
        // spawn fails (zsh cannot read the script) and no result exists.
        let fixture = tempfile::tempdir()?;
        fs::create_dir(fixture.path().join("src"))?;
        fs::write(fixture.path().join("Cargo.toml"), "[package]\n")?;
        let (context, _handle) = test_context();
        let result = run_gate(
            &ambient_environment(),
            "fmt",
            GateInput {
                workspace: fixture.path().to_string_lossy().into_owned(),
                workflow_id: "test-no-runner".to_owned(),
            },
            &context,
        )
        .await;
        assert!(
            result.is_err(),
            "a runner that cannot run must never yield a gate result"
        );
        Ok(())
    }

    /// M5: cancelling the activity kills the runner's WHOLE process group —
    /// verified by the child's disappearance, with a presence control first —
    /// and reports a cancelled measurement, never a result.
    #[tokio::test]
    async fn cancellation_kills_the_process_group_and_reports_no_result() -> TestResult {
        let fixture = tempfile::tempdir()?;
        let gates_dir = fixture.path().join("workflows/gates");
        fs::create_dir_all(&gates_dir)?;
        // A stand-in runner whose marker is this test's unique temp path, so
        // the pgrep probes below cannot match anything else on the box.
        let marker = gates_dir.join("gate-runner.zsh");
        fs::write(
            &marker,
            "#!/bin/zsh\n# stand-in runner: hold the group open until killed\nsleep 600\n",
        )?;

        let (context, handle) = test_context();
        let workspace = fixture.path().to_string_lossy().into_owned();
        let environment = ambient_environment();
        let running = tokio::spawn(async move {
            let (context, input) = (
                context,
                GateInput {
                    workspace,
                    workflow_id: "test-cancel".to_owned(),
                },
            );
            run_gate(&environment, "fmt", input, &context).await
        });

        // Presence control: the contained runner is genuinely alive before
        // cancellation, so its later absence is evidence of the kill.
        let marker_text = marker.to_string_lossy().into_owned();
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let seen = std::process::Command::new("pgrep")
                .args(["-f", &marker_text])
                .output()?;
            if seen.status.success() {
                break;
            }
            if Instant::now() > deadline {
                return Err("the stand-in runner never appeared in pgrep".into());
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        handle.cancel();
        let result = tokio::time::timeout(Duration::from_secs(30), running).await??;
        let failure = match result {
            Err(failure) => format!("{failure:?}"),
            Ok(result) => {
                return Err(
                    format!("a cancelled gate must never yield a result, got: {result:?}").into(),
                );
            }
        };
        assert!(
            failure.contains("CANCELLED MEASUREMENT"),
            "the failure names the cancellation: {failure}"
        );

        // The group is gone: nothing on the box still matches the unique
        // marker path.
        let survivors = std::process::Command::new("pgrep")
            .args(["-f", &marker_text])
            .output()?;
        assert!(
            !survivors.status.success(),
            "the runner's process group must be dead after cancellation: {}",
            String::from_utf8_lossy(&survivors.stdout)
        );
        Ok(())
    }

    /// Turn a fixture workspace into a git repository — the generator records
    /// HEAD and the full porcelain status, and refuses without them.
    fn git_init_fixture(workspace: &str) -> TestResult {
        for arguments in [
            vec!["init", "-q"],
            vec!["add", "-A"],
            vec![
                "-c",
                "user.email=gates@test",
                "-c",
                "user.name=gates",
                "commit",
                "-qm",
                "fixture",
            ],
        ] {
            let status = std::process::Command::new("git")
                .arg("-C")
                .arg(workspace)
                .args(&arguments)
                .status()?;
            if !status.success() {
                return Err(format!("git {arguments:?} failed in the fixture").into());
            }
        }
        Ok(())
    }

    /// Stub `aion` and `curl` ahead of the real PATH: the generator's
    /// describe and /build sources answer from canned bytes (a 2-leg
    /// history), so what a test measures is the COUNT comparison and nothing
    /// else. Returns the stubbed PATH.
    fn stub_manifest_sources(fixture: &Path) -> Result<String, Box<dyn std::error::Error>> {
        let stub_dir = fixture.join("stub-bin");
        fs::create_dir(&stub_dir)?;
        let describe = serde_json::json!({
            "summary": {"workflow_id": "w-1", "workflow_type": "gates", "status": "Running",
                         "started_at": "2026-08-11T00:00:00Z", "ended_at": null, "parent": null},
            "history": [
                {"type": "WorkflowStarted", "data": {"envelope": {"seq": 1}, "run_id": "r-1",
                  "package_version": "p-1"}},
                {"type": "ActivityCompleted", "data": {"activity_id": 0, "result":
                  {"gate": "fmt", "exit_code": 0, "passed": true, "output_tail": "a\n"}}},
                {"type": "ActivityCompleted", "data": {"activity_id": 1, "result":
                  {"gate": "bun", "exit_code": 1, "passed": false, "output_tail": "b\n"}}}
            ]
        });
        let canned = fixture.join("describe.json");
        fs::write(&canned, serde_json::to_vec_pretty(&describe)?)?;
        // The stubs answer `--version` as themselves, so the manifest's
        // tool_versions block has a KNOWN answer to be checked against — the
        // assertion that catches a tool-identity block that resolved nothing.
        write_executable(
            &stub_dir.join("aion"),
            &format!(
                "#!/bin/zsh\nif [[ \"$1\" == --version ]]; then print -r -- 'aion stub 0.0.0'; exit 0; fi\ncat -- {}\n",
                canned.display()
            ),
        )?;
        write_executable(
            &stub_dir.join("curl"),
            "#!/bin/zsh\nif [[ \"$1\" == --version ]]; then print -r -- 'curl stub 0.0.0'; exit 0; fi\nprint -r -- '{\"version\":\"stub\",\"built_at\":\"stub\"}'\n",
        )?;
        let ambient = std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".to_owned());
        Ok(format!("{}:{ambient}", stub_dir.display()))
    }

    /// Everything a manifest-derivation test needs, in one place: a git
    /// fixture workspace carrying the committed runner, stubbed `aion`/`curl`
    /// sources serving a canned two-leg history, and the tree and instrument
    /// values a real run's legs would have observed.
    struct ManifestFixture {
        fixture: tempfile::TempDir,
        workspace: String,
        runner: String,
        stubbed_path: String,
        out_path: PathBuf,
        head: String,
        runner_sha: String,
    }

    impl ManifestFixture {
        fn new() -> Result<Self, Box<dyn std::error::Error>> {
            let fixture = fixture_workspace(false)?;
            let workspace = fixture.path().to_string_lossy().into_owned();
            let stubbed_path = stub_manifest_sources(fixture.path())?;
            let runner = super::runner_path(&workspace);
            let head = fixture_head(&workspace)?;
            let runner_sha = sha256_of(Path::new(&runner))?;
            let out_path = fixture.path().join("manifest.json");
            Ok(Self {
                fixture,
                workspace,
                runner,
                stubbed_path,
                out_path,
                head,
                runner_sha,
            })
        }

        /// What two legs that measured one tree with one instrument leave
        /// behind.
        fn observe_agreeing_legs(&self) -> TestResult {
            self.observe(&[
                ("fmt", self.head.as_str(), self.runner_sha.as_str()),
                ("bun", self.head.as_str(), self.runner_sha.as_str()),
            ])
        }

        /// Replace the run's observations with exactly this set — an arm that
        /// inherits the previous arm's leftovers is measuring a state no
        /// single run ever had.
        fn observe(&self, observations: &[(&str, &str, &str)]) -> TestResult {
            let dir = self
                .fixture
                .path()
                .join("gate-logs/gates-runs/w-1/identity");
            if dir.exists() {
                fs::remove_dir_all(&dir)?;
            }
            write_identity_observations(self.fixture.path(), "w-1", observations)
        }

        fn forget_observations(&self) -> TestResult {
            fs::remove_dir_all(
                self.fixture
                    .path()
                    .join("gate-logs/gates-runs/w-1/identity"),
            )?;
            Ok(())
        }

        fn derive(
            &self,
            expected: &str,
            post_mortem: bool,
        ) -> Result<std::process::Output, std::io::Error> {
            let mut command = std::process::Command::new("zsh");
            command.arg("-f").arg(&self.runner).args([
                "manifest",
                "w-1",
                "127.0.0.1:1",
                "http://127.0.0.1:1",
                &self.out_path.to_string_lossy(),
                expected,
                &self.workspace,
            ]);
            if post_mortem {
                command.arg("--unobserved-instrument-post-mortem");
            }
            command.env_clear().env("PATH", &self.stubbed_path).output()
        }

        fn written_manifest(&self) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
            Ok(serde_json::from_slice(&fs::read(&self.out_path)?)?)
        }

        /// A refusal is asserted by its REASON, not only by its exit code.
        /// Every refusal in this derivation exits 65, so an arm that checks
        /// the code alone is satisfied by a DIFFERENT refusal firing — which
        /// is exactly how a weakened guard survived a mutation probe at the
        /// desk before this assertion was tightened.
        fn assert_refused(&self, outcome: &std::process::Output, what: &str, because: &str) {
            let stderr = String::from_utf8_lossy(&outcome.stderr);
            assert_eq!(
                outcome.status.code(),
                Some(65),
                "{what} must refuse the derivation; stderr:\n{stderr}"
            );
            assert!(
                stderr.contains(because),
                "{what} must refuse FOR ITS OWN REASON (expected `{because}`); stderr:\n{stderr}"
            );
            assert!(
                !self.out_path.exists(),
                "{what}: a refused derivation must write nothing"
            );
        }

        fn assert_derived(&self, outcome: &std::process::Output, what: &str) {
            assert_eq!(
                outcome.status.code(),
                Some(0),
                "{what} must derive; stderr:\n{}",
                String::from_utf8_lossy(&outcome.stderr)
            );
            assert!(
                self.out_path.is_file(),
                "{what}: the manifest must be written"
            );
        }
    }

    /// B-r2-1: the instrument block must NAME the toolchain. `local path`
    /// inside `tool_identity` emptied PATH for the whole function, so every
    /// one of these degraded to null in BOTH committed manifests while the
    /// gates themselves ran fine — an artifact nobody opened. This reads the
    /// ARTIFACT, and the stubs give two of the nine a known answer, which
    /// additionally proves the identities resolved under the PATH the runner
    /// was given rather than being copied from anywhere else.
    fn assert_instrument_names_its_toolchain(
        manifest: &serde_json::Value,
        fixture: &ManifestFixture,
    ) {
        let tools = &manifest["instrument"]["tool_versions"];
        for tool in [
            "zsh", "cargo", "rustc", "bun", "jq", "git", "aion", "curl", "shasum",
        ] {
            assert!(
                tools[tool]["path"].is_string(),
                "tool_versions.{tool}.path must name the resolved binary, got {:?} (block: {tools:#})",
                tools[tool]["path"]
            );
            assert!(
                tools[tool]["version"].is_string(),
                "tool_versions.{tool}.version must carry the tool's own version line, got {:?}",
                tools[tool]["version"]
            );
        }
        assert_eq!(
            tools["aion"]["path"],
            serde_json::Value::String(
                fixture
                    .fixture
                    .path()
                    .join("stub-bin/aion")
                    .to_string_lossy()
                    .into_owned()
            )
        );
        assert_eq!(tools["aion"]["version"], "aion stub 0.0.0");
        assert_eq!(tools["curl"]["version"], "curl stub 0.0.0");
    }

    /// M-r2-2: every hash the record pins has a preimage the record carries.
    /// The porcelain lines are inlined, and the pinned sha256 is re-derivable
    /// from those lines alone — the check a reader with only the committed
    /// bytes can run.
    fn assert_porcelain_preimage_is_verifiable(
        manifest: &serde_json::Value,
        fixture: &ManifestFixture,
    ) -> TestResult {
        let porcelain = manifest["workspace"]["status_porcelain_at_derivation"]
            .as_array()
            .ok_or("the porcelain status must be inlined as lines")?;
        assert!(
            porcelain
                .iter()
                .any(|line| line.as_str().is_some_and(|line| line.contains("stub-bin"))),
            "the inlined porcelain must be this fixture's real tree state: {porcelain:#?}"
        );
        let rebuilt = porcelain
            .iter()
            .map(|line| line.as_str().unwrap_or_default().to_owned())
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        let preimage = fixture.fixture.path().join("porcelain-rebuilt.txt");
        fs::write(&preimage, rebuilt)?;
        assert_eq!(
            manifest["workspace"]["status_porcelain_sha256"],
            serde_json::Value::String(sha256_of(&preimage)?),
            "the pinned hash must be re-derivable from the inlined lines alone"
        );
        Ok(())
    }

    /// B5a: the manifest generator REFUSES a history whose completed-leg
    /// count differs from what the workflow dispatched, in both directions,
    /// writing nothing — proven against a stubbed `aion` serving a canned
    /// 2-leg history, with the agreeing count as the vacuity control. This is
    /// the test that kills r1 mutation (b) (`… != … ]] && false`). The
    /// agreeing arm additionally reads the instrument block and the porcelain
    /// preimage, which is where the r2 review found the record claiming what
    /// it did not carry.
    #[tokio::test]
    async fn the_manifest_refuses_a_leg_count_the_document_did_not_dispatch() -> TestResult {
        let fixture = ManifestFixture::new()?;
        fixture.observe_agreeing_legs()?;

        // Refusal, under-count direction: the history holds 2, the document
        // "dispatched" 3 — nothing may be written.
        fixture.assert_refused(
            &fixture.derive("3", false)?,
            "a summarised-away leg",
            "summarises a leg away",
        );
        // Refusal, over-count direction.
        fixture.assert_refused(
            &fixture.derive("1", false)?,
            "a surplus leg",
            "legs the document never dispatched",
        );

        // Vacuity control: the agreeing count derives a manifest and a
        // receipt whose fields match the canned history.
        let agreed = fixture.derive("2", false)?;
        fixture.assert_derived(&agreed, "the agreeing count");
        let receipt: serde_json::Value = serde_json::from_slice(&agreed.stdout)?;
        assert_eq!(receipt["legs_recorded"], 2);
        assert_eq!(receipt["run_id"], "r-1");

        let manifest = fixture.written_manifest()?;
        assert_eq!(manifest["legs_recorded"], 2);
        assert_eq!(manifest["legs_failed"], 1);
        assert_eq!(
            manifest["instrument"]["runner_path"],
            serde_json::Value::String(
                fs::canonicalize(&fixture.runner)?
                    .to_string_lossy()
                    .into_owned()
            ),
            "the manifest names its instrument by path"
        );
        // The instrument's own hash, computed independently by this test over
        // the fixture's copy of the runner, in both of its recorded senses.
        assert_eq!(
            manifest["instrument"]["runner_sha256_at_derivation"],
            serde_json::Value::String(fixture.runner_sha.clone()),
            "the manifest must hash the runner it read"
        );
        assert_eq!(
            manifest["instrument"]["runner_sha256"],
            serde_json::Value::String(fixture.runner_sha.clone()),
            "the record's primary claim is the instrument the LEGS observed"
        );
        assert_eq!(
            manifest["workspace"]["head_commit"],
            serde_json::Value::String(fixture.head.clone())
        );
        assert_eq!(manifest["instrument"]["identity_capture"]["observed"], true);
        assert_instrument_names_its_toolchain(&manifest, &fixture);
        assert_porcelain_preimage_is_verifiable(&manifest, &fixture)?;
        Ok(())
    }

    /// M-r2-1: the record binds itself to what the LEGS observed, and refuses
    /// when they did not all measure one tree with one instrument — the r2a
    /// shape, where the 0.13.8 release landed mid-run and HEAD moved
    /// underneath the battery. Both disagreement axes, with the agreeing case
    /// as the vacuity control.
    #[tokio::test]
    async fn the_manifest_refuses_legs_that_did_not_measure_one_tree_with_one_instrument()
    -> TestResult {
        let fixture = ManifestFixture::new()?;

        // Two legs, two trees: the tree moved mid-battery.
        fixture.observe(&[
            ("fmt", fixture.head.as_str(), fixture.runner_sha.as_str()),
            (
                "bun",
                "0000000000000000000000000000000000000000",
                fixture.runner_sha.as_str(),
            ),
        ])?;
        fixture.assert_refused(
            &fixture.derive("2", false)?,
            "a tree that moved mid-battery",
            "did not all measure one tree with one instrument",
        );

        // Two legs, two instruments: the runner was edited mid-battery, which
        // is exactly what a mutation probe does.
        fixture.observe(&[
            ("fmt", fixture.head.as_str(), fixture.runner_sha.as_str()),
            (
                "bun",
                fixture.head.as_str(),
                "sha256:not-the-runner-that-ran",
            ),
        ])?;
        fixture.assert_refused(
            &fixture.derive("2", false)?,
            "an instrument that changed mid-battery",
            "did not all measure one tree with one instrument",
        );

        // Fewer observations than the document declares legs: a leg that
        // measured nothing observable cannot be vouched for either.
        fixture.observe(&[("fmt", fixture.head.as_str(), fixture.runner_sha.as_str())])?;
        fixture.assert_refused(
            &fixture.derive("2", false)?,
            "an unobserved leg",
            "but the workflow dispatched",
        );

        // Vacuity control: agreeing observations derive, with the observed
        // values recorded — so the refusals above are about the disagreement,
        // not about this fixture being unable to derive at all.
        fixture.observe_agreeing_legs()?;
        fixture.assert_derived(&fixture.derive("2", false)?, "agreeing observations");
        let manifest = fixture.written_manifest()?;
        assert_eq!(manifest["instrument"]["identity_capture"]["observed"], true);
        assert_eq!(
            manifest["workspace"]["head_commit"],
            serde_json::Value::String(fixture.head.clone())
        );
        Ok(())
    }

    /// M-r2-1, the post-mortem arm: a run whose legs left no observations
    /// cannot have its measured tree named, so the derivation refuses unless
    /// the operator says so explicitly — and then it RECORDS the gap (nulls
    /// plus a note) rather than quietly promoting a derivation-time reading
    /// into a claim about what was measured.
    #[tokio::test]
    async fn an_unobserved_instrument_derives_only_on_an_acknowledged_post_mortem() -> TestResult {
        let fixture = ManifestFixture::new()?;
        fixture.observe_agreeing_legs()?;
        fixture.forget_observations()?;

        fixture.assert_refused(
            &fixture.derive("2", false)?,
            "an unobserved derivation",
            "no per-leg identity observations under",
        );

        fixture.assert_derived(&fixture.derive("2", true)?, "an acknowledged post-mortem");
        let manifest = fixture.written_manifest()?;
        assert_eq!(
            manifest["instrument"]["identity_capture"]["observed"],
            false
        );
        assert!(
            manifest["workspace"]["head_commit"].is_null(),
            "an unobserved derivation must not claim the tree the legs measured"
        );
        assert!(
            manifest["instrument"]["runner_sha256"].is_null(),
            "an unobserved derivation must not claim the instrument that ran"
        );
        assert_eq!(
            manifest["workspace"]["head_commit_at_derivation"],
            serde_json::Value::String(fixture.head.clone()),
            "the at-derivation reading is still recorded, named as what it is"
        );
        assert!(
            manifest["instrument"]["identity_capture"]["note"]
                .as_str()
                .is_some_and(|note| note.contains("NULL")),
            "the artifact itself must say the gap is a gap"
        );
        Ok(())
    }
}
