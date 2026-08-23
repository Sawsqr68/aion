//! One physical line per stage, in canonical authored form (design §2.1).
//!
//! > Canonical authored form is one physical line per stage, keys in the
//! > displayed order, ASCII identifiers, no quoting, and no spaces inside
//! > comma-separated values. `-` means no predecessor.
//!
//! Every one of those five constraints is enforced here, each with its own
//! cause code, because a line that is not canonical has no parsed stage for
//! the §5.2 clauses to validate.

use crate::cause::CauseCode;
use crate::error::PlanRefusal;

/// The canonical seven keys, in the displayed order. The order IS the
/// contract: §2.1 says "keys in the displayed order".
pub const CANONICAL_KEYS: [&str; 7] = [
    "id",
    "legs",
    "after",
    "admit",
    "conclusions",
    "conclusion_leg",
    "advance",
];

/// One stage line, split into its raw values but not yet validated against
/// the graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RawStage {
    /// `id=`
    pub id: String,
    /// `legs=` split on `,`
    pub legs: Vec<String>,
    /// `after=`, with `-` rendered as `None`
    pub after: Option<String>,
    /// `admit=`, still unparsed
    pub admit: String,
    /// `conclusions=` split on `,`
    pub conclusions: Vec<String>,
    /// `conclusion_leg=`
    pub conclusion_leg: String,
    /// `advance=`, still unvalidated
    pub advance: String,
}

/// Parse one canonical stage line.
///
/// # Errors
///
/// Refuses with the §2.1 canonical-form cause codes: a line that does not
/// open with `stage`, a wrong key count, an unknown key, keys out of order,
/// a duplicated key, a quoted value, or an empty value or CSV element.
pub fn parse_stage_line(line: &str) -> Result<RawStage, PlanRefusal> {
    let line = line.trim();
    let values = split_canonical_pairs(line)?;

    let get = |wanted: &str| -> &str {
        values
            .iter()
            .find(|(k, _)| *k == wanted)
            .map_or("", |(_, v)| *v)
    };

    for (key, value) in &values {
        if is_quoted(value) {
            return Err(PlanRefusal::whole(
                CauseCode::QuotedValue,
                format!("§2.1 admits no quoting; key {key:?} carries a quoted value {value:?}"),
            ));
        }
    }

    let after_raw = get("after");
    let after = if after_raw == "-" {
        None
    } else if after_raw.is_empty() {
        return Err(PlanRefusal::whole(
            CauseCode::EmptyValue,
            "`after=` is empty; §2.1 spells no predecessor as `-`",
        ));
    } else {
        Some(after_raw.to_owned())
    };

    Ok(RawStage {
        id: get("id").to_owned(),
        legs: split_csv(get("legs"), "legs")?,
        after,
        admit: nonempty(get("admit"), "admit")?,
        conclusions: split_csv(get("conclusions"), "conclusions")?,
        conclusion_leg: nonempty(get("conclusion_leg"), "conclusion_leg")?,
        advance: nonempty(get("advance"), "advance")?,
    })
}

/// Split the line into its canonical seven `key=value` pairs, enforcing
/// §2.1's canonical authored form: the word `stage`, single-space
/// separation, the canonical key set, no duplicate key, no missing key, and
/// the displayed key ORDER.
fn split_canonical_pairs(line: &str) -> Result<Vec<(&str, &str)>, PlanRefusal> {
    let mut fields = line.split(' ');
    if fields.next() != Some("stage") {
        return Err(PlanRefusal::whole(
            CauseCode::MalformedStageLine,
            format!("a stage line opens with the word `stage`: {line:?}"),
        ));
    }

    let mut seen: Vec<&str> = Vec::new();
    let mut values: Vec<(&str, &str)> = Vec::new();
    for pair in fields {
        if pair.is_empty() {
            return Err(PlanRefusal::whole(
                CauseCode::MalformedStageLine,
                format!(
                    "canonical form is single-space separated; a repeated space produced an \
                     empty field in {line:?}"
                ),
            ));
        }
        let Some((key, value)) = pair.split_once('=') else {
            return Err(PlanRefusal::whole(
                CauseCode::MalformedStageLine,
                format!("field {pair:?} is not a `key=value` pair"),
            ));
        };
        if !CANONICAL_KEYS.contains(&key) {
            return Err(PlanRefusal::whole(
                CauseCode::UnknownKey,
                format!(
                    "key {key:?} is outside the canonical seven ({}); §2.1 admits no others",
                    CANONICAL_KEYS.join(", ")
                ),
            ));
        }
        if seen.contains(&key) {
            return Err(PlanRefusal::whole(
                CauseCode::DuplicateKey,
                format!("key {key:?} appears more than once on one stage line"),
            ));
        }
        seen.push(key);
        values.push((key, value));
    }

    for key in CANONICAL_KEYS {
        if !seen.contains(&key) {
            return Err(PlanRefusal::whole(
                CauseCode::MissingKey,
                format!("stage line is missing the canonical key {key:?}: {line:?}"),
            ));
        }
    }
    if seen != CANONICAL_KEYS {
        return Err(PlanRefusal::whole(
            CauseCode::KeyOutOfOrder,
            format!(
                "§2.1 fixes the key order as {}; this line reads {}",
                CANONICAL_KEYS.join(", "),
                seen.join(", ")
            ),
        ));
    }
    Ok(values)
}

/// A scalar value that §2.1 requires to be present.
fn nonempty(value: &str, key: &str) -> Result<String, PlanRefusal> {
    if value.is_empty() {
        return Err(PlanRefusal::whole(
            CauseCode::EmptyValue,
            format!("`{key}=` is empty"),
        ));
    }
    Ok(value.to_owned())
}

/// Is this value wrapped in quotes? §2.1: "no quoting".
fn is_quoted(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 2
        && ((bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\''))
}

/// Split a comma-separated value, refusing an empty element.
///
/// An empty element is `a,,b` or a trailing comma — neither is canonical,
/// and silently dropping one would let `legs=a,` declare a phantom leg.
fn split_csv(value: &str, key: &str) -> Result<Vec<String>, PlanRefusal> {
    if value.is_empty() {
        return Err(PlanRefusal::whole(
            CauseCode::EmptyValue,
            format!("`{key}=` is empty"),
        ));
    }
    let parts: Vec<&str> = value.split(',').collect();
    if parts.iter().any(|p| p.is_empty()) {
        return Err(PlanRefusal::whole(
            CauseCode::EmptyValue,
            format!("`{key}={value}` carries an empty comma-separated element"),
        ));
    }
    Ok(parts.into_iter().map(str::to_owned).collect())
}
