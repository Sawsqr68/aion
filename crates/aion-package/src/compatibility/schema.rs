//! Directional JSON Schema normalization and structural subset checks.

use std::collections::BTreeSet;

use serde_json::Value;

use super::schemas_equal;

/// Expands local `$ref`s and removes non-validating presentation annotations.
pub(super) fn normalize_schema(root: &Value) -> Value {
    normalize_node(root, root, &mut BTreeSet::new())
}

fn normalize_node(value: &Value, root: &Value, resolving: &mut BTreeSet<String>) -> Value {
    match value {
        Value::Object(object) => {
            if let Some(reference) = object.get("$ref").and_then(Value::as_str)
                && reference.starts_with('#')
            {
                if !resolving.insert(reference.to_owned()) {
                    return serde_json::json!({ "$ref": "#recursive" });
                }
                let target = if reference == "#" {
                    Some(root)
                } else {
                    root.pointer(&reference[1..])
                };
                if let Some(target) = target {
                    let mut expanded = normalize_node(target, root, resolving);
                    resolving.remove(reference);
                    if let Value::Object(expanded) = &mut expanded {
                        for (key, nested) in object {
                            if key != "$ref" && key != "$defs" && !is_annotation(key) {
                                expanded
                                    .insert(key.clone(), normalize_node(nested, root, resolving));
                            }
                        }
                    }
                    return expanded;
                }
                resolving.remove(reference);
            }
            Value::Object(
                object
                    .iter()
                    .filter(|(key, _)| key.as_str() != "$defs" && !is_annotation(key))
                    .map(|(key, nested)| (key.clone(), normalize_node(nested, root, resolving)))
                    .collect(),
            )
        }
        Value::Array(values) => Value::Array(
            values
                .iter()
                .map(|nested| normalize_node(nested, root, resolving))
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn is_annotation(key: &str) -> bool {
    matches!(
        key,
        "$schema"
            | "$id"
            | "$anchor"
            | "title"
            | "description"
            | "default"
            | "examples"
            | "deprecated"
            | "readOnly"
            | "writeOnly"
    )
}

/// Conservative subset check for the structural draft-2020-12 fragment emitted
/// by AWL and schemars. Unknown requirements on `sup` must appear identically on
/// `sub`; extra requirements on `sub` only narrow its value set and are safe.
pub(super) fn schema_is_subset(sub: &Value, sup: &Value) -> bool {
    if schemas_equal(sub, sup, None) || matches!(sup, Value::Bool(true)) {
        return true;
    }
    if matches!(sub, Value::Bool(false)) {
        return true;
    }
    let (Value::Object(sub), Value::Object(sup)) = (sub, sup) else {
        return false;
    };

    if !union_is_subset(sub, sup)
        || !type_is_subset(sub.get("type"), sup.get("type"))
        || !values_are_subset(
            sub.get("const"),
            sub.get("enum"),
            sup.get("const"),
            sup.get("enum"),
        )
    {
        return false;
    }
    for key in [
        "minimum",
        "exclusiveMinimum",
        "minLength",
        "minItems",
        "minProperties",
    ] {
        if !ordered_lower_bound(sub, sup, key) {
            return false;
        }
    }
    for key in [
        "maximum",
        "exclusiveMaximum",
        "maxLength",
        "maxItems",
        "maxProperties",
    ] {
        if !ordered_upper_bound(sub, sup, key) {
            return false;
        }
    }
    if !required_constraints_are_subset(sub, sup)
        || !object_properties_are_subset(sub, sup)
        || !items_are_subset(sub, sup)
    {
        return false;
    }
    for key in ["pattern", "format", "multipleOf", "not", "propertyNames"] {
        if let Some(required) = sup.get(key)
            && sub.get(key) != Some(required)
        {
            return false;
        }
    }
    if sup.get("uniqueItems") == Some(&Value::Bool(true))
        && sub.get("uniqueItems") != Some(&Value::Bool(true))
    {
        return false;
    }
    sup.iter().all(|(key, value)| {
        is_handled_constraint(key) || sub.get(key).is_some_and(|actual| actual == value)
    })
}

fn is_handled_constraint(key: &str) -> bool {
    matches!(
        key,
        "type"
            | "const"
            | "enum"
            | "anyOf"
            | "oneOf"
            | "minimum"
            | "exclusiveMinimum"
            | "maximum"
            | "exclusiveMaximum"
            | "minLength"
            | "maxLength"
            | "minItems"
            | "maxItems"
            | "minProperties"
            | "maxProperties"
            | "required"
            | "properties"
            | "additionalProperties"
            | "items"
            | "pattern"
            | "format"
            | "multipleOf"
            | "not"
            | "propertyNames"
            | "uniqueItems"
    )
}

fn union_is_subset(
    sub: &serde_json::Map<String, Value>,
    sup: &serde_json::Map<String, Value>,
) -> bool {
    let candidate_union = sub.get("anyOf").or_else(|| sub.get("oneOf"));
    let requirement_union = sup.get("anyOf").or_else(|| sup.get("oneOf"));
    match (
        candidate_union.and_then(Value::as_array),
        requirement_union.and_then(Value::as_array),
    ) {
        (None, None) => true,
        (Some(sub), Some(sup)) => sub
            .iter()
            .all(|branch| sup.iter().any(|allowed| schema_is_subset(branch, allowed))),
        (Some(sub), None) => {
            let sup = Value::Object(sup.clone());
            sub.iter().all(|branch| schema_is_subset(branch, &sup))
        }
        (None, Some(sup)) => {
            let sub = Value::Object(sub.clone());
            sup.iter().any(|allowed| schema_is_subset(&sub, allowed))
        }
    }
}

fn type_is_subset(sub: Option<&Value>, sup: Option<&Value>) -> bool {
    let Some(sup) = sup else { return true };
    let Some(sub) = sub else { return false };
    let sub = type_names(sub);
    let sup = type_names(sup);
    sub.iter()
        .all(|actual| sup.contains(actual) || (*actual == "integer" && sup.contains("number")))
}

fn type_names(value: &Value) -> BTreeSet<&str> {
    match value {
        Value::String(value) => BTreeSet::from([value.as_str()]),
        Value::Array(values) => values.iter().filter_map(Value::as_str).collect(),
        _ => BTreeSet::new(),
    }
}

fn values_are_subset(
    candidate_const: Option<&Value>,
    candidate_enum: Option<&Value>,
    requirement_const: Option<&Value>,
    requirement_enum: Option<&Value>,
) -> bool {
    match (
        constrained_values(candidate_const, candidate_enum),
        constrained_values(requirement_const, requirement_enum),
    ) {
        (_, None) => true,
        (Some(sub), Some(sup)) => sub.iter().all(|value| sup.contains(value)),
        (None, Some(_)) => false,
    }
}

fn constrained_values<'a>(
    constant: Option<&'a Value>,
    enumeration: Option<&'a Value>,
) -> Option<Vec<&'a Value>> {
    constant.map(|value| vec![value]).or_else(|| {
        enumeration
            .and_then(Value::as_array)
            .map(|values| values.iter().collect())
    })
}

fn ordered_lower_bound(
    sub: &serde_json::Map<String, Value>,
    sup: &serde_json::Map<String, Value>,
    key: &str,
) -> bool {
    match (
        sub.get(key).and_then(Value::as_f64),
        sup.get(key).and_then(Value::as_f64),
    ) {
        (_, None) => true,
        (Some(sub), Some(sup)) => sub >= sup,
        (None, Some(_)) => false,
    }
}

fn ordered_upper_bound(
    sub: &serde_json::Map<String, Value>,
    sup: &serde_json::Map<String, Value>,
    key: &str,
) -> bool {
    match (
        sub.get(key).and_then(Value::as_f64),
        sup.get(key).and_then(Value::as_f64),
    ) {
        (_, None) => true,
        (Some(sub), Some(sup)) => sub <= sup,
        (None, Some(_)) => false,
    }
}

fn required_constraints_are_subset(
    sub: &serde_json::Map<String, Value>,
    sup: &serde_json::Map<String, Value>,
) -> bool {
    let sub_required = string_set(sub.get("required"));
    string_set(sup.get("required")).is_subset(&sub_required)
}

fn string_set(value: Option<&Value>) -> BTreeSet<&str> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect()
}

fn object_properties_are_subset(
    sub: &serde_json::Map<String, Value>,
    sup: &serde_json::Map<String, Value>,
) -> bool {
    let candidate_properties = sub.get("properties").and_then(Value::as_object);
    let required_properties = sup.get("properties").and_then(Value::as_object);
    let Some(candidate_properties) = candidate_properties else {
        return string_set(sup.get("required")).is_empty();
    };
    for (name, schema) in candidate_properties {
        if let Some(expected) = required_properties.and_then(|properties| properties.get(name)) {
            if !schema_is_subset(schema, expected) {
                return false;
            }
        } else if let Some(additional) = sup.get("additionalProperties") {
            match additional {
                Value::Bool(false) => return false,
                Value::Bool(true) => {}
                expected if !schema_is_subset(schema, expected) => return false,
                _ => {}
            }
        }
    }
    if let Some(additional) = sub.get("additionalProperties")
        && additional != &Value::Bool(false)
    {
        match sup.get("additionalProperties") {
            Some(Value::Bool(false)) => return false,
            Some(Value::Bool(true)) | None => {}
            Some(expected) if !schema_is_subset(additional, expected) => return false,
            Some(_) => {}
        }
    }
    true
}

fn items_are_subset(
    sub: &serde_json::Map<String, Value>,
    sup: &serde_json::Map<String, Value>,
) -> bool {
    match (sub.get("items"), sup.get("items")) {
        (_, None) => true,
        (Some(sub), Some(sup)) => schema_is_subset(sub, sup),
        (None, Some(_)) => false,
    }
}
