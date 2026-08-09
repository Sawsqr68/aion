//! Source fence for process-spawn CONFINEMENT.
//!
//! # Amendment (W-1..W-4, the lifecycle spine)
//!
//! This began as the W-0 "nothing spawns anywhere" fence, and W-0's exit
//! criterion ("nothing spawns yet") has now been discharged deliberately: the
//! managed-worker supervisor spawns real OS processes. The order is amended
//! rather than deleted, and what it guards is narrowed to the claim that is
//! still true and still load-bearing: **the durable record, wire, registry,
//! state, and HTTP layers spawn nothing; process spawning lives in exactly one
//! module.** One spawn site is what makes process-group containment provable
//! (RUNTIME-OPERATIONS R8); a second copy is how a grandchild comes to outlive
//! the thing that spawned it.
//!
//! The confinement claim has two halves and both are tested: the regions below
//! must NOT contain the process API, and the supervisor MUST — a fence whose
//! positive half is missing passes just as happily over a tree where nothing
//! spawns at all.
//!
//! This deliberately small lexical scan strips nested block comments, line
//! comments, normal string literals, and Rust raw string literals, then removes
//! whitespace and matches process API token sequences with identifier boundaries.
//! It can see direct source uses (including whitespace around `::`); it cannot
//! see APIs assembled by token-pasting macros, generated code, or identifiers
//! hidden behind another helper. The focused file/region list below is therefore
//! part of the contract and must move when a W-0 implementation site moves.

struct SourceRegion {
    name: &'static str,
    source: &'static str,
    start: Option<&'static str>,
    end: Option<&'static str>,
    /// A token the selected region must still contain. Every `end`-bounded
    /// region carries one: `end` splits on the FIRST occurrence of its marker,
    /// so a marker drifting earlier in the file would silently shrink the
    /// scanned region to almost nothing — the anchor makes that truncation a
    /// loud failure instead of a comfortable pass.
    anchor: Option<&'static str>,
}

const WORKER_DEPLOYMENT_REGIONS: &[SourceRegion] = &[
    SourceRegion {
        name: "aion-store worker-deployment entity",
        source: include_str!("../../aion-store/src/worker_deployment.rs"),
        start: None,
        end: None,
        anchor: None,
    },
    SourceRegion {
        name: "aion-store worker-deployment conformance",
        source: include_str!("../../aion-store/src/conformance/worker_deployment.rs"),
        start: None,
        end: None,
        anchor: None,
    },
    SourceRegion {
        name: "aion-store in-memory backend",
        source: include_str!("../../aion-store/src/memory.rs"),
        start: None,
        end: None,
        anchor: None,
    },
    SourceRegion {
        name: "aion-store-libsql worker-deployment backend",
        source: include_str!("../../aion-store-libsql/src/worker_deployment.rs"),
        start: None,
        end: None,
        anchor: None,
    },
    SourceRegion {
        name: "aion-store-libsql worker-deployment delegation",
        source: include_str!("../../aion-store-libsql/src/store.rs"),
        start: Some("impl WorkerDeploymentStore for LibSqlStore"),
        end: Some("impl WritableEventStore for LibSqlStore"),
        anchor: Some("put_worker_deployment"),
    },
    SourceRegion {
        name: "aion-store-haematite worker-deployment backend",
        source: include_str!("../../aion-store-haematite/src/store.rs"),
        start: Some("pub async fn write_raw_worker_deployment"),
        end: Some("impl NamespaceStore for HaematiteStore"),
        anchor: Some("list_worker_deployments"),
    },
    SourceRegion {
        name: "worker-deployment HTTP handlers",
        source: include_str!("../src/api/http/worker_deployments.rs"),
        start: None,
        end: None,
        anchor: None,
    },
    SourceRegion {
        name: "worker-deployment HTTP tests",
        source: include_str!("../src/api/http/worker_deployments_tests.rs"),
        start: None,
        end: None,
        anchor: None,
    },
    SourceRegion {
        name: "worker instance wire",
        source: include_str!("../../aion-proto/src/worker.rs"),
        start: None,
        end: None,
        anchor: None,
    },
    SourceRegion {
        name: "generated worker instance wire",
        source: include_str!("../../aion-proto-generated/proto/worker.proto"),
        start: None,
        end: None,
        anchor: None,
    },
    SourceRegion {
        name: "gRPC worker association mapping",
        source: include_str!("../src/api/worker_grpc.rs"),
        start: None,
        end: None,
        anchor: None,
    },
    SourceRegion {
        name: "cluster association wire",
        source: include_str!("../../aion-core/src/cluster_event.rs"),
        start: None,
        end: None,
        anchor: None,
    },
    SourceRegion {
        name: "worker registry association",
        source: include_str!("../src/worker/registry.rs"),
        start: None,
        end: None,
        anchor: None,
    },
    SourceRegion {
        name: "cluster stream worker-deployment snapshot",
        source: include_str!("../src/stream/cluster_stream.rs"),
        start: None,
        end: None,
        anchor: None,
    },
    SourceRegion {
        name: "server state worker-deployment store wiring",
        source: include_str!("../src/state.rs"),
        start: None,
        end: Some("#[cfg(test)]"),
        anchor: Some("worker_deployment_store"),
    },
    SourceRegion {
        name: "HTTP worker-deployment route mounting",
        source: include_str!("../src/api/http/router.rs"),
        start: None,
        end: None,
        anchor: None,
    },
    SourceRegion {
        name: "cluster worker-deployment event stamping",
        source: include_str!("../src/cluster.rs"),
        start: None,
        end: Some("#[cfg(test)]"),
        anchor: Some("WorkerDeploymentPut"),
    },
];

#[test]
fn worker_deployment_modules_do_not_spawn_processes() -> Result<(), String> {
    for region in WORKER_DEPLOYMENT_REGIONS {
        let source = select_region(region)?;
        if let Some(anchor) = region.anchor {
            if !source.contains(anchor) {
                return Err(format!(
                    "{} region no longer contains its production anchor `{anchor}` — \
                     the end marker has drifted and the scanned region silently shrank",
                    region.name
                ));
            }
        }
        let compact = compact_source(&strip_comments_and_strings(source));
        for forbidden in ["std::process", "tokio::process", "Command::new"] {
            if contains_token_sequence(&compact, forbidden) {
                return Err(format!(
                    "{} contains forbidden process API `{forbidden}`",
                    region.name
                ));
            }
        }
    }
    Ok(())
}

/// The positive half of the confinement claim, and the scanner's live control.
///
/// The supervisor's instance loop is the ONE place a managed worker process is
/// created. Asserting it here does two jobs at once: it fails loudly if the
/// spawn is ever moved or duplicated elsewhere, and it proves the scanner can
/// see a real spawn in the real tree — so a green result above means "these
/// regions are clean", not "the scanner found nothing anywhere".
#[test]
fn process_spawning_lives_in_the_supervisor_instance_loop() -> Result<(), String> {
    let source = include_str!("../src/worker/supervisor/instance.rs");
    let compact = compact_source(&strip_comments_and_strings(source));
    if contains_token_sequence(&compact, "Command::new") {
        Ok(())
    } else {
        Err(String::from(
            "the supervisor instance loop no longer spawns a process — either the spawn moved \
             (and this fence's region list must move with it) or the scanner has stopped \
             detecting spawns, in which case every clean verdict above is meaningless",
        ))
    }
}

#[test]
fn scanner_detects_process_code_and_ignores_comment_and_string_decoys() {
    let code = compact_source(&strip_comments_and_strings(
        "fn spawn() { Command::new(\"worker\"); }",
    ));
    assert!(contains_token_sequence(&code, "Command::new"));

    let decoys = compact_source(&strip_comments_and_strings(
        "// Command::new must not count\nlet token = \"Command::new\";",
    ));
    assert!(!contains_token_sequence(&decoys, "Command::new"));
}

fn compact_source(source: &str) -> String {
    source
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn select_region(region: &SourceRegion) -> Result<&'static str, String> {
    let after_start = match region.start {
        Some(marker) => region
            .source
            .split_once(marker)
            .map(|(_, suffix)| suffix)
            .ok_or_else(|| format!("{} start marker moved", region.name))?,
        None => region.source,
    };
    match region.end {
        Some(marker) => after_start
            .split_once(marker)
            .map(|(prefix, _)| prefix)
            .ok_or_else(|| format!("{} end marker moved", region.name)),
        None => Ok(after_start),
    }
}

fn contains_token_sequence(source: &str, needle: &str) -> bool {
    source.match_indices(needle).any(|(index, _)| {
        let before = source[..index].chars().next_back();
        let after = source[index + needle.len()..].chars().next();
        !before.is_some_and(is_identifier_character) && !after.is_some_and(is_identifier_character)
    })
}

fn is_identifier_character(character: char) -> bool {
    character == '_' || character.is_ascii_alphanumeric()
}

fn strip_comments_and_strings(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut output = String::with_capacity(bytes.len());
    let mut index = 0;
    let mut block_depth = 0_u32;
    while index < bytes.len() {
        if block_depth > 0 {
            if bytes[index..].starts_with(b"/*") {
                block_depth = block_depth.saturating_add(1);
                output.push_str("  ");
                index += 2;
            } else if bytes[index..].starts_with(b"*/") {
                block_depth = block_depth.saturating_sub(1);
                output.push_str("  ");
                index += 2;
            } else {
                output.push(if bytes[index] == b'\n' { '\n' } else { ' ' });
                index += 1;
            }
        } else if bytes[index..].starts_with(b"//") {
            while index < bytes.len() && bytes[index] != b'\n' {
                output.push(' ');
                index += 1;
            }
        } else if bytes[index..].starts_with(b"/*") {
            block_depth = 1;
            output.push_str("  ");
            index += 2;
        } else if let Some((prefix_len, hashes)) = raw_string_prefix(&bytes[index..]) {
            output.extend(std::iter::repeat_n(' ', prefix_len));
            index += prefix_len;
            let closing = format!("\"{}", "#".repeat(hashes));
            while index < bytes.len() && !bytes[index..].starts_with(closing.as_bytes()) {
                output.push(if bytes[index] == b'\n' { '\n' } else { ' ' });
                index += 1;
            }
            let close_len = closing.len().min(bytes.len().saturating_sub(index));
            output.extend(std::iter::repeat_n(' ', close_len));
            index += close_len;
        } else if bytes[index] == b'"' {
            output.push(' ');
            index += 1;
            while index < bytes.len() {
                let byte = bytes[index];
                output.push(if byte == b'\n' { '\n' } else { ' ' });
                index += 1;
                if byte == b'\\' && index < bytes.len() {
                    output.push(' ');
                    index += 1;
                } else if byte == b'"' {
                    break;
                }
            }
        } else {
            output.push(char::from(bytes[index]));
            index += 1;
        }
    }
    output
}

fn raw_string_prefix(bytes: &[u8]) -> Option<(usize, usize)> {
    let raw_start = if bytes.starts_with(b"br") {
        2
    } else if bytes.starts_with(b"r") {
        1
    } else {
        return None;
    };
    let hashes = bytes[raw_start..]
        .iter()
        .take_while(|byte| **byte == b'#')
        .count();
    (bytes.get(raw_start + hashes) == Some(&b'"')).then_some((raw_start + hashes + 1, hashes))
}
