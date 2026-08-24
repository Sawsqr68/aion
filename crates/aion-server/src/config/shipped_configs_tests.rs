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
//! Two swept surfaces are TOML embedded in another format's source — the
//! `ci/smoke-embed.sh` heredoc and the conformance README's fenced block —
//! and are extracted textually with their placeholders substituted. The
//! cluster-demo heredocs (`scripts/demo/lib.sh`, `scripts/mp-failover-spike.sh`)
//! interpolate whole sections structurally (`$peers`, `$members`) and cannot
//! be parsed from here; they stay under the evidence document's standing
//! manual rule.

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

/// Pulls one embedded TOML config out of its carrier file's text.
type ConfigExtractor = fn(&str) -> Result<String, String>;

/// The shipped surfaces whose TOML is embedded in another file, as
/// `(source path, extractor)` — see the module doc for why the cluster-demo
/// heredocs cannot join them.
const EMBEDDED_CONFIG_SOURCES: [(&str, ConfigExtractor); 2] = [
    ("ci/smoke-embed.sh", extract_smoke_embed_heredoc),
    (
        "conformance/aion-clients/README.md",
        extract_first_toml_fence,
    ),
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
        let outcome = read_repo_file(relative)
            .and_then(|bytes| {
                String::from_utf8(bytes)
                    .map_err(|utf8_error| format!("{relative} is not UTF-8: {utf8_error}"))
            })
            .and_then(|source| {
                extract(&source).map_err(|extract_error| format!("{relative}: {extract_error}"))
            })
            .and_then(|toml| check_boot_required(relative, toml.as_bytes(), scratch.path()));
        if let Err(failure) = outcome {
            failures.push(failure);
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
/// placeholders substituted by parseable stand-ins. A leftover `${` after
/// substitution means the script grew a placeholder this extractor does not
/// know, and that is a loud failure rather than a silently skipped sweep.
fn extract_smoke_embed_heredoc(script: &str) -> Result<String, String> {
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
    if substituted.contains("${") {
        return Err(format!(
            "the heredoc carries a placeholder this sweep does not substitute: {substituted}"
        ));
    }
    Ok(substituted)
}

/// The first ` ```toml ` fenced block of a markdown document, with the
/// conformance README's `<repo>` path placeholder substituted (the paths are
/// carried as strings and are not opened by config validation).
fn extract_first_toml_fence(markdown: &str) -> Result<String, String> {
    const OPENER: &str = "```toml\n";
    let start = markdown
        .find(OPENER)
        .ok_or_else(|| "no ```toml fenced block found".to_owned())?
        + OPENER.len();
    let end = markdown[start..]
        .find("\n```")
        .ok_or_else(|| "the ```toml fence never closes".to_owned())?
        + start;
    Ok(markdown[start..end].replace("<repo>", "."))
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
