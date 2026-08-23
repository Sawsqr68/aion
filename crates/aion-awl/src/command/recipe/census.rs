//! The reference census: what a recipe reaches for outside itself, and the
//! measurement that must agree before anything fires.
//!
//! # The defect this closes
//!
//! Five stale scripts sat beside one load-bearing one, and nothing said
//! which was which until a flight fired the wrong one. The cure is not
//! vigilance: every artifact a recipe references outside itself is DECLARED
//! with its content hash, and the fire step measures the deployed bytes
//! against that census before it fires. A digest that no longer matches is a
//! refusal with both hashes in it, not a warning nobody reads.
//!
//! # Why the reader is a port
//!
//! [`ArtifactSource`] is a trait, not a filesystem call. Declaration, check
//! and projection stay in the language front, which opens no file and spawns
//! no process; the executing verb in `aion-cli` supplies the real reader.
//! That is the item-5 seam kept honest at the type level rather than by
//! convention — and it is also what lets every census test in this crate run
//! against bytes it states, so a red is a fact about the census and never
//! about a temporary directory.

use sha2::{Digest, Sha256};

use super::super::cause::CommandCause;
use super::super::error::CommandRefusal;
use super::decl::{ArtifactDecl, RecipeDecl, StepBody};

/// Why an address could not be read.
///
/// A typed error rather than a string so the executing side cannot report a
/// missing file and an unreadable one as the same thing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SourceUnreadable {
    /// Nothing stands at the address.
    #[error("nothing stands at `{address}`")]
    Absent {
        /// The address that named nothing.
        address: String,
    },
    /// Something stands there and could not be read.
    #[error("`{address}` could not be read: {detail}")]
    Refused {
        /// The address.
        address: String,
        /// What the reading side said, verbatim.
        detail: String,
    },
}

/// The bytes an address currently names.
///
/// The one seam between the language front and anything that touches a tree.
pub trait ArtifactSource {
    /// Read the bytes at `address`, as they stand right now.
    ///
    /// # Errors
    ///
    /// Returns [`SourceUnreadable`] when the address names nothing, or names
    /// something the reader could not read.
    fn read(&self, address: &str) -> Result<Vec<u8>, SourceUnreadable>;
}

/// The content digest of some bytes, in the estate's one textual form: 64
/// lowercase hexadecimal characters.
///
/// The algorithm is SHA-256 because that is the algorithm this workspace
/// already names for content identity (`aion_package::hash`, the `.aion`
/// format constraint). Minting a second one here would give the estate two
/// answers to "what do these bytes hash to".
#[must_use]
pub fn digest_of(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut text = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(text, "{byte:02x}");
    }
    text
}

/// One row of a measured census: what was declared, and what was there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CensusRow {
    /// The artifact's declared name.
    pub artifact: String,
    /// The address measured.
    pub address: String,
    /// The digest the declaration carries, if it carries one.
    pub declared: Option<String>,
    /// The digest the bytes at the address actually hash to.
    pub observed: String,
}

impl CensusRow {
    /// Whether the row agrees: a declared digest that the bytes match.
    ///
    /// A row with NO declared digest does not agree. Absence is a legal
    /// declared state for an artifact nobody fires; it is not one for an
    /// artifact a fire step reaches, because an unmeasurable reference is
    /// exactly the stale-script family this census exists to close.
    #[must_use]
    pub fn agrees(&self) -> bool {
        self.declared.as_deref() == Some(self.observed.as_str())
    }
}

/// Every artifact a recipe references outside itself, in the order its steps
/// reach for them, each named once.
#[must_use]
pub fn referenced_artifacts<'a>(
    recipe: &RecipeDecl,
    artifacts: &'a [ArtifactDecl],
) -> Vec<&'a ArtifactDecl> {
    let mut referenced: Vec<&ArtifactDecl> = Vec::new();
    for step in &recipe.steps {
        let name = match &step.body {
            Some(
                StepBody::FiresArtifact { artifact, .. }
                | StepBody::CapturesArtifact { artifact, .. },
            ) => artifact,
            Some(StepBody::RunsCommand { .. }) | None => continue,
        };
        let Some(declared) = artifacts.iter().find(|item| item.name == *name) else {
            continue;
        };
        if !referenced.iter().any(|seen| seen.name == declared.name) {
            referenced.push(declared);
        }
    }
    referenced
}

/// Measure the recipe's whole reference census against the tree the source
/// reads, and refuse the fire on the first row that disagrees.
///
/// The measured rows are returned WITH the refusal rather than instead of
/// it: a receipt must be able to say what it measured even when the answer
/// is what stopped the fire.
///
/// # Errors
///
/// Returns [`CommandCause::ArtifactUnreadable`] when a row could not be
/// measured at all and [`CommandCause::CensusMismatch`] when a row's
/// declared and observed digests disagree.
pub fn measure(
    recipe: &RecipeDecl,
    artifacts: &[ArtifactDecl],
    source: &dyn ArtifactSource,
) -> (Vec<CensusRow>, Option<CommandRefusal>) {
    let mut rows = Vec::new();
    let mut refusal = None;
    for artifact in referenced_artifacts(recipe, artifacts) {
        let bytes = match source.read(&artifact.address) {
            Ok(bytes) => bytes,
            Err(unreadable) => {
                if refusal.is_none() {
                    refusal = Some(CommandRefusal::refuse(
                        CommandCause::ArtifactUnreadable,
                        artifact.address_span,
                        &artifact.address,
                        format!(
                            "recipe `{}` references artifact `{}` and its bytes could not be \
                             measured: {unreadable}. The census is measured BEFORE the fire \
                             precisely so an unmeasurable reference stops the flight instead of \
                             the flight discovering it",
                            recipe.name, artifact.name
                        ),
                    ));
                }
                continue;
            }
        };
        let observed = digest_of(&bytes);
        let row = CensusRow {
            artifact: artifact.name.clone(),
            address: artifact.address.clone(),
            declared: artifact.digest.as_ref().map(|digest| digest.text.clone()),
            observed,
        };
        if !row.agrees() && refusal.is_none() {
            refusal = Some(mismatch(recipe, artifact, &row));
        }
        rows.push(row);
    }
    (rows, refusal)
}

fn mismatch(recipe: &RecipeDecl, artifact: &ArtifactDecl, row: &CensusRow) -> CommandRefusal {
    let (span, observed_bytes, said) = artifact.digest.as_ref().map_or_else(
        || {
            (
                artifact.span,
                artifact.name.clone(),
                "declares no digest at all".to_owned(),
            )
        },
        |digest| {
            (
                digest.span,
                digest.text.clone(),
                format!("declares `{}`", digest.text),
            )
        },
    );
    CommandRefusal::refuse(
        CommandCause::CensusMismatch,
        span,
        observed_bytes,
        format!(
            "recipe `{}` fires against artifact `{}` at `{}`, which {said} while its deployed \
             bytes hash to `{}`. Declared and observed disagree, so the fire measures a tree that \
             is not the tree the declaration was written against — five stale scripts beside one \
             load-bearing one is how this class is paid for",
            recipe.name, artifact.name, artifact.address, row.observed
        ),
    )
}
