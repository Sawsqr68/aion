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
