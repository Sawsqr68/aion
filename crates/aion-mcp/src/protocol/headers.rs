//! The Streamable-HTTP standard-header contract.
//!
//! Three headers are REQUIRED for compliance and every one of them duplicates a
//! value that is also in the body. The server's job is to prove they agree —
//! any disagreement, any absence, and any header value carrying characters a
//! field value may not carry is a `-32020` `HeaderMismatch` on HTTP 400.
//!
//! | Header | Body source | Required for |
//! |---|---|---|
//! | `MCP-Protocol-Version` | `params._meta["io.modelcontextprotocol/protocolVersion"]` | every POST |
//! | `Mcp-Method` | `method` | every request |
//! | `Mcp-Name` | `params.name` / `params.uri` / `params.taskId` | `tools/call`, `tasks/*` |
//!
//! `Mcp-Param-{Name}` headers mirror properties annotated `x-mcp-header` in a
//! tool's `inputSchema`. The rule is enforced here in full — a **recognised**
//! param header (one whose annotation the called tool declares) must agree with
//! the body value, and an **unrecognised** one is forwarded-and-ignored per
//! RFC 9110. Integers are compared NUMERICALLY: `42.0`, `+42` and `42` are one
//! value, and comparing their spellings would refuse a request that agrees.
//!
//! Aion's own catalog annotates nothing — a header value is visible to every
//! intermediary on the path, and every argument its tools take is either an
//! identity a caller should not be leaking into proxy logs or a cursor with no
//! routing value. That is a choice about Aion's tools, not a gap in the
//! transport: the rule is implemented, tested against a tool that does declare
//! annotations, and would hold the moment any host declared one.

use base64::Engine as _;
use serde_json::Value;

use super::error::ProtocolError;

/// The `MCP-Protocol-Version` header name, lowercased for comparison.
pub const HEADER_PROTOCOL_VERSION: &str = "mcp-protocol-version";
/// The `Mcp-Method` header name, lowercased for comparison.
pub const HEADER_METHOD: &str = "mcp-method";
/// The `Mcp-Name` header name, lowercased for comparison.
pub const HEADER_NAME: &str = "mcp-name";

/// Decode the Base64 sentinel form `=?base64?{B64}?=` when present.
///
/// The markers are lowercase and case-sensitive, exactly as the spec shows.
/// A value not wrapped in the sentinel is returned unchanged; a value wrapped
/// in it that does not decode to valid UTF-8 is a header mismatch, because the
/// server cannot then prove agreement with the body.
///
/// # Errors
///
/// [`ProtocolError::HeaderMismatch`] when the sentinel wrapper is present but
/// its contents are not valid Base64 or not valid UTF-8.
pub fn decode_sentinel(header: &str, value: &str) -> Result<String, ProtocolError> {
    let Some(encoded) = value
        .strip_prefix("=?base64?")
        .and_then(|rest| rest.strip_suffix("?="))
    else {
        return Ok(value.to_owned());
    };
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|error| ProtocolError::HeaderMismatch {
            detail: format!("{header} base64 sentinel value did not decode: {error}"),
        })?;
    String::from_utf8(bytes).map_err(|error| ProtocolError::HeaderMismatch {
        detail: format!("{header} base64 sentinel value is not valid UTF-8: {error}"),
    })
}

/// Reject a header value carrying characters an HTTP field value may not carry.
///
/// RFC 9110 field values are visible ASCII plus space and horizontal tab; a CR,
/// LF, NUL, or other control character is a response-splitting vector and is
/// refused outright rather than sanitised.
///
/// # Errors
///
/// [`ProtocolError::HeaderMismatch`] when the value contains a control
/// character other than horizontal tab, or a non-ASCII byte.
pub fn reject_invalid_characters(header: &str, value: &str) -> Result<(), ProtocolError> {
    if let Some(offender) = value
        .chars()
        .find(|character| (character.is_control() && *character != '\t') || !character.is_ascii())
    {
        return Err(ProtocolError::HeaderMismatch {
            detail: format!(
                "{header} header value contains an invalid character (U+{:04X})",
                u32::from(offender)
            ),
        });
    }
    Ok(())
}

/// A case-insensitive view of one request's headers.
///
/// Header *names* are compared case-insensitively; header *values* (method
/// names, tool names) are compared case-sensitively, because they are protocol
/// identifiers rather than field names.
#[derive(Clone, Debug, Default)]
pub struct HeaderView {
    entries: Vec<(String, String)>,
}

impl HeaderView {
    /// Build a view from `(name, value)` pairs in any casing.
    #[must_use]
    pub fn new<I, K, V>(entries: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        Self {
            entries: entries
                .into_iter()
                .map(|(name, value)| {
                    (
                        name.as_ref().to_ascii_lowercase(),
                        value.as_ref().to_owned(),
                    )
                })
                .collect(),
        }
    }

    /// The first value for `name` (already lowercased by the caller).
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(candidate, _)| candidate == name)
            .map(|(_, value)| value.as_str())
    }

    /// Every `Mcp-Param-{Name}` header present on the request, as
    /// `(suffix, value)` pairs with the suffix lowercased.
    #[must_use]
    pub fn param_headers(&self) -> Vec<(&str, &str)> {
        self.entries
            .iter()
            .filter_map(|(name, value)| {
                name.strip_prefix("mcp-param-")
                    .map(|suffix| (suffix, value.as_str()))
            })
            .collect()
    }
}

/// Validate a required header against the body value it must mirror.
///
/// # Errors
///
/// [`ProtocolError::HeaderMismatch`] when the header is absent, carries invalid
/// characters, or — after Base64-sentinel decoding — does not equal `expected`.
pub fn require_matching(
    headers: &HeaderView,
    header: &str,
    display_name: &str,
    expected: &str,
) -> Result<(), ProtocolError> {
    let value = headers
        .get(header)
        .ok_or_else(|| ProtocolError::HeaderMismatch {
            detail: format!("{display_name} header is required and was not sent"),
        })?;
    reject_invalid_characters(display_name, value)?;
    let decoded = decode_sentinel(display_name, value)?;
    if decoded == expected {
        return Ok(());
    }
    Err(ProtocolError::HeaderMismatch {
        detail: format!(
            "{display_name} header value '{decoded}' does not match body value '{expected}'"
        ),
    })
}

/// Validate one `Mcp-Param-{Name}` header against the body value it mirrors.
///
/// Integers compare NUMERICALLY, not as strings: `42`, `42.0` and `+42` are the
/// same integer, and a lexical comparison would reject a request whose header
/// and body agree perfectly. Strings compare exactly, after Base64-sentinel
/// decoding. Booleans compare against `true`/`false`.
///
/// # Errors
///
/// [`ProtocolError::HeaderMismatch`] when the header is absent, carries invalid
/// characters, fails to decode, or does not equal `expected`.
pub fn require_matching_param(
    headers: &HeaderView,
    header: &str,
    display_name: &str,
    expected: &Value,
) -> Result<(), ProtocolError> {
    let value = headers
        .get(header)
        .ok_or_else(|| ProtocolError::HeaderMismatch {
            detail: format!(
                "{display_name} header is required because the request body carries a value for it"
            ),
        })?;
    reject_invalid_characters(display_name, value)?;
    let decoded = decode_sentinel(display_name, value)?;
    if param_values_match(&decoded, expected) {
        return Ok(());
    }
    Err(ProtocolError::HeaderMismatch {
        detail: format!(
            "{display_name} header value '{decoded}' does not match body value '{expected}'"
        ),
    })
}

/// The largest magnitude an `x-mcp-header` integer may carry: `2^53 − 1`.
///
/// Beyond it a value cannot survive a round trip through a JSON `number` in
/// every conforming implementation on the path, so a header claiming to mirror
/// one cannot be proven to agree with the body. Such a value does not match.
const MAX_HEADER_INTEGER: i128 = (1_i128 << 53) - 1;

/// Whether a header spelling denotes the same value as a JSON body value.
///
/// Split out so the numeric rule is testable on its own and cannot be
/// accidentally bypassed by a second comparison site.
#[must_use]
pub fn param_values_match(header: &str, expected: &Value) -> bool {
    match expected {
        Value::String(text) => header == text,
        Value::Bool(flag) => header == if *flag { "true" } else { "false" },
        // Both sides go through the SAME text-to-integer rule, so "42", "42.0"
        // and "+42" are one value on either side and the two sides cannot
        // disagree about what a spelling means. No float arithmetic is
        // involved: an approximate comparison would make two genuinely
        // different values match, which is exactly what a routing header must
        // never do.
        Value::Number(number) => header_integer(&number.to_string())
            .is_some_and(|expected| header_integer(header) == Some(expected)),
        // Null / array / object are not values `x-mcp-header` may be applied
        // to; the catalog refuses such an annotation before any request runs,
        // so reaching here means the annotation escaped validation and the
        // safe answer is "does not match".
        _ => false,
    }
}

/// Parse a decimal integer spelling exactly.
///
/// Accepts a leading `+` (legal in an HTTP field value, not in JSON) and a
/// purely-zero fractional part, so `42`, `+42` and `42.0` are one integer.
/// Rejects a non-zero fraction, exponent notation, and anything past
/// [`MAX_HEADER_INTEGER`].
fn header_integer(text: &str) -> Option<i128> {
    let text = text.strip_prefix('+').unwrap_or(text);
    let whole = match text.split_once('.') {
        Some((whole, fraction)) => {
            if fraction.is_empty() || !fraction.chars().all(|digit| digit == '0') {
                return None;
            }
            whole
        }
        None => text,
    };
    whole
        .parse::<i128>()
        .ok()
        .filter(|value| value.abs() <= MAX_HEADER_INTEGER)
}

/// Reject a header that was sent when the body carries no corresponding value.
///
/// The matrix the server must implement has three arms; this is the third:
/// value absent in the body means the header MUST NOT be expected — and a
/// header asserting a value the body does not have cannot be proven to agree
/// with anything, so it is refused rather than ignored.
///
/// # Errors
///
/// [`ProtocolError::HeaderMismatch`] when `header` is present.
pub fn reject_unexpected(
    headers: &HeaderView,
    header: &str,
    display_name: &str,
) -> Result<(), ProtocolError> {
    if headers.get(header).is_some() {
        return Err(ProtocolError::HeaderMismatch {
            detail: format!(
                "{display_name} header was sent but this request's body carries no value for it"
            ),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        HEADER_METHOD, HEADER_NAME, HeaderView, ProtocolError, decode_sentinel, param_values_match,
        reject_invalid_characters, reject_unexpected, require_matching, require_matching_param,
    };

    fn headers(entries: &[(&str, &str)]) -> HeaderView {
        HeaderView::new(entries.iter().copied())
    }

    #[test]
    fn header_names_are_case_insensitive_and_values_are_not() {
        let view = headers(&[("MCP-Method", "tools/call")]);
        assert!(require_matching(&view, HEADER_METHOD, "Mcp-Method", "tools/call").is_ok());
        assert!(matches!(
            require_matching(&view, HEADER_METHOD, "Mcp-Method", "TOOLS/CALL"),
            Err(ProtocolError::HeaderMismatch { .. })
        ));
    }

    #[test]
    fn the_base64_sentinel_is_decoded_before_comparison() -> Result<(), Box<dyn std::error::Error>>
    {
        // "read_transcript" base64-encoded.
        let view = headers(&[("mcp-name", "=?base64?cmVhZF90cmFuc2NyaXB0?=")]);
        require_matching(&view, HEADER_NAME, "Mcp-Name", "read_transcript")?;
        assert_eq!(decode_sentinel("Mcp-Name", "plain")?, "plain");
        Ok(())
    }

    #[test]
    fn a_broken_sentinel_is_a_header_mismatch() {
        assert!(matches!(
            decode_sentinel("Mcp-Name", "=?base64?!!!!?="),
            Err(ProtocolError::HeaderMismatch { .. })
        ));
    }

    #[test]
    fn control_characters_are_refused() {
        assert!(matches!(
            reject_invalid_characters("Mcp-Name", "start\r\nInjected: yes"),
            Err(ProtocolError::HeaderMismatch { .. })
        ));
        assert!(reject_invalid_characters("Mcp-Name", "start_run").is_ok());
    }

    #[test]
    fn a_missing_required_header_is_a_mismatch_and_an_unexpected_one_is_too() {
        let empty = headers(&[]);
        assert!(matches!(
            require_matching(&empty, HEADER_NAME, "Mcp-Name", "start_run"),
            Err(ProtocolError::HeaderMismatch { .. })
        ));
        let present = headers(&[("mcp-name", "start_run")]);
        assert!(matches!(
            reject_unexpected(&present, HEADER_NAME, "Mcp-Name"),
            Err(ProtocolError::HeaderMismatch { .. })
        ));
        assert!(reject_unexpected(&empty, HEADER_NAME, "Mcp-Name").is_ok());
    }

    #[test]
    fn an_integer_param_compares_numerically_not_lexically() {
        let expected = serde_json::json!(42);
        for spelling in ["42", "42.0", "+42", "42.000"] {
            assert!(
                param_values_match(spelling, &expected),
                "`{spelling}` denotes the integer 42"
            );
        }
        for spelling in ["43", "4.2", "42.1", " 42", "42x", "0x2a", "4.2e1", ""] {
            assert!(
                !param_values_match(spelling, &expected),
                "`{spelling}` must not be accepted as the integer 42"
            );
        }
        // A negative integer round-trips under the same rule.
        assert!(param_values_match("-7", &serde_json::json!(-7)));
        assert!(!param_values_match("7", &serde_json::json!(-7)));
    }

    #[test]
    fn an_integer_beyond_the_safe_range_never_matches() {
        // 2^53 is one past the largest value the spec permits in a header.
        let too_large = serde_json::json!(9_007_199_254_740_992_i64);
        assert!(!param_values_match("9007199254740992", &too_large));
        // The boundary value itself still matches.
        let largest = serde_json::json!(9_007_199_254_740_991_i64);
        assert!(param_values_match("9007199254740991", &largest));
    }

    #[test]
    fn string_and_boolean_params_compare_exactly() {
        assert!(param_values_match("west", &serde_json::json!("west")));
        assert!(!param_values_match("West", &serde_json::json!("west")));
        assert!(param_values_match("true", &serde_json::json!(true)));
        assert!(!param_values_match("1", &serde_json::json!(true)));
        // A value type no annotation may carry never matches.
        assert!(!param_values_match("x", &serde_json::json!(["x"])));
    }

    #[test]
    fn a_param_header_is_sentinel_decoded_before_the_numeric_comparison() {
        // "42" base64-encoded, against the integer 42.
        let view = headers(&[("mcp-param-shard", "=?base64?NDI=?=")]);
        assert!(
            require_matching_param(
                &view,
                "mcp-param-shard",
                "Mcp-Param-Shard",
                &serde_json::json!(42)
            )
            .is_ok()
        );
    }

    #[test]
    fn param_headers_are_enumerated_by_suffix() {
        let view = headers(&[("Mcp-Param-Namespace", "default"), ("mcp-method", "x")]);
        assert_eq!(view.param_headers(), vec![("namespace", "default")]);
    }
}
