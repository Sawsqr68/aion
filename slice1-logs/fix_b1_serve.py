"""B1: emit the absence report on BOTH paths, through a channel that exists there."""

import pathlib

p = pathlib.Path("crates/aion-cli/src/worker_agent/serve.rs")
s = p.read_text()

s = s.replace(
    """use super::args::{self, AgentArgs, ConnectionSettings};
use super::surface::{self, AdvertisedSurface};""",
    """use super::args::{self, AgentArgs, ConnectionSettings};
use super::environment_report::report_absent_pass_through;
use super::surface::{self, AdvertisedSurface};""",
)

s = s.replace(
    """    let declared = worker_surface::compile_harness(&document, &surface.task_queue)?;
    let harness = match &declared {
        Some(declared) => crate::harness::resolve_declared(declared, &document)?,
        None => None,
    };
    if args.check {""",
    """    let declared = worker_surface::compile_harness(&document, &surface.task_queue)?;
    let harness = match &declared {
        Some(declared) => crate::harness::resolve_declared(declared, &document)?,
        None => None,
    };
    // BEFORE the `--check` branch, and long before the serve loop installs its tracing
    // subscriber, so both paths carry it and neither depends on a listener that does not
    // exist yet. Stderr, because stdout on this verb is the `--check` report's and an
    // operator piping that into `jq` must not have a warning land in the middle of it.
    if let Some(resolved) = &harness {
        report_absent_pass_through(
            &mut std::io::stderr().lock(),
            resolved.environment(),
            &document,
            &surface.task_queue,
        )?;
    }
    if args.check {""",
)
p.write_text(s)
print("wired:", "report_absent_pass_through(" in s)

p = pathlib.Path("crates/aion-cli/src/worker_agent/mod.rs")
s = p.read_text()
s = s.replace(
    """mod args;
mod serve;""",
    """mod args;
mod environment_report;
mod serve;""",
)
# m4: the `pub(super)` widening had no consumer — a child module already sees a private
# sibling of its own parent.
s = s.replace("pub(super) mod serve;", "mod serve;")
s = s.replace(
    """//! * [`serve`] reads the document's declared harness,""",
    """//! * [`environment_report`] tells the operator which declared pass-through variables
//!   this machine does not carry — written to a real channel, because the first version
//!   of it logged into a subscriber that did not exist yet.
//! * [`serve`] reads the document's declared harness,""",
)
p.write_text(s)
print("mod declared:", "mod environment_report;" in s)
print("no pub(super) mod:", "pub(super) mod serve;" not in s)
