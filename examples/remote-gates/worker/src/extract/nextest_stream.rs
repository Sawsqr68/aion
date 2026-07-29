//! Decoding of nextest's `libtest-json-plus` stream.
//!
//! Failed test events become findings carrying the test's own name as the code
//! and its captured output as the message. The denominator is the count of test
//! events that actually reached a verdict — a run that produced no verdicts
//! cannot certify anything, however it exited.

use serde_json::Value;

use super::StreamReport;
use crate::types::Finding;

/// File reported for a test failure: the leg's finding is a test, not a span.
const TEST_FILE: &str = "unknown";
/// Code reported when a failed event carries no test name.
const UNNAMED_TEST: &str = "(unnamed test)";
/// Message reported when a failed event captured no output.
const NO_OUTPUT: &str = "(test captured no output)";

/// Decodes a nextest libtest-json-plus stream into findings and a denominator.
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
        let (Some(kind), Some(event)) = (
            record.get("type").and_then(Value::as_str),
            record.get("event").and_then(Value::as_str),
        ) else {
            report.unparsed += 1;
            continue;
        };
        if kind != "test" {
            continue;
        }
        match event {
            "ok" => report.denominator += 1,
            "failed" => {
                report.denominator += 1;
                report.findings.push(finding(leg, &record));
            }
            _ => {}
        }
    }
    report
}

fn finding(leg: &str, record: &Value) -> Finding {
    let name = record
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or(UNNAMED_TEST);
    let output = record
        .get("stdout")
        .and_then(Value::as_str)
        .or_else(|| record.get("stderr").and_then(Value::as_str))
        .unwrap_or(NO_OUTPUT);
    Finding {
        leg: leg.to_owned(),
        level: "error".to_owned(),
        file: TEST_FILE.to_owned(),
        line: None,
        column: None,
        code: name.to_owned(),
        message: output.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::extract;

    const SUITE_START: &str = r#"{"type":"suite","event":"started","test_count":2}"#;
    const TEST_START: &str = r#"{"type":"test","event":"started","name":"fixt::tests::fails"}"#;
    const TEST_OK: &str =
        r#"{"type":"test","event":"ok","name":"fixt::tests::passes","exec_time":0.01}"#;
    const TEST_FAILED: &str = r#"{"type":"test","event":"failed","name":"fixt::tests::fails","stdout":"assertion `left == right` failed"}"#;
    const SUITE_FAILED: &str = r#"{"type":"suite","event":"failed","passed":1,"failed":1}"#;

    #[test]
    fn failed_events_become_findings_and_verdicts_are_the_denominator() {
        let stream = [SUITE_START, TEST_START, TEST_OK, TEST_FAILED, SUITE_FAILED].join("\n");
        let report = extract("nextest", &stream);
        assert_eq!(report.denominator, 2);
        assert_eq!(report.findings.len(), 1);
        let finding = &report.findings[0];
        assert_eq!(finding.leg, "nextest");
        assert_eq!(finding.level, "error");
        assert_eq!(finding.code, "fixt::tests::fails");
        assert_eq!(finding.message, "assertion `left == right` failed");
        assert_eq!(finding.line, None);
    }

    #[test]
    fn a_green_run_reports_a_denominator_and_no_findings() {
        let stream = [SUITE_START, TEST_OK, TEST_OK].join("\n");
        let report = extract("nextest", &stream);
        assert_eq!(report.denominator, 2);
        assert!(report.findings.is_empty());
        assert!(!report.is_silent());
    }

    #[test]
    fn a_non_json_stream_yields_a_silent_report_with_counted_noise() {
        let report = extract("nextest", "error: no such command: `nextest`\n");
        assert!(report.is_silent());
        assert_eq!(report.unparsed, 1);
    }
}
