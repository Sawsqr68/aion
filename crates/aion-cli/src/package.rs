//! Local `package` subcommand: a thin shell over [`aion_package::package_project`].

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use aion_package::{ExcludedModule, PackageOptions, ProjectReport, package_project};
use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde_json::Value;

use crate::output::to_value;

/// JSON document printed on stdout after a successful `package` run.
#[derive(Serialize)]
struct PackageOutput<'a> {
    packages: Vec<PackagedOutput>,
    excluded: &'a [ExcludedModule],
    /// Packages that built successfully and cannot be STARTED on a server that
    /// routes activities through task queues. Empty for every package that can.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    unstartable: Vec<UnstartableWorkflow>,
}

/// A package whose activities carry no queue declaration.
///
/// A project manifest records an activity as a bare name — [`DeclaredActivity`]
/// has one field — so `PackageContract::from_manifest` commits every one of
/// them as UNSCOPED, and a server serving activities through task queues
/// refuses to start the workflow with `NO_QUEUE_DECLARATION`.
///
/// The refusal is correct. What was wrong is WHEN the author heard about it:
/// packaging succeeded, deploying succeeded and reported `route_changed`, and
/// the first start — potentially days later, on someone else's machine — was
/// the first sign. This surfaces the same fact at the moment the package is
/// built, when the author is still holding the problem.
///
/// [`DeclaredActivity`]: aion_package::DeclaredActivity
#[derive(Serialize)]
struct UnstartableWorkflow {
    workflow_type: String,
    unscoped_activities: Vec<String>,
    reason: &'static str,
    remedy: &'static str,
}

/// One packaged workflow in the `package` result document.
#[derive(Serialize)]
struct PackagedOutput {
    workflow_type: String,
    output: String,
    version: String,
    deployed_name: String,
    modules: usize,
}

/// Runs the `package` subcommand: optionally builds the Gleam project, then
/// packages every workflow its `workflow.toml` declares.
///
/// `out` is resolved against the current directory before it reaches the
/// library, which would otherwise resolve it against the project root.
pub(crate) fn run(path: &Path, out: Option<&Path>, build: bool) -> Result<Value> {
    if build {
        run_gleam_build(path)?;
    }
    let options = PackageOptions {
        output_override: out.map(absolute_out).transpose()?,
    };
    let report = package_project(path, &options)
        .with_context(|| format!("failed to package workflow project at {}", path.display()))?;
    to_value(report_output(&report))
}

/// Resolves a `--out` value against the invoker's current directory.
fn absolute_out(out: &Path) -> Result<PathBuf> {
    std::path::absolute(out)
        .with_context(|| format!("failed to resolve --out path {}", out.display()))
}

/// Spawns `gleam build` in the project directory with inherited stdio, so the
/// user sees compiler output on stderr. Process spawning lives only in this
/// CLI layer; the packaging library never builds.
///
/// # Why this one is deliberately NOT classified (#125)
///
/// The other `gleam` call sites classify a dependency-layer failure so they
/// stop blaming the author's source for a registry outage. This one does not,
/// and the reason is the inherited stdio: `gleam`'s own error — including the
/// hex.pm failure — is already on the user's terminal, verbatim and live, and
/// the bail below asserts nothing about their code. There is no false claim to
/// remove. Classifying would mean switching to captured output and printing it
/// after the fact, trading a real capability (streaming progress on a build
/// that runs for seconds) for wording that is already honest.
fn run_gleam_build(path: &Path) -> Result<()> {
    let status = Command::new("gleam")
        .arg("build")
        .current_dir(path)
        .status()
        .with_context(|| format!("failed to run `gleam build` in {}", path.display()))?;
    if !status.success() {
        bail!("`gleam build` failed in {} with {status}", path.display());
    }
    Ok(())
}

/// Maps the library report to the printed JSON document.
fn report_output(report: &ProjectReport) -> PackageOutput<'_> {
    PackageOutput {
        packages: report
            .packages
            .iter()
            .map(|packaged| PackagedOutput {
                workflow_type: packaged.workflow_type.clone(),
                output: packaged.output_path.display().to_string(),
                version: packaged.version.content_hash.to_string(),
                deployed_name: packaged.package.deployed_entry_module(),
                modules: packaged.package.beams().len(),
            })
            .collect(),
        excluded: &report.excluded,
        unstartable: report.packages.iter().filter_map(unstartable).collect(),
    }
}

/// Reports a packaged workflow whose activities are all unscoped.
///
/// Reads the SAME `unscoped_activities` field the engine's start admission
/// reads (`aion::lifecycle::start_admission`), rather than re-deriving the
/// condition here — a second derivation could drift from the gate and report
/// the opposite of the truth.
///
/// A package whose contract cannot be read at all is not reported: that is the
/// separate pre-`.v4` identity failure, which `deploy` already refuses on with
/// its own typed error, and guessing about it here would add noise to a
/// message whose value is that it is exact.
fn unstartable(packaged: &aion_package::PackagedWorkflow) -> Option<UnstartableWorkflow> {
    let contract = packaged.package.contract().ok()?;
    unstartable_contract(&packaged.workflow_type, contract)
}

/// The decision itself, over the contract alone.
///
/// Split from [`unstartable`] so the SILENT case can be tested: a diagnostic
/// that has only ever been seen to fire is indistinguishable from one that
/// always fires.
fn unstartable_contract(
    workflow_type: &str,
    contract: &aion_package::PackageContract,
) -> Option<UnstartableWorkflow> {
    if contract.unscoped_activities.is_empty() {
        return None;
    }
    let mut unscoped_activities = contract.unscoped_activities.clone();
    unscoped_activities.sort();
    Some(UnstartableWorkflow {
        workflow_type: workflow_type.to_owned(),
        unscoped_activities,
        reason: "a project manifest records activities as bare names and has no field for a \
                 task queue, so every activity above is committed to the package contract as \
                 UNSCOPED. A server that routes activities through task queues refuses to \
                 START this workflow with NO_QUEUE_DECLARATION. Packaging and deploying both \
                 still succeed, so nothing else will tell you",
        remedy: "author the workflow as an AWL document, which declares its worker queue and \
                 compiles to a scoped contract, and deploy that. There is no manifest or Gleam \
                 change that scopes a project-manifest activity — `activity.task_queue` sets \
                 the DISPATCH queue on the in-process activity value and never reaches the \
                 package contract, so it does not clear this refusal",
    })
}

#[cfg(test)]
mod tests {
    use aion_package::PackageContract;
    use serde_json::json;

    use super::unstartable_contract;

    fn contract(unscoped: &[&str]) -> PackageContract {
        PackageContract {
            input_schema: json!({ "type": "object" }),
            output_schema: json!({ "type": "object" }),
            workers: Vec::new(),
            children: Vec::new(),
            signals: Vec::new(),
            additional_workflows: Vec::new(),
            unscoped_activities: unscoped.iter().map(|name| (*name).to_owned()).collect(),
        }
    }

    /// THE CONTROL, and the reason `unstartable_contract` exists apart from the
    /// package: a scoped contract must produce NOTHING. Without this the
    /// positive case below proves only that the diagnostic can fire, not that
    /// it discriminates — and a warning printed over every package is a warning
    /// nobody reads.
    #[test]
    fn a_scoped_contract_is_silent() {
        assert!(
            unstartable_contract("staged_rounds", &contract(&[])).is_none(),
            "a contract with no unscoped activities must produce no warning"
        );
    }

    #[test]
    fn an_unscoped_contract_names_every_activity_in_sorted_order() {
        let reported = unstartable_contract(
            "assistant",
            &contract(&["assistant_provision", "assistant"]),
        );
        let Some(reported) = reported else {
            unreachable!("an unscoped contract must produce a warning")
        };
        assert_eq!(reported.workflow_type, "assistant");
        // Sorted, because the manifest order is authoring order and this text
        // is compared across runs by whoever is chasing a NO_QUEUE_DECLARATION.
        assert_eq!(
            reported.unscoped_activities,
            vec!["assistant".to_owned(), "assistant_provision".to_owned()]
        );
        assert!(
            reported.reason.contains("NO_QUEUE_DECLARATION"),
            "the warning must name the exact refusal it predicts, so a search \
             for that error reaches this message"
        );
        assert!(
            reported.remedy.contains("activity.task_queue"),
            "the remedy must name the API that LOOKS like the fix and is not — \
             that dead end is what makes this expensive"
        );
    }
}
