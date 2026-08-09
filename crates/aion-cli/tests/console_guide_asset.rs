//! The ops console renders the same glossary the CLI prints, and this is what
//! keeps that true.
//!
//! The console cannot call into `aion-awl`: it is a browser application, and
//! it must teach the language with no server reachable — an author who cannot
//! deploy is exactly the author who needs to look a word up. So the glossary
//! is checked into the console's assets as JSON, which is a COPY, and a copy
//! is a thing that drifts. This test removes the drift by refusing any
//! difference between the checked-in asset and what
//! `aion awl guide --json` renders right now.
//!
//! The failure message names the one command that fixes it, because a gate
//! that reports a difference without saying how to resolve it teaches people
//! to edit the asset by hand, which is the drift arriving by another door.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The console asset, relative to the repository root.
const ASSET: &str = "apps/aion-ops-console/src/features/authoring/assets/awl-guide.json";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map_or_else(
            || unreachable!("crates/aion-cli always has two ancestors"),
            Path::to_path_buf,
        )
}

/// The `aion` binary built alongside this test.
///
/// Taken from the test executable's own directory rather than from `PATH`, so
/// the gate reads THIS tree's renderer and can never quietly compare the
/// asset against some other installed version of the CLI.
fn aion_binary() -> Option<PathBuf> {
    let mut path = std::env::current_exe().ok()?;
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    let candidate = path.join(if cfg!(windows) { "aion.exe" } else { "aion" });
    candidate.is_file().then_some(candidate)
}

#[test]
fn the_console_asset_is_what_the_cli_renders_today() {
    let asset_path = repo_root().join(ASSET);
    let checked_in = fs::read_to_string(&asset_path).unwrap_or_default();
    assert!(
        !checked_in.is_empty(),
        "{ASSET} is missing or empty; regenerate it with `aion awl guide --json > {ASSET}`"
    );

    // An unmeasured run must be RED, never quietly green. A skip written to
    // stderr would be captured by the harness on a passing test and read as a
    // comparison that happened, so the absence of the binary fails here
    // instead. `cargo test -p aion-cli` builds it.
    let found = aion_binary();
    assert!(
        found.is_some(),
        "the `aion` binary is not built beside this test, so the asset was not compared; run \
         `cargo test -p aion-cli`, which builds it"
    );
    let Some(binary) = found else {
        return;
    };

    let rendered = Command::new(&binary)
        .args(["awl", "guide", "--json"])
        .output();
    let Ok(rendered) = rendered else {
        unreachable!("the aion binary beside this test must be runnable");
    };
    assert!(
        rendered.status.success(),
        "`aion awl guide --json` failed: {}",
        String::from_utf8_lossy(&rendered.stderr)
    );
    let rendered = String::from_utf8_lossy(&rendered.stdout).into_owned();

    assert_eq!(
        checked_in, rendered,
        "{ASSET} no longer matches the language's glossary. Regenerate it:\n    aion awl guide \
         --json > {ASSET}"
    );
}

/// The asset is only useful to the console if it is what the console can
/// actually read: valid JSON, with the shape the panel indexes.
#[test]
fn the_console_asset_is_readable_json_with_the_shape_the_panel_uses() {
    let asset_path = repo_root().join(ASSET);
    let checked_in = fs::read_to_string(&asset_path).unwrap_or_default();
    let parsed: serde_json::Value =
        serde_json::from_str(&checked_in).unwrap_or(serde_json::Value::Null);
    let entries = parsed["entries"].as_array().cloned().unwrap_or_default();
    assert!(!entries.is_empty(), "{ASSET} carries no entries");
    for entry in &entries {
        for field in ["word", "kind", "summary", "citation"] {
            assert!(
                entry[field].as_str().is_some_and(|text| !text.is_empty()),
                "an entry is missing `{field}`: {entry}"
            );
        }
        assert!(
            entry["section"]["number"].as_u64().is_some(),
            "an entry is missing its section number: {entry}"
        );
        assert!(
            entry["section"]["title"]
                .as_str()
                .is_some_and(|title| !title.is_empty()),
            "an entry is missing its section title: {entry}"
        );
    }
}
