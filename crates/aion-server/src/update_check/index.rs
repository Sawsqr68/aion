//! Reading "the greatest installable version" out of a crates.io sparse-index
//! line file.
//!
//! The sparse index serves one file per crate at
//! `https://index.crates.io/<prefix>/<name>` — newline-delimited JSON, one
//! object per published version, each carrying (among other fields) `name`,
//! `vers`, and `yanked`. The update check's declared command fetches that
//! file for `aion-cli` verbatim; THIS module is the parse, kept server-side
//! and unit-tested on captured fixture lines because the gates run offline
//! and a parser proven only against the live index is a parser proven never.
//!
//! # What "latest" means here: the greatest version `cargo install` would take
//!
//! Two whole classes of published line are excluded from the answer, for the
//! same reason:
//!
//! - **Yanked releases.** A yanked version cannot be `cargo install`ed fresh,
//!   so reporting one would point the operator at an upgrade the registry
//!   refuses.
//! - **Prereleases.** `cargo install aion-cli` never selects a prerelease
//!   unless the operator names it explicitly (`--version 0.14.0-rc.1`), so a
//!   higher-core prerelease is exactly as uninstallable-by-default as a
//!   yanked release — announcing `0.14.0-rc.1` against a stable `0.13.7`
//!   would send the operator through the documented upgrade and hand them
//!   `0.13.7` back. Both arms (same-core and higher-core) are pinned in the
//!   tests below.
//!
//! An index whose every line is excluded yields a typed refusal carrying the
//! exclusion counts, not an invented answer.
//!
//! # Every line must name the crate being asked about
//!
//! Each real index line carries the crate's own `name`. A body whose lines
//! name some OTHER crate is not the `aion-cli` index — a proxy rewrite, a
//! wrong file, a poisoned answer — and computing a maximum over it would be
//! the wrong-but-plausible number this module exists to refuse. Any line
//! naming a different crate refuses the whole read.
//!
//! # Malformation refuses loudly — and the refusals echo NOTHING remote
//!
//! Any line that is not a JSON object with the expected fields fails the
//! WHOLE read, naming the line NUMBER. A partial read would happily compute a
//! maximum over the subset that parsed — the "comfortable number" failure
//! mode: plausible, wrong, and silent.
//!
//! The fetched body is remote input, so no refusal on this path ever
//! interpolates fetched bytes into its message: every reason is a bounded,
//! statically-known string plus a line number ([`super::semver::SemVerError`]
//! holds the same rule). The raw answer is already on the run's transcript,
//! streamed by the declared-body spine — an operator who needs the offending
//! bytes reads them there, where they belong, instead of finding a
//! many-megabyte line in the server log.

use serde_json::Value;

use super::semver::{SemVer, SemVerError};

/// The one crate this parse will accept an index body for. Tied to the
/// fetched URL by a document test: [`super::document::FETCH_COMMAND`] must
/// end in `/{INDEX_CRATE_NAME}`.
pub(crate) const INDEX_CRATE_NAME: &str = "aion-cli";

/// A refusal to read the index body, naming what refused — by line number
/// and bounded description only, never by echoing fetched bytes.
#[derive(Debug, thiserror::Error)]
pub(crate) enum IndexError {
    /// The body has no non-blank lines at all.
    #[error("the crate index body is empty — not one version line to read")]
    Empty,
    /// A line is not JSON. The reason is serde's syntax diagnostic, which
    /// names positions (line/column/EOF), never input content — the parse
    /// targets [`Value`], so the content-echoing typed-mismatch errors cannot
    /// arise.
    #[error("crate index line {line_number} is not JSON: {reason}")]
    Syntax {
        /// 1-based line number in the fetched body.
        line_number: usize,
        /// serde's positional diagnostic.
        reason: String,
    },
    /// A line parsed as JSON but is not an object. Also the arm that refuses
    /// the sequence form (`["1.0.0", false]`) a derived struct deserializer
    /// would silently accept.
    #[error("crate index line {line_number} is {found}, not an object")]
    NotAnObject {
        /// 1-based line number in the fetched body.
        line_number: usize,
        /// The JSON kind actually found.
        found: &'static str,
    },
    /// A required field is absent.
    #[error("crate index line {line_number} has no `{field}` field")]
    MissingField {
        /// 1-based line number in the fetched body.
        line_number: usize,
        /// The absent field.
        field: &'static str,
    },
    /// A required field is present with the wrong JSON type.
    #[error("crate index line {line_number}: `{field}` is {found}, not {expected}")]
    WrongFieldType {
        /// 1-based line number in the fetched body.
        line_number: usize,
        /// The field with the wrong type.
        field: &'static str,
        /// What the field must be.
        expected: &'static str,
        /// The JSON kind actually found.
        found: &'static str,
    },
    /// A line names a crate other than [`INDEX_CRATE_NAME`] — this body is
    /// not the index file that was asked for. The found name is remote input
    /// and is deliberately NOT echoed; the raw body is on the transcript.
    #[error(
        "crate index line {line_number} names a different crate than `{INDEX_CRATE_NAME}` — \
         this is not the `{INDEX_CRATE_NAME}` index; the fetched body is on the run's transcript"
    )]
    WrongCrate {
        /// 1-based line number in the fetched body.
        line_number: usize,
    },
    /// A line's `vers` is not a spec-exact semver version. The offending
    /// value is remote input and is deliberately NOT echoed; the source
    /// refusal is bounded by construction.
    #[error("crate index line {line_number} carries an unparseable version: {source}")]
    MalformedVersion {
        /// 1-based line number in the fetched body.
        line_number: usize,
        /// The semver grammar's bounded refusal.
        source: SemVerError,
    },
    /// Every parsed line is yanked or a prerelease — there is no version a
    /// plain `cargo install` would take.
    #[error(
        "the crate index holds no installable version: {yanked} yanked and {prereleases} \
         prerelease lines, nothing a plain `cargo install` would select"
    )]
    NoInstallableVersion {
        /// How many lines were excluded as yanked.
        yanked: usize,
        /// How many lines were excluded as prereleases.
        prereleases: usize,
    },
}

/// The greatest installable version in a sparse-index line body: published,
/// not yanked, not a prerelease, and belonging to [`INDEX_CRATE_NAME`].
///
/// Blank lines (a trailing newline, most commonly) are skipped; every
/// non-blank line must classify or the whole read refuses.
///
/// # Errors
///
/// Returns [`IndexError`] naming the refusing line, or the emptiness /
/// nothing-installable condition, when no maximum can be honestly computed.
pub(crate) fn latest_published(body: &str) -> Result<SemVer, IndexError> {
    let mut latest: Option<SemVer> = None;
    let mut yanked_lines: usize = 0;
    let mut prerelease_lines: usize = 0;
    let mut parsed_lines: usize = 0;

    for (position, line) in body.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let line_number = position + 1;
        let entry = classify_line(line, line_number)?;
        parsed_lines += 1;
        if entry.yanked {
            yanked_lines += 1;
            continue;
        }
        let version = SemVer::parse(&entry.vers).map_err(|error| IndexError::MalformedVersion {
            line_number,
            source: error,
        })?;
        if !version.prerelease.is_empty() {
            prerelease_lines += 1;
            continue;
        }
        latest = Some(match latest.take() {
            Some(current) if current >= version => current,
            _ => version,
        });
    }

    match latest {
        Some(version) => Ok(version),
        None if parsed_lines == 0 => Err(IndexError::Empty),
        None => Err(IndexError::NoInstallableVersion {
            yanked: yanked_lines,
            prereleases: prerelease_lines,
        }),
    }
}

/// The fields of one classified index line this question needs.
struct IndexLine {
    vers: String,
    yanked: bool,
}

/// Classify one non-blank line by hand over [`Value`].
///
/// By hand rather than a derived struct so that (a) the sequence form
/// `["…", …]` refuses instead of positionally matching fields, and (b) no
/// deserializer error can interpolate fetched bytes into a message.
fn classify_line(line: &str, line_number: usize) -> Result<IndexLine, IndexError> {
    let value: Value = serde_json::from_str(line).map_err(|error| IndexError::Syntax {
        line_number,
        reason: error.to_string(),
    })?;
    let Some(object) = value.as_object() else {
        return Err(IndexError::NotAnObject {
            line_number,
            found: json_kind(&value),
        });
    };

    let name = required_str(object, "name", line_number)?;
    if name != INDEX_CRATE_NAME {
        return Err(IndexError::WrongCrate { line_number });
    }
    let vers = required_str(object, "vers", line_number)?;
    let yanked_value = object.get("yanked").ok_or(IndexError::MissingField {
        line_number,
        field: "yanked",
    })?;
    let Some(yanked) = yanked_value.as_bool() else {
        return Err(IndexError::WrongFieldType {
            line_number,
            field: "yanked",
            expected: "a boolean",
            found: json_kind(yanked_value),
        });
    };

    Ok(IndexLine {
        vers: vers.to_owned(),
        yanked,
    })
}

/// A required string field, refused by name and kind otherwise.
fn required_str<'body>(
    object: &'body serde_json::Map<String, Value>,
    field: &'static str,
    line_number: usize,
) -> Result<&'body str, IndexError> {
    let value = object
        .get(field)
        .ok_or(IndexError::MissingField { line_number, field })?;
    value.as_str().ok_or_else(|| IndexError::WrongFieldType {
        line_number,
        field,
        expected: "a string",
        found: json_kind(value),
    })
}

/// A JSON value's kind, named for a refusal message.
const fn json_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

#[cfg(test)]
mod tests {
    use super::{INDEX_CRATE_NAME, IndexError, latest_published};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    /// The REAL index body for `aion-cli`, captured verbatim from
    /// `https://index.crates.io/ai/on/aion-cli` on 2026-08-11 (23 published
    /// versions, 0.1.0 through 0.13.7, none yanked, none prerelease). The
    /// parser's primary oracle is the actual wire format, not a hand-typed
    /// imitation of it.
    const CAPTURED_INDEX: &str = include_str!("fixtures/aion-cli-index.jsonl");

    /// One well-formed line for `vers`, with everything else real-shaped.
    fn line(vers: &str, yanked: bool) -> String {
        format!("{{\"name\":\"{INDEX_CRATE_NAME}\",\"vers\":\"{vers}\",\"yanked\":{yanked}}}\n")
    }

    #[test]
    fn the_captured_real_index_answers_its_greatest_version() -> TestResult {
        assert_eq!(latest_published(CAPTURED_INDEX)?.to_string(), "0.13.7");
        Ok(())
    }

    /// The maximum is a comparison, not a position: with the greatest version
    /// mid-file, last-line and lexicographic readings both give the wrong
    /// answer by name.
    #[test]
    fn the_greatest_version_need_not_be_the_last_line() -> TestResult {
        // Lexically "0.10.1" < "0.2.0"; numerically it is the reverse.
        let body = [
            line("0.9.0", false),
            line("0.10.1", false),
            line("0.2.0", false),
        ]
        .concat();
        assert_eq!(latest_published(&body)?.to_string(), "0.10.1");
        Ok(())
    }

    /// The yanked-skip pin, on a fixture where skipping and not skipping give
    /// DIFFERENT answers: the yanked line is the greatest version, so a parser
    /// that forgot the skip reports 9.9.9 and fails here by name.
    #[test]
    fn a_yanked_version_never_becomes_the_answer() -> TestResult {
        let body = [line("0.13.7", false), line("9.9.9", true)].concat();
        assert_eq!(latest_published(&body)?.to_string(), "0.13.7");
        Ok(())
    }

    /// THE HIGHER-CORE PRERELEASE PIN: a prerelease of a NEWER core than any
    /// stable release is excluded from the answer — `cargo install` would not
    /// take it, so announcing it points the operator at an upgrade that hands
    /// them their own version back. The fixture is chosen so inclusion and
    /// exclusion give different answers.
    #[test]
    fn a_higher_core_prerelease_never_becomes_the_answer() -> TestResult {
        let body = [line("0.13.7", false), line("0.14.0-rc.1", false)].concat();
        assert_eq!(latest_published(&body)?.to_string(), "0.13.7");
        Ok(())
    }

    /// The same-core arm of the prerelease pin.
    #[test]
    fn a_prerelease_does_not_outrank_its_release() -> TestResult {
        let body = [line("1.0.0", false), line("1.0.0-rc.1", false)].concat();
        assert_eq!(latest_published(&body)?.to_string(), "1.0.0");
        Ok(())
    }

    /// An index with ONLY excluded lines refuses with the counts, rather than
    /// inventing an answer or silently reporting nothing.
    #[test]
    fn an_index_with_nothing_installable_refuses_with_the_counts() {
        let body = [line("1.0.0", true), line("2.0.0-beta.1", false)].concat();
        assert!(matches!(
            latest_published(&body),
            Err(IndexError::NoInstallableVersion {
                yanked: 1,
                prereleases: 1
            })
        ));
    }

    #[test]
    fn an_empty_body_refuses() {
        assert!(matches!(latest_published(""), Err(IndexError::Empty)));
        assert!(matches!(latest_published("\n\n"), Err(IndexError::Empty)));
    }

    /// One malformed line fails the whole read, naming the line — a maximum
    /// computed over "the lines that happened to parse" is a silent subset.
    #[test]
    fn a_malformed_line_fails_the_whole_read_by_line_number() {
        let body = format!("{}an HTML error page, say\n", line("1.0.0", false));
        assert!(matches!(
            latest_published(&body),
            Err(IndexError::Syntax { line_number: 2, .. })
        ));
    }

    /// The sequence form a derived deserializer would accept positionally is
    /// refused by shape — an index line is an object or it is nothing.
    #[test]
    fn the_sequence_form_is_refused_by_shape() {
        let body = "[\"1.0.0\", false]\n";
        match latest_published(body) {
            Err(IndexError::NotAnObject {
                line_number: 1,
                found,
            }) => assert_eq!(found, "an array"),
            other => assert!(
                matches!(other, Err(IndexError::NotAnObject { .. })),
                "the sequence form must refuse as NotAnObject, got {other:?}"
            ),
        }
    }

    /// THE WRONG-CRATE PIN: a body whose lines name another crate is not the
    /// `aion-cli` index, and no maximum is computed over it — this is exactly
    /// the wrong-but-plausible answer the module exists to refuse.
    #[test]
    fn a_body_for_a_different_crate_refuses() {
        let body = "{\"name\":\"serde\",\"vers\":\"1.0.999\",\"yanked\":false}\n";
        assert!(matches!(
            latest_published(body),
            Err(IndexError::WrongCrate { line_number: 1 })
        ));
    }

    /// Field-shape refusals name the line, the field, and the kinds — and a
    /// missing field is its own arm.
    #[test]
    fn missing_and_mistyped_fields_refuse_by_name() {
        let missing_yanked = format!("{{\"name\":\"{INDEX_CRATE_NAME}\",\"vers\":\"1.0.0\"}}\n");
        assert!(matches!(
            latest_published(&missing_yanked),
            Err(IndexError::MissingField {
                line_number: 1,
                field: "yanked"
            })
        ));
        let numeric_vers =
            format!("{{\"name\":\"{INDEX_CRATE_NAME}\",\"vers\":5,\"yanked\":false}}\n");
        assert!(matches!(
            latest_published(&numeric_vers),
            Err(IndexError::WrongFieldType {
                line_number: 1,
                field: "vers",
                ..
            })
        ));
    }

    /// A malformed version fails the whole read naming the line, and the
    /// refusal's rendering never contains the offending value — the fetched
    /// body is remote input, and the raw answer belongs on the transcript,
    /// not multiplied into the server log.
    #[test]
    fn a_malformed_version_refuses_without_echoing_the_remote_value() -> TestResult {
        let hostile = "9".repeat(4096);
        let body = line(&format!("not-a-version-{hostile}"), false);
        match latest_published(&body) {
            Err(error @ IndexError::MalformedVersion { line_number: 1, .. }) => {
                let rendered = error.to_string();
                assert!(
                    !rendered.contains(&hostile),
                    "the refusal must not echo remote input: {} bytes rendered",
                    rendered.len()
                );
                assert!(
                    rendered.len() < 512,
                    "the refusal must stay bounded; got {} bytes",
                    rendered.len()
                );
                Ok(())
            }
            other => Err(format!("expected a MalformedVersion refusal, got {other:?}").into()),
        }
    }
}
