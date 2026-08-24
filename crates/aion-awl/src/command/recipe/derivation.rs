//! The closed extraction grammar, and resolution AT THE FIRE MINUTE.
//!
//! # Why the set is closed and small
//!
//! An extraction is what a derivation takes out of the bytes its source
//! address names. The set is exactly two forms, and widening it is a design
//! change rather than an implementation decision — the same discipline
//! cut-1's derived-shape set carries, for the same reason: an extraction
//! language would be an evaluator, and an evaluator in a declaration is the
//! thing the determinism boundary exists to prevent.
//!
//! # Why resolution happens at FIRE and not at COMPOSE
//!
//! The law is the corpus's own, verbatim from
//! `.fleet/fixtures/corpus/issue-investigations.README.md` at `b1db6ae3…`:
//! read the literal "at the dispatch minute (not the compose minute — they
//! drift)". A value resolved at compose is a value that was true when the
//! composer ran; a value resolved at fire is a value that is true when the
//! thing runs. Making the second the only path is what turns compose/fire
//! drift from a thing to check for into a thing that cannot happen.

use super::super::cause::CommandCause;
use super::super::error::CommandRefusal;
use super::census::ArtifactSource;
use super::decl::Derivation;

/// What a derivation takes out of its source's bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Extraction {
    /// `line <n>` — the nth line, one-based, without its terminator.
    Line(usize),
    /// `whole` — every byte, verbatim.
    Whole,
}

/// The extraction a declared string names, or the admitted set to print.
///
/// # Errors
///
/// Returns the admitted set, rendered, when the string names no extraction
/// this surface performs. An unknown extraction is never a nearest match.
pub fn parse_extraction(declared: &str) -> Result<Extraction, String> {
    let trimmed = declared.trim();
    if trimmed == "whole" {
        return Ok(Extraction::Whole);
    }
    if let Some(number) = trimmed.strip_prefix("line ")
        && let Ok(line) = number.trim().parse::<usize>()
        && line > 0
    {
        return Ok(Extraction::Line(line));
    }
    Err("`line <n>` (n counted from 1) or `whole`".to_owned())
}

/// Read the derivation's source and take its extraction out, AT FIRE TIME.
///
/// # Errors
///
/// Returns a fire-layer refusal when the source cannot be read, and a
/// check-layer refusal when the extraction names nothing this surface
/// performs or reaches past the end of the source.
pub fn resolve(
    derivation: &Derivation,
    parameter: &str,
    source: &dyn ArtifactSource,
) -> Result<String, CommandRefusal> {
    let extraction = parse_extraction(&derivation.extraction).map_err(|admitted| {
        CommandRefusal::refuse(
            CommandCause::ExtractionUnknown,
            derivation.extraction_span,
            &derivation.extraction,
            format!(
                "parameter `{parameter}`'s extraction `{}` is not one this surface performs — \
                 {admitted}",
                derivation.extraction
            ),
        )
    })?;
    let bytes = source.read(&derivation.source).map_err(|unreadable| {
        CommandRefusal::refuse(
            CommandCause::ArtifactUnreadable,
            derivation.source_span,
            &derivation.source,
            format!(
                "parameter `{parameter}` is re-derived from `{}`, which could not be read at the \
                 fire minute: {unreadable}. A derivation whose source cannot be read has no value \
                 to supply, and firing on the last value it had is exactly the drift the \
                 fire-minute law removes",
                derivation.source
            ),
        )
    })?;
    let text = String::from_utf8_lossy(&bytes);
    match extraction {
        Extraction::Whole => Ok(text.into_owned()),
        Extraction::Line(line) => text
            .lines()
            .nth(line - 1)
            .map(str::to_owned)
            .ok_or_else(|| {
                CommandRefusal::refuse(
                    CommandCause::ExtractionUnknown,
                    derivation.extraction_span,
                    &derivation.extraction,
                    format!(
                        "parameter `{parameter}` is re-derived from `{}` at line {line}, and the \
                     source carries {} line(s). An extraction past the end of its source names \
                     nothing, and a derivation that names nothing must refuse rather than supply \
                     an empty value that reads like a real one",
                        derivation.source,
                        text.lines().count()
                    ),
                )
            }),
    }
}
