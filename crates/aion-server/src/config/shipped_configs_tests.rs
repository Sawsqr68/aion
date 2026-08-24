//! The shipped hand-written configurations must boot the server they ship with.
//!
//! `docs/evidence/store-required-fields-shipped-configs.md` records the
//! standing rule: a change that adds a required-no-default field to the server
//! config MUST sweep every hand-written config in the tree in the same change.
//! This module is that rule's tripwire. Each shipped TOML is pushed through
//! the SAME functions the boot path uses, applied exactly where boot applies
//! them: [`crate::state::required_transcript_batch_policy`] on every config
//! (the transcript channel is always mounted, so its two batch bounds are
//! required on every backend), and [`crate::state::required_node_cache_budget`]
//! on haematite-backend configs only — haematite is the only backend with a
//! node cache, and boot enforces the budget at the haematite connect seam
//! alone. A new requirement that reaches the boot path without reaching the
//! shipped files fails here by construction, instead of stranding a reader
//! whose first `aion server` dies at the door; a shipped file teaching a
//! retired key fails the retired-key sweep beside it, because a fresh config
//! must not carry keys the server only warns about and ignores.
//!
//! Beside the plain TOML files, the sweep extracts every server config that
//! ships EMBEDDED in another file: the `ci/smoke-embed.sh` heredoc, and every
//! fenced `toml` code block declaring a `[server]` section in the conformance
//! README, the getting-started guide, and the operations guide — the fences are
//! selected by that content, never by position, so a new fence added above a
//! config block cannot silently retarget the sweep, and every extractor
//! fails loudly on a placeholder it does not know how to substitute. The
//! only hand-written configs left to the evidence document's manual rule are
//! the two cluster-demo heredocs (`scripts/demo/lib.sh`,
//! `scripts/mp-failover-spike.sh`), whose `[store]`/`[store.cluster]`
//! sections are assembled per-node by shell interpolation (`$peers`,
//! `$members`) and so have no complete TOML body to extract.

use std::path::Path;

use super::{ServerConfig, StoreBackend};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Repo-relative paths of the shipped configs that are plain TOML files. The
/// first-run scaffold is checked from its embedded copy in its own test
/// below, and the `aion new` template is pinned identical to that scaffold
/// from `[server]` onward by `scaffold_tests`, which covers every key the
/// sweep checks.
const SHIPPED_CONFIG_FILES: [&str; 4] = [
    "dev-config.toml",
    "examples/agent-dev/demo-config.toml",
    "examples/incident-triage/demo-config.toml",
    "docs/authoring/tutorial-examples/aion.toml",
];

/// Pulls every embedded server config out of one carrier file's text.
type ConfigExtractor = fn(&str) -> Result<Vec<String>, String>;

/// The shipped surfaces whose server configs are embedded in another file,
/// as `(source path, extractor)` — see the module doc for the selection
/// rules and for why the cluster-demo heredocs cannot join them.
const EMBEDDED_CONFIG_SOURCES: [(&str, ConfigExtractor); 4] = [
    ("ci/smoke-embed.sh", extract_smoke_embed_heredocs),
    (
        "conformance/aion-clients/README.md",
        extract_server_toml_fences,
    ),
    ("docs/GETTING-STARTED.md", extract_server_toml_fences),
    ("docs/operations/operations.md", extract_server_toml_fences),
];

#[test]
fn every_shipped_config_satisfies_the_boot_required_set() -> TestResult {
    let scratch = crate::test_support::private_tempdir()?;
    let mut failures = Vec::new();
    for relative in SHIPPED_CONFIG_FILES {
        match read_repo_file(relative) {
            Ok(bytes) => {
                if let Err(failure) = check_boot_required(relative, &bytes, scratch.path()) {
                    failures.push(failure);
                }
            }
            Err(failure) => failures.push(failure),
        }
    }
    for (relative, extract) in EMBEDDED_CONFIG_SOURCES {
        let extracted = read_repo_file(relative)
            .and_then(|bytes| {
                String::from_utf8(bytes)
                    .map_err(|utf8_error| format!("{relative} is not UTF-8: {utf8_error}"))
            })
            .and_then(|source| {
                extract(&source).map_err(|extract_error| format!("{relative}: {extract_error}"))
            });
        match extracted {
            Ok(configs) => {
                for (index, config) in configs.iter().enumerate() {
                    let name = format!("{relative} (embedded config {})", index + 1);
                    if let Err(failure) =
                        check_boot_required(&name, config.as_bytes(), scratch.path())
                    {
                        failures.push(failure);
                    }
                }
            }
            Err(failure) => failures.push(failure),
        }
    }
    assert!(
        failures.is_empty(),
        "shipped configs fail the boot-side required sweep:\n{}",
        failures.join("\n")
    );
    Ok(())
}

#[test]
fn the_first_run_scaffold_satisfies_the_boot_required_set() -> TestResult {
    let scratch = crate::test_support::private_tempdir()?;
    check_boot_required(
        "crates/aion-server/templates/first-run-config.toml",
        crate::config::FIRST_RUN_CONFIG.as_bytes(),
        scratch.path(),
    )?;
    Ok(())
}

/// Read a repo-relative file from the workspace this crate is built in.
fn read_repo_file(relative: &str) -> Result<Vec<u8>, String> {
    let path = format!("{}/../../{relative}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path)
        .map_err(|io_error| format!("cannot read shipped config `{path}`: {io_error}"))
}

/// The TOML that `ci/smoke-embed.sh` writes for the server it boots: the body
/// of its `cat >"$CONFIG" <<EOF` heredoc, with the script's two port
/// placeholders substituted by parseable stand-ins. Any shell-expansion token
/// left after substitution — braced or bare, since an unquoted heredoc
/// expands `${NAME}` and `$NAME` identically — means the script grew a
/// placeholder this extractor does not know, and that is a loud failure
/// naming the token rather than a silently mis-swept config.
fn extract_smoke_embed_heredocs(script: &str) -> Result<Vec<String>, String> {
    const OPENER: &str = "<<EOF\n";
    let start = script
        .find(OPENER)
        .ok_or_else(|| "no `<<EOF` heredoc opener found".to_owned())?
        + OPENER.len();
    let end = script[start..]
        .find("\nEOF\n")
        .ok_or_else(|| "no closing `EOF` line found for the heredoc".to_owned())?
        + start;
    let substituted = script[start..end]
        .replace("${HTTP_ADDR}", "127.0.0.1:18099")
        .replace("${GRPC_PORT}", "51099");
    if let Some(token) = residual_dollar_token(&substituted) {
        return Err(format!(
            "the heredoc carries a shell-expansion token this sweep does not \
             substitute: `{token}`"
        ));
    }
    Ok(vec![substituted])
}

/// Every fenced `toml` code block that declares a `[server]` section — the shape of a
/// whole server config, as opposed to keyword tables, worker manifests, and
/// add-this-section fragments — with the documents' `<repo>` path
/// placeholder substituted. A quoted value still starting with `<` after
/// substitution is a placeholder this extractor does not know, and fails
/// loudly naming its line.
fn extract_server_toml_fences(markdown: &str) -> Result<Vec<String>, String> {
    const OPENER: &str = "```toml\n";
    let mut configs = Vec::new();
    let mut cursor = 0;
    while let Some(found) = markdown[cursor..].find(OPENER) {
        let start = cursor + found + OPENER.len();
        let end = start
            + markdown[start..]
                .find("\n```")
                .ok_or_else(|| "a ```toml fence never closes".to_owned())?;
        let block = &markdown[start..end];
        if block.contains("[server]") {
            let substituted = block.replace("<repo>", ".");
            if let Some(line) = substituted.lines().find(|line| line.contains("\"<")) {
                return Err(format!(
                    "a config block carries a value placeholder this sweep does \
                     not substitute: `{line}`"
                ));
            }
            configs.push(substituted);
        }
        cursor = end;
    }
    if configs.is_empty() {
        return Err("no ```toml fence containing a [server] section found".to_owned());
    }
    Ok(configs)
}

/// The first shell-expansion token (`${NAME…` or `$NAME`) left in `text`,
/// carried whole so the refusal can name it. A `$` followed by anything that
/// cannot start an expansion (a digit, punctuation, end of text) is not one.
fn residual_dollar_token(text: &str) -> Option<String> {
    for (start, _) in text.match_indices('$') {
        let tail = &text[start + 1..];
        let mut token = String::from("$");
        let mut rest = tail.chars();
        match rest.next() {
            Some('{') => {
                token.push('{');
                for c in rest {
                    if c == '\n' {
                        break;
                    }
                    token.push(c);
                    if c == '}' {
                        break;
                    }
                }
                return Some(token);
            }
            Some(first) if first.is_ascii_alphabetic() || first == '_' => {
                token.push(first);
                for c in rest {
                    if c.is_ascii_alphanumeric() || c == '_' {
                        token.push(c);
                    } else {
                        break;
                    }
                }
                return Some(token);
            }
            _ => {}
        }
    }
    None
}

/// Parse one shipped config exactly as an embedded caller would (no
/// environment or CLI overlays, no dependence on the process working
/// directory — the file must stand on its own) and hold it to the boot
/// path's own requirement functions, each applied where boot applies it.
fn check_boot_required(name: &str, bytes: &[u8], scratch: &Path) -> Result<(), String> {
    let config = ServerConfig::from_slice_with_home_in(bytes, scratch, scratch)
        .map_err(|error| format!("{name} does not parse: {error}"))?;
    let (store, runtime) = config.into_parts();
    if store.lock_acquisition_patience_ms.is_some()
        || store.lock_acquisition_retry_cadence_ms.is_some()
    {
        return Err(format!(
            "{name} teaches the retired lock-acquisition keys; remove them \
             (the server warns about and ignores them)"
        ));
    }
    if matches!(store.backend, StoreBackend::Haematite)
        && let Err(error) = crate::state::required_node_cache_budget(&store)
    {
        return Err(format!(
            "{name} fails the boot-side store requirements: {error}"
        ));
    }
    if let Err(error) = crate::state::required_transcript_batch_policy(&runtime) {
        return Err(format!(
            "{name} fails the boot-side observability requirements: {error}"
        ));
    }
    Ok(())
}
