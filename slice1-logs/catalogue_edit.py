"""Rewrite the harness catalogue for document-declared harness kinds."""

import pathlib

p = pathlib.Path("crates/aion-cli/src/harness/catalogue.rs")
s = p.read_text()

s = s.replace(
    '''//! # Harness identity is resolved, never assumed
//!
//! A build can carry several adapters, so "the" harness is not a compile-time
//! constant. The resolution is the same three-case shape
//! [`crate::worker_surface::select_worker`] uses for a task queue, for the same
//! reason — the difference between DERIVING a value and ASSUMING one is whether
//! anything was left to choose:
//!
//! * exactly one compiled in → derived, because there is no other answer;
//! * several, none selected → refused, naming every choice, because picking one would
//!   silently run an operator\'s agent through an adapter they did not ask for;
//! * a name this build does not carry → refused, naming what it does carry, because
//!   the operator\'s mental model of the binary is wrong and only the binary can say
//!   how;
//! * none compiled in → `None`, which is NOT an error here: the action surface is
//!   still derived and reported (`--check` must work on a build that could not serve),
//!   and the serve path refuses with the build-shaped message it already had.''',
    '''//! # Harness identity is DECLARED, never assumed
//!
//! A build can carry several adapters, so "the" harness is not a compile-time constant
//! — and it is not a flag either. The document\'s `harness` section names its `kind`,
//! because the harness a queue is served by is a property of that queue and not of
//! whoever happened to type the command. This module\'s whole job is to check the
//! declared name against what the running binary actually links:
//!
//! * the build carries it → resolved;
//! * a name this build does not carry → refused, naming what it does carry, because a
//!   harness is compiled in by a cargo feature and cannot be supplied at runtime;
//! * none compiled in → `None`, which is NOT an error here: the action surface is
//!   still derived and reported (`--check` must work on a build that could not serve),
//!   and the serve path refuses with the build-shaped message it already had.''',
)

s = s.replace(
    '''    /// The name `--harness` accepts, and the name every report prints.
    ///
    /// Short and stable: it is what an operator types, so it must not carry a crate
    /// name or a version that could change under them.''',
    '''    /// The name a document\'s `harness kind` declares, and the name every report prints.
    ///
    /// Short and stable: it is what an operator writes into a deployed document, so it
    /// must not carry a crate name or a version that could change under them.''',
)

s = s.replace(
    '''    /// Which group of configuration settings this harness reads.
    ///
    /// The hop through an unconditional type is what keeps every cfg-gated match in
    /// this file, on methods that take nothing but `self`: a build with no adapter
    /// makes [`HarnessKind`] uninhabited, and a function that matched it while also
    /// taking arguments would leave those arguments provably unused.
    pub(crate) const fn settings(self) -> super::settings::HarnessSettings {
        match self {
            #[cfg(feature = "norn")]
            Self::Norn => super::settings::HarnessSettings::Norn,
            #[cfg(feature = "acp")]
            Self::Acp => super::settings::HarnessSettings::Acp,
        }
    }

''',
    "",
)

s = s.replace(
    '''            #[cfg(feature = "norn")]
            Self::Norn => HarnessConfig::Norn(super::settings::NornConfig {
                binary: None,
                args: Vec::new(),
                unset_env: Vec::new(),
            }),
            #[cfg(feature = "acp")]
            Self::Acp => HarnessConfig::Acp(super::settings::AcpConfig {
                command: std::path::PathBuf::from("opencode"),
                args: vec![String::from("acp")],
                cwd: std::path::PathBuf::from("/srv/workspace"),
                permission: aion_integration_acp::PermissionPolicy::Deny,
                env: Vec::new(),
                exit_grace: std::time::Duration::from_secs(2),
            }),''',
    '''            #[cfg(feature = "norn")]
            Self::Norn => HarnessConfig::Norn(super::settings::NornConfig {
                binary: None,
                args: Vec::new(),
                environment: super::test_environment(),
            }),
            #[cfg(feature = "acp")]
            Self::Acp => HarnessConfig::Acp(super::settings::AcpConfig {
                command: std::path::PathBuf::from("/usr/local/bin/opencode"),
                args: vec![String::from("acp")],
                cwd: std::path::PathBuf::from("/srv/workspace"),
                permission: aion_integration_acp::PermissionPolicy::Deny,
                environment: super::test_environment(),
                exit_grace: std::time::Duration::from_secs(2),
            }),''',
)

s = s.replace(
    '''/// Resolves WHICH compiled-in harness this invocation serves.
///
/// `requested` is an explicit selection (`requested_by` names where it came from, for
/// the refusal text). See the module documentation for why each case resolves the way
/// it does.
///
/// # Errors
///
/// Returns an error when `requested` names a harness this build does not carry
/// (naming what it does carry), or when the build carries several and none was
/// selected (naming every choice).
pub(crate) fn resolve(requested: Option<&str>, requested_by: &str) -> Result<Option<HarnessKind>> {
    let compiled = compiled();
    match requested {
        Some(name) => match compiled.iter().copied().find(|kind| kind.name() == name) {
            Some(kind) => Ok(Some(kind)),
            None => bail!(
                "{requested_by} selects agent harness `{name}`, which this `aion` build does not \\
                 carry (it carries: {}). A harness is compiled in by a cargo feature, so a name \\
                 this binary does not know cannot be supplied at runtime — rebuild with the \\
                 feature, or select one of the harnesses above.",
                carried()
            ),
        },
        None => match compiled.as_slice() {
            [only] => Ok(Some(*only)),
            // Nothing to serve with. Deliberately not an error: the caller still
            // derives and reports the action surface, and refuses to SERVE with a
            // message naming the missing piece of the build.
            [] => Ok(None),
            several => bail!(
                "this `aion` build carries {} agent harnesses, so which one to serve is \\
                 ambiguous; select one with {requested_by}:\\n{}",
                several.len(),
                several
                    .iter()
                    .map(|harness| format!("  {}", harness.description()))
                    .collect::<Vec<_>>()
                    .join("\\n")
            ),
        },
    }
}''',
    '''/// Resolves the harness `document` declares against the adapters this binary links.
///
/// `declared` is the `kind` word from the document\'s `harness` section — always
/// present, because the checker refuses a section without one. See the module
/// documentation for why each case resolves the way it does.
///
/// # Errors
///
/// Returns an error when `declared` names a harness this build does not carry, naming
/// what it does carry.
pub(crate) fn resolve(declared: &str, document: &std::path::Path) -> Result<Option<HarnessKind>> {
    let compiled = compiled();
    // Nothing to serve with. Deliberately not an error: the caller still derives and
    // reports the action surface, and refuses to SERVE with a message naming the
    // missing piece of the build.
    if compiled.is_empty() {
        return Ok(None);
    }
    match compiled
        .iter()
        .copied()
        .find(|kind| kind.name() == declared)
    {
        Some(kind) => Ok(Some(kind)),
        None => bail!(
            "{} declares `harness kind {declared}`, which this `aion` build does not carry (it \\
             carries: {}). A harness is compiled in by a cargo feature, so a name this binary \\
             does not know cannot be supplied at runtime — rebuild with the feature, or declare \\
             one of the harnesses above.",
            document.display(),
            carried()
        ),
    }
}''',
)

s = s.replace(
    '''/// Announces the agent harnesses composed into this binary at server start.
///
/// Reports the whole compiled-in SET rather than one harness, because at server start
/// nothing has been selected: which harness a worker serves is decided per
/// `aion worker agent` invocation, and an ACP harness additionally needs configuration
/// no server-start report has. Constructing one here would therefore mean inventing a
/// launch command, so this reports what the build carries and names the flag that
/// chooses among them.''',
    '''/// Announces the agent harnesses composed into this binary at server start.
///
/// Reports the whole compiled-in SET rather than one harness, because at server start
/// nothing has been selected: which harness a worker serves is DECLARED by the worker
/// document it is started from, and that document also carries the launch settings no
/// server-start report has. Constructing one here would therefore mean inventing a
/// launch command, so this reports what the build carries and names where the choice is
/// written.''',
)

s = s.replace(
    '''    if compiled.len() > 1 {
        eprintln!(
            "aion server: {} harnesses are composed in, so `aion worker agent` requires \\
             --harness <{}> to select one.",
            compiled.len(),
            carried()
        );
    }''',
    '''    if compiled.len() > 1 {
        eprintln!(
            "aion server: {} harnesses are composed in, so a worker document\\'s `harness` \\
             section selects one by name with `kind <{}>`.",
            compiled.len(),
            carried()
        );
    }''',
)
p.write_text(s)
print("settings() present:", "fn settings(self)" in s)
print("resolve rewritten:", "declares `harness kind" in s)
