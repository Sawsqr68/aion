//! The four gate legs, and the rule that decides whether one of them is green.
//!
//! A leg is green only when its command exited zero AND its own stream proved
//! it looked at something. That second half is the denominator rule: a stream
//! that yields neither a finding nor a processed unit is a broken stream, and a
//! broken stream is a failed leg — never a pass.

use std::path::{Path, PathBuf};

use aion_worker::{ActivityContext, ActivityFailure};

use crate::extract::{StreamReport, cargo_stream, fmt_stream, nextest_stream};
use crate::logs::log_path;
use crate::process::{
    CommandError, CommandRun, CommandSpec, NO_PROCESS_EXIT_CODE, run_logged, tail,
};
use crate::settings::Settings;
use crate::types::{Finding, LegInput, LegResult};

/// The build tool every leg drives, resolved from `PATH` on the gate host.
const CARGO: &str = "cargo";
/// Findings that describe the leg itself rather than the code under gate.
const CODE_REFUSAL: &str = "workspace-refusal";
const CODE_SPAWN: &str = "spawn-failure";
const CODE_LOG: &str = "log-failure";
const CODE_INTEGRITY: &str = "stream-integrity";
const CODE_NOISE: &str = "stream-noise";
const CODE_MISSING_TOOL: &str = "missing-tool";
/// File recorded on a finding that is about the leg, not about a source file.
const NO_FILE: &str = "unknown";

/// One gate leg.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Leg {
    /// `cargo check --workspace`.
    Check,
    /// `cargo clippy --workspace --all-targets -- -D warnings`.
    Clippy,
    /// `cargo nextest run --workspace --no-fail-fast`.
    Nextest,
    /// `cargo fmt --all --check`.
    Fmt,
}

impl Leg {
    /// Stable leg name, as it appears in `LegResult.leg` and in log file names.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Check => "check",
            Self::Clippy => "clippy",
            Self::Nextest => "nextest",
            Self::Fmt => "fmt",
        }
    }

    fn args(self) -> Vec<String> {
        let args: &[&str] = match self {
            Self::Check => &["check", "--workspace", "--message-format", "json"],
            Self::Clippy => &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--message-format",
                "json",
                "--",
                "-D",
                "warnings",
            ],
            Self::Nextest => &[
                "nextest",
                "run",
                "--workspace",
                "--cargo-quiet",
                "--message-format",
                "libtest-json-plus",
                "--no-fail-fast",
            ],
            Self::Fmt => &[
                "fmt",
                "--all",
                "--verbose",
                "--check",
                "--",
                "--color",
                "never",
            ],
        };
        args.iter().map(|arg| (*arg).to_owned()).collect()
    }

    fn env(self) -> Vec<(String, String)> {
        let mut env = vec![("CARGO_TERM_COLOR".to_owned(), "never".to_owned())];
        if matches!(self, Self::Nextest) {
            env.push((
                "NEXTEST_EXPERIMENTAL_LIBTEST_JSON".to_owned(),
                "1".to_owned(),
            ));
        }
        env
    }

    fn decode(self, stdout: &str) -> StreamReport {
        match self {
            Self::Check | Self::Clippy => cargo_stream::extract(self.name(), stdout),
            Self::Nextest => nextest_stream::extract(self.name(), stdout),
            Self::Fmt => fmt_stream::extract(self.name(), stdout),
        }
    }
}

/// Runs one leg over a checkout and returns its real, structured result.
///
/// # Errors
///
/// Returns [`ActivityFailure`] only when the activity itself cannot continue:
/// the workflow cancelled it, or its heartbeat seam is gone. Everything about
/// the code under gate — including a command that could not be spawned — comes
/// back as a [`LegResult`] so the workflow computes its verdict from reality.
pub async fn run(
    leg: Leg,
    input: LegInput,
    settings: &Settings,
    context: &ActivityContext,
) -> Result<LegResult, ActivityFailure> {
    let workspace = PathBuf::from(&input.path);
    if let Some(refusal) = unusable_workspace(leg, &workspace) {
        return Ok(refusal);
    }
    if matches!(leg, Leg::Nextest)
        && let Some(refusal) = nextest_preflight(&workspace, settings, context).await?
    {
        return Ok(refusal);
    }
    let log_path = log_path(settings, &workspace, leg.name(), context);
    let spec = CommandSpec {
        program: CARGO.to_owned(),
        args: leg.args(),
        env: leg.env(),
        cwd: workspace,
    };
    let run = match run_logged(
        &spec,
        &log_path,
        leg.name(),
        context,
        settings.progress_interval(),
    )
    .await
    {
        Ok(run) => run,
        Err(error) => return command_failure(leg, &log_path, &error),
    };
    let report = leg.decode(&run.stdout);
    Ok(verdict(leg, &run, report))
}

/// Confirms `cargo nextest` exists before the tests leg claims anything.
///
/// A gate host without nextest is named as such, with the fix — it is never
/// quietly downgraded to `cargo test`.
async fn nextest_preflight(
    workspace: &Path,
    settings: &Settings,
    context: &ActivityContext,
) -> Result<Option<LegResult>, ActivityFailure> {
    let log_path = log_path(settings, workspace, "nextest-preflight", context);
    let spec = CommandSpec {
        program: CARGO.to_owned(),
        args: vec!["nextest".to_owned(), "--version".to_owned()],
        env: Leg::Nextest.env(),
        cwd: workspace.to_path_buf(),
    };
    let run = match run_logged(
        &spec,
        &log_path,
        "nextest-preflight",
        context,
        settings.progress_interval(),
    )
    .await
    {
        Ok(run) => run,
        Err(error) => return command_failure(Leg::Nextest, &log_path, &error).map(Some),
    };
    if run.exit_code == 0 {
        return Ok(None);
    }
    Ok(Some(refusal(
        Leg::Nextest,
        CODE_MISSING_TOOL,
        &format!(
            "`cargo nextest --version` exited {}; install it on this gate host with `cargo install cargo-nextest --locked`. This leg is nextest by ruling and never falls back to `cargo test`.\n{}",
            run.exit_code, run.output_tail
        ),
        run.exit_code,
        &log_path,
    )))
}

fn verdict(leg: Leg, run: &CommandRun, mut report: StreamReport) -> LegResult {
    // Silence is judged on the decode alone: the findings this function adds are
    // about the leg, and must not talk the leg out of its own integrity failure.
    let silent = report.is_silent();
    if report.unparsed > 0 {
        let unparsed = report.unparsed;
        report.push(leg_finding(
            leg,
            "warning",
            CODE_NOISE,
            &format!(
                "{unparsed} line(s) on the machine-readable stream were not records of the expected shape; see {}",
                run.log_path.display()
            ),
        ));
    }
    if silent {
        report.push(leg_finding(
            leg,
            "error",
            CODE_INTEGRITY,
            &format!(
                "the leg's stream yielded no findings and no processed units (denominator 0), so it certifies nothing; exit code was {}. See {}",
                run.exit_code,
                run.log_path.display()
            ),
        ));
    }
    LegResult {
        leg: leg.name().to_owned(),
        exit_code: run.exit_code,
        passed: run.exit_code == 0 && !silent,
        findings: report.findings,
        denominator: report.denominator,
        log_path: run.log_path.display().to_string(),
        output_tail: run.output_tail.clone(),
    }
}

fn unusable_workspace(leg: Leg, workspace: &Path) -> Option<LegResult> {
    if !workspace.is_dir() {
        return Some(refusal(
            leg,
            CODE_REFUSAL,
            &format!(
                "checkout path does not exist or is not a directory: {}",
                workspace.display()
            ),
            NO_PROCESS_EXIT_CODE,
            workspace,
        ));
    }
    if !workspace.join("Cargo.toml").is_file() {
        return Some(refusal(
            leg,
            CODE_REFUSAL,
            &format!("checkout contains no Cargo.toml: {}", workspace.display()),
            NO_PROCESS_EXIT_CODE,
            workspace,
        ));
    }
    None
}

/// Splits a command failure into what the workflow may reason about and what
/// ends the activity outright.
fn command_failure(
    leg: Leg,
    log_path: &Path,
    error: &CommandError,
) -> Result<LegResult, ActivityFailure> {
    match error {
        CommandError::Cancelled { .. }
        | CommandError::Heartbeat { .. }
        | CommandError::HeartbeatPayload { .. } => {
            Err(ActivityFailure::terminal(format!("{leg:?} leg: {error}")))
        }
        CommandError::Process { .. } => Ok(refusal(
            leg,
            CODE_SPAWN,
            &error.to_string(),
            NO_PROCESS_EXIT_CODE,
            log_path,
        )),
        CommandError::Log { .. } => Ok(refusal(
            leg,
            CODE_LOG,
            &error.to_string(),
            NO_PROCESS_EXIT_CODE,
            log_path,
        )),
    }
}

fn refusal(leg: Leg, code: &str, message: &str, exit_code: i32, log_path: &Path) -> LegResult {
    LegResult {
        leg: leg.name().to_owned(),
        exit_code,
        passed: false,
        findings: vec![leg_finding(leg, "error", code, message)],
        denominator: 0,
        log_path: log_path.display().to_string(),
        output_tail: tail(message),
    }
}

fn leg_finding(leg: Leg, level: &str, code: &str, message: &str) -> Finding {
    Finding {
        leg: leg.name().to_owned(),
        level: level.to_owned(),
        file: NO_FILE.to_owned(),
        line: None,
        column: None,
        code: code.to_owned(),
        message: message.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{Leg, verdict};
    use crate::extract::StreamReport;
    use crate::process::CommandRun;
    use crate::types::Finding;

    fn run(exit_code: i32) -> CommandRun {
        CommandRun {
            exit_code,
            stdout: String::new(),
            stderr: String::new(),
            log_path: PathBuf::from("/srv/gates/logs/aion/check-activity_1-attempt1.log"),
            output_tail: String::new(),
        }
    }

    fn finding() -> Finding {
        Finding {
            leg: "clippy".to_owned(),
            level: "warning".to_owned(),
            file: "src/lib.rs".to_owned(),
            line: Some(2),
            column: Some(9),
            code: "unused_variables".to_owned(),
            message: "unused variable".to_owned(),
        }
    }

    #[test]
    fn exit_zero_with_a_real_denominator_is_the_only_green() {
        let report = StreamReport {
            findings: Vec::new(),
            denominator: 7,
            unparsed: 0,
        };
        let result = verdict(Leg::Check, &run(0), report);
        assert!(result.passed);
        assert_eq!(result.denominator, 7);
        assert!(result.findings.is_empty());
    }

    #[test]
    fn exit_zero_with_a_zero_denominator_fails_on_stream_integrity() {
        let result = verdict(Leg::Check, &run(0), StreamReport::default());
        assert!(!result.passed, "a silent stream must never read as green");
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.denominator, 0);
        assert_eq!(result.findings.len(), 1);
        assert_eq!(result.findings[0].code, "stream-integrity");
        assert_eq!(result.findings[0].level, "error");
    }

    #[test]
    fn stream_noise_is_reported_without_flipping_a_real_green() {
        let report = StreamReport {
            findings: Vec::new(),
            denominator: 3,
            unparsed: 2,
        };
        let result = verdict(Leg::Clippy, &run(0), report);
        assert!(result.passed);
        assert_eq!(result.findings.len(), 1);
        assert_eq!(result.findings[0].code, "stream-noise");
        assert_eq!(result.findings[0].level, "warning");
    }

    #[test]
    fn findings_survive_a_nonzero_exit_untouched() {
        let report = StreamReport {
            findings: vec![finding()],
            denominator: 4,
            unparsed: 0,
        };
        let result = verdict(Leg::Clippy, &run(101), report);
        assert!(!result.passed);
        assert_eq!(result.exit_code, 101);
        assert_eq!(result.findings, vec![finding()]);
    }

    #[test]
    fn a_refusal_records_no_denominator_and_never_passes() {
        let result = super::refusal(
            Leg::Fmt,
            "workspace-refusal",
            "checkout contains no Cargo.toml: /srv/gates/checkouts/aion",
            -1,
            std::path::Path::new("/srv/gates/logs/aion/fmt.log"),
        );
        assert!(!result.passed);
        assert_eq!(result.exit_code, -1);
        assert_eq!(result.denominator, 0);
        assert_eq!(result.findings.len(), 1);
        assert_eq!(result.findings[0].code, "workspace-refusal");
    }
}
