//! B-2 proof obligations for the programmatic AWL assembler: determinism,
//! format-v1 admission, SDK closure coverage, bundle integrity, collision
//! refusal, and the dev-time regeneration script's product-path isolation.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use aion_awl::{ActionRequirement, CompiledWorkflow};
use aion_awl_package::{
    AssembleError, AwlAssembleOptions, assemble_awl, sdk_closure_modules, sdk_closure_version,
};
use aion_package::{ExtractionLimits, Package, PackageContract};
use beamr::atom::AtomTable;
use beamr::loader::load_beam_chunks;
use sha2::{Digest, Sha256};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Pinned digest of the committed SDK closure (names, lengths, bytes, in
/// canonical order). A silent edit to the embedded bundle fails here; a
/// deliberate regeneration updates this pin in the same commit. Repinned on
/// 2026-07-24: the regeneration drill rebuilt the source-unchanged 46-module
/// `aion_flow` 0.7.0 closure with the build box's upgraded Erlang (`compile_info`
/// version 9.0.6 -> 10.0.2), which changed every module's bytes. The full
/// workspace suite executed the new closure on the embedded beamr runtime.
/// Repinned on 2026-07-31 for #83 (`AwlEngineFailed`), and the regeneration
/// carried TWO independent causes, separated by measurement rather than
/// assumed: a clean-tree regen with the #83 edits reverted still rewrote all
/// 46 modules to `2b1cc2af…`, so the bulk is TOOLCHAIN drift the committed
/// closure had accumulated; re-applying the #83 source edit then moved it to
/// this value, which is what proves the edit reached the shipped bytes.
/// Repinned 2026-08-09: query arguments changed `aion@query:handler` from
/// arity 3 to 4 and added `aion@codec:null_codec/0`, so the shipped closure
/// was regenerated. The runtime-surface guard is what makes this repin
/// load-bearing rather than bookkeeping: an unregenerated closure would still
/// export `handler/3` while the lowering minted `handler/4`, which is the
/// `undef`-at-runtime skew that guard exists to catch.
/// Repinned 2026-08-11 for #181: the canonical agent-outcome record added the
/// `aion@agent` module to `aion_flow` (46 -> 47 modules), and the
/// regeneration refreshed accumulated toolchain drift across the other
/// modules' bytes. The freshness gate
/// (`committed_closure_matches_a_fresh_oracle_harvest`) is what forced this
/// repin: a workspace battery red on `fresh-only: ["aion@agent"]`, caught at
/// the 0.13.7 release gate rather than at an operator's runtime.
/// Repinned 2026-08-15 for #241: the canonical record's mandatory
/// `final_message` field changed the committed `aion@agent` BEAM. The closure
/// freshness gate and its focused green prove the committed bytes match a fresh
/// harvest; this checksum pins those shipped bytes rather than the pre-#241
/// module. Repinned 2026-08-18 after the beamr 0.19.2 migration's full gate
/// forced a fresh Gleam 1.18.1/OTP 27 closure harvest (box venue). Repinned
/// again 2026-08-19 for the v0.18.1 landing: the closure was re-harvested on
/// the landing gate's venue (Mac, Gleam 1.17.0/OTP 29) — BEAM bytes follow
/// the host toolchain, so the freshness oracle is venue-bound; estate
/// toolchain unification is tracked as its own issue. The module set and SDK
/// version are unchanged; `committed_closure_matches_a_fresh_oracle_harvest`
/// proves these are the current deterministic shipped bytes. Repinned
/// 2026-08-22 for `PolicyRefused` L3: `activity.fallback/2` changed the shipped
/// activity module, and a fresh Gleam 1.18.1 harvest on this Linux venue
/// refreshed the venue-bound BEAM bytes across the closure.
/// Repinned 2026-08-22 for the estate toolchain alignment: the closure was
/// re-harvested on an OTP 29.0.5 venue (Dean, Gleam 1.18.1/erts 17.0.5) after
/// every fleet venue converged on OTP 29 — BEAM bytes follow the host
/// toolchain, and the alignment's step-2 harvest proved rf (erts 17.0.1) and
/// Dean (erts 17.0.5) produce byte-identical modules, so this pin now holds
/// across the aligned fleet instead of one venue. Module set and SDK version
/// unchanged (47 modules, `aion_flow` 0.7.0).
const BUNDLE_CHECKSUM: &str = "afbc1b1d82aee2e30af0ae03a293f53164e5bd06b5023fb65610de6d0bbe2b8c";
const BUNDLE_MODULE_COUNT: usize = 47;
const BUNDLE_SDK_VERSION: &str = "0.7.0";

fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.join("../..")
}

fn compile_awl_hello() -> Result<CompiledWorkflow, Box<dyn std::error::Error>> {
    let path = workspace_root().join("examples/awl-hello/awl_hello.awl");
    let source = fs::read_to_string(&path)?;
    let root = path
        .parent()
        .ok_or("awl_hello.awl has no parent directory")?
        .to_path_buf();
    Ok(aion_awl::compile(&source, &root)?)
}

/// Distinct module names a BEAM imports, read through the same beamr chunk
/// loader the runtime rename machinery builds on. An unresolvable import
/// atom is a hard error, never a silent drop — an honesty test that
/// silently shrank its coverage set would pass vacuously.
fn imported_modules(beam_bytes: &[u8]) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let atoms = AtomTable::with_common_atoms();
    let parsed = load_beam_chunks(beam_bytes, &atoms)?;
    let mut modules = Vec::new();
    for import in &parsed.imports {
        let Some(name) = atoms.resolve(import.module) else {
            return Err(format!("unresolvable import module atom: {:?}", import.module).into());
        };
        modules.push(name.to_owned());
    }
    modules.sort_unstable();
    modules.dedup();
    Ok(modules)
}

/// Proof 1 — same-process determinism: the same compile output assembles
/// to byte-identical archives whose content hash round-trips through the
/// loader. (This alone cannot catch wall-clock leakage — two calls land in
/// the same zip timestamp window — so cross-process stability is proven
/// separately by the pinned-archive test below.)
#[test]
fn assembling_twice_yields_byte_identical_archives() -> TestResult {
    let compiled = compile_awl_hello()?;
    let first = assemble_awl(&compiled, AwlAssembleOptions::default())?;
    let second = assemble_awl(&compiled, AwlAssembleOptions::default())?;
    assert!(!first.is_empty());
    assert_eq!(first, second);

    let package = Package::load_from_bytes(&first, ExtractionLimits::unbounded())?;
    assert_eq!(
        package.manifest().version.as_str(),
        package.content_hash().to_string()
    );
    Ok(())
}

/// Pinned SHA-256 of the archive assembled from the fixed hand-built
/// compile output below. Unlike the `awl_hello` archive (whose BEAM bytes
/// vary with the cargo feature graph), every input here is a literal, so
/// this pin holds across processes, machines, and time: any wall-clock
/// timestamp, environment read, or ordering instability anywhere in the
/// assembly path changes these bytes and fails this test. A deliberate SDK
/// closure regeneration updates this pin in the same commit, exactly like
/// `BUNDLE_CHECKSUM`. Repinned on 2026-07-24: the regeneration drill rebuilt
/// the source-unchanged closure with the build box's upgraded Erlang
/// (`compile_info` version 9.0.6 -> 10.0.2), so the embedded archive bytes
/// changed with the compiler, not with any `aion_flow` source edit.
/// Repinned on 2026-07-25: the worker-contract handshake embeds the canonical
/// `.v4` contract record in the archive and binds it into the identity, so
/// the archive bytes changed with the ARCHIVE FORMAT, not with any workflow
/// source edit; `assembling_twice_yields_byte_identical_archives` proves the
/// new bytes remain deterministic.
/// Repinned on 2026-07-30: declarative action bodies made every
/// `ActionContract` record end with an always-encoded body block, under the
/// bumped `aion.package.version.worker-contract.v5` identity domain. Both
/// inputs to the embedded contract record changed by construction, so the
/// archive bytes changed with the ARCHIVE FORMAT again, not with any
/// workflow source edit — and determinism still holds (that test passes).
/// Repinned on 2026-07-31: the #83 SDK-closure regeneration changed the
/// embedded `aion@awl@error` beam (and, independently, refreshed accumulated
/// toolchain drift across the other 45 modules), so the archive bytes changed
/// with the CLOSURE, not with any workflow source edit.
/// `assembling_twice_yields_byte_identical_archives` still passes, so the new
/// bytes remain deterministic.
/// Repinned on 2026-08-09: query arguments regenerated the embedded SDK
/// closure (`aion@query:handler` 3 -> 4, plus `aion@codec:null_codec/0`), and
/// the closure is an INPUT to every assembled archive, so the archive bytes
/// changed with the CLOSURE, not with any workflow source edit. The bundle
/// checksum moved in the same commit for the same reason, and
/// `assembling_twice_yields_byte_identical_archives` still passes, so the new
/// bytes remain deterministic.
/// Repinned on 2026-08-11: the #181 SDK-closure regeneration added
/// `aion@agent` (the canonical agent-outcome record) and refreshed
/// accumulated toolchain drift across the other modules, and the closure is
/// an INPUT to every assembled archive — so the archive bytes changed with
/// the CLOSURE, not with any workflow source edit. The bundle checksum moved
/// in the same commit for the same reason, and
/// `assembling_twice_yields_byte_identical_archives` still passes, so the
/// new bytes remain deterministic. Repinned on 2026-08-15 for #241: adding
/// mandatory `final_message` regenerated the committed `aion@agent` closure
/// module, which is an input to every archive. The same-process determinism and
/// fresh-closure tests are the controls: this pin moves with those intentional,
/// deterministic shipped bytes. Repinned 2026-08-18 with the fresh Gleam
/// landing-venue closure (see `BUNDLE_CHECKSUM` notes); the
/// bundle checksum and freshness proof move in the same change, while the
/// fixed-input assembly remains deterministic. Repinned 2026-08-22 because the
/// L3 `activity.fallback/2` closure harvest is an input to every archive; the
/// same-process determinism and fresh-closure tests remain the controls.
/// Repinned 2026-08-22 with the estate toolchain-alignment closure harvest
/// (OTP 29.0.5 venue; see `BUNDLE_CHECKSUM` notes): the closure is an input
/// to every archive, so the archive bytes moved with the closure while the
/// fixed-input assembly remains deterministic.
const FIXED_INPUT_ARCHIVE_SHA256: &str =
    "8529350178f674902fb23be87b8d06622263a4f09785eb7fd0cd67626920fe38";

/// Proof 1b — cross-process determinism: a fully fixed input assembles to
/// pinned archive bytes.
#[test]
fn fixed_input_archive_bytes_match_the_cross_process_pin() -> TestResult {
    let bytes = assemble_awl(&hand_built("pin_case"), AwlAssembleOptions::default())?;
    assert_eq!(sha256_hex(&bytes)?, FIXED_INPUT_ARCHIVE_SHA256);
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
    let mut digest = Sha256::new();
    digest.update(bytes);
    let mut hex = String::new();
    for byte in digest.finalize() {
        use std::fmt::Write as _;
        write!(hex, "{byte:02x}")?;
    }
    Ok(hex)
}

/// Proof 2 — admission: `Package::load_from_bytes` accepts the archive and
/// every derived manifest field round-trips exactly; the entry module is in
/// the BEAM set with the exact compiled bytes.
#[test]
fn format_v1_loader_admits_the_archive_with_derived_manifest() -> TestResult {
    let compiled = compile_awl_hello()?;
    let bytes = assemble_awl(&compiled, AwlAssembleOptions::default())?;
    let package = Package::load_from_bytes(bytes, ExtractionLimits::unbounded())?;

    let manifest = package.manifest();
    assert_eq!(manifest.entry_module, "awl_hello");
    assert_eq!(manifest.entry_function, "run");
    // `awl_hello` declares no timeout, so the manifest carries none — no buried
    // default — and the package cannot arm a deadline.
    assert_eq!(manifest.timeout, None);
    assert!(!package.has_declared_timeout());
    assert_eq!(manifest.input_schema, compiled.input_schema);
    assert_eq!(manifest.output_schema, compiled.output_schema);

    // Non-circular legacy parity: the derived manifest schemas equal the
    // schema documents the legacy `package_project` path embeds for this
    // same example — including the `$schema` dialect pin — not merely the
    // compile output they were copied from.
    let schemas = workspace_root().join("examples/awl-hello/schemas");
    let legacy_input: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(schemas.join("input.json"))?)?;
    let legacy_output: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(schemas.join("output.json"))?)?;
    assert_eq!(
        manifest.input_schema, legacy_input,
        "derived input schema drifted from the legacy embedded document"
    );
    assert_eq!(
        manifest.output_schema, legacy_output,
        "derived output schema drifted from the legacy embedded document"
    );
    let activities: Vec<&str> = manifest
        .activities
        .iter()
        .map(|activity| activity.activity_type.as_str())
        .collect();
    assert_eq!(activities, vec!["greet", "shout"]);

    assert_eq!(
        package.beams().get("awl_hello"),
        Some(compiled.beam_bytes.as_slice())
    );
    assert_eq!(package.beams().len(), BUNDLE_MODULE_COUNT + 1);
    assert!(package.source().is_empty());
    Ok(())
}

#[test]
fn document_timeout_reaches_the_manifest_and_absence_arms_nothing() -> TestResult {
    let path = workspace_root()
        .join("crates/aion-awl/tests/fixtures/rev2/dag-fork/valid/after_single.awl");
    let source = fs::read_to_string(&path)?;
    let root = path.parent().ok_or("fixture has no parent directory")?;

    let plain = aion_awl::compile(&source, root)?;
    let plain_bytes = assemble_awl(
        &plain,
        AwlAssembleOptions {
            timeout: plain.timeout,
            ..AwlAssembleOptions::default()
        },
    )?;
    let plain_package = Package::load_from_bytes(plain_bytes, ExtractionLimits::unbounded())?;
    // No authored timeout: absent in the manifest and not declared by identity.
    assert_eq!(plain_package.manifest().timeout, None);
    assert!(!plain_package.has_declared_timeout());
    assert_eq!(plain_package.declared_timeout(), None);

    let declared_source = source.replacen(
        "workflow after_single\n",
        "workflow after_single\n  timeout 6h\n",
        1,
    );
    let declared = aion_awl::compile(&declared_source, root)?;
    let declared_bytes = assemble_awl(
        &declared,
        AwlAssembleOptions {
            timeout: declared.timeout,
            ..AwlAssembleOptions::default()
        },
    )?;
    let rebuilt_bytes = assemble_awl(
        &declared,
        AwlAssembleOptions {
            timeout: declared.timeout,
            ..AwlAssembleOptions::default()
        },
    )?;
    assert_eq!(declared_bytes, rebuilt_bytes);
    let declared_package =
        Package::load_from_bytes(&declared_bytes, ExtractionLimits::unbounded())?;
    let rebuilt_package = Package::load_from_bytes(rebuilt_bytes, ExtractionLimits::unbounded())?;
    assert_eq!(
        declared_package.manifest().timeout,
        Some(std::time::Duration::from_secs(21_600))
    );
    assert!(declared_package.has_declared_timeout());
    assert_eq!(
        declared_package.declared_timeout(),
        Some(std::time::Duration::from_secs(21_600))
    );
    assert_ne!(
        plain_package.content_hash(),
        declared_package.content_hash()
    );
    assert_eq!(
        declared_package.content_hash(),
        rebuilt_package.content_hash()
    );
    Ok(())
}

/// Proof 3 — closure coverage (the honesty test): every module the compiled
/// workflow BEAM imports is shipped in the bundle or is the workflow module
/// itself. A missing import fails here, not as a runtime surprise later.
/// Also pins the name contract the assembler relies on: the BEAM's internal
/// module name equals the AWL workflow name.
#[test]
fn every_workflow_beam_import_is_shipped_in_the_closure() -> TestResult {
    let compiled = compile_awl_hello()?;

    let atoms = AtomTable::with_common_atoms();
    let parsed = load_beam_chunks(&compiled.beam_bytes, &atoms)?;
    assert_eq!(
        atoms.resolve(parsed.name),
        Some(compiled.workflow_name.as_str())
    );

    // Vacuous-pass guard: the coverage set must be non-empty and contain a
    // module the emitter is known to call, or the loop below proves nothing.
    let imports = imported_modules(&compiled.beam_bytes)?;
    assert!(
        !imports.is_empty(),
        "the workflow BEAM reports zero imports — import extraction is broken"
    );
    assert!(
        imports
            .iter()
            .any(|module| module == WORKFLOW_IMPORT_ANCHOR),
        "expected emitter call target `{WORKFLOW_IMPORT_ANCHOR}` among imports: {imports:?}"
    );

    let bundle: BTreeSet<&str> = sdk_closure_modules().map(|(name, _)| name).collect();
    let mut missing = Vec::new();
    for module in imports {
        if module != compiled.workflow_name && !bundle.contains(module.as_str()) {
            missing.push(module);
        }
    }
    assert!(
        missing.is_empty(),
        "workflow BEAM imports missing from the SDK closure: {missing:?}"
    );
    Ok(())
}

/// A module the direct compiler's emitter always calls from a workflow BEAM
/// (its runtime support module) — the named anchor that keeps the
/// import-coverage proofs from passing vacuously.
const WORKFLOW_IMPORT_ANCHOR: &str = "aion@awl@runtime";

/// Modules the engine provides natively (the SDK's NIF/FFI namespace,
/// registered by the runtime itself) — deliberately excluded from the
/// bundle by the same filter the legacy discovery applies.
const ENGINE_PROVIDED_MODULES: &[&str] = &["aion_flow_ffi"];

/// Erlang/OTP modules present on any ERTS the engine runs on (`json` is the
/// OTP 27+ builtin `gleam_json_ffi` targets); never shipped by either
/// packaging path. Kept exhaustive on purpose: a new bundle-external import
/// must be classified here deliberately, not absorbed by a pattern.
const ERTS_PROVIDED_MODULES: &[&str] = &[
    "base64",
    "binary",
    "erlang",
    "io",
    "io_lib",
    "io_lib_format",
    "json",
    "lists",
    "maps",
    "math",
    "rand",
    "string",
    "unicode",
    "uri_string",
];

/// Proof 3b — closure closedness: the bundle itself is closed under
/// imports, modulo the engine-provided FFI namespace and ERTS builtins.
/// This is the transitive guarantee proof 3 cannot give: if a future SDK
/// regeneration misses a package (say `aion_flow` gains a new production
/// dependency the harvest overlooks), its modules surface here as
/// bundle-external imports and this test fails — at regen time, not as an
/// `undef` in a deployed worker.
#[test]
fn bundle_is_closed_under_imports_modulo_engine_and_erts_modules() -> TestResult {
    let bundle: BTreeSet<&str> = sdk_closure_modules().map(|(name, _)| name).collect();

    let mut leaked = BTreeSet::new();
    let mut engine_seen = false;
    for (name, bytes) in sdk_closure_modules() {
        let imports = imported_modules(bytes)?;
        assert!(
            !imports.is_empty(),
            "bundle module `{name}` reports zero imports"
        );
        for import in imports {
            if ENGINE_PROVIDED_MODULES.contains(&import.as_str()) {
                engine_seen = true;
                continue;
            }
            if bundle.contains(import.as_str()) || ERTS_PROVIDED_MODULES.contains(&import.as_str())
            {
                continue;
            }
            leaked.insert(format!("{name} -> {import}"));
        }
    }
    assert!(
        leaked.is_empty(),
        "bundle is not closed under imports (missing modules would be `undef` at runtime): {leaked:?}"
    );
    // The engine-provided whitelist must be live (the SDK really does call
    // its FFI namespace), and no whitelisted module may also be shipped —
    // a stale whitelist entry would quietly widen the escape hatch.
    assert!(
        engine_seen,
        "no bundle module imports the engine FFI namespace"
    );
    for allowed in ENGINE_PROVIDED_MODULES.iter().chain(ERTS_PROVIDED_MODULES) {
        assert!(
            !bundle.contains(allowed),
            "whitelisted module `{allowed}` is also shipped in the bundle"
        );
    }
    Ok(())
}

/// Proof 4 — bundle integrity: non-empty, versioned, carries the `aion_flow`
/// and gleam stdlib modules `awl_hello`'s BEAM actually imports, and matches
/// the pinned checksum so silent bundle edits fail loudly.
#[test]
fn bundle_is_versioned_pinned_and_carries_the_imported_sdk_modules() -> TestResult {
    assert_eq!(sdk_closure_modules().count(), BUNDLE_MODULE_COUNT);
    assert_eq!(sdk_closure_version(), BUNDLE_SDK_VERSION);
    for (_, bytes) in sdk_closure_modules() {
        assert!(!bytes.is_empty());
    }

    let bundle: BTreeSet<&str> = sdk_closure_modules().map(|(name, _)| name).collect();
    let compiled = compile_awl_hello()?;
    for module in imported_modules(&compiled.beam_bytes)? {
        assert!(
            bundle.contains(module.as_str()),
            "imported module `{module}` absent from bundle"
        );
    }

    let mut digest = Sha256::new();
    for (name, bytes) in sdk_closure_modules() {
        digest.update(name.as_bytes());
        digest.update([0]);
        digest.update((bytes.len() as u64).to_be_bytes());
        digest.update(bytes);
    }
    let mut hex = String::new();
    for byte in digest.finalize() {
        use std::fmt::Write as _;
        write!(hex, "{byte:02x}")?;
    }
    assert_eq!(hex, BUNDLE_CHECKSUM);
    Ok(())
}

fn hand_built(name: &str) -> CompiledWorkflow {
    CompiledWorkflow {
        workflow_name: name.to_owned(),
        first_worker: Some("q".to_owned()),
        timeout: None,
        beam_bytes: b"opaque".to_vec(),
        input_schema: serde_json::json!({ "type": "object" }),
        output_schema: serde_json::json!({ "type": "object" }),
        contract: PackageContract {
            input_schema: serde_json::json!({ "type": "object" }),
            output_schema: serde_json::json!({ "type": "object" }),
            workers: Vec::new(),
            children: Vec::new(),
            signals: Vec::new(),
            additional_workflows: Vec::new(),
            unscoped_activities: Vec::new(),
        },
        action_bodies: Vec::new(),
        actions: vec![ActionRequirement {
            task_queue: "q".to_owned(),
            action: "act".to_owned(),
            node: None,
        }],
        sidecar_bytes: Vec::new(),
        synthesized_workflows: Vec::new(),
    }
}

/// Proof 5 — negative: a workflow whose name collides with a bundle module
/// is refused with the dedicated typed error, before any archive is built.
#[test]
fn workflow_name_colliding_with_a_bundle_module_is_refused_typed() {
    let result = assemble_awl(&hand_built("aion_flow"), AwlAssembleOptions::default());
    assert!(matches!(
        result,
        Err(AssembleError::BundleCollision { module }) if module == "aion_flow"
    ));
}

#[test]
fn unsafe_workflow_name_is_refused_typed() {
    let result = assemble_awl(&hand_built("../escape"), AwlAssembleOptions::default());
    assert!(matches!(
        result,
        Err(AssembleError::InvalidWorkflowName { name }) if name == "../escape"
    ));
}

/// Proof 6 — the dev-time regeneration script exists, documents its gleam
/// requirement, and is invoked by no `build.rs` anywhere in the workspace
/// (`aion-awl-package` itself has no build script at all).
#[test]
fn regeneration_script_is_documented_and_outside_the_product_path() -> TestResult {
    let root = workspace_root();
    let script = root.join("scripts/regen-awl-sdk-closure.sh");
    let text = fs::read_to_string(&script)?;
    assert!(text.contains("gleam build"));
    assert!(text.contains("DEV-TIME"));
    assert!(text.contains("PRODUCT path never invokes this script"));

    assert!(
        !Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("build.rs")
            .exists(),
        "aion-awl-package must not grow a build script"
    );

    let crates_dir = root.join("crates");
    for entry in fs::read_dir(&crates_dir)? {
        let build_rs = entry?.path().join("build.rs");
        if !build_rs.is_file() {
            continue;
        }
        let contents = fs::read_to_string(&build_rs)?;
        assert!(
            !contents.contains("regen-awl-sdk-closure"),
            "{} invokes the dev-time regeneration script",
            build_rs.display()
        );
    }
    Ok(())
}
