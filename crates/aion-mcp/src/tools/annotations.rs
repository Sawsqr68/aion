//! `x-mcp-header` annotation discovery and validation.
//!
//! An annotation binds one tool argument to one HTTP header, so a proxy can
//! route on it without parsing the body. The constraints exist because a
//! header is a far weaker place than a JSON body: it carries no types, it is
//! visible to every intermediary, and its name has a strict grammar.
//!
//! The reachability rule is the subtle one. An annotation counts ONLY when the
//! property it sits on is reachable from the schema root through a chain made
//! entirely of `properties` keys. Not through `items`, not through
//! `oneOf`/`anyOf`/`allOf`/`not`, not through `if`/`then`/`else`, and not
//! through `$ref`. The reason is that every one of those makes the property's
//! presence CONDITIONAL on the instance — and a header contract that only
//! sometimes applies is one a proxy cannot honour. An annotation anywhere else
//! does not make the tool weaker; it makes the tool definition INVALID, and
//! this module says so rather than quietly ignoring it.

use std::collections::BTreeSet;

use serde_json::Value;

/// One validated `x-mcp-header` binding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeaderBinding {
    /// The annotation's header suffix, exactly as declared.
    pub header_name: String,
    /// The path of `properties` keys from the schema root to the annotated
    /// property.
    pub path: Vec<String>,
}

impl HeaderBinding {
    /// The full lowercase header name a request must carry.
    #[must_use]
    pub fn full_header(&self) -> String {
        format!("mcp-param-{}", self.header_name.to_ascii_lowercase())
    }

    /// The annotated property's value within an arguments object, if present.
    #[must_use]
    pub fn resolve<'a>(&self, arguments: &'a Value) -> Option<&'a Value> {
        let mut cursor = arguments;
        for segment in &self.path {
            cursor = cursor.get(segment)?;
        }
        Some(cursor).filter(|value| !value.is_null())
    }
}

/// Why an `x-mcp-header` annotation is invalid.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AnnotationError {
    /// The annotation value is not a string, or is empty.
    #[error("x-mcp-header at {path} must be a non-empty string")]
    NotAName {
        /// Where the annotation was found.
        path: String,
    },
    /// The name violates the HTTP field-name token grammar.
    #[error("x-mcp-header `{name}` at {path} is not a valid HTTP field-name token")]
    NotAToken {
        /// The offending name.
        name: String,
        /// Where the annotation was found.
        path: String,
    },
    /// Two annotations share a name, case-insensitively.
    #[error("x-mcp-header `{name}` is declared more than once in one inputSchema")]
    Duplicate {
        /// The repeated name.
        name: String,
    },
    /// The annotated property is not a permitted primitive type.
    #[error(
        "x-mcp-header `{name}` at {path} annotates type `{declared}`; only integer, string, and \
         boolean may be carried in a header"
    )]
    NotPrimitive {
        /// The offending name.
        name: String,
        /// Where the annotation was found.
        path: String,
        /// The type that was declared.
        declared: String,
    },
    /// The annotation sits somewhere a proxy cannot statically reach.
    #[error(
        "x-mcp-header `{name}` at {path} is not statically reachable from the schema root through \
         `properties` alone, which makes the tool definition invalid"
    )]
    Unreachable {
        /// The offending name.
        name: String,
        /// Where the annotation was found.
        path: String,
    },
}

/// The JSON Schema types a header may carry.
///
/// `number` is deliberately absent. A header is text, and a float has no single
/// text spelling — `0.1` and `1e-1` are the same number and different strings,
/// so a server and a proxy could disagree about whether a request matched. An
/// integer has one canonical value per spelling, which is why it is allowed
/// and `number` is not.
const PERMITTED_TYPES: &[&str] = &["integer", "string", "boolean"];

/// Every keyword through which a subschema is NOT statically reachable.
const CONDITIONAL_KEYWORDS: &[&str] = &[
    "items",
    "prefixItems",
    "additionalProperties",
    "patternProperties",
    "oneOf",
    "anyOf",
    "allOf",
    "not",
    "if",
    "then",
    "else",
    "$ref",
    "$defs",
    "definitions",
    "contains",
    "propertyNames",
];

/// Collect and validate every `x-mcp-header` binding in one input schema.
///
/// # Errors
///
/// [`AnnotationError`] for the first violation found. Everything is checked:
/// an annotation in an unreachable position is an error, not a silent
/// omission, because silently ignoring it would publish a tool whose author
/// believes a header contract exists when none does.
pub fn header_bindings(input_schema: &Value) -> Result<Vec<HeaderBinding>, AnnotationError> {
    let mut bindings = Vec::new();
    collect_reachable(input_schema, &mut Vec::new(), &mut bindings)?;
    reject_annotations_out_of_reach(input_schema, &mut Vec::new())?;

    let mut seen: BTreeSet<String> = BTreeSet::new();
    for binding in &bindings {
        if !seen.insert(binding.header_name.to_ascii_lowercase()) {
            return Err(AnnotationError::Duplicate {
                name: binding.header_name.clone(),
            });
        }
    }
    Ok(bindings)
}

/// Walk the `properties` spine, validating every annotation on it.
fn collect_reachable(
    schema: &Value,
    path: &mut Vec<String>,
    bindings: &mut Vec<HeaderBinding>,
) -> Result<(), AnnotationError> {
    if let Some(annotation) = schema.get("x-mcp-header") {
        let location = render_path(path);
        let name = annotation
            .as_str()
            .filter(|name| !name.is_empty())
            .ok_or_else(|| AnnotationError::NotAName {
                path: location.clone(),
            })?;
        if !is_field_token(name) {
            return Err(AnnotationError::NotAToken {
                name: name.to_owned(),
                path: location,
            });
        }
        let declared = schema
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("<none>");
        if !PERMITTED_TYPES.contains(&declared) {
            return Err(AnnotationError::NotPrimitive {
                name: name.to_owned(),
                path: location,
                declared: declared.to_owned(),
            });
        }
        bindings.push(HeaderBinding {
            header_name: name.to_owned(),
            path: path.clone(),
        });
    }
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return Ok(());
    };
    for (name, subschema) in properties {
        path.push(name.clone());
        collect_reachable(subschema, path, bindings)?;
        drop(path.pop());
    }
    Ok(())
}

/// Find an annotation hiding behind any conditional keyword and refuse it.
fn reject_annotations_out_of_reach(
    schema: &Value,
    path: &mut Vec<String>,
) -> Result<(), AnnotationError> {
    for keyword in CONDITIONAL_KEYWORDS {
        let Some(branch) = schema.get(*keyword) else {
            continue;
        };
        if let Some(name) = first_annotation(branch) {
            path.push((*keyword).to_owned());
            let location = render_path(path);
            drop(path.pop());
            return Err(AnnotationError::Unreachable {
                name,
                path: location,
            });
        }
    }
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return Ok(());
    };
    for (name, subschema) in properties {
        path.push(name.clone());
        reject_annotations_out_of_reach(subschema, path)?;
        drop(path.pop());
    }
    Ok(())
}

/// The first `x-mcp-header` name anywhere inside a JSON value.
fn first_annotation(value: &Value) -> Option<String> {
    match value {
        Value::Object(object) => {
            if let Some(name) = object.get("x-mcp-header").and_then(Value::as_str) {
                return Some(name.to_owned());
            }
            object.values().find_map(first_annotation)
        }
        Value::Array(values) => values.iter().find_map(first_annotation),
        _ => None,
    }
}

fn render_path(path: &[String]) -> String {
    if path.is_empty() {
        "<root>".to_owned()
    } else {
        format!("/{}", path.join("/"))
    }
}

/// RFC 9110 §5.1 field-name token: one or more `tchar`.
fn is_field_token(name: &str) -> bool {
    !name.is_empty()
        && name.chars().all(|character| {
            character.is_ascii_alphanumeric() || "!#$%&'*+-.^_`|~".contains(character)
        })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{AnnotationError, header_bindings, is_field_token};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn a_schema_with_no_annotations_yields_none() -> TestResult {
        let bindings = header_bindings(&json!({
            "type": "object",
            "properties": { "namespace": { "type": "string" } },
        }))?;
        assert!(bindings.is_empty());
        Ok(())
    }

    #[test]
    fn a_reachable_annotation_is_collected_with_its_property_path() -> TestResult {
        let bindings = header_bindings(&json!({
            "type": "object",
            "properties": {
                "target": {
                    "type": "object",
                    "properties": {
                        "shard": { "type": "integer", "x-mcp-header": "Shard" },
                    },
                },
            },
        }))?;
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].header_name, "Shard");
        assert_eq!(bindings[0].path, vec!["target", "shard"]);
        assert_eq!(bindings[0].full_header(), "mcp-param-shard");
        assert_eq!(
            bindings[0].resolve(&json!({ "target": { "shard": 4 } })),
            Some(&json!(4))
        );
        assert_eq!(bindings[0].resolve(&json!({ "target": {} })), None);
        Ok(())
    }

    #[test]
    fn a_number_typed_annotation_is_refused() {
        let outcome = header_bindings(&json!({
            "type": "object",
            "properties": { "ratio": { "type": "number", "x-mcp-header": "Ratio" } },
        }));
        assert!(matches!(
            outcome,
            Err(AnnotationError::NotPrimitive { declared, .. }) if declared == "number"
        ));
    }

    #[test]
    fn a_case_insensitive_duplicate_is_refused() {
        let outcome = header_bindings(&json!({
            "type": "object",
            "properties": {
                "a": { "type": "string", "x-mcp-header": "Shard" },
                "b": { "type": "string", "x-mcp-header": "shard" },
            },
        }));
        assert!(matches!(outcome, Err(AnnotationError::Duplicate { .. })));
    }

    /// Each of these positions makes the property's presence conditional on the
    /// instance, so a proxy cannot know whether the header contract applies.
    #[test]
    fn an_annotation_behind_any_conditional_keyword_is_refused() {
        for keyword in ["items", "oneOf", "anyOf", "allOf", "not", "if", "$defs"] {
            let schema = json!({
                "type": "object",
                "properties": {
                    "field": {
                        keyword: { "type": "string", "x-mcp-header": "Sneaky" },
                    },
                },
            });
            assert!(
                matches!(
                    header_bindings(&schema),
                    Err(AnnotationError::Unreachable { .. })
                ),
                "an annotation under `{keyword}` must invalidate the tool definition"
            );
        }
    }

    #[test]
    fn a_name_that_is_not_a_field_token_is_refused() {
        for name in ["has space", "has\r\n", "sémantique", ""] {
            let schema = json!({
                "type": "object",
                "properties": { "a": { "type": "string", "x-mcp-header": name } },
            });
            assert!(
                matches!(
                    header_bindings(&schema),
                    Err(AnnotationError::NotAToken { .. } | AnnotationError::NotAName { .. })
                ),
                "`{name}` is not a legal header field name"
            );
        }
        assert!(is_field_token("Shard-Id"));
        assert!(!is_field_token("Shard Id"));
    }
}
