//! Strict Semantic Versioning 2.0.0 parsing and precedence, for comparing
//! published crate versions.
//!
//! # Why hand-carried rather than a dependency
//!
//! The workspace carries no semver crate, and adding one for the update check
//! would put a new supply-chain member into the default build of a binary
//! whose default build deliberately keeps dependencies minimal. The grammar
//! and precedence rules below are the published SemVer 2.0.0 specification —
//! items 9–11 — implemented and pinned by tests, including the prerelease
//! rules crates.io versions rarely exercise, **with exactly one deliberate
//! bound**: a numeric identifier larger than `u64::MAX` (the spec's grammar
//! places no bound) is REFUSED rather than parsed. The divergence is in the
//! refuse direction — no input can be mis-ranked by it, only rejected loudly
//! — and it is what keeps every comparison exact instead of floating-point
//! approximate. (Differentially audited against the crates.io `semver` crate
//! in review; the one acceptance divergence is this bound, and on
//! build-metadata ordering THIS implementation is the spec-correct side.)
//!
//! # Refusal over guessing — and refusals echo nothing they read
//!
//! Anything that is not a spec-exact version string is a typed
//! [`SemVerError`], never a best-effort read. The caller is comparing "what is
//! installed" against "what is published"; a guessed comparison is worse than
//! a loud refusal, because it can point an operator at a downgrade.
//!
//! Version strings on the update-check path come from a fetched index body —
//! remote input — so every [`SemVerError`] is built from bounded,
//! statically-known reason strings and NEVER interpolates the text it
//! refused. The caller that knows a safe context for the offending value
//! (a unit test, a bounded local string) can print it itself; the error will
//! not smuggle a many-megabyte remote line into a log sink.

use std::cmp::Ordering;
use std::fmt;

/// One dot-separated prerelease identifier, already classified.
///
/// The spec compares numeric identifiers numerically and alphanumeric ones
/// ASCII-lexically, with every numeric identifier ranking below every
/// alphanumeric one — so the classification is part of the value, not a
/// comparison-time re-parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrereleaseIdent {
    /// Digits only, no leading zero. Compared numerically.
    Numeric(u64),
    /// Contains at least one non-digit. Compared by ASCII byte order.
    Alphanumeric(String),
}

impl PrereleaseIdent {
    /// Spec item 11.4: numeric < alphanumeric; numeric vs numeric compares the
    /// values; alphanumeric vs alphanumeric compares ASCII bytes.
    fn precedence(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Numeric(left), Self::Numeric(right)) => left.cmp(right),
            (Self::Numeric(_), Self::Alphanumeric(_)) => Ordering::Less,
            (Self::Alphanumeric(_), Self::Numeric(_)) => Ordering::Greater,
            (Self::Alphanumeric(left), Self::Alphanumeric(right)) => left.cmp(right),
        }
    }
}

impl fmt::Display for PrereleaseIdent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Numeric(value) => write!(formatter, "{value}"),
            Self::Alphanumeric(text) => formatter.write_str(text),
        }
    }
}

/// A parsed `SemVer` 2.0.0 version.
///
/// # Equality and ordering are PRECEDENCE, not identity
///
/// The spec (item 10) excludes build metadata from precedence, so `Eq`,
/// `Ord`, and friends ignore [`Self::build`] — `1.0.0+a` and `1.0.0+b`
/// compare equal, which is what keeps `Ord` lawful (consistent with `Eq`).
/// [`fmt::Display`] renders the full original form, build metadata included.
#[derive(Debug, Clone)]
pub struct SemVer {
    /// Major version.
    pub major: u64,
    /// Minor version.
    pub minor: u64,
    /// Patch version.
    pub patch: u64,
    /// Prerelease identifiers, in order. Empty means a release version, which
    /// ranks ABOVE any prerelease of the same version core (spec item 11.3).
    pub prerelease: Vec<PrereleaseIdent>,
    /// Build metadata, verbatim (without the leading `+`), retained for
    /// display only. `None` when the version carries none.
    pub build: Option<String>,
}

impl SemVer {
    /// Parse a spec-exact version string.
    ///
    /// # Errors
    ///
    /// Returns [`SemVerError`] naming exactly what refused: a missing core
    /// part, a leading zero, an empty or malformed identifier, or a numeric
    /// overflow.
    pub fn parse(text: &str) -> Result<Self, SemVerError> {
        let (rest, build) = match text.split_once('+') {
            Some((rest, build)) => (rest, Some(build)),
            None => (text, None),
        };
        let (core, prerelease) = match rest.split_once('-') {
            Some((core, prerelease)) => (core, Some(prerelease)),
            None => (rest, None),
        };

        let mut parts = core.split('.');
        let major = core_part(parts.next(), "major")?;
        let minor = core_part(parts.next(), "minor")?;
        let patch = core_part(parts.next(), "patch")?;
        if parts.next().is_some() {
            return Err(SemVerError::ExtraCorePart);
        }

        let prerelease = match prerelease {
            Some(idents) => parse_prerelease(idents)?,
            None => Vec::new(),
        };
        let build = match build {
            Some(metadata) => Some(parse_build(metadata)?),
            None => None,
        };

        Ok(Self {
            major,
            minor,
            patch,
            prerelease,
            build,
        })
    }
}

impl fmt::Display for SemVer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)?;
        for (position, ident) in self.prerelease.iter().enumerate() {
            let separator = if position == 0 { '-' } else { '.' };
            write!(formatter, "{separator}{ident}")?;
        }
        if let Some(build) = &self.build {
            write!(formatter, "+{build}")?;
        }
        Ok(())
    }
}

impl PartialEq for SemVer {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for SemVer {}

impl PartialOrd for SemVer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SemVer {
    fn cmp(&self, other: &Self) -> Ordering {
        self.major
            .cmp(&other.major)
            .then_with(|| self.minor.cmp(&other.minor))
            .then_with(|| self.patch.cmp(&other.patch))
            .then_with(|| prerelease_precedence(&self.prerelease, &other.prerelease))
    }
}

/// Spec item 11.3/11.4: a release outranks every prerelease of the same core;
/// otherwise identifiers compare pairwise and, when one list is a prefix of
/// the other, the longer list ranks higher.
fn prerelease_precedence(left: &[PrereleaseIdent], right: &[PrereleaseIdent]) -> Ordering {
    match (left.is_empty(), right.is_empty()) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Greater,
        (false, true) => Ordering::Less,
        (false, false) => {
            for (this, that) in left.iter().zip(right.iter()) {
                let ordering = this.precedence(that);
                if ordering != Ordering::Equal {
                    return ordering;
                }
            }
            left.len().cmp(&right.len())
        }
    }
}

/// A refusal to read a version string, naming what refused.
///
/// Every message is assembled from statically-known strings — the refused
/// text is remote input on this module's one production path (the fetched
/// crate index) and is deliberately never carried or echoed. Boundedness is
/// structural: the `reason` fields are `&'static str`, so no arm CAN
/// interpolate input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SemVerError {
    /// The dotted version core has fewer than three parts.
    #[error("the version is missing its {part} part")]
    MissingCorePart {
        /// Which of major/minor/patch was absent.
        part: &'static str,
    },
    /// The dotted version core has more than three parts.
    #[error("the version has more than three dotted version-core parts")]
    ExtraCorePart,
    /// A version-core part is empty, non-numeric, zero-padded, or over the
    /// deliberate 64-bit bound.
    #[error("the {part} part is not an accepted numeric identifier: {reason}")]
    MalformedCorePart {
        /// Which of major/minor/patch refused.
        part: &'static str,
        /// What about it refused (statically known, never input text).
        reason: &'static str,
    },
    /// A prerelease identifier is empty, carries a character outside
    /// `[0-9A-Za-z-]`, is a zero-padded number, or is a number over the
    /// deliberate 64-bit bound.
    #[error("the prerelease is malformed: {reason}")]
    MalformedPrerelease {
        /// What about it refused (statically known, never input text).
        reason: &'static str,
    },
    /// A build-metadata identifier is empty or carries a character outside
    /// `[0-9A-Za-z-]`.
    #[error("the build metadata is malformed: {reason}")]
    MalformedBuild {
        /// What about it refused (statically known, never input text).
        reason: &'static str,
    },
}

/// Read one major/minor/patch part: digits only, no leading zero, within
/// the deliberate `u64` bound.
fn core_part(part: Option<&str>, name: &'static str) -> Result<u64, SemVerError> {
    let Some(part) = part else {
        return Err(SemVerError::MissingCorePart { part: name });
    };
    numeric_identifier(part).map_err(|reason| SemVerError::MalformedCorePart { part: name, reason })
}

/// A spec numeric identifier: non-empty, ASCII digits, no leading zero,
/// bounded at `u64::MAX` (the module-level deliberate bound).
fn numeric_identifier(text: &str) -> Result<u64, &'static str> {
    if text.is_empty() {
        return Err("it is empty");
    }
    if !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("it contains a non-digit");
    }
    if text.len() > 1 && text.starts_with('0') {
        return Err("it has a leading zero");
    }
    text.parse::<u64>()
        .map_err(|_| "it overflows the deliberate 64-bit bound")
}

/// Parse the dot-separated prerelease identifiers after `-`.
fn parse_prerelease(idents: &str) -> Result<Vec<PrereleaseIdent>, SemVerError> {
    idents.split('.').map(prerelease_identifier).collect()
}

/// Classify one prerelease identifier per the spec grammar.
fn prerelease_identifier(ident: &str) -> Result<PrereleaseIdent, SemVerError> {
    if ident.is_empty() {
        return Err(SemVerError::MalformedPrerelease {
            reason: "an identifier is empty",
        });
    }
    if !ident
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(SemVerError::MalformedPrerelease {
            reason: "an identifier contains a character outside [0-9A-Za-z-]",
        });
    }
    if ident.bytes().all(|byte| byte.is_ascii_digit()) {
        return numeric_identifier(ident)
            .map(PrereleaseIdent::Numeric)
            .map_err(|reason| SemVerError::MalformedPrerelease { reason });
    }
    Ok(PrereleaseIdent::Alphanumeric(ident.to_owned()))
}

/// Validate the build metadata after `+`; leading zeros are legal here.
fn parse_build(metadata: &str) -> Result<String, SemVerError> {
    for ident in metadata.split('.') {
        if ident.is_empty() {
            return Err(SemVerError::MalformedBuild {
                reason: "an identifier is empty",
            });
        }
        if !ident
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err(SemVerError::MalformedBuild {
                reason: "an identifier contains a character outside [0-9A-Za-z-]",
            });
        }
    }
    Ok(metadata.to_owned())
}

#[cfg(test)]
mod tests {
    use super::{PrereleaseIdent, SemVer, SemVerError};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    /// A predicate over the refusal a malformed version must produce.
    type ExpectedRefusal = fn(&SemVerError) -> bool;

    /// The spec's own worked precedence chain (items 11.3/11.4), in ascending
    /// order. Every adjacent pair must rank strictly, so a single mis-rule in
    /// numeric-vs-alphanumeric, prefix length, or release-over-prerelease
    /// breaks a specific pair by name.
    #[test]
    fn the_spec_precedence_chain_ranks_strictly_ascending() -> TestResult {
        let chain = [
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0-alpha.beta",
            "1.0.0-beta",
            "1.0.0-beta.2",
            "1.0.0-beta.11",
            "1.0.0-rc.1",
            "1.0.0",
            "2.0.0",
            "2.1.0",
            "2.1.1",
        ];
        for pair in chain.windows(2) {
            let lower = SemVer::parse(pair[0])?;
            let higher = SemVer::parse(pair[1])?;
            assert!(
                lower < higher,
                "`{}` must rank strictly below `{}`",
                pair[0],
                pair[1]
            );
        }
        Ok(())
    }

    /// Build metadata is display-only: it never separates precedence (spec
    /// item 10), and it round-trips through `Display`.
    #[test]
    fn build_metadata_is_ignored_for_precedence_and_kept_for_display() -> TestResult {
        let bare = SemVer::parse("1.0.0")?;
        let stamped = SemVer::parse("1.0.0+20130313144700")?;
        assert_eq!(bare, stamped);
        assert_eq!(stamped.to_string(), "1.0.0+20130313144700");
        Ok(())
    }

    #[test]
    fn a_full_form_round_trips_through_display() -> TestResult {
        let text = "1.2.3-rc.1.x-y+exp.sha.5114f85";
        assert_eq!(SemVer::parse(text)?.to_string(), text);
        Ok(())
    }

    #[test]
    fn numeric_prerelease_identifiers_compare_numerically_not_lexically() -> TestResult {
        // Lexically "11" < "2"; the spec compares numerically.
        assert!(SemVer::parse("1.0.0-beta.2")? < SemVer::parse("1.0.0-beta.11")?);
        Ok(())
    }

    #[test]
    fn classification_is_part_of_the_parse() -> TestResult {
        let parsed = SemVer::parse("1.0.0-alpha.7.0a")?;
        assert_eq!(
            parsed.prerelease,
            vec![
                PrereleaseIdent::Alphanumeric("alpha".to_owned()),
                PrereleaseIdent::Numeric(7),
                PrereleaseIdent::Alphanumeric("0a".to_owned()),
            ]
        );
        Ok(())
    }

    /// Every malformation the grammar names is refused with its own reason —
    /// none of these may fall through to a best-effort read.
    #[test]
    fn malformed_versions_refuse_by_name() -> TestResult {
        let refusals: [(&str, ExpectedRefusal); 10] = [
            ("1.0", |error| {
                matches!(error, SemVerError::MissingCorePart { part: "patch" })
            }),
            ("1.2.3.4", |error| {
                matches!(error, SemVerError::ExtraCorePart)
            }),
            ("01.2.3", |error| {
                matches!(error, SemVerError::MalformedCorePart { part: "major", .. })
            }),
            ("1.2.x", |error| {
                matches!(error, SemVerError::MalformedCorePart { part: "patch", .. })
            }),
            ("v1.2.3", |error| {
                matches!(error, SemVerError::MalformedCorePart { part: "major", .. })
            }),
            // The deliberate 64-bit bound: spec-legal, refused by name (the
            // one documented refuse-direction divergence from the grammar).
            ("18446744073709551616.0.0", |error| {
                matches!(
                    error,
                    SemVerError::MalformedCorePart {
                        part: "major",
                        reason: "it overflows the deliberate 64-bit bound",
                    }
                )
            }),
            ("1.2.3-", |error| {
                matches!(error, SemVerError::MalformedPrerelease { .. })
            }),
            ("1.2.3-rc..1", |error| {
                matches!(error, SemVerError::MalformedPrerelease { .. })
            }),
            ("1.2.3-01", |error| {
                matches!(error, SemVerError::MalformedPrerelease { .. })
            }),
            ("1.2.3+a_b", |error| {
                matches!(error, SemVerError::MalformedBuild { .. })
            }),
        ];
        for (text, expected) in refusals {
            match SemVer::parse(text) {
                Ok(parsed) => {
                    return Err(format!("`{text}` must refuse, parsed as `{parsed}`").into());
                }
                Err(error) => {
                    assert!(
                        expected(&error),
                        "`{text}` refused with the wrong arm: {error}"
                    );
                }
            }
        }
        Ok(())
    }

    /// A whitespace-padded or empty string refuses rather than being trimmed
    /// into an answer.
    #[test]
    fn padding_is_not_forgiven() {
        assert!(SemVer::parse(" 1.2.3").is_err());
        assert!(SemVer::parse("1.2.3 ").is_err());
        assert!(SemVer::parse("").is_err());
    }

    /// A refusal never echoes the text it refused: the production input is a
    /// fetched index body, and an error that interpolated it would multiply
    /// remote bytes into whatever sink renders the error. Boundedness is
    /// structural (`reason: &'static str`), and this pins the rendering.
    #[test]
    fn refusals_never_echo_the_refused_text() -> TestResult {
        let hostile_digits = "9".repeat(4096);
        let hostile_ident = "a".repeat(4096);
        for text in [
            format!("{hostile_digits}.0.0"),
            format!("1.0.0-{hostile_ident}!"),
            format!("1.0.0+{hostile_ident}_"),
            format!("x{hostile_ident}.2.3"),
        ] {
            let Err(error) = SemVer::parse(&text) else {
                return Err(format!("a hostile {}-byte version must refuse", text.len()).into());
            };
            let rendered = error.to_string();
            assert!(
                rendered.len() < 256,
                "a refusal must stay bounded; got {} bytes",
                rendered.len()
            );
            assert!(
                !rendered.contains(&hostile_digits) && !rendered.contains(&hostile_ident),
                "a refusal must not echo the refused text"
            );
        }
        Ok(())
    }
}
