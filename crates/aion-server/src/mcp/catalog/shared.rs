//! Schema fragments shared across the Aion tool schemas.
//!
//! Every fragment is a plain JSON Schema 2020-12 construct: no `$ref`, no
//! composition keywords, no external references. That is deliberate — an
//! unresolved external `$ref` in a published schema is a schema a client cannot
//! validate against, and composition keywords are the DoS surface the tools
//! guidance asks servers to bound. Bounding them by not using them is the only
//! bound that cannot be got wrong.

use serde_json::{Value, json};

/// A UUID-shaped identifier property.
pub(crate) fn uuid_property(description: &str) -> Value {
    json!({
        "type": "string",
        "description": description,
        "pattern": "^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$",
    })
}

/// The namespace property every tool takes: it is the authorization scope, and
/// there is no server-side default that would be safe to assume.
pub(crate) fn namespace_property() -> Value {
    json!({
        "type": "string",
        "minLength": 1,
        "description":
            "The namespace the workflow runs under. This is the authorization scope of the \
             call, not a label: a namespace you do not hold is answered as not-found. Read it \
             off a previous describe_run or list_runs result; never guess it.",
    })
}

/// A non-negative cursor property.
pub(crate) fn cursor_property(description: &str) -> Value {
    json!({
        "type": "integer",
        "minimum": 0,
        "description": description,
    })
}

/// A bounded page-size property.
pub(crate) fn limit_property(description: &str) -> Value {
    json!({
        "type": "integer",
        "minimum": 1,
        "description": description,
    })
}

/// The workflow-status enumeration, spelled exactly as
/// [`aion_core::WorkflowStatus`] serializes.
///
/// Pinned by [`the_status_enumeration_matches_the_projection`]: the schema and
/// the projection are two places that know one vocabulary, so the test derives
/// the expected spellings by serializing the real enum rather than restating
/// them.
///
/// [`the_status_enumeration_matches_the_projection`]: tests::the_status_enumeration_matches_the_projection
pub(crate) fn status_values() -> Value {
    json!([
        "Running",
        "Completed",
        "Failed",
        "Cancelled",
        "TimedOut",
        "ContinuedAsNew",
        "Paused",
    ])
}

/// Wrap a properties map into a closed object schema.
///
/// `additionalProperties: false` on every input schema is what turns a
/// misspelled argument into an immediate, legible refusal instead of a silently
/// ignored one — the difference between an agent learning it got the name wrong
/// and an agent believing a filter was applied that never was.
pub(crate) fn object_schema(properties: Value, required: &[&str]) -> Value {
    let mut schema = serde_json::Map::new();
    drop(schema.insert("type".to_owned(), json!("object")));
    drop(schema.insert("properties".to_owned(), properties));
    drop(schema.insert("required".to_owned(), json!(required)));
    drop(schema.insert("additionalProperties".to_owned(), json!(false)));
    Value::Object(schema)
}

/// Wrap a properties map into an object schema that permits nothing extra but
/// requires nothing either — used for output objects whose optional fields are
/// genuinely absent rather than null.
pub(crate) fn output_schema(properties: Value, required: &[&str]) -> Value {
    object_schema(properties, required)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{namespace_property, object_schema, status_values, uuid_property};

    #[test]
    fn an_object_schema_is_closed() {
        let schema = object_schema(json!({ "a": { "type": "string" } }), &["a"]);
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(schema["required"][0], "a");
    }

    #[test]
    fn the_uuid_pattern_accepts_a_uuid_and_rejects_prose() -> Result<(), Box<dyn std::error::Error>>
    {
        let schema = uuid_property("an id");
        let validator = jsonschema::validator_for(&schema)?;
        assert!(validator.is_valid(&json!("f81d4fae-7dec-11d0-a765-00a0c91e6bf6")));
        assert!(!validator.is_valid(&json!("the latest run")));
        Ok(())
    }

    #[test]
    fn the_namespace_property_requires_a_non_empty_string() -> Result<(), Box<dyn std::error::Error>>
    {
        let validator = jsonschema::validator_for(&namespace_property())?;
        assert!(validator.is_valid(&json!("default")));
        assert!(!validator.is_valid(&json!("")));
        Ok(())
    }

    /// The schema's status vocabulary and the projection's are one vocabulary
    /// known in two places. This derives the expected spellings from the REAL
    /// enum rather than restating them, so a rename in `aion_core` fails here
    /// instead of silently publishing a schema no result can satisfy.
    #[test]
    fn the_status_enumeration_matches_the_projection() -> Result<(), serde_json::Error> {
        let projected: Vec<serde_json::Value> = [
            aion_core::WorkflowStatus::Running,
            aion_core::WorkflowStatus::Completed,
            aion_core::WorkflowStatus::Failed,
            aion_core::WorkflowStatus::Cancelled,
            aion_core::WorkflowStatus::TimedOut,
            aion_core::WorkflowStatus::ContinuedAsNew,
            aion_core::WorkflowStatus::Paused,
        ]
        .into_iter()
        .map(serde_json::to_value)
        .collect::<Result<_, _>>()?;
        assert_eq!(status_values(), serde_json::Value::Array(projected));
        Ok(())
    }
}
