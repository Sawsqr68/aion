//! Native one-motion AWL compile, package, input validation, and result waiting.

use std::fmt;
use std::path::Path;
use std::time::{Duration, Instant};

use aion_awl::{DurationLiteral, TokenKind};
use aion_awl_package::compile_and_assemble_awl;
use aion_client::WorkflowHandle;
use aion_core::{Event, Payload, WorkflowError};
use anyhow::{Context, Result};
use serde_json::{Value, json};

use crate::payload::{json_payload, payload_to_json};

/// A native AWL package ready for the unchanged operator deploy operation.
pub(crate) struct PreparedAwl {
    pub(crate) workflow_name: String,
    pub(crate) archive: Vec<u8>,
    pub(crate) input: Payload,
}

/// A stable client-side refusal emitted before any deploy connection is made.
#[derive(Debug)]
pub(crate) struct InputValidationError {
    expected_schema: String,
}

impl fmt::Display for InputValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "input validation failed: --input does not match the workflow input schema\nexpected schema: {}",
            self.expected_schema
        )
    }
}

impl std::error::Error for InputValidationError {}

/// Returns whether `path` names the case-sensitive `.awl` direct-compile path.
pub(crate) fn is_awl(path: &Path) -> bool {
    path.as_os_str().to_string_lossy().ends_with(".awl")
}

/// Reads, compiles, and assembles an AWL document without contacting a server.
pub(crate) fn package_file(path: &Path) -> Result<(String, Value, Vec<u8>)> {
    if !is_awl(path) {
        anyhow::bail!("run requires a .awl file: `{}`", path.display());
    }
    let source = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read AWL file `{}`", path.display()))?;
    let filename = path
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .ok_or_else(|| anyhow::anyhow!("AWL path `{}` has no filename", path.display()))?;
    package_source(&source, document_root(path), filename)
}

/// Prepares one-motion run input. Validation completes before the caller can
/// hand `archive` to the deploy operation.
pub(crate) fn prepare_run(path: &Path, input: &str) -> Result<PreparedAwl> {
    let (workflow_name, input_schema, archive) = package_file(path)?;
    let input_value: Value = serde_json::from_str(input).context("invalid --input JSON")?;
    validate_input(&input_schema, &input_value)?;
    let input = json_payload(input).context("invalid --input JSON")?;
    Ok(PreparedAwl {
        workflow_name,
        archive,
        input,
    })
}

/// Compiles and assembles source into complete format-v1 package bytes,
/// carrying the document and its imported schemas as archive provenance.
fn package_source(
    source: &str,
    schema_root: &Path,
    document_filename: &str,
) -> Result<(String, Value, Vec<u8>)> {
    let prepared = compile_and_assemble_awl(source, schema_root, document_filename)?;
    Ok((
        prepared.compiled.workflow_name,
        prepared.compiled.input_schema,
        prepared.archive,
    ))
}

/// Mirrors `aion awl check`: schema imports resolve against the document's
/// containing directory, with `.` for a bare relative filename.
fn document_root(file: &Path) -> &Path {
    match file.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    }
}

fn validate_input(schema: &Value, input: &Value) -> Result<()> {
    let validator = jsonschema::validator_for(schema)
        .context("compiled workflow produced an invalid input schema")?;
    if validator.is_valid(input) {
        return Ok(());
    }
    let expected_schema =
        serde_json::to_string(schema).context("failed to encode expected workflow input schema")?;
    Err(InputValidationError { expected_schema }.into())
}

/// How often the one-motion run re-reads the run's durable history while
/// waiting for a terminal event.
const RESULT_POLL_INTERVAL: Duration = Duration::from_millis(400);

/// Parses the operator's `--timeout` bound on the one-motion wait.
///
/// There is no default and no invented margin: the bound exists only when the
/// operator writes the flag. The accepted spelling IS the AWL duration literal
/// — lexed and converted by `aion-awl` itself — so `aion run --timeout 5m` and a
/// document's own `timeout 5m` can never drift into two vocabularies.
///
/// # Errors
///
/// Returns the rejection clap renders when the value is not a single whole AWL
/// duration literal, overflows a `Duration`, or is zero (a zero bound would stop
/// the wait before the first poll, which no operator means to ask for).
pub(crate) fn parse_await_timeout(raw: &str) -> Result<Duration, String> {
    let tokens =
        aion_awl::lex(raw).map_err(|error| format!("invalid --timeout value `{raw}`: {error}"))?;
    // The literal must cover the WHOLE argument: `5m extra` lexes to a leading
    // duration token and must still be refused.
    let literal = tokens
        .first()
        .and_then(|token| match token.kind {
            TokenKind::Duration { magnitude, unit } => Some(DurationLiteral {
                span: token.span,
                magnitude,
                unit,
            }),
            _ => None,
        })
        .filter(|literal| literal.span.start == 0 && literal.span.end == raw.len())
        .ok_or_else(|| {
            format!(
                "--timeout takes one AWL duration literal (`30s`, `5m`, `3h`, `2d`), not `{raw}`"
            )
        })?;
    let bound = literal
        .checked_duration()
        .ok_or_else(|| format!("--timeout value `{raw}` is out of range"))?;
    if bound.is_zero() {
        return Err(String::from("--timeout must be a positive duration"));
    }
    Ok(bound)
}

/// Waits for the concrete run's terminal event and emits the existing JSON
/// identifier fields plus its decoded result. The wait polls `describe` over
/// the one configured gRPC endpoint — event subscriptions need a separate
/// stream listener address the CLI does not require operators to provide.
/// AWL currently cannot continue as new, so that terminal is reported as a
/// refusal rather than silently detached.
///
/// `wait_limit` is the operator's `--timeout`, and is the ONLY client-side bound.
/// A document that declares its own `timeout` is already driven to a
/// `WorkflowTimedOut` terminal by the engine's deadline and ends this wait
/// through the terminal arm below, so nothing here duplicates it with a margin.
/// With neither, the wait is indefinite by design: the run has no declared end
/// and inventing one would be a policy the author never wrote.
pub(crate) async fn await_result(
    handle: WorkflowHandle,
    workflow_type: &str,
    wait_limit: Option<Duration>,
) -> Result<Value> {
    let workflow_id = handle.workflow_id().to_string();
    let run_id = handle.run_id().to_string();
    let started = Instant::now();
    loop {
        let description = handle
            .describe()
            .await
            .context("failed while awaiting workflow result")?;
        if let Some(event) = description.terminal_event.as_ref() {
            match event {
                Event::WorkflowCompleted { result, .. } => {
                    return Ok(json!({
                        "workflow_type": workflow_type,
                        "workflow_id": workflow_id,
                        "run_id": run_id,
                        "result": payload_to_json(result)?,
                    }));
                }
                Event::WorkflowFailed { error, .. } => {
                    anyhow::bail!("workflow run failed: {}", render_failure(error));
                }
                Event::WorkflowCancelled { reason, .. } => {
                    anyhow::bail!("workflow run cancelled: {reason}");
                }
                Event::WorkflowTimedOut { timeout, .. } => {
                    anyhow::bail!("workflow run timed out: {timeout}");
                }
                Event::WorkflowContinuedAsNew { .. } => {
                    anyhow::bail!("workflow run continued as new before producing a result");
                }
                _ => {}
            }
        }
        let Some(limit) = wait_limit else {
            tokio::time::sleep(RESULT_POLL_INTERVAL).await;
            continue;
        };
        let Some(remaining) = limit.checked_sub(started.elapsed()) else {
            anyhow::bail!(
                "stopped waiting after --timeout {limit}s with no terminal event; the run CONTINUES on the server\n  workflow_id: {workflow_id}\n  run_id: {run_id}\n  inspect it with: aion describe {workflow_id} --run-id {run_id}",
                limit = limit.as_secs(),
            );
        };
        tokio::time::sleep(RESULT_POLL_INTERVAL.min(remaining)).await;
    }
}

/// Renders a recorded workflow failure WITH its structured details.
///
/// `WorkflowError`'s `Display` is its `message` alone, and an AWL failure
/// outcome records the outcome name and its routed payload in `details`.
/// Reporting the message alone left the operator holding only the engine's
/// process-ordinal text ("workflow 3 returned error"), which names neither the
/// outcome nor the run. An undecodable `details` payload is reported as such
/// rather than dropped.
fn render_failure(error: &WorkflowError) -> String {
    let Some(details) = error.details.as_ref() else {
        return error.to_string();
    };
    match payload_to_json(details) {
        Ok(decoded) => format!("{error}\n  details: {decoded}"),
        Err(decode_error) => {
            format!(
                "{error}\n  details: <undecodable {:?} payload: {decode_error}>",
                details.content_type()
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use aion_package::{ExtractionLimits, Package};
    use serde_json::json;

    use super::{
        InputValidationError, package_file, package_source, parse_await_timeout, prepare_run,
    };

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn fixture(relative: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../aion-awl/tests/fixtures/rev2")
            .join(relative)
    }

    #[test]
    fn known_good_corpus_fixture_compiles_and_assembles_without_a_server() -> TestResult {
        let (workflow_name, input_schema, bytes) =
            package_file(&fixture("flagship/valid/awl_hello.awl"))?;
        let package = Package::load_from_bytes(bytes, ExtractionLimits::unbounded())?;

        assert_eq!(workflow_name, "awl_hello");
        assert_eq!(package.manifest().entry_module, "awl_hello");
        assert_eq!(package.manifest().entry_function, "run");
        assert_eq!(package.manifest().input_schema, input_schema);
        assert!(package.beams().get("awl_hello").is_some());
        Ok(())
    }

    #[test]
    fn document_timeout_reaches_the_one_motion_archive_manifest() -> TestResult {
        let path = fixture("dag-fork/valid/after_single.awl");
        let source = std::fs::read_to_string(&path)?.replacen(
            "workflow after_single\n",
            "workflow after_single\n  timeout 6h\n",
            1,
        );
        let root = path.parent().ok_or("fixture has no parent directory")?;
        let (_, _, bytes) = package_source(&source, root, "after_single.awl")?;
        let package = Package::load_from_bytes(bytes, ExtractionLimits::unbounded())?;

        assert_eq!(
            package.manifest().timeout,
            Some(std::time::Duration::from_secs(21_600))
        );
        Ok(())
    }

    #[test]
    fn mismatching_input_is_refused_before_an_archive_can_be_deployed() -> TestResult {
        let Err(error) = prepare_run(
            &fixture("flagship/valid/awl_hello.awl"),
            &json!({ "name": 42 }).to_string(),
        ) else {
            return Err("mismatching input unexpectedly passed validation".into());
        };
        let refusal = error
            .downcast_ref::<InputValidationError>()
            .ok_or("input mismatch did not preserve its typed refusal")?;
        let rendered = refusal.to_string();
        assert!(rendered.starts_with("input validation failed:"));
        assert!(rendered.contains("expected schema: {"));
        Ok(())
    }

    #[test]
    fn await_timeout_accepts_every_awl_duration_unit() {
        use std::time::Duration;

        assert_eq!(parse_await_timeout("30s"), Ok(Duration::from_secs(30)));
        assert_eq!(parse_await_timeout("5m"), Ok(Duration::from_secs(300)));
        assert_eq!(parse_await_timeout("3h"), Ok(Duration::from_secs(10_800)));
        assert_eq!(parse_await_timeout("2d"), Ok(Duration::from_secs(172_800)));
    }

    #[test]
    fn await_timeout_refuses_anything_but_one_whole_duration_literal() {
        for rejected in ["", "5", "5x", "5m extra", "extra 5m", "5m5m", "-5m", "0s"] {
            assert!(
                parse_await_timeout(rejected).is_err(),
                "`{rejected}` must be refused as a --timeout value"
            );
        }
    }
}
