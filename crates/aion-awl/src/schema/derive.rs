//! JSON Schema (draft 2020-12) derivation from AWL type declarations — the
//! one pure public derivation every consumer shares (start forms, worker
//! contracts, model output contracts).
//!
//! Shorthand records and enums derive structurally; `///` doc lines flow to
//! `description`s; `?` maps to "not in `required`". Schema-door types
//! (inline `schema { … }` and `schema("file")`) re-emit their source JSON
//! verbatim, so constraint keywords ride through untouched — with their
//! internal `$ref`s rebased for the position the door lands in.
//!
//! Every derived schema is proven COMPILABLE before it is returned, against
//! the very validator the engine admits payloads with. A declared schema the
//! engine cannot compile does not fail loudly there: admission logs a warning
//! and lets the payload through unchecked, so an uncompilable schema silently
//! switches validation off for whatever it declares. Refusing it here turns
//! that silence into a diagnostic at `aion awl check`, where it is the
//! author's to fix.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::{Map, Value, json};

use crate::Span;
use crate::ast::{Document, ParamDecl, TypeBody, TypeDecl, TypeRef, doc_text};

use super::error::SchemaError;

/// Derive the JSON Schema for a named declared type, with schema imports
/// unresolvable (no document directory). Prefer [`schema_for_type_in`] when
/// the `.awl` file's directory is known.
///
/// # Errors
///
/// Returns [`SchemaError`] when the type is undeclared, or when its schema
/// door cannot be resolved or parsed.
pub fn schema_for_type(document: &Document, name: &str) -> Result<Value, SchemaError> {
    Deriver::new(document, None).type_schema(name)
}

/// Derive the JSON Schema for a named declared type, resolving schema
/// imports relative to `root` (the document's directory).
///
/// # Errors
///
/// Returns [`SchemaError`] when the type is undeclared, or when its schema
/// door cannot be resolved or parsed.
pub fn schema_for_type_in(
    document: &Document,
    root: &Path,
    name: &str,
) -> Result<Value, SchemaError> {
    Deriver::new(document, Some(root)).type_schema(name)
}

/// Derive the workflow's start contract: one object schema over its inputs,
/// `?`-typed inputs omitted from `required`, narration as the description.
///
/// # Errors
///
/// Returns [`SchemaError`] when an input's schema door cannot be resolved
/// or parsed.
pub fn schema_for_workflow(document: &Document) -> Result<Value, SchemaError> {
    Deriver::new(document, None).workflow_schema()
}

/// Derive the workflow's start contract, resolving schema imports relative
/// to `root` (the document's directory).
///
/// # Errors
///
/// Returns [`SchemaError`] when an input's schema door cannot be resolved
/// or parsed.
pub fn schema_for_workflow_in(document: &Document, root: &Path) -> Result<Value, SchemaError> {
    Deriver::new(document, Some(root)).workflow_schema()
}

/// Derive the workflow's outcome envelope, with schema imports unresolvable
/// (no document directory). Prefer [`schema_for_outcomes_in`] when the
/// `.awl` file's directory is known.
///
/// # Errors
///
/// Returns [`SchemaError`] when an outcome payload type is undeclared, or
/// when its schema door cannot be resolved or parsed.
pub fn schema_for_outcomes(document: &Document) -> Result<Value, SchemaError> {
    Deriver::new(document, None).envelope_schema()
}

/// Derive the workflow's outcome envelope — the `{ "outcome": name,
/// "payload": … }` object the emitted codec produces — resolving schema
/// imports relative to `root` (the document's directory). A single-outcome
/// workflow pins `outcome` with `const`; multiple outcomes become `oneOf`
/// arms pairing each outcome name with its payload schema.
///
/// # Errors
///
/// Returns [`SchemaError`] when an outcome payload type is undeclared, or
/// when its schema door cannot be resolved or parsed.
pub fn schema_for_outcomes_in(document: &Document, root: &Path) -> Result<Value, SchemaError> {
    Deriver::new(document, Some(root)).envelope_schema()
}

/// Derive the workflow's read-only surface: one object whose properties are
/// the declared query names and whose values are their answer schemas, with
/// schema imports unresolvable (no document directory). Prefer
/// [`schema_for_queries_in`] when the `.awl` file's directory is known.
///
/// # Errors
///
/// Returns [`SchemaError`] when an answer type is undeclared, or when its
/// schema door cannot be resolved or parsed.
pub fn schema_for_queries(document: &Document) -> Result<Value, SchemaError> {
    Deriver::new(document, None).queries_schema()
}

/// Derive the workflow's read-only surface — every declared query name paired
/// with the schema of the answer an operator receives — resolving schema
/// imports relative to `root` (the document's directory).
///
/// A workflow declaring no queries derives an empty `properties` object, which
/// is the honest answer: the run reports nothing. Query answers are never
/// `required`; whether a query has been answered yet is a runtime fact.
///
/// # Errors
///
/// Returns [`SchemaError`] when an answer type is undeclared, or when its
/// schema door cannot be resolved or parsed.
pub fn schema_for_queries_in(document: &Document, root: &Path) -> Result<Value, SchemaError> {
    Deriver::new(document, Some(root)).queries_schema()
}

/// Derive an object schema over callable parameters, resolving schema imports
/// relative to `root`.
///
/// # Errors
///
/// Returns [`SchemaError`] when a parameter type is undeclared or its schema
/// door cannot be resolved or parsed.
pub fn schema_for_params_in(
    document: &Document,
    root: &Path,
    params: &[ParamDecl],
) -> Result<Value, SchemaError> {
    Deriver::new(document, Some(root)).params_schema(params)
}

/// Derive one callable result or signal payload schema, resolving schema
/// imports relative to `root`.
///
/// # Errors
///
/// Returns [`SchemaError`] when the type is undeclared or its schema door
/// cannot be resolved or parsed.
pub fn schema_for_type_ref_in(
    document: &Document,
    root: &Path,
    ty: &TypeRef,
) -> Result<Value, SchemaError> {
    Deriver::new(document, Some(root)).standalone_type_ref_schema(ty)
}

/// Every schema the package contract derives FROM THE DOCUMENT, in one place.
///
/// The contract's entry input and result schemas are supplied by the compiler
/// from the workflow header, so they are not here; everything else the
/// contract commits to — each declared signal's payload, each child's
/// parameters and result, each activity's parameters and result on each
/// worker — is derived exactly here, exactly as `compile::package_contract`
/// derives it.
///
/// It exists so the CHECKER can prove those schemas compile without owning a
/// second copy of the enumeration. A checker that listed a different set from
/// the one the compiler derives would pass documents the compiler then refuses,
/// which is the divergence this is deliberately built to avoid.
///
/// `root` is optional because the checker can be called without a document
/// directory. A schema IMPORT is unresolvable then, and that failure is an
/// artefact of how the checker was called rather than anything about the
/// document — callers filter for the failure they asked about.
pub(crate) fn derived_contract_schemas(
    document: &Document,
    root: Option<&Path>,
) -> Vec<Result<Value, SchemaError>> {
    let deriver = Deriver::new(document, root);
    let mut schemas = Vec::new();
    for signal in &document.signals {
        schemas.push(attributed(
            deriver.standalone_type_ref_schema(&signal.ty),
            format!("signal {}", signal.name),
            signal.name_span,
        ));
    }
    for child in &document.children {
        schemas.push(attributed(
            deriver.params_schema(&child.params),
            format!("the parameters of child {}", child.name),
            child.name_span,
        ));
        schemas.push(attributed(
            deriver.standalone_type_ref_schema(&child.returns),
            format!("the result of child {}", child.name),
            child.name_span,
        ));
    }
    for worker in &document.workers {
        for action in &worker.actions {
            schemas.push(attributed(
                deriver.params_schema(&action.params),
                format!("the parameters of action {}", action.name),
                action.name_span,
            ));
            schemas.push(attributed(
                deriver.standalone_type_ref_schema(&action.returns),
                format!("the result of action {}", action.name),
                action.name_span,
            ));
        }
    }
    schemas
}

/// Re-anchors an unusable-schema refusal on the DECLARATION that carries it.
///
/// `params_schema` and `standalone_type_ref_schema` are shared by every
/// callable surface, so they can only name the document. That is fine for one
/// refusal and useless for three: the checker sorts and dedups its
/// diagnostics, so three declarations failing for the same reason collapse
/// into a single message pointing at the workflow's name — the author is told
/// something is wrong and not where. Naming the declaration makes each
/// refusal distinct, which is both a better diagnostic and the only reason
/// they survive dedup as separate findings.
fn attributed(
    result: Result<Value, SchemaError>,
    name: String,
    span: Span,
) -> Result<Value, SchemaError> {
    match result {
        Err(SchemaError::UnusableSchema { detail, .. }) => {
            Err(SchemaError::UnusableSchema { name, detail, span })
        }
        other => other,
    }
}

/// Refuses a derived schema that cannot be compiled into a validator.
///
/// The check runs through `aion_package::schema_is_usable`, which is the same
/// `jsonschema` compilation the engine performs at admission — so a schema
/// that passes here is one the engine can actually enforce, not merely one
/// that looks well-formed.
fn verified(schema: Value, name: &str, span: Span) -> Result<Value, SchemaError> {
    match aion_package::schema_is_usable(&schema) {
        Ok(()) => Ok(schema),
        Err(detail) => Err(SchemaError::UnusableSchema {
            name: name.to_owned(),
            detail,
            span,
        }),
    }
}

struct Deriver<'a> {
    document: &'a Document,
    root: Option<&'a Path>,
}

impl<'a> Deriver<'a> {
    const fn new(document: &'a Document, root: Option<&'a Path>) -> Self {
        Self { document, root }
    }

    fn decl(&self, name: &str) -> Option<&'a TypeDecl> {
        self.document.types.iter().find(|decl| decl.name == name)
    }

    fn type_schema(&self, name: &str) -> Result<Value, SchemaError> {
        if let Some(schema) = builtin_schema(name) {
            return Ok(schema);
        }
        let Some(decl) = self.decl(name) else {
            return Err(SchemaError::UnknownType {
                name: name.to_owned(),
                span: self.document.name_span,
            });
        };
        let mut defs = Map::new();
        let mut visited = BTreeSet::new();
        visited.insert(name.to_owned());
        let mut schema = self.decl_schema(decl, name, &mut defs, &mut visited)?;
        if !defs.is_empty()
            && let Some(object) = schema.as_object_mut()
        {
            object.insert("$defs".to_owned(), Value::Object(defs));
        }
        verified(schema, name, decl.name_span)
    }

    /// Refuse a workflow-only derivation over a `worker` document.
    ///
    /// A worker document has no inputs, no outcomes and no queries, so each
    /// of these derivations would otherwise return a well-formed schema of
    /// nothing — the shape an operator would then validate a start payload
    /// against. An empty answer to a question the document cannot be asked
    /// is worse than no answer, so the family is refused by name.
    fn require_workflow(&self, contract: &'static str) -> Result<(), SchemaError> {
        if self.document.family == crate::ast::DocumentFamily::Worker {
            return Err(SchemaError::NotAWorkflow {
                name: self.document.name.clone(),
                contract,
                span: self.document.name_span,
            });
        }
        Ok(())
    }

    fn workflow_schema(&self) -> Result<Value, SchemaError> {
        self.require_workflow("start contract")?;
        let mut defs = Map::new();
        let mut properties = Map::new();
        let mut required = Vec::new();
        for input in &self.document.inputs {
            let mut visited = BTreeSet::new();
            let schema = self.type_ref_schema(&input.ty, "", &mut defs, &mut visited)?;
            if !matches!(input.ty, TypeRef::Optional { .. }) {
                required.push(Value::String(input.name.clone()));
            }
            properties.insert(input.name.clone(), schema);
        }
        let mut object = Map::new();
        // The dialect pin mirrors `envelope_schema` (and the legacy authored
        // schema documents): both derived contracts carry it, so the native
        // path's manifest schemas match what the legacy path embedded.
        object.insert(
            "$schema".to_owned(),
            json!("https://json-schema.org/draft/2020-12/schema"),
        );
        object.insert("type".to_owned(), json!("object"));
        let narration = doc_text(&self.document.narration);
        if !narration.is_empty() {
            object.insert("description".to_owned(), Value::String(narration));
        }
        object.insert("properties".to_owned(), Value::Object(properties));
        object.insert("required".to_owned(), Value::Array(required));
        if !defs.is_empty() {
            object.insert("$defs".to_owned(), Value::Object(defs));
        }
        verified(
            Value::Object(object),
            &self.document.name,
            self.document.name_span,
        )
    }

    fn queries_schema(&self) -> Result<Value, SchemaError> {
        self.require_workflow("read-only query surface")?;
        let mut defs = Map::new();
        let mut properties = Map::new();
        for query in &self.document.queries {
            let mut visited = BTreeSet::new();
            let schema = self.type_ref_schema(&query.ty, "", &mut defs, &mut visited)?;
            properties.insert(query.name.clone(), schema);
        }
        let mut object = Map::new();
        object.insert(
            "$schema".to_owned(),
            json!("https://json-schema.org/draft/2020-12/schema"),
        );
        object.insert(
            "description".to_owned(),
            Value::String(format!(
                "The {} read-only surface: each query an operator may ask a running \
                 instance, with the answer it reports.",
                self.document.name
            )),
        );
        object.insert("type".to_owned(), json!("object"));
        object.insert("properties".to_owned(), Value::Object(properties));
        if !defs.is_empty() {
            object.insert("$defs".to_owned(), Value::Object(defs));
        }
        verified(
            Value::Object(object),
            &self.document.name,
            self.document.name_span,
        )
    }

    fn params_schema(&self, params: &[ParamDecl]) -> Result<Value, SchemaError> {
        let mut defs = Map::new();
        let mut properties = Map::new();
        let mut required = Vec::new();
        for param in params {
            let mut visited = BTreeSet::new();
            let schema = self.type_ref_schema(&param.ty, "", &mut defs, &mut visited)?;
            if !matches!(param.ty, TypeRef::Optional { .. }) {
                required.push(Value::String(param.name.clone()));
            }
            properties.insert(param.name.clone(), schema);
        }
        let mut object = Map::new();
        object.insert(
            "$schema".to_owned(),
            json!("https://json-schema.org/draft/2020-12/schema"),
        );
        object.insert("type".to_owned(), json!("object"));
        object.insert("properties".to_owned(), Value::Object(properties));
        object.insert("required".to_owned(), Value::Array(required));
        if !defs.is_empty() {
            object.insert("$defs".to_owned(), Value::Object(defs));
        }
        verified(
            Value::Object(object),
            &self.document.name,
            self.document.name_span,
        )
    }

    fn standalone_type_ref_schema(&self, ty: &TypeRef) -> Result<Value, SchemaError> {
        let mut defs = Map::new();
        let mut visited = BTreeSet::new();
        let mut schema = self.type_ref_schema(ty, "", &mut defs, &mut visited)?;
        if !defs.is_empty()
            && let Some(object) = schema.as_object_mut()
        {
            object.insert("$defs".to_owned(), Value::Object(defs));
        }
        verified(schema, &self.document.name, self.document.name_span)
    }

    fn envelope_schema(&self) -> Result<Value, SchemaError> {
        self.require_workflow("outcome envelope")?;
        let mut defs = Map::new();
        let mut arms = Vec::new();
        for outcome in &self.document.outcomes {
            let payload = self.outcome_payload_schema(&outcome.ty, &mut defs)?;
            arms.push((outcome.name.clone(), payload));
        }
        let mut object = Map::new();
        object.insert(
            "$schema".to_owned(),
            json!("https://json-schema.org/draft/2020-12/schema"),
        );
        object.insert(
            "description".to_owned(),
            Value::String(format!(
                "The {} outcome envelope: which declared workflow outcome fired, \
                 with its payload.",
                self.document.name
            )),
        );
        object.insert("type".to_owned(), json!("object"));
        object.insert("required".to_owned(), json!(["outcome", "payload"]));
        if arms.len() == 1 {
            let mut properties = Map::new();
            for (name, payload) in arms {
                properties.insert("outcome".to_owned(), json!({ "const": name }));
                properties.insert("payload".to_owned(), payload);
            }
            object.insert("properties".to_owned(), Value::Object(properties));
        } else {
            let branches: Vec<Value> = arms
                .into_iter()
                .map(|(name, payload)| {
                    json!({
                        "properties": {
                            "outcome": { "const": name },
                            "payload": payload,
                        }
                    })
                })
                .collect();
            object.insert("oneOf".to_owned(), Value::Array(branches));
        }
        if !defs.is_empty() {
            object.insert("$defs".to_owned(), Value::Object(defs));
        }
        Ok(Value::Object(object))
    }

    /// An outcome payload's schema, inlined like the reference envelope. A
    /// named payload type inlines its body; a payload whose inlining would
    /// produce a document-root `"#"` self-reference (which would resolve to
    /// the envelope, not the payload) derives through `$defs` instead.
    fn outcome_payload_schema(
        &self,
        ty: &TypeRef,
        defs: &mut Map<String, Value>,
    ) -> Result<Value, SchemaError> {
        if let TypeRef::Named { name, .. } = ty
            && builtin_schema(name).is_none()
            && let Some(decl) = self.decl(name)
        {
            let mut trial = defs.clone();
            let mut visited = BTreeSet::new();
            visited.insert(name.clone());
            let inline = self.decl_schema(decl, name, &mut trial, &mut visited)?;
            if !contains_root_ref(&inline) && !trial.values().any(contains_root_ref) {
                *defs = trial;
                return Ok(inline);
            }
        }
        let mut visited = BTreeSet::new();
        self.type_ref_schema(ty, "", defs, &mut visited)
    }

    /// The schema of one declaration's body, with `root_name` the type the
    /// emitted document is rooted at (self-references become `"#"`).
    fn decl_schema(
        &self,
        decl: &TypeDecl,
        root_name: &str,
        defs: &mut Map<String, Value>,
        visited: &mut BTreeSet<String>,
    ) -> Result<Value, SchemaError> {
        match &decl.body {
            TypeBody::Record { fields } => {
                let mut properties = Map::new();
                let mut required = Vec::new();
                for field in fields {
                    let mut schema = self.type_ref_schema(&field.ty, root_name, defs, visited)?;
                    let docs = doc_text(&field.docs);
                    if !docs.is_empty()
                        && let Some(object) = schema.as_object_mut()
                    {
                        object.insert("description".to_owned(), Value::String(docs));
                    }
                    if !matches!(field.ty, TypeRef::Optional { .. }) {
                        required.push(Value::String(field.name.clone()));
                    }
                    properties.insert(field.name.clone(), schema);
                }
                let mut object = Map::new();
                object.insert("type".to_owned(), json!("object"));
                let docs = doc_text(&decl.docs);
                if !docs.is_empty() {
                    object.insert("description".to_owned(), Value::String(docs));
                }
                object.insert("properties".to_owned(), Value::Object(properties));
                object.insert("required".to_owned(), Value::Array(required));
                Ok(Value::Object(object))
            }
            TypeBody::Enum { variants } => {
                let mut object = Map::new();
                object.insert("type".to_owned(), json!("string"));
                let docs = doc_text(&decl.docs);
                if !docs.is_empty() {
                    object.insert("description".to_owned(), Value::String(docs));
                }
                object.insert(
                    "enum".to_owned(),
                    Value::Array(
                        variants
                            .iter()
                            .map(|variant| Value::String(variant.name.clone()))
                            .collect(),
                    ),
                );
                Ok(Value::Object(object))
            }
            TypeBody::SchemaInline { body, .. } => {
                let mut door: Value =
                    serde_json::from_str(body).map_err(|error| SchemaError::InvalidJson {
                        name: decl.name.clone(),
                        detail: error.to_string(),
                        span: decl.name_span,
                    })?;
                rebase_door_refs(&mut door, &decl.name, root_name);
                Ok(door)
            }
            TypeBody::SchemaImport { path, path_span } => {
                let Some(root) = self.root else {
                    return Err(SchemaError::ImportUnresolved {
                        path: path.clone(),
                        span: *path_span,
                    });
                };
                let text = std::fs::read_to_string(root.join(path)).map_err(|error| {
                    SchemaError::ImportUnreadable {
                        path: path.clone(),
                        detail: error.to_string(),
                        span: *path_span,
                    }
                })?;
                let mut door: Value =
                    serde_json::from_str(&text).map_err(|error| SchemaError::InvalidJson {
                        name: decl.name.clone(),
                        detail: error.to_string(),
                        span: *path_span,
                    })?;
                rebase_door_refs(&mut door, &decl.name, root_name);
                Ok(door)
            }
        }
    }

    fn type_ref_schema(
        &self,
        type_ref: &TypeRef,
        root_name: &str,
        defs: &mut Map<String, Value>,
        visited: &mut BTreeSet<String>,
    ) -> Result<Value, SchemaError> {
        match type_ref {
            TypeRef::Named { name, span } => {
                if let Some(schema) = builtin_schema(name) {
                    return Ok(schema);
                }
                if name == root_name {
                    return Ok(json!({ "$ref": "#" }));
                }
                let Some(decl) = self.decl(name) else {
                    return Err(SchemaError::UnknownType {
                        name: name.clone(),
                        span: *span,
                    });
                };
                if visited.insert(name.clone()) {
                    let schema = self.decl_schema(decl, root_name, defs, visited)?;
                    defs.insert(name.clone(), schema);
                }
                Ok(json!({ "$ref": format!("#/$defs/{name}") }))
            }
            TypeRef::List { inner, .. } => {
                let items = self.type_ref_schema(inner, root_name, defs, visited)?;
                Ok(json!({ "type": "array", "items": items }))
            }
            TypeRef::Optional { inner, .. } => {
                // Optionality is the field's membership in `required`; the
                // value schema is the inner type's — never nullable.
                self.type_ref_schema(inner, root_name, defs, visited)
            }
        }
    }
}

/// Rebases a schema door's own internal `$ref` pointers for the position the
/// door document is about to occupy.
///
/// A door is a COMPLETE JSON Schema document written on its own terms, so its
/// internal references are rooted at ITS root: `order.schema.json` says
/// `"$ref": "#/$defs/Line"` and carries a matching `$defs/Line`. When that
/// document is folded in as `#/$defs/Order` of a derived contract, `#` stops
/// meaning the door and starts meaning the WHOLE contract — where there is no
/// `$defs/Line`. The composed schema was then unresolvable, and the engine's
/// response to a schema it cannot compile is to log a warning and admit the
/// payload unchecked (`aion/src/loader/catalog_admission.rs`). So a door with
/// an internal `$ref` silently turned its workflow's input validation OFF.
///
/// The rebase is positional and total: `#` becomes the door's new home, and
/// every other document-local pointer is re-rooted underneath it. Nothing is
/// renamed and nothing is hoisted, so two doors can carry the same `$defs`
/// name without colliding. External `$ref`s are left exactly as written.
///
/// `root_name` is the type the derivation is rooted AT. When the door is that
/// type it stays at the document root, its pointers already mean what they say,
/// and rebasing them would break the thing it is meant to fix.
fn rebase_door_refs(door: &mut Value, door_name: &str, root_name: &str) {
    if door_name == root_name {
        return;
    }
    rebase_refs_under(door, &format!("#/$defs/{door_name}"));
}

/// Rewrites every document-local `$ref` in a schema tree to sit under `home`.
fn rebase_refs_under(value: &mut Value, home: &str) {
    match value {
        Value::Object(object) => {
            if let Some(Value::String(target)) = object.get("$ref") {
                let rebased = if target == "#" {
                    Some(home.to_owned())
                } else {
                    target
                        .strip_prefix("#/")
                        .map(|rest| format!("{home}/{rest}"))
                };
                if let Some(rebased) = rebased {
                    object.insert("$ref".to_owned(), Value::String(rebased));
                }
            }
            for (key, inner) in object.iter_mut() {
                // `$ref` was just rewritten in place; recursing into a string
                // is a no-op, but skipping it keeps the intent obvious.
                if key != "$ref" {
                    rebase_refs_under(inner, home);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                rebase_refs_under(item, home);
            }
        }
        _ => {}
    }
}

/// Whether a schema tree carries a document-root `{"$ref": "#"}` reference.
fn contains_root_ref(value: &Value) -> bool {
    match value {
        Value::Object(object) => object.iter().any(|(key, inner)| {
            (key == "$ref" && inner.as_str() == Some("#")) || contains_root_ref(inner)
        }),
        Value::Array(items) => items.iter().any(contains_root_ref),
        _ => false,
    }
}

fn builtin_schema(name: &str) -> Option<Value> {
    match name {
        "Bool" => Some(json!({ "type": "boolean" })),
        "Int" => Some(json!({ "type": "integer" })),
        "Float" => Some(json!({ "type": "number" })),
        "String" | "Dir" => Some(json!({ "type": "string" })),
        "Nil" => Some(json!({ "type": "null" })),
        _ => None,
    }
}
