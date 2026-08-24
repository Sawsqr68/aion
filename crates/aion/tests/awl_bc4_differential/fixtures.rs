//! Fixture loading for the differential: the covered ratchet path list, the
//! rev-2 fixture tree, and a deterministic schema-driven input generator so
//! every fixture starts with a payload its own generated input codec accepts.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use aion_awl::{Document, TypeRef};
use serde_json::{Map, Value, json};

/// `<repo>/crates/aion-awl` — the crate whose `tests/fixtures/rev2` tree and
/// `src/mir/covered.rs` ratchet the differential reads.
pub fn awl_crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../aion-awl")
}

/// The rev-2 fixture root.
pub fn rev2_dir() -> PathBuf {
    awl_crate_dir().join("tests/fixtures/rev2")
}

/// Absolute path of a fixture given its covered-ratchet relative path.
pub fn fixture_path(relative: &str) -> PathBuf {
    rev2_dir().join(format!("{relative}.awl"))
}

/// What the shared dispatcher answers one declared action with: its canned
/// schema-valid success payload, or a terminal failure reason.
pub type ActionResult = Result<String, String>;

/// A parsed fixture: its ratchet name, source, parsed document, and directory
/// (schema-door imports resolve against the directory).
pub struct Loaded {
    /// Covered-ratchet path, relative to `tests/fixtures/rev2`, no extension.
    pub name: String,
    /// Parsed document.
    pub document: Document,
    /// Directory the fixture lives in (import root).
    pub dir: PathBuf,
}

/// Reads and parses one covered fixture.
///
/// # Errors
///
/// Fails when the fixture cannot be read or does not parse.
pub fn load(relative: &str) -> Result<Loaded, Box<dyn std::error::Error>> {
    let path = fixture_path(relative);
    let source = fs::read_to_string(&path)
        .map_err(|error| format!("cannot read fixture {relative}: {error}"))?;
    let document = aion_awl::parse(&source)
        .map_err(|error| format!("fixture {relative} does not parse: {error}"))?;
    let dir = path
        .parent()
        .ok_or_else(|| format!("fixture {relative} has no parent directory"))?
        .to_path_buf();
    Ok(Loaded {
        name: relative.to_owned(),
        document,
        dir,
    })
}

/// Parses the covered ratchet (`aion-awl/src/mir/covered.rs`) directly from
/// source so the differential fixture set stays in lockstep with the lowering
/// ratchet — a fixture added or removed there flows here with no second edit.
///
/// # Errors
///
/// Fails when the ratchet file cannot be read.
pub fn covered_paths() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let path = awl_crate_dir().join("src/mir/covered.rs");
    let text =
        fs::read_to_string(&path).map_err(|error| format!("cannot read covered.rs: {error}"))?;
    Ok(quoted_strings(&text)
        .into_iter()
        .filter(|entry| entry.contains("/valid/"))
        .collect())
}

/// Extracts every double-quoted string literal from `text`. The ratchet file
/// contains only path literals and (backtick-only) doc comments, so a simple
/// scan is exact.
fn quoted_strings(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = text.chars();
    while let Some(character) = chars.next() {
        if character == '"' {
            let mut literal = String::new();
            for inner in chars.by_ref() {
                if inner == '"' {
                    break;
                }
                literal.push(inner);
            }
            out.push(literal);
        }
    }
    out
}

/// Builds a deterministic, minimal JSON value that satisfies `schema` (a
/// draft-2020-12 schema as produced by the AWL schema deriver). Required
/// object properties are populated; optional ones are omitted; enums take
/// their first variant; scalars take the smallest value their declared bounds
/// allow.
///
/// **This is a heuristic, and [`witness_fault`] is what makes it safe.** The
/// generator honours the constraint keywords the corpus uses (`minLength`,
/// `maxLength`, `minItems`, `maxItems`, `minimum`, `maximum`, the exclusive
/// bounds, and integer `multipleOf`) but it cannot solve a `pattern` or a
/// `format`, and it does not try. Every witness it produces is checked against
/// the very schema it was generated for, so a shape it cannot satisfy fails
/// LOUDLY at the fixture, naming the fixture and the failing keyword.
///
/// It was not always so: this returned `"x"` for every string, including one a
/// fixture schema declared `minLength: 3`. That fixture started with input its
/// own declared contract forbids for the whole life of this harness, and
/// nothing noticed, because nothing was enforcing the contract at the door
/// either. When enforcement arrived the fixture went red — and the red was
/// scored as infrastructure noise. Both halves of that are fixed here.
pub fn example_for_schema(schema: &Value) -> Value {
    // Record-typed properties lower to `$ref: "#/$defs/Name"`; resolve those
    // against the top-level `$defs` so a required record input/result becomes a
    // real object, not `null` (an accidental decode-detour otherwise).
    let defs = schema.get("$defs");
    example_with_defs(schema, defs)
}

/// [`example_for_schema`], threading the root `$defs` map through the recursion
/// so nested `$ref`s resolve.
fn example_with_defs(schema: &Value, defs: Option<&Value>) -> Value {
    let Value::Object(map) = schema else {
        return Value::Null;
    };
    if let Some(Value::String(reference)) = map.get("$ref") {
        return match reference
            .strip_prefix("#/$defs/")
            .and_then(|name| defs.and_then(|table| table.get(name)))
        {
            Some(resolved) => example_with_defs(resolved, defs),
            None => Value::Null,
        };
    }
    if let Some(Value::Array(variants)) = map.get("enum") {
        return variants.first().cloned().unwrap_or(Value::Null);
    }
    if let Some(Value::Array(branches)) = map.get("anyOf") {
        // Optionals lower to `anyOf: [T, null]`; the null branch is the
        // simplest satisfying value.
        if branches.iter().any(is_null_schema) {
            return Value::Null;
        }
        return branches
            .first()
            .map_or(Value::Null, |branch| example_with_defs(branch, defs));
    }
    match map.get("type").and_then(Value::as_str) {
        Some("object") => example_object(map, defs),
        Some("array") => example_array(map, defs),
        Some("string") => Value::String(example_string(map)),
        Some("integer") => example_integer(map),
        Some("number") => example_number(map),
        Some("boolean") => Value::Bool(false),
        // `"null"` and an untyped `{}` schema (AWL `Unknown`, accepts
        // anything) both take the simplest satisfying value.
        _ => Value::Null,
    }
}

/// The shortest run of `x` that satisfies the schema's declared length bounds.
///
/// A `pattern` is deliberately NOT solved: regex inversion is not tractable in
/// general, a run of `x` satisfies every pattern this corpus applies to a
/// REQUIRED property, and [`witness_fault`] turns any future fixture where it
/// does not into a named failure rather than a silent one.
fn example_string(map: &Map<String, Value>) -> String {
    let shortest = map.get("minLength").and_then(Value::as_u64).unwrap_or(1);
    let length = map
        .get("maxLength")
        .and_then(Value::as_u64)
        .map_or(shortest, |longest| shortest.min(longest));
    "x".repeat(usize::try_from(length).unwrap_or(usize::MAX))
}

/// The smallest integer inside the schema's declared bounds.
///
/// Bounds that are not themselves integral JSON numbers (`1E5` parses as a
/// float) are not applied — they cannot narrow an integer witness without a
/// lossy cast, and a bound this skips can only ever leave the witness at `0`,
/// which [`witness_fault`] then judges against the real schema.
fn example_integer(map: &Map<String, Value>) -> Value {
    let mut value = 0i64;
    if let Some(smallest) = map.get("minimum").and_then(Value::as_i64) {
        value = value.max(smallest);
    }
    if let Some(below) = map.get("exclusiveMinimum").and_then(Value::as_i64) {
        value = value.max(below.saturating_add(1));
    }
    if let Some(largest) = map.get("maximum").and_then(Value::as_i64) {
        value = value.min(largest);
    }
    if let Some(above) = map.get("exclusiveMaximum").and_then(Value::as_i64) {
        value = value.min(above.saturating_sub(1));
    }
    if let Some(step) = map.get("multipleOf").and_then(Value::as_i64)
        && step > 0
    {
        let remainder = value.rem_euclid(step);
        if remainder != 0 {
            value = value.saturating_add(step - remainder);
        }
    }
    json!(value)
}

/// The smallest number inside the schema's declared bounds.
///
/// `multipleOf` is not applied to a float witness: stepping a binary float to
/// an exact multiple of a decimal step is not reliably representable, and
/// pretending otherwise would produce a witness that LOOKS satisfying. `0` is
/// an exact multiple of every step, so the default already satisfies it; a
/// bound that forces the witness off zero and onto a step is left to
/// [`witness_fault`] to report rather than to a rounding guess.
fn example_number(map: &Map<String, Value>) -> Value {
    let mut value = 0f64;
    if let Some(smallest) = map.get("minimum").and_then(Value::as_f64)
        && value < smallest
    {
        value = smallest;
    }
    if let Some(below) = map.get("exclusiveMinimum").and_then(Value::as_f64)
        && value <= below
    {
        value = below + 1.0;
    }
    if let Some(largest) = map.get("maximum").and_then(Value::as_f64)
        && value > largest
    {
        value = largest;
    }
    if let Some(above) = map.get("exclusiveMaximum").and_then(Value::as_f64)
        && value >= above
    {
        value = above - 1.0;
    }
    json!(value)
}

/// An array carrying exactly as many generated items as `minItems` demands
/// (empty when it demands none), clamped by `maxItems`.
fn example_array(map: &Map<String, Value>, defs: Option<&Value>) -> Value {
    let fewest = map.get("minItems").and_then(Value::as_u64).unwrap_or(0);
    let count = map
        .get("maxItems")
        .and_then(Value::as_u64)
        .map_or(fewest, |most| fewest.min(most));
    let count = usize::try_from(count).unwrap_or(0);
    let items = map.get("items");
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        out.push(items.map_or(Value::Null, |schema| example_with_defs(schema, defs)));
    }
    Value::Array(out)
}

/// Judges a generated witness against the schema it was generated FOR, and
/// describes the fault when it does not satisfy it.
///
/// This is the assertion that makes [`example_for_schema`] a heuristic the
/// harness can trust rather than one it merely hopes about. Without it a
/// witness the engine will refuse reaches the run boundary and surfaces as
/// `engine run error`, which reads as an environment problem — so the harness
/// is quietest exactly where its own inputs are wrong.
pub fn witness_fault(schema: &Value, witness: &Value) -> Option<String> {
    let validator = match jsonschema::validator_for(schema) {
        Ok(validator) => validator,
        Err(error) => return Some(format!("schema is not usable as a validator: {error}")),
    };
    let faults: Vec<String> = validator
        .iter_errors(witness)
        .map(|fault| format!("{}: {fault}", fault.instance_path()))
        .collect();
    if faults.is_empty() {
        return None;
    }
    Some(format!(
        "generated witness {witness} does not satisfy the schema it was \
         generated for: {}",
        faults.join("; ")
    ))
}

/// Whether a schema branch is exactly the `null` type.
fn is_null_schema(schema: &Value) -> bool {
    schema
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|ty| ty == "null")
}

/// The action-name prefix that makes an action FAIL deterministically.
///
/// An action declared `fail_<something>` is answered with a terminal `Err`
/// instead of a canned success value. That is the differential's only way to
/// EXECUTE a failure-driven arm — an `on failure` compensation block, above
/// all — inside a run whose activities are otherwise all canned successes;
/// without it the arm is lowered but never run, and unrun is unproven.
///
/// The convention is safe by CONSTRUCTION, not by discipline: `action_results`
/// builds ONE map per fixture and `driver::differential_one` hands the SAME
/// map to both the reference run and the direct run, so a `fail_*` action
/// fails identically on both backends and can never manufacture a divergence.
/// The reason carries no retryable prefix, so the engine fails it on the first
/// delivery with no retry timing in the trail.
const FAILING_ACTION_PREFIX: &str = "fail_";

/// A deterministic, schema-valid canned activity result per declared worker
/// action, keyed by action name. Both backends' runs use the SAME map, so an
/// activity that decodes on one decodes identically on the other. Unlike a
/// generic echo, each value satisfies its action's declared return-type codec,
/// so the workflow runs its REAL body instead of taking an activity-decode
/// detour (the flagship's greet -> shout path actually executes).
/// An action whose name starts with [`FAILING_ACTION_PREFIX`] is the deliberate
/// exception: it maps to a terminal `Err`, so failure-driven arms actually run.
/// This map answers ONLY `ActivityDispatcher` dispatches — i.e. declared worker
/// ACTIONS. Declared `child` workflows are launched through `workflow.spawn`,
/// not the dispatcher (the reference emitter's `CHILD_WITNESS` is a type anchor
/// the engine never calls), so children are deliberately NOT in this map: their
/// workflow types are not registered for spawning in the differential's package.
/// Three child-spawning fixtures fail AT that boundary with `AwlChildFailed`
/// (`child_call_awaited`, `child_spawn_combo`, `spawn_detached`); a fourth,
/// `declarations_combined`, takes a data-driven route-failure first — both
/// classes proven in `child_boundary.rs` (see `covered.rs`).
/// Returns the map alongside every witness fault found while building it, so a
/// canned result that does NOT satisfy its action's declared return type is
/// named at the fixture rather than discovered later as an activity-decode
/// detour that quietly changes what the run proves.
pub fn action_results(
    document: &Document,
    root: &Path,
) -> (HashMap<String, ActionResult>, Vec<String>) {
    let mut results = HashMap::new();
    let mut faults = Vec::new();
    for worker in &document.workers {
        for action in &worker.actions {
            if action.name.starts_with(FAILING_ACTION_PREFIX) {
                results.insert(
                    action.name.clone(),
                    Err(format!(
                        "terminal:`{}` fails by name (the `{FAILING_ACTION_PREFIX}` convention)",
                        action.name
                    )),
                );
                continue;
            }
            let schema = schema_for_typeref(document, root, &action.returns);
            let value = example_for_schema(&schema);
            if let Some(fault) = witness_fault(&schema, &value) {
                faults.push(format!("action `{}` result: {fault}", action.name));
            }
            match serde_json::to_string(&value) {
                Ok(json) => {
                    results.insert(action.name.clone(), Ok(json));
                }
                Err(error) => faults.push(format!(
                    "action `{}` result cannot be serialized: {error}",
                    action.name
                )),
            }
        }
    }
    (results, faults)
}

/// Resolves a declared return `TypeRef` to a JSON schema, using the same
/// deriver the manifest schemas use for named/builtin leaves and composing the
/// list/optional wrappers structurally.
fn schema_for_typeref(document: &Document, root: &Path, ty: &TypeRef) -> Value {
    match ty {
        TypeRef::Named { name, .. } => {
            aion_awl::schema_for_type_in(document, root, name).unwrap_or_else(|_| json!({}))
        }
        TypeRef::List { inner, .. } => {
            json!({ "type": "array", "items": schema_for_typeref(document, root, inner) })
        }
        TypeRef::Optional { inner, .. } => {
            json!({ "anyOf": [schema_for_typeref(document, root, inner), { "type": "null" }] })
        }
    }
}

/// Generates an object instance carrying only its required properties,
/// resolving nested `$ref`s against `defs`.
fn example_object(map: &Map<String, Value>, defs: Option<&Value>) -> Value {
    let mut instance = Map::new();
    let required: Vec<&str> = map
        .get("required")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    if let Some(Value::Object(properties)) = map.get("properties") {
        for (name, property_schema) in properties {
            if required.contains(&name.as_str()) {
                instance.insert(name.clone(), example_with_defs(property_schema, defs));
            }
        }
    }
    Value::Object(instance)
}
