//! The agent worker's flag and environment vocabulary, and its resolution.
//!
//! # What this verb no longer takes
//!
//! Every LAUNCH setting — the harness kind, the concurrency, the three reconnect
//! settings, the ACP command/cwd/permission/exit-grace, the agent's arguments, and the
//! environment the agent subprocess gets — used to be a flag here. They are now
//! declared in the worker document's `harness` section, and the flags are DELETED. The
//! ruling behind that is one sentence long: keeping both would be a hand-maintained
//! drift pair, and a launch setting that can come from two places has already come from
//! the wrong one.
//!
//! What is left is the CONNECTION: where this worker dials, who it says it is, and
//! where it sits. Those are properties of this process on this box, not of the queue,
//! and two workers serving one document legitimately differ in all of them.
//!
//! # Nothing is invented
//!
//! Every remaining setting is supplied by a flag or by the named environment variable,
//! the flag wins, and a missing one is reported BY NAME — with every missing setting
//! listed at once, because a worker started from a shell should need one correction
//! rather than several. That is the same contract the Python worker host publishes, and
//! the variable names are the ones the rest of the product already reads.
//!
//! # Why the address is not `--endpoint`
//!
//! The global `--endpoint` is a gRPC target, and the agent-harness seam does not exist
//! on the gRPC worker transport: `AgentHarnessConfig` is threaded only through the
//! liminal serve path, and the server's own bridge notes that the gRPC arm carries no
//! agent binding. An agent worker therefore dials LIMINAL candidate addresses, which is
//! what `--liminal-address` supplies. Repeating it gives the redial driver an ordered
//! candidate list, so a worker whose server is `kill -9`'d migrates to a survivor and
//! re-registers there.

use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::Args;

use crate::settings::{self, Environment, Missing};

/// Ordered liminal candidate addresses, comma-separated.
const LIMINAL_ADDRESSES_ENV: &str = "AION_WORKER_LIMINAL_ADDRESSES";
/// The queue selector, when the document declares more than one `worker` block.
const TASK_QUEUE_ENV: &str = "AION_TASK_QUEUE";
/// The worker's stable build identity.
const IDENTITY_ENV: &str = "AION_WORKER_IDENTITY";

/// `aion worker agent` command-line surface.
#[derive(Clone, Debug, Args)]
pub(crate) struct AgentArgs {
    /// The `.awl` document declaring the queue, the actions to serve, and the
    /// `harness` section that says how to launch the agent.
    ///
    /// Optional at PARSE time only, so a bare invocation is refused with the shape it
    /// needs rather than clap's "required argument was not provided", which says
    /// nothing about the document a reader has never had to supply before.
    pub(crate) awl: Option<PathBuf>,
    /// Liminal listen address to dial, for example `127.0.0.1:50061`. Repeat it
    /// to supply ordered redial candidates: when the connected server dies, the
    /// worker migrates to the next candidate and re-registers there.
    ///
    /// Env: `AION_WORKER_LIMINAL_ADDRESSES` (comma-separated, same order). Not
    /// the global `--endpoint`: that is a gRPC target, and the agent-harness
    /// seam exists only on the liminal worker transport.
    #[arg(long = "liminal-address")]
    pub(crate) liminal_addresses: Vec<String>,
    /// Serve this queue instead of deriving it from the document.
    ///
    /// A document declaring exactly one `worker` block leaves nothing to choose,
    /// so the queue is derived from it. This selects among several.
    /// Env: `AION_TASK_QUEUE`.
    #[arg(long)]
    pub(crate) task_queue: Option<String>,
    /// Stable worker-build identity, used to correlate compatibility decisions.
    /// Env: `AION_WORKER_IDENTITY`.
    #[arg(long)]
    pub(crate) identity: Option<String>,
    /// Locality affinity advertised at registration. Defaults to this machine's
    /// hostname, the worker SDK's own documented default.
    #[arg(long)]
    pub(crate) node: Option<String>,
    /// Print the typed action surface this worker would advertise, then exit
    /// without connecting to anything.
    ///
    /// Those descriptors are the whole admission decision on a contract-bearing
    /// queue, so seeing them before dialling turns a remote contract-mismatch
    /// refusal into something readable at the desk.
    #[arg(long)]
    pub(crate) check: bool,
}

/// Every connection setting, resolved from flags and environment.
///
/// The LAUNCH settings are not here: they come from the document's `harness` section
/// and reach the serve path as an `aion_awl::CompiledHarness`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ConnectionSettings {
    /// Ordered liminal candidate addresses the redial driver cycles.
    pub(crate) addresses: Vec<String>,
    /// The worker's advertised identity.
    pub(crate) identity: String,
    /// Locality affinity, or `None` to take the SDK's hostname default.
    pub(crate) node: Option<String>,
}

impl AgentArgs {
    /// The queue selection, if the operator made one explicitly.
    ///
    /// `None` leaves the queue to be DERIVED from the document, which is
    /// unambiguous only when it declares exactly one `worker` block.
    pub(crate) fn requested_queue(&self, environment: &Environment) -> Option<String> {
        settings::optional(self.task_queue.clone(), TASK_QUEUE_ENV, environment)
    }

    /// The document this invocation serves, or the LOUD refusal for an invocation that
    /// named none.
    ///
    /// Deliberately not clap's own required-argument error: this verb used to take a
    /// dozen launch flags and now takes a document, so an operator arriving with the
    /// old shape in their fingers needs to be told what replaced it, not that an
    /// argument is missing.
    ///
    /// # Errors
    ///
    /// Returns an error naming the shape the verb takes now.
    pub(crate) fn document(&self) -> Result<&std::path::Path> {
        match &self.awl {
            Some(document) => Ok(document.as_path()),
            None => bail!(
                "`aion worker agent` serves the queue an `.awl` document declares, and no \
                 document was named.\n\n  aion worker agent <FILE.awl> --liminal-address \
                 <HOST:PORT> --identity <NAME>\n\nEverything about the LAUNCH — the harness \
                 kind, the concurrency, the reconnect settings, the agent's command, working \
                 directory, permission policy and shutdown grace, and the environment \
                 variables the agent subprocess is given — is declared in that document's \
                 `harness` section and is no longer a flag. Check a document with `aion awl \
                 check <FILE.awl>`; the checker names any setting it is missing."
            ),
        }
    }
}

/// Resolves every connection setting from `args` and `environment`.
///
/// Flags win over the environment. `node` falls through to the worker SDK's own
/// documented hostname default when absent, because that default is the library's and
/// not one invented here; every other setting must be supplied.
///
/// # Errors
///
/// Returns an error listing EVERY setting that has no source, each named by its flag
/// and its variable.
pub(crate) fn resolve(args: &AgentArgs, environment: &Environment) -> Result<ConnectionSettings> {
    let mut missing = Vec::new();
    let addresses = resolve_addresses(args, environment, &mut missing);
    let identity = settings::resolve_one(
        args.identity.clone(),
        IDENTITY_ENV,
        "--identity",
        environment,
        &mut missing,
    );
    settings::refuse_if_missing(missing)?;

    // Present by construction: the missing-list was empty, so `resolve_one` returned
    // `Some`. The `let else` refusal names the setting rather than unwrapping, so a
    // future edit that drops it from the missing-list is a reported fault instead of a
    // panic.
    let Some(identity) = identity else {
        bail!("--identity must be resolved before use");
    };
    Ok(ConnectionSettings {
        addresses,
        identity,
        node: args.node.clone(),
    })
}

/// The ordered liminal candidates: the repeated flag when given, otherwise the
/// comma-separated variable. Blank entries are dropped, so a trailing comma or a
/// stray space never becomes an address nobody can dial.
fn resolve_addresses(
    args: &AgentArgs,
    environment: &Environment,
    missing: &mut Vec<Missing>,
) -> Vec<String> {
    let from_flags = args
        .liminal_addresses
        .iter()
        .map(|address| address.trim().to_owned())
        .filter(|address| !address.is_empty())
        .collect::<Vec<_>>();
    if !from_flags.is_empty() {
        return from_flags;
    }
    let from_environment = environment
        .read(LIMINAL_ADDRESSES_ENV)
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|address| !address.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if from_environment.is_empty() {
        missing.push(Missing {
            flag: "--liminal-address",
            variable: LIMINAL_ADDRESSES_ENV,
        });
    }
    from_environment
}
