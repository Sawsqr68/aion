//! Decoding of Cargo's `--message-format json` stream.
//!
//! The shape is the one the operator specified: `compiler-message` records at
//! `error` or `warning` level become findings; the primary span supplies file,
//! line, and column, falling back to the first span and then to `unknown`; the
//! lint code falls back to `compile-error`. The denominator is the stream's own
//! `compiler-artifact` count — the units Cargo actually processed.

use serde_json::Value;

use super::StreamReport;
use crate::types::Finding;

/// File reported when a diagnostic carries no span at all.
const UNKNOWN_FILE: &str = "unknown";
/// Code reported for a compiler message that carries no lint code.
const NO_CODE: &str = "compile-error";
/// Message reported for a compiler message that carries no text.
const NO_MESSAGE: &str = "(compiler message carried no text)";

/// Decodes a Cargo JSON message stream into findings and a denominator.
#[must_use]
pub fn extract(leg: &str, stdout: &str) -> StreamReport {
    let mut report = StreamReport::default();
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(record) = serde_json::from_str::<Value>(line) else {
            report.unparsed += 1;
            continue;
        };
        match record.get("reason").and_then(Value::as_str) {
            Some("compiler-artifact") => report.denominator += 1,
            Some("compiler-message") => {
                if let Some(finding) = finding(leg, &record) {
                    report.findings.push(finding);
                }
            }
            Some(_) => {}
            None => report.unparsed += 1,
        }
    }
    report
}

fn finding(leg: &str, record: &Value) -> Option<Finding> {
    let message = record.get("message")?;
    let level = message.get("level").and_then(Value::as_str)?;
    if level != "error" && level != "warning" {
        return None;
    }
    let span = primary_span(message);
    Some(Finding {
        leg: leg.to_owned(),
        level: level.to_owned(),
        file: span
            .and_then(|span| span.get("file_name"))
            .and_then(Value::as_str)
            .unwrap_or(UNKNOWN_FILE)
            .to_owned(),
        line: span
            .and_then(|span| span.get("line_start"))
            .and_then(Value::as_i64),
        column: span
            .and_then(|span| span.get("column_start"))
            .and_then(Value::as_i64),
        code: message
            .get("code")
            .and_then(|code| code.get("code"))
            .and_then(Value::as_str)
            .unwrap_or(NO_CODE)
            .to_owned(),
        message: message
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or(NO_MESSAGE)
            .to_owned(),
    })
}

fn primary_span(message: &Value) -> Option<&Value> {
    let spans = message.get("spans").and_then(Value::as_array)?;
    spans
        .iter()
        .find(|span| span.get("is_primary").and_then(Value::as_bool) == Some(true))
        .or_else(|| spans.first())
}

#[cfg(test)]
mod tests {
    use super::extract;

    const WARNING: &str = r#"{"reason":"compiler-message","message":{"level":"warning","message":"unused variable: `unused`","code":{"code":"unused_variables"},"spans":[{"file_name":"src/lib.rs","line_start":2,"column_start":9,"is_primary":true}]}}"#;
    const SECONDARY_ONLY: &str = r#"{"reason":"compiler-message","message":{"level":"error","message":"mismatched types","spans":[{"file_name":"src/main.rs","line_start":7,"column_start":3,"is_primary":false}]}}"#;
    const SPANLESS: &str = r#"{"reason":"compiler-message","message":{"level":"error","message":"linking failed","spans":[]}}"#;
    const NOTE: &str = r#"{"reason":"compiler-message","message":{"level":"note","message":"see the docs","spans":[]}}"#;
    const ARTIFACT: &str = r#"{"reason":"compiler-artifact","target":{"name":"fixt"}}"#;
    const FINISHED: &str = r#"{"reason":"build-finished","success":true}"#;

    #[test]
    fn a_primary_span_supplies_file_line_and_column() {
        let report = extract("clippy", WARNING);
        assert_eq!(report.findings.len(), 1);
        let finding = &report.findings[0];
        assert_eq!(finding.leg, "clippy");
        assert_eq!(finding.level, "warning");
        assert_eq!(finding.file, "src/lib.rs");
        assert_eq!(finding.line, Some(2));
        assert_eq!(finding.column, Some(9));
        assert_eq!(finding.code, "unused_variables");
        assert_eq!(finding.message, "unused variable: `unused`");
    }

    #[test]
    fn the_first_span_stands_in_when_none_is_primary() {
        let report = extract("check", SECONDARY_ONLY);
        let finding = &report.findings[0];
        assert_eq!(finding.file, "src/main.rs");
        assert_eq!(finding.line, Some(7));
        assert_eq!(finding.code, "compile-error");
    }

    #[test]
    fn a_spanless_diagnostic_is_still_a_finding() {
        let report = extract("check", SPANLESS);
        let finding = &report.findings[0];
        assert_eq!(finding.file, "unknown");
        assert_eq!(finding.line, None);
        assert_eq!(finding.column, None);
    }

    #[test]
    fn notes_are_not_findings_and_artifacts_are_the_denominator() {
        let stream = [NOTE, ARTIFACT, ARTIFACT, FINISHED].join("\n");
        let report = extract("check", &stream);
        assert!(report.findings.is_empty());
        assert_eq!(report.denominator, 2);
        assert_eq!(report.unparsed, 0);
    }

    #[test]
    fn a_non_json_stream_yields_a_silent_report_with_counted_noise() {
        let report = extract("check", "error: could not compile\nnot json at all\n");
        assert!(report.is_silent());
        assert_eq!(report.unparsed, 2);
    }
}
