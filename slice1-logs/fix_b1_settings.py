"""B1: the absence is DATA on the resolved environment, not a log line nobody hears."""

import pathlib

p = pathlib.Path("crates/aion-cli/src/harness/settings.rs")
s = p.read_text()

s = s.replace(
    """/// The document names the variables; the box supplies their values. A name the box does
/// not carry contributes nothing and is reported, so a missing credential surfaces here
/// rather than three layers down inside an agent's own error message.""",
    """/// The document names the variables; the box supplies their values. A name the box does
/// not carry contributes nothing, and WHICH names those were rides on the returned
/// [`ChildEnvironment`] for the verb to report — see
/// [`crate::worker_agent::environment_report`] for why that report is a written value
/// and not a log line. Reporting it from HERE would emit it before any tracing
/// subscriber exists, which is exactly how it went to the floor the first time.""",
)

s = s.replace(
    """    let environment = declaration.resolve_from_process();
    if !environment.absent().is_empty() {
        tracing::warn!(
            absent = %environment.absent().join(", "),
            document = %document.display(),
            "these variables are named in the document's `env_pass` but are not set on this \\
             machine, so they will be absent from the agent child"
        );
    }
    Ok(environment)""",
    """    Ok(declaration.resolve_from_process())""",
)

s = s.replace(
    """#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResolvedHarness {
    /// Which compiled-in harness this is.
    pub(crate) kind: HarnessKind,
    /// How to launch it.
    pub(crate) config: HarnessConfig,
}""",
    """#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResolvedHarness {
    /// Which compiled-in harness this is.
    pub(crate) kind: HarnessKind,
    /// How to launch it.
    pub(crate) config: HarnessConfig,
}

impl ResolvedHarness {
    /// The child environment this launch was resolved with — the same value the adapter
    /// clears the child's environment down to.
    ///
    /// Reached THROUGH the resolved harness rather than returned alongside it so there is
    /// one environment in play: a caller cannot report on one value and launch with
    /// another.
    pub(crate) const fn environment(&self) -> &ChildEnvironment {
        match &self.config {
            #[cfg(feature = "norn")]
            HarnessConfig::Norn(norn) => &norn.environment,
            #[cfg(feature = "acp")]
            HarnessConfig::Acp(acp) => &acp.environment,
        }
    }
}""",
)
p.write_text(s)
print("tracing::warn removed:", "tracing::warn" not in s)
print("accessor added:", "fn environment(&self)" in s)
