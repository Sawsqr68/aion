//! Declared-contract admission: does a caller-supplied value satisfy the
//! schema the package committed to?
//!
//! A `.v4` package identity binds its declared shapes — the workflow input
//! schema and every signal payload schema — into the content hash. This module
//! is the one place that answers "does this value satisfy that declaration",
//! so the engine can refuse a mismatch at the boundary the value enters
//! through, BEFORE anything is recorded or consumed.
//!
//! It is deliberately pure: no I/O, no logging, no state. The caller supplies
//! the schema and the value and receives either admission or a refusal naming
//! every field that did not match.

use serde_json::Value;

use crate::contract::{PackageContract, SignalContract};

/// One declared schema in a package contract that no validator can compile,
/// named by the declaration that carries it.
///
/// An unenforceable declaration is not a harmless one: [`admit_value`] can only
/// answer [`AdmissionError::UnusableSchema`] for it, and every admission
/// boundary's response to that is to let the value through unchecked. A
/// contract carrying one is a contract the engine cannot honour, and the
/// operator needs to be told WHICH declaration — a package can declare dozens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnenforceableSchema {
    /// The declaration the schema belongs to, in the author's own vocabulary.
    pub declaration: String,
    /// The compiler's own reason for refusing the schema.
    pub reason: String,
}

impl std::fmt::Display for UnenforceableSchema {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.declaration, self.reason)
    }
}

/// A caller-supplied value did not satisfy a declared contract schema.
///
/// Both variants are caller-facing refusals, never engine faults: they are
/// returned to whoever supplied the value, with nothing recorded.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AdmissionError {
    /// The declared schema itself is not a usable JSON Schema.
    ///
    /// This is a defect in the package, not in the caller's value, and is kept
    /// distinct so it can never be reported as "your payload is wrong".
    #[error("the declared schema is not a valid JSON Schema: {reason}")]
    UnusableSchema {
        /// Why the schema could not be compiled.
        reason: String,
    },

    /// The value did not satisfy the declared schema.
    #[error("{violations}")]
    Mismatch {
        /// Every violation, each naming the JSON-pointer location that failed.
        violations: String,
    },
}

/// Whether `schema` declares nothing about the values it covers.
///
/// `null` is what a manifest that never declared a shape carries — the
/// [`PackageContract::default`] input and output schemas are both `null` — and
/// JSON Schema's own accept-everything forms (`{}` and `true`) say the same
/// thing in the schema's own vocabulary. None of them constrains anything, so
/// there is nothing to admit a value against and admission is skipped entirely
/// rather than run against a vacuous ruler.
#[must_use]
pub fn declares_nothing(schema: &Value) -> bool {
    match schema {
        Value::Null | Value::Bool(true) => true,
        Value::Object(members) => members.is_empty(),
        _ => false,
    }
}

/// Checks that `schema` can be compiled into a validator at all, describing
/// why when it cannot.
///
/// An uncompilable declared schema is not a harmless one. [`admit_value`] can
/// only answer [`AdmissionError::UnusableSchema`], and the engine's response to
/// that is to admit the value UNCHECKED and log a warning — so a schema that
/// does not compile silently switches admission off for everything it declares.
/// Authoring and packaging tools call this so the author meets the problem at
/// the door, where it is a diagnostic, instead of at a running server, where it
/// is an absence.
///
/// A schema that declares nothing ([`declares_nothing`]) is usable by
/// definition: admission is skipped for it deliberately, not by failure.
///
/// # Errors
///
/// Returns the compiler's own reason when `schema` is not a usable validator.
pub fn schema_is_usable(schema: &Value) -> Result<(), String> {
    if declares_nothing(schema) {
        return Ok(());
    }
    jsonschema::validator_for(schema)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// Checks `value` against the declared JSON Schema `schema`.
///
/// Every violation is reported, each prefixed with the JSON-pointer location
/// of the field that failed (or `<root>` when the whole value is wrong), so a
/// caller can see exactly what to correct. Nothing is truncated: the refusal
/// is the operator's only diagnostic, and a partial list would hide a second
/// mistake behind the first.
///
/// # Errors
///
/// Returns [`AdmissionError::UnusableSchema`] when the declared schema cannot
/// be compiled, and [`AdmissionError::Mismatch`] when `value` violates it.
pub fn admit_value(schema: &Value, value: &Value) -> Result<(), AdmissionError> {
    let validator =
        jsonschema::validator_for(schema).map_err(|error| AdmissionError::UnusableSchema {
            reason: error.to_string(),
        })?;
    if validator.is_valid(value) {
        return Ok(());
    }
    let violations = validator
        .iter_errors(value)
        .map(|error| {
            let location = error.instance_path().to_string();
            if location.is_empty() {
                format!("<root>: {error}")
            } else {
                format!("{location}: {error}")
            }
        })
        .collect::<Vec<_>>()
        .join("; ");
    // `is_valid` said no, so `iter_errors` yields at least one error; an empty
    // render would still be an honest refusal rather than a silent admission.
    Err(AdmissionError::Mismatch { violations })
}

impl PackageContract {
    /// Every declared schema in this contract that cannot be compiled into a
    /// validator, in declaration order.
    ///
    /// This is the whole contract surface, not only the schemas today's
    /// admission boundaries happen to read: a declaration is identity-bound,
    /// so a package that commits to a shape no validator can compile has
    /// promised something it can never be held to, whichever boundary reaches
    /// it first. An empty result means every declaration this package makes is
    /// one the engine can actually enforce.
    ///
    /// A declaration that constrains nothing ([`declares_nothing`]) is not a
    /// failure — it says nothing on purpose, and admission skips it
    /// deliberately rather than by breaking.
    #[must_use]
    pub fn unenforceable_schemas(&self) -> Vec<UnenforceableSchema> {
        let mut found = Vec::new();
        let mut check = |declaration: String, schema: &Value| {
            if let Err(reason) = schema_is_usable(schema) {
                found.push(UnenforceableSchema {
                    declaration,
                    reason,
                });
            }
        };

        check("the workflow input type".to_owned(), &self.input_schema);
        check("the workflow result type".to_owned(), &self.output_schema);
        for entry in &self.additional_workflows {
            let workflow_type = &entry.workflow_type;
            check(
                format!("the input type of workflow `{workflow_type}`"),
                &entry.input_schema,
            );
            check(
                format!("the result type of workflow `{workflow_type}`"),
                &entry.output_schema,
            );
        }
        for signal in &self.signals {
            let name = &signal.name;
            check(
                format!("the payload type of signal `{name}`"),
                &signal.input_schema,
            );
        }
        for child in &self.children {
            let name = &child.name;
            check(
                format!("the input type of child workflow `{name}`"),
                &child.input_schema,
            );
            check(
                format!("the result type of child workflow `{name}`"),
                &child.output_schema,
            );
        }
        for worker in &self.workers {
            let queue = &worker.task_queue;
            for action in &worker.actions {
                let name = &action.name;
                check(
                    format!("the parameter types of activity `{name}` on queue `{queue}`"),
                    &action.input_schema,
                );
                check(
                    format!("the result type of activity `{name}` on queue `{queue}`"),
                    &action.output_schema,
                );
            }
        }
        found
    }

    /// The declared signal record for `signal_name`, when this contract
    /// declares one.
    #[must_use]
    pub fn declared_signal(&self, signal_name: &str) -> Option<&SignalContract> {
        self.signals
            .iter()
            .find(|signal| signal.name == signal_name)
    }

    /// Every declared signal name, in stable sorted order.
    ///
    /// Used to tell a caller which names the package actually accepts when it
    /// named one the package does not declare.
    #[must_use]
    pub fn declared_signal_names(&self) -> Vec<&str> {
        let mut names = self
            .signals
            .iter()
            .map(|signal| signal.name.as_str())
            .collect::<Vec<_>>();
        names.sort_unstable();
        names
    }

    /// The declared input schema for the entry `workflow_type`.
    ///
    /// A package archive can carry several workflow entries under one identity:
    /// the primary entry's schema is [`PackageContract::input_schema`] and every
    /// additional entry carries its own in
    /// [`PackageContract::additional_workflows`]. An additional entry is matched
    /// by name first, so a synthesized child entry is never validated against
    /// the primary entry's shape; any other name is the primary entry, which is
    /// the only other type a catalog can hold for this identity.
    #[must_use]
    pub fn entry_input_schema(&self, workflow_type: &str) -> &Value {
        self.additional_workflows
            .iter()
            .find(|entry| entry.workflow_type == workflow_type)
            .map_or(&self.input_schema, |entry| &entry.input_schema)
    }
}

#[cfg(test)]
#[path = "admission_tests.rs"]
mod tests;
