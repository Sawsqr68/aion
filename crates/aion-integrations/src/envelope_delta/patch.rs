//! The structural JSON diff and its inverse.
//!
//! Both directions are pure functions over `serde_json` values with no knowledge of provider
//! envelopes, so they can be reasoned about — and tested — on their own terms. The encoder is what
//! decides a diff is safe to persist, by round-tripping it through [`apply`] and comparing bytes;
//! nothing here assumes it.

use serde_json::{Map, Value};

/// Why a patch could not be applied to the value it was handed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PatchError {
    /// The base is not a JSON object, so it has no fields a patch can address.
    #[error("patch base is not a JSON object")]
    BaseNotAnObject,
    /// An `unset` entry is not a well-formed RFC 6901 JSON Pointer.
    #[error("unset pointer {pointer:?} is not a well-formed JSON Pointer")]
    PointerMalformed {
        /// The offending pointer, verbatim.
        pointer: String,
    },
    /// An `unset` pointer names a path the base does not contain, which means the patch was built
    /// against a different base than the one supplied.
    #[error("unset pointer {pointer:?} does not resolve in the base")]
    PointerMissing {
        /// The offending pointer, verbatim.
        pointer: String,
    },
}

/// The two halves of a structural diff: what to set, and what to remove.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Patch {
    /// Nested changed values, mirroring the target's shape.
    pub set: Map<String, Value>,
    /// JSON Pointers to paths the target no longer carries.
    pub unset: Vec<String>,
}

/// Diffs `next` against `base`, both JSON objects.
///
/// Recurses only where *both* sides hold an object, so a field that changed type is carried whole
/// and never merged into a value of a different shape. Arrays are leaves by design (see
/// [`EnvelopeDelta::set`](super::EnvelopeDelta::set)).
#[must_use]
pub fn diff(base: &Map<String, Value>, next: &Map<String, Value>) -> Patch {
    let mut unset = Vec::new();
    let set = diff_objects(base, next, "", &mut unset);
    Patch { set, unset }
}

/// The recursive worker behind [`diff`]; `prefix` is the JSON Pointer of the object being walked.
fn diff_objects(
    base: &Map<String, Value>,
    next: &Map<String, Value>,
    prefix: &str,
    unset: &mut Vec<String>,
) -> Map<String, Value> {
    let mut set = Map::new();
    for (key, next_value) in next {
        match (base.get(key), next_value) {
            // Unchanged: carried by the base, so the delta says nothing about it at all. This is
            // the arm that drops the repeated instructions block.
            (Some(base_value), _) if base_value == next_value => {}
            (Some(Value::Object(base_object)), Value::Object(next_object)) => {
                let nested =
                    diff_objects(base_object, next_object, &child_pointer(prefix, key), unset);
                if !nested.is_empty() {
                    set.insert(key.clone(), Value::Object(nested));
                }
            }
            _ => {
                set.insert(key.clone(), next_value.clone());
            }
        }
    }
    for key in base.keys().filter(|key| !next.contains_key(key.as_str())) {
        unset.push(child_pointer(prefix, key));
    }
    set
}

/// Applies a patch to the base it was diffed from, yielding the value that was diffed.
///
/// Removals run before merges so a path that was removed and a differently-shaped value written
/// at the same path cannot interfere.
///
/// # Errors
///
/// Returns [`PatchError`] when `base` is not an object, or when an `unset` pointer is malformed or
/// does not resolve — both of which mean the patch does not belong to this base. The caller's
/// correct response is to refuse to reconstruct, never to return a partially-patched value.
pub fn apply(base: &Value, patch: &Patch) -> Result<Value, PatchError> {
    let Value::Object(base_object) = base else {
        return Err(PatchError::BaseNotAnObject);
    };
    let mut out = base_object.clone();
    for pointer in &patch.unset {
        remove_pointer(&mut out, pointer)?;
    }
    merge(&mut out, &patch.set);
    Ok(Value::Object(out))
}

/// Merges `set` into `target`, recursing where both sides hold an object.
fn merge(target: &mut Map<String, Value>, set: &Map<String, Value>) {
    for (key, value) in set {
        match (target.get_mut(key), value) {
            (Some(Value::Object(target_object)), Value::Object(nested)) => {
                merge(target_object, nested);
            }
            _ => {
                target.insert(key.clone(), value.clone());
            }
        }
    }
}

/// Removes the single path a pointer names, erroring rather than tolerating a miss.
fn remove_pointer(root: &mut Map<String, Value>, pointer: &str) -> Result<(), PatchError> {
    let Some(body) = pointer.strip_prefix('/') else {
        return Err(PatchError::PointerMalformed {
            pointer: pointer.to_owned(),
        });
    };
    let tokens: Vec<String> = body.split('/').map(unescape_token).collect();
    let Some((leaf, parents)) = tokens.split_last() else {
        return Err(PatchError::PointerMalformed {
            pointer: pointer.to_owned(),
        });
    };
    let mut cursor = root;
    for parent in parents {
        match cursor.get_mut(parent) {
            Some(Value::Object(next)) => cursor = next,
            _ => {
                return Err(PatchError::PointerMissing {
                    pointer: pointer.to_owned(),
                });
            }
        }
    }
    if cursor.remove(leaf).is_none() {
        return Err(PatchError::PointerMissing {
            pointer: pointer.to_owned(),
        });
    }
    Ok(())
}

/// The JSON Pointer of `key` inside the object at `prefix`.
fn child_pointer(prefix: &str, key: &str) -> String {
    format!("{prefix}/{}", escape_token(key))
}

/// RFC 6901 reference-token escaping: `~` then `/`, in that order.
fn escape_token(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

/// RFC 6901 reference-token unescaping: `~1` then `~0`, in that order, which is the exact inverse
/// of [`escape_token`] (doing it the other way round would turn an escaped `~1` into a `/`).
fn unescape_token(token: &str) -> String {
    token.replace("~1", "/").replace("~0", "~")
}
