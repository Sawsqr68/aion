//! Decoding of `cargo fmt --verbose --check` output.
//!
//! `cargo fmt --message-format json` is a nightly-only surface, so on the stable
//! toolchain this repository pins, the verbose check stream is the machine-
//! readable one available: it names every target it hands to rustfmt (the
//! denominator) and prints a `Diff in <file>:<line>:` header before each hunk
//! (the findings). The hunk itself travels in the finding's message.

use super::StreamReport;
use crate::types::Finding;

/// Prefix rustfmt prints before each formatting difference.
const DIFF_PREFIX: &str = "Diff in ";
/// Code reported for every formatting difference.
const FMT_CODE: &str = "rustfmt";
/// Message reported when a diff header is followed by no hunk lines.
const NO_HUNK: &str = "(rustfmt reported a difference with no hunk)";

/// Decodes a `cargo fmt --all --verbose --check` stream.
#[must_use]
pub fn extract(leg: &str, stdout: &str) -> StreamReport {
    let mut report = StreamReport::default();
    let mut pending: Option<Finding> = None;
    let mut hunk: Vec<&str> = Vec::new();
    for line in stdout.lines() {
        if is_target_header(line) {
            // A new target closes the previous target's last hunk, so the
            // rustfmt invocation cargo prints next never lands in a finding.
            flush(&mut report, &mut pending, &mut hunk);
            report.denominator += 1;
            continue;
        }
        if let Some(rest) = line.strip_prefix(DIFF_PREFIX) {
            flush(&mut report, &mut pending, &mut hunk);
            pending = Some(diff_finding(leg, rest));
            continue;
        }
        if pending.is_some() {
            hunk.push(line);
        }
    }
    flush(&mut report, &mut pending, &mut hunk);
    report
}

/// True for the `[lib (2021)] "/path/to/src/lib.rs"` target lines cargo prints
/// in verbose mode, one per target it submits to rustfmt.
fn is_target_header(line: &str) -> bool {
    line.starts_with('[') && line.contains("] \"") && line.ends_with('"')
}

/// Splits `<file>:<line>:` into a finding; a header without a trailing line
/// number still yields a finding, with no line.
fn diff_finding(leg: &str, rest: &str) -> Finding {
    let trimmed = rest.trim_end().trim_end_matches(':');
    let (file, line) = match trimmed.rsplit_once(':') {
        Some((file, number)) => match number.parse::<i64>() {
            Ok(number) => (file, Some(number)),
            Err(_) => (trimmed, None),
        },
        None => (trimmed, None),
    };
    Finding {
        leg: leg.to_owned(),
        level: "error".to_owned(),
        file: file.to_owned(),
        line,
        column: None,
        code: FMT_CODE.to_owned(),
        message: String::new(),
    }
}

fn flush(report: &mut StreamReport, pending: &mut Option<Finding>, hunk: &mut Vec<&str>) {
    if let Some(mut finding) = pending.take() {
        let body = hunk
            .iter()
            .map(|line| line.trim_end())
            .collect::<Vec<_>>()
            .join("\n");
        let body = body.trim_matches('\n').to_owned();
        finding.message = if body.is_empty() {
            NO_HUNK.to_owned()
        } else {
            body
        };
        report.findings.push(finding);
    }
    hunk.clear();
}

#[cfg(test)]
mod tests {
    use super::extract;

    const CLEAN: &str = concat!(
        "[lib (2024)] \"/srv/gates/checkouts/aion/src/lib.rs\"\n",
        "rustfmt --edition 2024 --color never --check /srv/gates/checkouts/aion/src/lib.rs\n"
    );
    const DIRTY: &str = concat!(
        "[lib (2021)] \"/srv/fixt/src/lib.rs\"\n",
        "rustfmt --edition 2021 --color never --check /srv/fixt/src/lib.rs\n",
        "Diff in /srv/fixt/src/lib.rs:6:\n",
        " #[cfg(test)]\n",
        "-    fn fails() { assert_eq!(1, 2); }\n",
        "+    fn fails() {\n"
    );

    #[test]
    fn a_clean_tree_still_reports_the_targets_it_checked() {
        let report = extract("fmt", CLEAN);
        assert!(report.findings.is_empty());
        assert_eq!(report.denominator, 1);
        assert!(!report.is_silent());
    }

    #[test]
    fn a_diff_header_becomes_a_finding_carrying_its_hunk() {
        let report = extract("fmt", DIRTY);
        assert_eq!(report.denominator, 1);
        assert_eq!(report.findings.len(), 1);
        let finding = &report.findings[0];
        assert_eq!(finding.file, "/srv/fixt/src/lib.rs");
        assert_eq!(finding.line, Some(6));
        assert_eq!(finding.code, "rustfmt");
        assert_eq!(finding.level, "error");
        assert!(finding.message.contains("fn fails() { assert_eq!(1, 2); }"));
    }

    #[test]
    fn an_empty_stream_is_silent() {
        assert!(extract("fmt", "").is_silent());
    }

    #[test]
    fn a_second_target_closes_the_first_targets_hunk() {
        let stream = format!(
            "{DIRTY}[bin (2021)] \"/srv/fixt/src/main.rs\"\nrustfmt --edition 2021 --color never --check /srv/fixt/src/main.rs\n"
        );
        let report = extract("fmt", &stream);
        assert_eq!(report.denominator, 2);
        assert_eq!(report.findings.len(), 1);
        assert!(!report.findings[0].message.contains("rustfmt --edition"));
    }
}
