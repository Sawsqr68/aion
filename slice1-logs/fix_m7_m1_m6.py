"""m7 (a refusal that teaches), m1 (the broken contrast), M6 (a stale doc pointer)."""

import pathlib

# ---------------------------------------------------------------- m7
p = pathlib.Path("crates/aion-cli/src/main.rs")
s = p.read_text()

s = s.replace(
    """#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();""",
    """/// Every launch setting that moved out of `aion worker agent` and into the worker
/// document's `harness` section, with the field that replaced it.
///
/// This is a REFUSAL that teaches, never a compat shim: not one of these is accepted,
/// and the process still exits on clap's own argument error. What it adds is the
/// sentence clap cannot know — that the setting still exists, in a different file, under
/// a different name. Without it an operator reads "unexpected argument
/// `--acp-permission`" and has no way to tell a deleted setting from a typo.
const MIGRATED_LAUNCH_FLAGS: [(&str, &str); 14] = [
    ("--harness", "kind"),
    ("--concurrency", "concurrency"),
    ("--reconnect-initial-backoff", "reconnect_initial_backoff"),
    ("--reconnect-max-backoff", "reconnect_max_backoff"),
    ("--reconnect-max-attempts", "reconnect_max_attempts"),
    ("--acp-command", "command"),
    ("--acp-arg", "args"),
    ("--acp-cwd", "cwd"),
    ("--acp-permission", "permission"),
    ("--acp-env", "env_pass"),
    ("--acp-exit-grace", "exit_grace"),
    ("--norn-binary", "binary"),
    ("--norn-arg", "args"),
    ("--norn-unset-env", "env_pass"),
];

/// The extra sentence a deleted launch flag earns, when `argv` carries one.
///
/// Read off the RAW argv rather than off clap's error, because clap reports the first
/// unexpected argument and an operator pasting an old runbook line has typically
/// supplied several — naming one of six would send them round the loop six times.
///
/// `=`-joined forms (`--norn-arg=--account`) are matched on their flag half, because
/// that is the shape market-mirror's own runbook used.
fn migrated_flag_advice(argv: &[String]) -> Option<String> {
    let supplied: Vec<&(&str, &str)> = MIGRATED_LAUNCH_FLAGS
        .iter()
        .filter(|(flag, _)| {
            argv.iter().any(|argument| {
                argument == flag || argument.split_once('=').is_some_and(|(head, _)| head == *flag)
            })
        })
        .collect();
    if supplied.is_empty() {
        return None;
    }
    let moved = supplied
        .iter()
        .map(|(flag, field)| format!("  {flag}  ->  `{field}` in the document's `harness` section"))
        .collect::<Vec<_>>()
        .join("\\n");
    Some(format!(
        "\\nThese are not unknown arguments — they are DELETED ones. An agent worker's launch \\
         is declared in the `.awl` document it serves, in the `harness` section of its \\
         `worker` block, and there is no flag for any of it:\\n\\n{moved}\\n\\nRun `aion awl \\
         check <FILE.awl>` after writing the section: it names every setting the section is \\
         missing, scoped to the kind the section declares."
    ))
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            // clap's own rendering and its own exit code, unchanged — then the sentence
            // clap has no way to know.
            let advice = migrated_flag_advice(&std::env::args().collect::<Vec<_>>());
            let _ = error.print();
            if let Some(advice) = advice {
                eprintln!("{advice}");
            }
            return ExitCode::from(u8::try_from(error.exit_code()).unwrap_or(2));
        }
    };""",
)
p.write_text(s)
print("m7 wired:", "migrated_flag_advice" in s)

# ---------------------------------------------------------------- m1
p = pathlib.Path("crates/aion-cli/src/settings.rs")
s = p.read_text()
s = s.replace(
    """//! # Why this lives in one place
//!
//! Two independent groups of settings now share it — the agent worker's CONNECTION
//! settings ([`crate::worker_agent`]) and the per-harness CONFIGURATION the
//! composition root resolves after a harness is selected ([`crate::harness`]) — and
//! an operator cannot tell the two apart from the command line. If each resolved its
//! own way, one shell invocation could be told "supply `--identity`" on one run
//! and "supply `--identity`" on the next, when both were missing all along. So the
//! collection of misses is shared, and the refusal is rendered once from the whole
//! set.""",
    """//! # Why this lives in one place
//!
//! Two verbs' connection settings share it — the agent worker's
//! ([`crate::worker_agent`]) and the AWL shell worker's ([`crate::worker_awl`]) — and an
//! operator cannot tell one verb's resolution from the other's at a shell. If each
//! resolved its own way, one invocation missing both `--identity` and
//! `--liminal-address` could be told "supply `--identity`" on one run and "supply
//! `--liminal-address`" on the next, when both were missing all along. So the collection
//! of misses is shared, and the refusal is rendered once from the whole set.
//!
//! An agent worker's LAUNCH settings no longer pass through here at all: they are
//! declared in the worker document's `harness` section, and a missing one is named by
//! `aion awl check` against the document rather than by this contract against a command
//! line. That is a different surface with a different refusal, and deliberately so — the
//! document is reviewed and deployed once, where a command line is retyped on every
//! box.""",
)
p.write_text(s)
print("m1 fixed:", "retyped on every" in s)

# ---------------------------------------------------------------- M6
p = pathlib.Path("crates/aion-cli/src/harness/compose.rs")
s = p.read_text()
s = s.replace(
    """//! [`HarnessConfig`]'s variants are cfg-gated on the same features that link the
//! adapters, so a configuration for a harness this build does not carry is not a value
//! that exists. There is no unreachable arm to write and nothing to refuse: by the time
//! a configuration reaches this module, the adapter it names is present. The refusal for
//! a harness a build does not compile in lives where a supplied configuration is first
//! validated (`super::settings::Supplied::parse`), which already has an error channel.""",
    """//! [`HarnessConfig`]'s variants are cfg-gated on the same features that link the
//! adapters, so a configuration for a harness this build does not carry is not a value
//! that exists. There is no unreachable arm to write and nothing to refuse: by the time
//! a configuration reaches this module, the adapter it names is present. The refusal for
//! a harness a build does not compile in lives where the document's declared `kind` is
//! first checked against the catalogue (`super::catalogue::resolve`, reached from
//! `super::settings::resolve`), which already has an error channel and can name what the
//! build does carry.""",
)
p.write_text(s)
print("M6 fixed:", "super::catalogue::resolve" in s)
