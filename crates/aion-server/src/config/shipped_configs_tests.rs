//! The shipped hand-written configurations must boot the server they ship with.
//!
//! `docs/evidence/store-required-fields-shipped-configs.md` records the
//! standing rule: a change that adds a required-no-default field to the server
//! config MUST sweep every hand-written config in the tree in the same change.
//! This module is that rule's tripwire. Each shipped TOML is pushed through the
//! SAME functions the boot path uses to enforce the unconditional
//! required-no-default set — [`crate::state::required_node_cache_budget`] and
//! [`crate::state::required_transcript_batch_policy`] — so a new requirement
//! that reaches the boot path without reaching the shipped files fails here by
//! construction, instead of stranding a reader whose first `aion server` dies
//! at the door. A shipped file teaching a retired key fails the retired-key
//! sweep beside it, because a fresh config must not carry keys the server only
//! warns about and ignores.
//!
//! The demo-script heredocs (`scripts/demo/lib.sh`,
//! `scripts/mp-failover-spike.sh`) hand-write configs through shell
//! interpolation and cannot be parsed from here; the standing rule's manual
//! sweep still covers them.

use std::path::Path;

use super::ServerConfig;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Repo-relative paths of every hand-written shipped config that must parse
/// and satisfy the boot-side required set. The first-run scaffold is checked
/// from its embedded copy in the test below, and the `aion new` template is
/// pinned byte-identical to that scaffold by `scaffold_tests`, so both
/// template surfaces are covered without another cross-crate read.
const SHIPPED_CONFIGS: [&str; 4] = [
    "dev-config.toml",
    "examples/agent-dev/demo-config.toml",
    "examples/incident-triage/demo-config.toml",
    "docs/authoring/tutorial-examples/aion.toml",
];

#[test]
fn every_shipped_config_satisfies_the_boot_required_set() -> TestResult {
    let scratch = crate::test_support::private_tempdir()?;
    for relative in SHIPPED_CONFIGS {
        let path = format!("{}/../../{relative}", env!("CARGO_MANIFEST_DIR"));
        let bytes = std::fs::read(&path)
            .map_err(|io_error| format!("cannot read shipped config `{path}`: {io_error}"))?;
        check_boot_required(relative, &bytes, scratch.path())?;
    }
    Ok(())
}

#[test]
fn the_first_run_scaffold_satisfies_the_boot_required_set() -> TestResult {
    let scratch = crate::test_support::private_tempdir()?;
    check_boot_required(
        "crates/aion-server/templates/first-run-config.toml",
        crate::config::FIRST_RUN_CONFIG.as_bytes(),
        scratch.path(),
    )
}

/// Parse one shipped config exactly as an embedded caller would (no
/// environment or CLI overlays — the file must stand on its own) and hold it
/// to the boot path's own requirement functions.
fn check_boot_required(name: &str, bytes: &[u8], home: &Path) -> TestResult {
    let config = ServerConfig::from_slice_with_home(bytes, home)
        .map_err(|error| format!("{name} does not parse: {error}"))?;
    let (store, runtime) = config.into_parts();
    assert!(
        store.lock_acquisition_patience_ms.is_none()
            && store.lock_acquisition_retry_cadence_ms.is_none(),
        "{name} teaches the retired lock-acquisition keys; remove them \
         (the server warns about and ignores them)"
    );
    if let Err(error) = crate::state::required_node_cache_budget(&store) {
        return Err(format!("{name} fails the boot-side store requirements: {error}").into());
    }
    if let Err(error) = crate::state::required_transcript_batch_policy(&runtime) {
        return Err(
            format!("{name} fails the boot-side observability requirements: {error}").into(),
        );
    }
    Ok(())
}
