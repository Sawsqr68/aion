//! Freshness guard for the committed SDK closure: rebuild the oracle Gleam
//! project (`examples/awl-hello`, the exact tree `regen-awl-sdk-closure.sh`
//! harvests) and export-set-compare the fresh harvest against the committed
//! `sdk-closure/` beams — the same rebuild-and-compare pattern as the
//! ops-console embed freshness guard.
//!
//! A stale committed bundle (an SDK edit without a closure regeneration)
//! fails here in CI instead of failing at runtime in a deployed direct
//! archive. Requires a local `gleam` toolchain, exactly like the AWL emitter
//! compile-proof tests.
//!
//! Comparison runs on two axes, because neither alone catches what the guard
//! exists to catch:
//!
//! 1. **Export sets** — the wire surface generated code links against. This
//!    axis produces the better diagnostic: it names the functions gained and
//!    lost.
//! 2. **Behavioural chunk bytes** — what actually *runs*. An SDK edit that
//!    changes a function body (a bug fix, a corrected branch, a changed error
//!    message) changes neither the module set nor any export set, so axis 1 is
//!    blind to precisely the case the guard was written for. The closure ships
//!    as bytes and the bytes execute in a deployed direct archive.
//!
//! Axis 2 compares an ALLOWLIST of chunks rather than whole files. A beam
//! carries `CInf` (compile time, source path, options) and other metadata that
//! differs between two builds of identical source, so whole-file equality would
//! flap. An allowlist — rather than an exclude list — means an unrecognised or
//! newly-introduced volatile chunk cannot leak in and make the gate a coin
//! flip: anything not named here is ignored by construction.
//!
//! The remedy for a failure on either axis is the same single action — run
//! `scripts/regen-awl-sdk-closure.sh` and commit the result — so the messages
//! name which axis diverged only to tell an SDK edit from a toolchain move at a
//! glance.

#[path = "test_support/gleam.rs"]
mod gleam_test_support;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use aion_awl_package::{sdk_closure_modules, sdk_closure_version};
use beamr::atom::AtomTable;
use beamr::loader::load_beam_chunks;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// SDK test-only modules the regeneration script excludes from the
/// `aion_flow` package (mirrors `excluded()` in
/// `scripts/regen-awl-sdk-closure.sh`).
fn excluded_sdk_module(package: &str, module: &str) -> bool {
    package == "aion_flow"
        && (module == "aion_flow_ffi"
            || module == "aion@testing"
            || module.starts_with("aion@testing@"))
}

/// Exported `(function, arity)` set of one beam file.
fn export_set(bytes: &[u8]) -> Result<BTreeSet<(String, u32)>, Box<dyn std::error::Error>> {
    let atoms = AtomTable::with_common_atoms();
    let parsed = load_beam_chunks(bytes, &atoms)?;
    let mut set = BTreeSet::new();
    for export in &parsed.exports {
        let function = atoms
            .resolve(export.function)
            .ok_or_else(|| format!("unresolvable export atom: {:?}", export.function))?;
        set.insert((function.to_owned(), u32::from(export.arity)));
    }
    Ok(set)
}

/// Beam chunks whose bytes decide what the module DOES.
///
/// `Code` is the instruction stream; `AtU8`/`Atom` the atom table those
/// instructions index; `LitT`/`StrT` the literal and string tables they load
/// from; `ImpT`/`ExpT`/`LocT` the import, export and local-function tables that
/// resolve calls; `FunT` the closure table. Every one is a deterministic
/// function of source plus compiler, so an unchanged source rebuilt by an
/// unchanged toolchain reproduces them exactly.
///
/// Everything else — `CInf` (compile timestamp, source path, options), `Dbgi`,
/// `Docs`, `Line` — is metadata or debug information and is ignored by
/// omission. An allowlist is deliberate: an exclude list would have to be
/// extended every time a toolchain introduces a new volatile chunk, and the
/// failure mode of forgetting is a gate that flaps.
const BEHAVIOURAL_CHUNKS: &[&[u8; 4]] = &[
    b"Atom", b"AtU8", b"Code", b"ExpT", b"FunT", b"ImpT", b"LitT", b"LocT", b"StrT",
];

/// The behaviour-deciding chunks of one beam file, keyed by chunk id.
///
/// Walks the IFF container directly (`FOR1` + length + `BEAM`, then
/// id/length/payload records padded to a four-byte boundary) rather than going
/// through a beam loader: the guard must compare the bytes as shipped, and a
/// loader that normalised or re-encoded anything would compare its own output
/// instead of the artifact.
fn behavioural_chunks(
    bytes: &[u8],
) -> Result<std::collections::BTreeMap<String, Vec<u8>>, Box<dyn std::error::Error>> {
    if bytes.len() < 12 || &bytes[0..4] != b"FOR1" || &bytes[8..12] != b"BEAM" {
        return Err("not a BEAM IFF container".into());
    }
    let mut chunks = std::collections::BTreeMap::new();
    let mut offset = 12usize;
    while offset + 8 <= bytes.len() {
        let id: [u8; 4] = bytes[offset..offset + 4].try_into()?;
        let size = u32::from_be_bytes(bytes[offset + 4..offset + 8].try_into()?) as usize;
        let start = offset + 8;
        let end = start
            .checked_add(size)
            .ok_or("beam chunk length overflows the file")?;
        if end > bytes.len() {
            return Err(format!(
                "beam chunk `{}` claims {size} bytes but only {} remain",
                String::from_utf8_lossy(&id),
                bytes.len() - start
            )
            .into());
        }
        if BEHAVIOURAL_CHUNKS.contains(&&id) {
            chunks.insert(
                String::from_utf8_lossy(&id).into_owned(),
                bytes[start..end].to_vec(),
            );
        }
        // Chunk payloads are padded to a four-byte boundary; the pad is not
        // part of the chunk and is never compared.
        offset = end + (4 - (size % 4)) % 4;
    }
    Ok(chunks)
}

/// Chunk ids that differ between two beams, restricted to the behavioural set.
///
/// Returns the ids rather than a boolean so a failure names what moved: a
/// `Code`-only divergence reads as a source edit, while a whole-set divergence
/// reads as a toolchain move.
fn divergent_chunks(
    fresh: &[u8],
    committed: &[u8],
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let fresh = behavioural_chunks(fresh)?;
    let committed = behavioural_chunks(committed)?;
    // A BTreeSet, not `dedup()`: chaining two sorted key sets interleaves
    // them, so duplicates are not adjacent and `dedup()` would report every
    // divergent chunk twice.
    let divergent: BTreeSet<String> = fresh
        .keys()
        .chain(committed.keys())
        .filter(|id| fresh.get(*id) != committed.get(*id))
        .cloned()
        .collect();
    Ok(divergent.into_iter().collect())
}

/// Flip the last byte of one named chunk's payload, in place.
///
/// Length-preserving on purpose: every following chunk keeps its offset, so the
/// result is a well-formed container that differs from the original in exactly
/// one chunk. Returns `None` when the beam has no such chunk or its payload is
/// empty.
fn flip_last_byte_of_chunk(bytes: &[u8], target: [u8; 4]) -> Option<Vec<u8>> {
    let mut offset = 12usize;
    while offset + 8 <= bytes.len() {
        let id: [u8; 4] = bytes[offset..offset + 4].try_into().ok()?;
        let size = u32::from_be_bytes(bytes[offset + 4..offset + 8].try_into().ok()?) as usize;
        let start = offset + 8;
        let end = start.checked_add(size)?;
        if end > bytes.len() {
            return None;
        }
        if id == target && size > 0 {
            let mut mutated = bytes.to_vec();
            mutated[end - 1] ^= 0xFF;
            return Some(mutated);
        }
        offset = end + (4 - (size % 4)) % 4;
    }
    None
}

/// A committed closure module, for the mechanism pins below.
fn a_committed_beam() -> Result<(String, &'static [u8]), Box<dyn std::error::Error>> {
    sdk_closure_modules()
        .next()
        .map(|(name, bytes)| (name.to_owned(), bytes))
        .ok_or_else(|| "the committed SDK closure is empty".into())
}

/// **The pin.** A changed function body with the export surface held constant
/// is invisible to the export-set axis and caught by the byte axis.
///
/// This is the whole finding behind #115: the export-set comparison was chosen
/// precisely because beam bytes are volatile, and the exclusion swallowed the
/// inclusion — "an SDK edit without a closure regeneration" is the case the
/// guard exists for, and it changes no export set. Asserting the export sets
/// are EQUAL first is what makes the byte assertion non-vacuous: without it, a
/// mutation that happened to alter exports would satisfy the second assertion
/// while proving nothing about blindness.
///
/// Runs without a `gleam` toolchain: it mutates a committed beam rather than
/// harvesting a fresh one, so the mechanism is gated in every battery even
/// where the oracle project cannot be built.
#[test]
fn a_changed_body_is_invisible_to_exports_and_caught_by_bytes() -> TestResult {
    // Try the behavioural chunks a body edit actually moves. `Code` is the
    // instruction stream; `StrT`/`LitT` hold the string and literal tables a
    // changed error message lands in. Whichever beam and chunk are used, the
    // mutant must still parse — a corrupted beam that fails to load would prove
    // nothing about export-set blindness. Search the closure rather than pinning
    // its first module: compiler releases may make a particular module's final
    // byte structural while another module still carries a safe behavioural byte.
    let mut proven = None;
    'modules: for (module, committed) in sdk_closure_modules() {
        let committed_exports = export_set(committed)?;
        for chunk in [*b"Code", *b"StrT", *b"LitT"] {
            let Some(mutated) = flip_last_byte_of_chunk(committed, chunk) else {
                continue;
            };
            let Ok(mutated_exports) = export_set(&mutated) else {
                continue;
            };
            if mutated_exports != committed_exports {
                continue;
            }
            proven = Some((
                module.to_owned(),
                committed,
                String::from_utf8_lossy(&chunk).into_owned(),
                mutated,
            ));
            break 'modules;
        }
    }
    let (module, committed, chunk, mutated) = proven.ok_or(
        "no behavioural chunk in the committed closure could be perturbed while holding the \
         export set constant — the pin cannot demonstrate what it exists to demonstrate",
    )?;

    assert_eq!(
        export_set(&mutated)?,
        export_set(committed)?,
        "`{module}`: the export-set axis was expected to be BLIND to a changed {chunk} chunk"
    );
    let divergent = divergent_chunks(&mutated, committed)?;
    assert_eq!(
        divergent,
        vec![chunk.clone()],
        "`{module}`: the byte axis must catch a changed {chunk} chunk, and name exactly it"
    );
    Ok(())
}

/// Control: without this, a byte axis that reported divergence unconditionally
/// would satisfy the pin above and fail every real harvest.
#[test]
fn byte_identical_beams_do_not_diverge() -> TestResult {
    let (module, committed) = a_committed_beam()?;
    assert!(
        divergent_chunks(committed, committed)?.is_empty(),
        "`{module}`: a beam must not diverge from itself"
    );
    Ok(())
}

/// Control for the allowlist: `CInf` carries compile time, source path and
/// compiler options, so it differs between two builds of identical source.
/// If it were compared, the gate would fail on every rebuild and be useless.
#[test]
fn a_changed_metadata_chunk_does_not_diverge() -> TestResult {
    let (module, committed) = a_committed_beam()?;
    let mutated = flip_last_byte_of_chunk(committed, *b"CInf")
        .ok_or_else(|| format!("`{module}` carries no CInf chunk to perturb"))?;
    assert_ne!(
        mutated,
        committed.to_vec(),
        "`{module}`: the CInf perturbation must actually change the file, or this control is \
         vacuous"
    );
    assert!(
        divergent_chunks(&mutated, committed)?.is_empty(),
        "`{module}`: a changed CInf chunk is compile metadata and must NOT trip the byte axis"
    );
    Ok(())
}

/// `aion_flow`'s locked version in the oracle project's lockfile.
fn oracle_sdk_version(manifest: &str) -> Result<String, Box<dyn std::error::Error>> {
    for line in manifest.lines() {
        if let Some(rest) = line
            .trim()
            .strip_prefix("{ name = \"aion_flow\", version = \"")
            && let Some((version, _)) = rest.split_once('"')
        {
            return Ok(version.to_owned());
        }
    }
    Err("oracle manifest.toml has no aion_flow entry".into())
}

/// Rebuild the oracle project and compare the fresh harvest with the
/// committed closure: same package set, same module set (modulo the script's
/// SDK test-only exclusions), same per-module export sets, and the committed
/// version stamp equals the oracle's locked `aion_flow` version.
#[test]
fn committed_closure_matches_a_fresh_oracle_harvest() -> TestResult {
    if crate::gleam_test_support::skip_if_unavailable() {
        return Ok(());
    }
    let root = workspace_root();
    let oracle = root.join("examples/awl-hello");
    // Proves the oracle's own modules were really compiled, retries a
    // transient Hex registry, and names an unreachable registry as
    // infrastructure instead of as a stale-closure verdict. A zero exit that
    // compiled nothing would otherwise let this guard compare the committed
    // closure against a stale `build/` tree.
    crate::gleam_test_support::compile_whole_project(&oracle)?;

    let manifest = fs::read_to_string(oracle.join("manifest.toml"))?;
    assert_eq!(
        oracle_sdk_version(&manifest)?,
        sdk_closure_version(),
        "the committed closure's version stamp lags the oracle's locked aion_flow version — \
         run scripts/regen-awl-sdk-closure.sh and commit the result"
    );

    // Fresh harvest: every built production package (everything under
    // build/dev/erlang except the oracle project itself) and its module set.
    let erlang = oracle.join("build/dev/erlang");
    let mut fresh: BTreeSet<String> = BTreeSet::new();
    let mut fresh_beams: Vec<(String, PathBuf)> = Vec::new();
    for entry in fs::read_dir(&erlang)? {
        let entry = entry?;
        let package = entry.file_name().to_string_lossy().into_owned();
        if package == "awl_hello" {
            continue;
        }
        let ebin = entry.path().join("ebin");
        if !ebin.is_dir() {
            continue;
        }
        for beam in fs::read_dir(&ebin)? {
            let beam = beam?.path();
            if beam.extension().is_none_or(|extension| extension != "beam") {
                continue;
            }
            let Some(module) = beam
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
            else {
                continue;
            };
            if excluded_sdk_module(&package, &module) {
                continue;
            }
            fresh.insert(module.clone());
            fresh_beams.push((module, beam));
        }
    }

    let committed: BTreeSet<String> = sdk_closure_modules()
        .map(|(name, _)| name.to_owned())
        .collect();
    let missing: Vec<&String> = fresh.difference(&committed).collect();
    let stale: Vec<&String> = committed.difference(&fresh).collect();
    assert!(
        missing.is_empty() && stale.is_empty(),
        "committed closure module set differs from a fresh harvest — run \
         scripts/regen-awl-sdk-closure.sh and commit the result.\n  fresh-only: {missing:?}\n  \
         committed-only: {stale:?}"
    );

    // Export-set equality per module: the wire surface generated code links
    // against must be exactly what ships.
    let mut skewed = Vec::new();
    // Behavioural-byte equality per module: what the wire surface links against
    // is not what runs. A changed function body moves these and moves nothing
    // above.
    let mut byte_skewed = Vec::new();
    for (module, path) in fresh_beams {
        let fresh_bytes = fs::read(&path)?;
        let fresh_exports = export_set(&fresh_bytes)?;
        let committed_bytes = sdk_closure_modules()
            .find(|(name, _)| *name == module)
            .map(|(_, bytes)| bytes)
            .ok_or_else(|| format!("module `{module}` vanished from the committed closure"))?;
        let committed_exports = export_set(committed_bytes)?;
        if fresh_exports != committed_exports {
            let gained: Vec<String> = fresh_exports
                .difference(&committed_exports)
                .map(|(function, arity)| format!("{function}/{arity}"))
                .collect();
            let lost: Vec<String> = committed_exports
                .difference(&fresh_exports)
                .map(|(function, arity)| format!("{function}/{arity}"))
                .collect();
            skewed.push(format!("{module} (+{gained:?} -{lost:?})"));
        }
        let divergent = divergent_chunks(&fresh_bytes, committed_bytes)?;
        if !divergent.is_empty() {
            byte_skewed.push(format!("{module} ({})", divergent.join(", ")));
        }
    }
    assert!(
        skewed.is_empty(),
        "committed closure export sets lag the SDK source — run \
         scripts/regen-awl-sdk-closure.sh and commit the result: {skewed:?}"
    );
    assert!(
        byte_skewed.is_empty(),
        "committed closure BYTES lag the harvest while the wire surface matches — the shipped \
         beams are not what this source and toolchain produce. Run \
         scripts/regen-awl-sdk-closure.sh and commit the result. Divergent behavioural chunks \
         per module: {byte_skewed:?}"
    );
    Ok(())
}
