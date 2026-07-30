//! Canonical JSON text encoding for emitted artifacts.
//!
//! Emitted artifacts — the AWL sidecars, the `schema of <Type>` text frozen
//! into generated Gleam, `manifest.json`, and `contract.json` — must be
//! byte-reproducible from the same model. `serde_json::Map` is not a
//! reproducible ordering: with the `preserve_order` feature it is an `IndexMap`
//! that iterates in insertion order, and without it a `BTreeMap` that iterates
//! key-sorted. Because Cargo unifies features across a whole build graph, an
//! unrelated dependency enabling `preserve_order` silently changes the bytes of
//! every artifact serialized through a `Map`.
//!
//! This module removes that coupling. [`CanonicalJson`] and [`serialize_value`]
//! sort each object's entries explicitly at every depth and drive
//! `Serializer::serialize_map` from that sorted vector, so the emitted bytes
//! never observe `Map`'s own iteration order. The rule is the same one
//! [`crate::contract::PackageContract::canonical_bytes`] already applies to
//! package identity: JSON object keys are sorted, at every depth.
//!
//! Array order is content, not layout, and is preserved verbatim. Only object
//! key order — which JSON does not define — is normalised.

use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Serialize, Serializer};
use serde_json::{Map, Value};

/// A JSON document that can only serialize canonically.
///
/// Constructing one is the way an emitter hands a JSON document to a caller
/// without also handing over the chance to write non-reproducible bytes: every
/// `serde_json::to_vec`/`to_string`/`to_writer` call on a `CanonicalJson`
/// produces object keys in sorted order at every depth, whatever `Map`
/// representation `serde_json` was compiled with.
#[derive(Clone, Debug, PartialEq)]
pub struct CanonicalJson(Value);

impl CanonicalJson {
    /// Wraps `value` so that serializing it emits canonical bytes.
    #[must_use]
    pub const fn new(value: Value) -> Self {
        Self(value)
    }

    /// Borrows the underlying document, for structural inspection.
    ///
    /// The borrow is read-only on purpose: canonicality is a property of the
    /// serialization, so nothing here can be edited into a non-canonical state.
    #[must_use]
    pub const fn as_value(&self) -> &Value {
        &self.0
    }
}

impl Serialize for CanonicalJson {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_value(&self.0, serializer)
    }
}

/// A JSON object's entries in key order.
///
/// Use this wherever a `serde_json` object is *walked* to build something
/// order-sensitive — a record's field list, a decoder's field sequence, a
/// rendered document — rather than merely looked up by key. `Map`'s own
/// iteration order is the authored insertion order with the `preserve_order`
/// feature resolved anywhere in the build graph and key-sorted without it, so
/// it can never be the source of a reproducible order.
#[must_use]
pub fn sorted_entries(object: &Map<String, Value>) -> Vec<(&String, &Value)> {
    let mut sorted = object.iter().collect::<Vec<_>>();
    sorted.sort_unstable_by_key(|(key, _)| *key);
    sorted
}

/// Serializes `value` with every JSON object's keys in sorted order, at every
/// depth.
///
/// Usable directly as a `#[serde(serialize_with = "…")]` target for a
/// [`Value`] field of a derived `Serialize` type, which keeps that type's own
/// field order the declared contract it already is while making its embedded
/// JSON documents reproducible.
///
/// # Errors
///
/// Propagates whatever failure `serializer` reports.
pub fn serialize_value<S>(value: &Value, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match value {
        Value::Null => serializer.serialize_unit(),
        Value::Bool(value) => serializer.serialize_bool(*value),
        Value::Number(number) => number.serialize(serializer),
        Value::String(text) => serializer.serialize_str(text),
        Value::Array(items) => {
            let mut sequence = serializer.serialize_seq(Some(items.len()))?;
            for item in items {
                sequence.serialize_element(&Borrowed(item))?;
            }
            sequence.end()
        }
        Value::Object(entries) => {
            // Sorted here rather than iterated in place: `Map`'s own iteration
            // order is exactly what must not reach the output.
            let sorted = sorted_entries(entries);
            let mut map = serializer.serialize_map(Some(sorted.len()))?;
            for (key, value) in sorted {
                map.serialize_entry(key, &Borrowed(value))?;
            }
            map.end()
        }
    }
}

/// Borrowed canonical view of a nested value, so the recursion never clones.
struct Borrowed<'value>(&'value Value);

impl Serialize for Borrowed<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_value(self.0, serializer)
    }
}

#[cfg(test)]
mod tests {
    use serde::Serialize;
    use serde_json::{Map, Value, json};

    use super::{CanonicalJson, serialize_value, sorted_entries};

    /// The same three-key object built in two different insertion orders. Under
    /// `preserve_order` these two objects serialize to different bytes through
    /// plain `serde_json`; they must not through the canonical encoder.
    fn two_insertion_orders() -> (Map<String, Value>, Map<String, Value>) {
        let mut forward = Map::new();
        forward.insert("alpha".to_owned(), json!(1));
        forward.insert("beta".to_owned(), json!(2));
        forward.insert("gamma".to_owned(), json!(3));

        let mut reversed = Map::new();
        reversed.insert("gamma".to_owned(), json!(3));
        reversed.insert("beta".to_owned(), json!(2));
        reversed.insert("alpha".to_owned(), json!(1));

        (forward, reversed)
    }

    #[test]
    fn canonical_bytes_are_independent_of_insertion_order() -> Result<(), serde_json::Error> {
        let (forward, reversed) = two_insertion_orders();
        assert_eq!(
            serde_json::to_string(&CanonicalJson::new(Value::Object(forward)))?,
            serde_json::to_string(&CanonicalJson::new(Value::Object(reversed)))?
        );
        Ok(())
    }

    #[test]
    fn canonical_bytes_are_key_sorted_at_every_depth() -> Result<(), serde_json::Error> {
        let mut outer = Map::new();
        outer.insert("zulu".to_owned(), json!({ "yankee": 1, "xray": 2 }));
        outer.insert(
            "alpha".to_owned(),
            json!([{ "delta": 1, "charlie": 2 }, { "bravo": 3 }]),
        );
        let document = CanonicalJson::new(Value::Object(outer));

        // Every object key sorted; array element order untouched.
        assert_eq!(
            serde_json::to_string(&document)?,
            r#"{"alpha":[{"charlie":2,"delta":1},{"bravo":3}],"zulu":{"xray":2,"yankee":1}}"#
        );
        Ok(())
    }

    #[test]
    fn canonical_encoding_matches_a_key_sorted_expectation() -> Result<(), serde_json::Error> {
        // The expectation is written in sorted order by hand, so it holds with
        // `serde_json/preserve_order` on and off alike.
        let (_, reversed) = two_insertion_orders();
        assert_eq!(
            serde_json::to_string_pretty(&CanonicalJson::new(Value::Object(reversed)))?,
            "{\n  \"alpha\": 1,\n  \"beta\": 2,\n  \"gamma\": 3\n}"
        );
        Ok(())
    }

    #[test]
    fn scalars_arrays_and_nulls_round_trip_unchanged() -> Result<(), serde_json::Error> {
        let value = json!({
            "flag": true,
            "count": -7,
            "ratio": 1.5,
            "absent": Value::Null,
            "order": ["c", "a", "b"],
            "text": "verbatim"
        });
        let encoded = serde_json::to_string(&CanonicalJson::new(value.clone()))?;
        let decoded: Value = serde_json::from_str(&encoded)?;
        assert_eq!(decoded, value, "canonical encoding must be lossless");
        assert!(
            encoded.contains(r#""order":["c","a","b"]"#),
            "array order is content and must survive verbatim: {encoded}"
        );
        Ok(())
    }

    #[test]
    fn serialize_value_canonicalises_a_field_of_a_derived_type() -> Result<(), serde_json::Error> {
        #[derive(Serialize)]
        struct Record {
            // Declared field order is the contract for the record itself…
            name: String,
            // …while the embedded document is key-sorted at every depth.
            #[serde(serialize_with = "serialize_value")]
            schema: Value,
        }

        let (forward, reversed) = two_insertion_orders();
        let first = Record {
            name: "probe".to_owned(),
            schema: Value::Object(forward),
        };
        let second = Record {
            name: "probe".to_owned(),
            schema: Value::Object(reversed),
        };
        assert_eq!(
            serde_json::to_string(&first)?,
            r#"{"name":"probe","schema":{"alpha":1,"beta":2,"gamma":3}}"#
        );
        assert_eq!(
            serde_json::to_string(&first)?,
            serde_json::to_string(&second)?
        );
        Ok(())
    }

    #[test]
    fn sorted_entries_is_key_order_whatever_the_insertion_order() {
        let (forward, reversed) = two_insertion_orders();
        let keys = |object| {
            sorted_entries(object)
                .into_iter()
                .map(|(key, _)| key.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(keys(&forward), vec!["alpha", "beta", "gamma"]);
        assert_eq!(keys(&reversed), keys(&forward));
    }

    #[test]
    fn as_value_exposes_the_wrapped_document() {
        let document = CanonicalJson::new(json!({ "beta": 1, "alpha": 2 }));
        assert_eq!(document.as_value()["alpha"], json!(2));
    }
}
