//! Executes a generated wire-compat golden against a real `gleam test`, proving
//! the golden's type-derived literal is byte-exact what `gleam_json` produces
//! on the Erlang target for the risky value shapes — a Float (`0.0`, never `0`),
//! an optional field omitted from the wire, an enum rendered as its first
//! variant's wire string, and a nested record — none of which the all-`String`/
//! `Int` `examples/order-saga` golden ever exercises (checklist C3).
//!
//! Types-first: the risky shapes are AUTHORED as Gleam types appended to the
//! example's types module, the real compiler exports the package interface,
//! and the whole pipeline (interface → model → codecs module → emitted
//! schemas → wrappers/golden) runs through the `aion_package` library API —
//! never the `aion` binary — before `gleam test` executes the golden. The
//! generator derives both the canonical Gleam sample and the expected JSON
//! literal from the same model the codecs come from, so the only thing that
//! can make this fail is a genuine disagreement between the derived literal
//! and real `gleam_json`. A second leg deletes every generated file and
//! regenerates, asserting the round-trip is byte-identical.
//!
//! Per project policy this never uses `#[ignore]`: when the `gleam` toolchain is
//! absent it announces a skip and returns `Ok(())`; when `gleam` is present it
//! runs `gleam test` for real and fails the Rust test if that exits non-zero.
//! The announcement goes through the shared gate, because the skip line this
//! file used to print was captured by `libtest` and never shown (#74).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "test_support/gleam.rs"]
mod gleam_test_support;

use aion_package::{
    ActivityDeclaration, CodegenMode, Tier, boundary_types_from_interface, emit_schemas,
    generate_activities, generate_codecs,
};

type TestError = Box<dyn std::error::Error>;

/// The authored Gleam types that exercise every risky wire shape in one value
/// type, appended to the example's types module and used as the input of the
/// `golden_probe` activity.
///
/// - `ratio: Float` (required) → a `Float`, whose zero value is `0.0` (the
///   shape order-saga never has — its only numbers are `Int`).
/// - `line: GoldenInputLine` (required) → a nested record, whose wire literal
///   is a nested `{...}` object.
/// - `kind: GoldenInputKind` (required) → an enum, whose sample is the first
///   variant and whose literal is that variant's wire string (the type-name
///   prefix stripped: `GoldenInputKindStandard` → `"standard"`).
/// - `note: option.Option(String)` → an optional field whose sample is
///   `option.None` and is omitted from the wire literal entirely.
const GOLDEN_INPUT_TYPES: &str = r"
import gleam/option

/// Wire-shape probe input for the golden gate.
pub type GoldenInput {
  GoldenInput(
    ratio: Float,
    line: GoldenInputLine,
    kind: GoldenInputKind,
    note: option.Option(String),
  )
}

/// Nested record on the probe input.
pub type GoldenInputLine {
  GoldenInputLine(sku: String)
}

/// Enum on the probe input.
pub type GoldenInputKind {
  GoldenInputKindStandard
  GoldenInputKindRush
}
";

/// The hand-written stub body the generated `golden_probe` wrapper references
/// via `activities.golden_probe`. It never executes during `gleam test` (the
/// wire-compat golden only encodes a sample value), but it must type-check, so
/// it consumes `io.GoldenInput` and returns the existing `io.Shipment`.
const GOLDEN_PROBE_BODY: &str = "
pub fn golden_probe(
  _input: io.GoldenInput,
) -> Result(io.Shipment, error.ActivityError) {
  Ok(io.Shipment(order_id: \"\", shipment_id: \"\"))
}
";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Copies `examples/order-saga` into a fresh temp directory, rewriting the
/// `aion_flow` path dependency to an absolute path in both `gleam.toml` and the
/// lockfile `manifest.toml` so it resolves from the new location. Mirrors
/// `crates/aion-cli/tests/generate_e2e.rs`. Build artifacts and stale archives
/// are not copied.
fn stage_order_saga() -> Result<(tempfile::TempDir, PathBuf), TestError> {
    let root = repo_root().canonicalize()?;
    let source = root.join("examples/order-saga");
    let temp = tempfile::tempdir()?;
    let project = temp.path().join("order-saga");
    copy_tree(&source, &project)?;

    let aion_flow = root.join("gleam/aion_flow");
    let absolute = format!("\"{}\"", aion_flow.display());
    for descriptor in ["gleam.toml", "manifest.toml"] {
        let path = project.join(descriptor);
        let contents = fs::read_to_string(&path)?.replace("\"../../gleam/aion_flow\"", &absolute);
        fs::write(&path, contents)?;
    }
    Ok((temp, project))
}

/// Recursively copies `from` to `to`, skipping the Gleam `build` directory and
/// any prebuilt `.aion` archive.
fn copy_tree(from: &Path, to: &Path) -> Result<(), TestError> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == "build" {
            continue;
        }
        let source = entry.path();
        let target = to.join(&name);
        if source.is_dir() {
            copy_tree(&source, &target)?;
        } else if source.extension().is_none_or(|ext| ext != "aion") {
            fs::copy(&source, &target)?;
        }
    }
    Ok(())
}

/// The order-saga activities the staged workflow module still references; the
/// generated wrappers must keep covering them, so they are re-declared verbatim
/// alongside the new `golden_probe` probe. Each tuple is
/// `(name, input_type, output_type)` and every one is `RemotePython` so it
/// contributes to the wire-compat golden.
const ORDER_SAGA_ACTIVITIES: &[(&str, &str, &str)] = &[
    ("reserve_inventory", "OrderInput", "InventoryReservation"),
    ("charge_payment", "OrderInput", "PaymentReceipt"),
    ("ship_order", "OrderInput", "Shipment"),
    (
        "release_inventory",
        "ReleaseInventoryInput",
        "CompensationOutput",
    ),
    ("refund_payment", "RefundPaymentInput", "CompensationOutput"),
    (
        "cancel_shipment",
        "CancelShipmentInput",
        "CompensationOutput",
    ),
];

/// Builds the declaration list: the existing order-saga activities (so their
/// wrappers, which the workflow module calls, keep existing) plus `golden_probe`
/// carrying the risky `GoldenInput` value type.
fn declarations() -> Vec<ActivityDeclaration> {
    let mut declarations: Vec<ActivityDeclaration> = ORDER_SAGA_ACTIVITIES
        .iter()
        .map(|(name, input, output)| ActivityDeclaration {
            name: (*name).to_owned(),
            tier: Tier::RemotePython,
            input_type: (*input).to_owned(),
            output_type: (*output).to_owned(),
        })
        .collect();
    declarations.push(ActivityDeclaration {
        name: "golden_probe".to_owned(),
        tier: Tier::RemotePython,
        input_type: "GoldenInput".to_owned(),
        output_type: "Shipment".to_owned(),
    });
    declarations
}

/// Runs `gleam test` in `project`, returning whether it succeeded and the
/// combined stdout+stderr.
fn run_gleam_test(project: &Path) -> Result<(bool, String), TestError> {
    let output = Command::new("gleam")
        .arg("test")
        .current_dir(project)
        .output()
        .map_err(|error| format!("failed to spawn `gleam test`: {error}"))?;
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok((output.status.success(), combined))
}

/// Runs `gleam export package-interface` in `project` and returns the JSON.
/// The staged example compiles as committed (the appended probe types are
/// additive), so no rename-aside dance is needed here.
fn export_interface(project: &Path) -> Result<Vec<u8>, TestError> {
    let out = project.join("build/test-interface.json");
    let output = Command::new("gleam")
        .args(["export", "package-interface", "--out"])
        .arg(&out)
        .current_dir(project)
        .output()
        .map_err(|error| format!("failed to spawn `gleam export package-interface`: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "`gleam export package-interface` failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(fs::read(&out)?)
}

/// The test whose skip [`the_skip_is_visible_in_a_passing_run`] re-runs. Named
/// once so the pin and the `#[test]` below cannot drift apart under a rename.
const GATED_TEST: &str =
    "generated_golden_passes_real_gleam_test_for_float_optional_enum_and_nested";

/// Set on the child process so a filter that ever matched this pin itself
/// could not recurse.
const PIN_CHILD_MARKER: &str = "AION_GLEAM_GATE_PIN_CHILD";

/// THE PIN (#74). A skipped Gleam gate announces itself where a reader of a
/// PLAIN `cargo test` log will see it — inside a run that reports `ok`.
///
/// This cannot be checked by inspection and it cannot be checked in-process:
/// the failure being guarded is `libtest`'s output capture, which replays a
/// test's `print!`/`eprintln!` output only when that test FAILS. A skip line
/// written through the macros is therefore perfectly correct, perfectly
/// invisible, and indistinguishable from a gate that ran — which is what all
/// three of this crate's gates did before this commit.
///
/// So the pin re-runs one real gated test in a child process with `gleam`
/// removed from `PATH`, with capture left ON exactly as a normal run has it,
/// and requires BOTH halves:
///
/// - the announcement reaches the child's stderr, and
/// - the child reports the test as PASSED.
///
/// The second half is the control, and it is the load-bearing one. Capture is
/// replayed on failure, so an announcement seen in a FAILING run would prove
/// nothing at all about the case that matters.
#[test]
fn the_skip_is_visible_in_a_passing_run() -> Result<(), TestError> {
    if std::env::var_os(PIN_CHILD_MARKER).is_some() {
        return Ok(());
    }

    // An EMPTY `PATH` is not a reliable way to hide a binary — an empty search
    // path falls back to a system default on some platforms. An empty
    // DIRECTORY is unambiguous: the search succeeds and finds nothing.
    let empty = tempfile::tempdir()?;
    let output = Command::new(std::env::current_exe()?)
        .args(["--exact", GATED_TEST, "--test-threads", "1"])
        .env(PIN_CHILD_MARKER, "1")
        // The binary is invoked by absolute path and needs no `PATH` itself.
        .env("PATH", empty.path())
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stdout.contains("1 passed"),
        "the control failed: the child run must PASS, or a visible announcement would only prove \
         that `libtest` replays capture on failure.\n--- stdout ---\n{stdout}\n--- stderr \
         ---\n{stderr}"
    );
    assert!(
        stderr.contains(gleam_test_support::SKIP_MARKER),
        "a gate that skipped inside a passing run announced nothing a reader of a plain `cargo \
         test` log could see — the skip is invisible and the `ok` is \
         vacuous.\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
    );
    assert!(
        stderr.contains("golden_gleam_exec.rs:"),
        "the announcement must name the CALLER's file and line, so a reader lands on the test \
         that did not run rather than on the shared helper.\n--- stderr ---\n{stderr}"
    );
    Ok(())
}

#[test]
fn generated_golden_passes_real_gleam_test_for_float_optional_enum_and_nested()
-> Result<(), TestError> {
    if gleam_test_support::skip_if_unavailable() {
        return Ok(());
    }

    let (_temp, project) = stage_order_saga()?;

    // Author the types that introduce the Float, optional, enum, and nested
    // record into the types module — the types-first front door.
    let io_path = project.join("src/aion_order_saga_io.gleam");
    let mut io_module = fs::read_to_string(&io_path)?;
    io_module.push_str(GOLDEN_INPUT_TYPES);
    fs::write(&io_path, io_module)?;

    // The generated `golden_probe` wrapper references `activities.golden_probe`;
    // give the hand-written activities module a type-checking stub so the
    // project compiles as a whole.
    let activities_path = project.join("src/aion_order_saga_activities.gleam");
    let mut activities = fs::read_to_string(&activities_path)?;
    activities.push_str(GOLDEN_PROBE_BODY);
    fs::write(&activities_path, activities)?;

    // Run the full types-first pipeline through the library API: interface →
    // model → codecs module + emitted schemas → activity plumbing (including
    // the wire-compat golden the rest of this test executes).
    let package_name = "aion_order_saga";
    let interface = export_interface(&project)?;
    let types = boundary_types_from_interface(&interface, package_name)?;
    generate_codecs(&project, &types, CodegenMode::Write)?;
    emit_schemas(&project, package_name, &types, CodegenMode::Write)?;
    generate_activities(&project, &declarations(), &types, CodegenMode::Write)?;

    // Cheap structural guard: assert the generated golden actually exercises
    // each risky shape, so a future generator change that stops emitting one is
    // caught here even before `gleam test` runs.
    let golden_path = project.join("test/aion_order_saga_wire_compat_test.gleam");
    let golden = fs::read_to_string(&golden_path)?;
    let probe_test = golden
        .split("pub fn golden_input_wire_test()")
        .nth(1)
        .ok_or("generated golden has no golden_input_wire_test (GoldenInput not pinned)")?;
    // Stop at the next test function so the asserts below describe only the
    // GoldenInput case.
    let probe_test = probe_test.split("\npub fn ").next().unwrap_or(probe_test);

    let mut missing = Vec::new();
    // Float zero value, never the integer `0`.
    if !probe_test.contains("ratio: 0.0") || !probe_test.contains("\\\"ratio\\\":0.0") {
        missing.push("Float `0.0` (sample `ratio: 0.0` and literal `\\\"ratio\\\":0.0`)");
    }
    // Nested record: a `{...}` object literal for the `line` field.
    if !probe_test.contains("line: io.GoldenInputLine(sku: \"\")")
        || !probe_test.contains("\\\"line\\\":{\\\"sku\\\":\\\"\\\"}")
    {
        missing.push("nested record `line` (sample `io.GoldenInputLine(...)` and `{...}` literal)");
    }
    // Enum: first variant constructor and its wire string.
    if !probe_test.contains("kind: io.GoldenInputKindStandard")
        || !probe_test.contains("\\\"kind\\\":\\\"standard\\\"")
    {
        missing.push("enum `kind` (first variant `GoldenInputKindStandard` / wire `standard`)");
    }
    // Optional field omitted: `note` is `option.None` in the sample and absent
    // from the literal.
    if !probe_test.contains("note: option.None") {
        missing.push("optional `note` as `option.None` in the sample");
    }
    if probe_test.contains("\\\"note\\\"") {
        missing.push("optional `note` MUST be omitted from the wire literal but appears in it");
    }
    if !missing.is_empty() {
        return Err(format!(
            "generated golden_input_wire_test does not exercise the risky shapes it must \
             ({}). The generated test was:\n{probe_test}",
            missing.join("; ")
        )
        .into());
    }

    // Round-trip: delete every generated file and regenerate — byte-identical.
    let generated = [
        "src/aion_order_saga_codecs.gleam",
        "src/aion_order_saga_activity_wrappers.gleam",
        "worker/worker.py",
        "test/aion_order_saga_wire_compat_test.gleam",
        "schemas/golden_input.json",
        "schemas/order_input.json",
    ];
    let mut snapshot = Vec::new();
    for relative in generated {
        let path = project.join(relative);
        snapshot.push((relative, fs::read(&path)?));
        fs::remove_file(&path)?;
    }
    generate_codecs(&project, &types, CodegenMode::Write)?;
    emit_schemas(&project, package_name, &types, CodegenMode::Write)?;
    generate_activities(&project, &declarations(), &types, CodegenMode::Write)?;
    for (relative, original) in &snapshot {
        let regenerated = fs::read(project.join(relative))?;
        if &regenerated != original {
            return Err(
                format!("round-trip drift in {relative}: regeneration not byte-identical").into(),
            );
        }
    }

    // The proof: run the generated golden through real `gleam_json` on the BEAM.
    // A non-zero exit means the type-derived literal disagrees with what
    // `gleam_json` actually produces — exactly the latent wire-compat bug this
    // gate exists to surface — so it fails the Rust test with gleam's output.
    let (ok, combined) = run_gleam_test(&project)?;
    if !ok {
        return Err(format!(
            "`gleam test` failed on the generated wire-compat golden — the type-derived \
             literal does not match real `gleam_json` output. This is a wire-compat BLOCKER, \
             not a flaky test.\n\n{combined}"
        )
        .into());
    }

    Ok(())
}
