//! Telling the operator which declared pass-through variables this machine does not
//! carry.
//!
//! # Why this is written, and not logged
//!
//! It was `tracing::warn!` first, and that was a silent failure wearing a warning's
//! clothes. The report is produced while the launch is being resolved — inside
//! [`crate::harness::resolve_declared`], reached from the verb's preparation — and the
//! agent worker does not install a tracing subscriber until it is already inside its
//! serve loop. Nothing was listening, so the warning went to the floor. On the `--check`
//! path it was worse: that path returns before the serve loop exists at all, so the
//! subscriber was never installed on that run in any form.
//!
//! Moving the subscriber earlier would have fixed one instance of the problem and left
//! the shape of it in place: a report whose delivery depends on which branch the caller
//! took, and which no test can observe. So the report is a VALUE written to a writer the
//! caller supplies. The verb hands it stderr, which exists from the first instruction of
//! the process; a test hands it a buffer and reads what an operator would have read.
//!
//! # Why it is a warning and not a refusal
//!
//! A declared name the box does not carry is not necessarily a mistake. A document can
//! legitimately name a credential that only some deployments set, and refusing the
//! launch would make one machine's optional variable another machine's outage. But it is
//! never a detail to swallow either: the agent is about to run without it, and the
//! failure that produces surfaces inside the agent's own error message, three layers
//! from the cause. So it is said out loud, by name, before anything is spawned.

use std::io::Write;

use aion_integrations::ChildEnvironment;
use anyhow::{Context, Result};

/// Writes the absence report for `environment` to `writer`, or nothing at all when every
/// declared variable is present.
///
/// Silence is the correct output for a complete environment: a line saying "nothing is
/// missing" on every start would train an operator to skip the one that matters.
///
/// # Errors
///
/// Returns an error when the report cannot be written — a closed stderr is a broken
/// promise to the operator, not a detail to swallow.
pub(super) fn report_absent_pass_through(
    writer: &mut impl Write,
    environment: &ChildEnvironment,
    document: &std::path::Path,
    task_queue: &str,
) -> Result<()> {
    let absent = environment.absent();
    if absent.is_empty() {
        return Ok(());
    }
    let names = absent.join(", ");
    let carried = if environment.pairs().is_empty() {
        "no variable it names".to_owned()
    } else {
        format!("{} of them", environment.pairs().len())
    };
    writeln!(
        writer,
        "warning: queue `{queue}` in {path} declares {count} environment pass-through \
         variable(s) that are NOT set on this machine, so the agent subprocess will run \
         without them: {names}.\n\
         \x20 The `env_pass` list is the agent's whole environment — this worker carries \
         {carried} — and an agent missing a credential or a home directory fails inside \
         its own error message, a long way from this cause. Set them here, or drop them \
         from the document's `harness` section.",
        queue = task_queue,
        path = document.display(),
        count = absent.len(),
    )
    .context("failed to write the environment pass-through report")
}

#[cfg(test)]
mod tests;
