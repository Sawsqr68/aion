//! The launch seam's backstop, exercised where the checker is NOT in the loop.
//!
//! [`HarnessConfigError`]'s own documentation promises that every variant is a checker
//! defect "reported rather than unwrapped so a future edit that drops a rule from the
//! checker names itself instead of panicking in a worker process at three in the
//! morning". These tests are what makes that promise checkable: they parse a document
//! WITHOUT running the checker — which is precisely the world a dropped checker rule
//! creates — and assert the seam names the field rather than carrying the bad value
//! through.

use std::path::{Path, PathBuf};

use crate::ast::{HARNESS_BINARY_FIELD, HARNESS_COMMAND_FIELD, HARNESS_CWD_FIELD};

use super::{HarnessConfigError, compile_harness};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// A workflow document declaring one agent queue with `settings` as its harness.
///
/// Parsed only. `crate::parse` does not run the checker, so a value the checker would
/// have refused reaches the launch seam — the exact condition under test.
fn worker_with(settings: &str) -> Result<crate::WorkerDecl, Box<dyn std::error::Error>> {
    let block = settings.lines().fold(String::new(), |mut block, line| {
        block.push_str("    ");
        block.push_str(line);
        block.push('\n');
        block
    });
    let source = format!(
        "//! Launch-seam backstop probe.\nworkflow probe\n  input question: String\n  outcome \
         answered: type Answer, route success\n\ntype Answer {{ reply: String }}\n\ntype Reply \
         {{ text: String, stop_reason: String, session_id: String }}\n\nworker assistants\n  harness\n{block}  \
         action ask(prompt: String) -> Reply\n    agent\n\nstep ask\n  ask(prompt: question) -> \
         reply\n  route answered(reply: reply.text)\n"
    );
    let document = crate::parse(&source).map_err(|error| {
        format!(
            "{}:{}: {}",
            error.span.line, error.span.column, error.message
        )
    })?;
    document
        .workers
        .into_iter()
        .next()
        .ok_or_else(|| "the probe declares one worker".into())
}

/// A complete Norn section, with `binary` written as given.
fn norn_with_binary(binary: &str) -> String {
    norn_with(&format!("cwd \"/srv/agents\"\nbinary \"{binary}\""))
}

/// A Norn section complete but for the kind-specific settings `tail` supplies.
fn norn_with(tail: &str) -> String {
    format!(
        "kind norn\nconcurrency 2\nreconnect_initial_backoff 5s\nreconnect_max_backoff \
         1m\nreconnect_max_attempts 50\nenv_pass \"PATH\"\n{tail}"
    )
}

#[test]
fn a_relative_norn_binary_is_named_at_the_launch_seam_not_carried_through() -> TestResult {
    let worker = worker_with(&norn_with_binary("bin/norn"))?;
    let Err(error) = compile_harness(&worker) else {
        return Err(
            "a relative `binary` must not reach a launcher: resolving it against whatever \
             directory a supervisor happens to be in is the silent scoping the whole rule \
             exists to prevent"
                .into(),
        );
    };
    let HarnessConfigError::Unusable { field, value, .. } = &error else {
        return Err(format!("expected an Unusable naming the field, got: {error}").into());
    };
    assert_eq!(*field, HARNESS_BINARY_FIELD);
    assert_eq!(value, "bin/norn");
    assert!(
        error.to_string().contains("checker defect"),
        "the seam says whose defect this is, so it is not mistaken for an operator \
         mistake: {error}"
    );
    Ok(())
}

/// The presence twin: an ABSOLUTE binary passes the same net. Without this the test above
/// could pass against a seam that refused every `binary` there is.
#[test]
fn an_absolute_norn_binary_passes_the_same_net() -> TestResult {
    let worker = worker_with(&norn_with_binary("/usr/local/bin/norn"))?;
    let Some(launch) = compile_harness(&worker)? else {
        return Err("the probe declares a harness".into());
    };
    let crate::CompiledHarnessKind::Norn { binary, cwd } = launch.kind else {
        return Err("a norn declaration must produce a norn launch".into());
    };
    assert_eq!(
        binary.as_deref(),
        Some(std::path::Path::new("/usr/local/bin/norn"))
    );
    assert_eq!(
        cwd.literal().map(PathBuf::as_path),
        Some(Path::new("/srv/agents"))
    );
    Ok(())
}

/// The absence twin: `binary` omitted is a first-class state, and the net does not turn
/// it into a refusal. The adapter's own documented `norn`-on-`PATH` default stands.
#[test]
fn an_omitted_norn_binary_is_absence_and_not_a_refusal() -> TestResult {
    let worker = worker_with(&norn_with("cwd \"/srv/agents\""))?;
    let Some(launch) = compile_harness(&worker)? else {
        return Err("the probe declares a harness".into());
    };
    let crate::CompiledHarnessKind::Norn { binary, .. } = launch.kind else {
        return Err("a norn declaration must produce a norn launch".into());
    };
    assert_eq!(binary, None);
    Ok(())
}

/// `cwd` has no absence arm at all, and the seam is where that is proven when the checker
/// is out of the loop. The only default available would be whichever directory the worker
/// process was launched from — the inheritance the whole setting exists to end — so the
/// seam must name the missing field rather than invent one.
#[test]
fn a_missing_norn_cwd_is_named_at_the_launch_seam() -> TestResult {
    let worker = worker_with(&norn_with("binary \"/usr/local/bin/norn\""))?;
    let Err(error) = compile_harness(&worker) else {
        return Err(
            "a norn section with no `cwd` must not produce a launch: the agent would work in \
             whichever tree the worker happened to be started in, recorded nowhere"
                .into(),
        );
    };
    let HarnessConfigError::Missing { field, .. } = &error else {
        return Err(format!("expected a Missing naming the field, got: {error}").into());
    };
    assert_eq!(*field, HARNESS_CWD_FIELD);
    Ok(())
}

/// The second accepted form survives the seam INTACT.
///
/// A `{workspace_root}`-leading value is not resolved here and must not be: where this
/// box keeps its workspaces is not a fact the language layer has. Carrying it verbatim is
/// what lets the composition root expand it against the box that will actually spawn the
/// agent — so the assertion is on the placeholder still being there, character for
/// character, and NOT on any expansion having happened.
#[test]
fn a_workspace_rooted_cwd_reaches_the_launch_seam_unexpanded() -> TestResult {
    let worker = worker_with(&norn_with("cwd \"{workspace_root}/assistant\""))?;
    let Some(launch) = compile_harness(&worker)? else {
        return Err("the probe declares a harness".into());
    };
    let crate::CompiledHarnessKind::Norn { cwd, .. } = launch.kind else {
        return Err("a norn declaration must produce a norn launch".into());
    };
    assert_eq!(
        cwd.literal().map(PathBuf::as_path),
        Some(Path::new("{workspace_root}/assistant"))
    );
    Ok(())
}

/// The refusal that new form did NOT widen: a placeholder anywhere but the front cannot
/// expand to an absolute path, so the seam refuses it exactly as it refuses a plain
/// relative one. Without this, the accept-set test above would be consistent with a seam
/// that had simply stopped checking `cwd` at all.
#[test]
fn a_misplaced_workspace_root_in_cwd_is_refused_at_the_seam() -> TestResult {
    let worker = worker_with(&norn_with("cwd \"agents/{workspace_root}\""))?;
    let Err(HarnessConfigError::Unusable { field, reason, .. }) = compile_harness(&worker) else {
        return Err("a placeholder that cannot lead cannot make an absolute path".into());
    };
    assert_eq!(field, HARNESS_CWD_FIELD);
    assert!(
        reason.contains("somewhere other than its start"),
        "the reason must name the actual fault rather than a generic one: {reason}"
    );
    Ok(())
}

/// The rule the optional path was given a backstop to match: `command` has always had
/// one, and both must name their own field rather than a shared blank.
#[test]
fn a_relative_acp_command_is_named_by_its_own_field() -> TestResult {
    let worker = worker_with(
        "kind acp\nconcurrency 2\nreconnect_initial_backoff 5s\nreconnect_max_backoff \
         1m\nreconnect_max_attempts 50\nenv_pass \"PATH\"\ncommand \
         \"claude-code-acp\"\ncwd \"/srv/agents\"\npermission \"deny\"\nexit_grace 5s",
    )?;
    let Err(HarnessConfigError::Unusable { field, .. }) = compile_harness(&worker) else {
        return Err("a relative `command` must be named at the seam".into());
    };
    assert_eq!(field, HARNESS_COMMAND_FIELD);
    Ok(())
}
