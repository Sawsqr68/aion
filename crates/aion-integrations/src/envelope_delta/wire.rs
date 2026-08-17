//! The persisted delta document and the keys that name it on the wire.
//!
//! A compacted frame's [`ActivityEventKind::Raw`] value is an object with exactly one key,
//! [`ENVELOPE_DELTA_KEY`], whose value is an [`EnvelopeDelta`]. Requiring the delta to be the
//! *sole* key is what makes detection unambiguous: a provider envelope always carries several
//! top-level fields, so no genuine provider frame can be mistaken for a delta document.
//!
//! [`ActivityEventKind::Raw`]: aion_core::ActivityEventKind::Raw

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

/// The sole top-level key of a compacted frame's raw value.
///
/// Namespaced to Aion so it cannot collide with a provider field, and checked as the *only* key
/// so a provider frame that happened to carry it alongside its own fields is still treated as a
/// verbatim envelope rather than a delta.
pub const ENVELOPE_DELTA_KEY: &str = "aion_envelope_delta";

/// The delta-document format version this crate writes and reads.
///
/// A reader that meets a higher version reports
/// [`UnresolvedReason::UnsupportedVersion`](super::UnresolvedReason::UnsupportedVersion) rather
/// than guessing at a shape it does not implement.
pub const ENVELOPE_DELTA_VERSION: u64 = 1;

/// One persisted frame expressed as the difference from its turn's base envelope.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct EnvelopeDelta {
    /// The document format version; see [`ENVELOPE_DELTA_VERSION`].
    #[serde(rename = "v")]
    pub version: u64,
    /// The provider's response identifier, which is also the turn identity the base was recorded
    /// under. Carried so a reader that cannot resolve the delta can still say *which* turn it
    /// belongs to.
    pub base: String,
    /// The `worker_seq` of the event carrying the base envelope, so an unresolvable delta names a
    /// concrete missing record rather than an abstraction.
    pub base_worker_seq: u64,
    /// Lowercase hex SHA-256 over `serde_json::to_vec` of the base envelope value.
    ///
    /// The decoder recomputes this over the base it holds and refuses to reconstruct on a
    /// mismatch. This is what stops a base that was truncated (by the server's per-event size
    /// ceiling) or otherwise altered after the encoder read it from silently producing a wrong
    /// envelope.
    pub base_digest: String,
    /// Every field whose value differs from the base, nested to mirror the envelope's own shape.
    ///
    /// A leaf here replaces the base's value at that path wholesale; an object here is merged
    /// into the base's object at that path. Arrays are always leaves — an array that changed at
    /// all is carried whole, because a positional array diff buys little on these payloads and
    /// costs a great deal of ambiguity.
    pub set: Map<String, Value>,
    /// RFC 6901 JSON Pointers naming paths present in the base and absent from this frame.
    ///
    /// Removals are explicit rather than encoded as a `null` value, so a field genuinely set to
    /// `null` (which these envelopes do carry — an unfinished turn's `usage`, for one) round-trips
    /// as `null` instead of vanishing.
    pub unset: Vec<String>,
}

/// The lowercase hex SHA-256 of a JSON value's serialized bytes.
///
/// Serialization of a `serde_json::Value` cannot fail for any value that was itself parsed from
/// or built as JSON, but the fallible API is honoured rather than unwrapped: an unserializable
/// value yields `None`, and both the encoder and decoder treat that as "not compactible" /
/// "not resolvable" rather than proceeding on a digest they could not compute.
#[must_use]
pub fn value_digest(value: &Value) -> Option<String> {
    let bytes = serde_json::to_vec(value).ok()?;
    let mut digest = Sha256::new();
    digest.update(&bytes);
    Some(hex_lower(&digest.finalize()))
}

/// Renders bytes as lowercase hex.
fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        // Two nibbles, most significant first; `from_digit` is total for radix 16 inputs, and the
        // `None` arm is unreachable for a 4-bit value but is still handled rather than unwrapped.
        for nibble in [byte >> 4, byte & 0x0f] {
            match char::from_digit(u32::from(nibble), 16) {
                Some(digit) => out.push(digit),
                None => out.push('?'),
            }
        }
    }
    out
}

/// Reads the turn identity a provider frame belongs to: its `response.id`.
///
/// A frame without a string `response.id` is not part of a recognisable turn and is left entirely
/// alone by both the encoder and the decoder — it is neither recorded as a base nor compacted.
#[must_use]
pub fn envelope_response_id(value: &Value) -> Option<&str> {
    value.get("response")?.get("id")?.as_str()
}

/// Reads a delta document out of a raw value, or `None` when the value is not one.
///
/// Returns the delta's JSON so the caller can report a malformed document honestly instead of
/// discarding it: a value whose sole key is [`ENVELOPE_DELTA_KEY`] *is* a delta document, whether
/// or not it parses.
#[must_use]
pub fn delta_document(value: &Value) -> Option<&Value> {
    let object = value.as_object()?;
    if object.len() != 1 {
        return None;
    }
    object.get(ENVELOPE_DELTA_KEY)
}
