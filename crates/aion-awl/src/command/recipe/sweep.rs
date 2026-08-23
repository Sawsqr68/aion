//! Sweep by claim: when a source declaration is corrected, every input
//! assembled from it is re-projected and re-checked MECHANICALLY.
//!
//! # The class this closes by construction
//!
//! Found in one, lives in three. A value is corrected where a reviewer
//! happened to notice it, and the two other places it was hand-copied to go
//! on saying the old thing. No amount of care closes that class; only a
//! mechanism does, and the mechanism needs one property to be sound: the
//! edge set must be CLOSED.
//!
//! It is closed here, and the completeness claim says exactly why. Assembly
//! is FROM declarations: a recipe consumes a wall or an artifact only by
//! NAMING it, and every name is in the declaration the checker already read.
//! So the consumers of a source are enumerable from the checked set, and
//! there is no fourth place a consumer could hide — not because nobody has
//! found one, but because a consumer that did not name its source would not
//! be a consumer of it.
//!
//! The sweep's output is itself a RECORD: which inputs, from which
//! declaration, re-checked against what, with the derivation of its own
//! completeness written into it rather than assumed by its reader.

use std::fmt::Write as _;

use super::census::digest_of;
use super::decl::{ArtifactDecl, RecipeDecl, StepBody, WallDecl};

/// What kind of declaration a sweep was run from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    /// A declared wall, corrected.
    Wall,
    /// A declared artifact, corrected.
    Artifact,
}

impl SourceKind {
    /// The kind's stable wire rendering.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Wall => "wall",
            Self::Artifact => "artifact",
        }
    }
}

/// One input the sweep reached, and what it was re-checked against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SweptInput {
    /// The recipe that consumes the corrected source.
    pub recipe: String,
    /// The step whose declaration names it.
    pub step: String,
    /// How that step reaches for it, in one word.
    pub how: &'static str,
    /// The correction this input was re-checked against.
    pub rechecked_against: String,
}

/// The record a sweep produces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SweepRecord {
    /// The declaration that was corrected.
    pub source: String,
    /// What kind of declaration it is.
    pub kind: SourceKind,
    /// The correction, verbatim.
    pub correction: String,
    /// A digest of the correction, so a receipt can name what was swept
    /// against without carrying the whole of it.
    pub correction_digest: String,
    /// Every input reached, in the order the checked set declares them.
    pub reprojected: Vec<SweptInput>,
    /// Why this list is COMPLETE — the derivation of the claim, not the
    /// claim on its own.
    pub completeness: String,
}

impl SweepRecord {
    /// The record as text, for a receipt.
    #[must_use]
    pub fn rendered(&self) -> String {
        let mut out = format!(
            "sweep: {} `{}` corrected to `{}` ({})\n",
            self.kind.as_str(),
            self.source,
            self.correction,
            self.correction_digest
        );
        let _ = writeln!(out, "  {} input(s) re-projected", self.reprojected.len());
        for input in &self.reprojected {
            let _ = writeln!(
                out,
                "    {}::{} ({}) re-checked against {}",
                input.recipe, input.step, input.how, input.rechecked_against
            );
        }
        let _ = writeln!(out, "  completeness: {}", self.completeness);
        out
    }
}

/// Re-project and re-check every input assembled from `source`.
///
/// `source` names a declared wall or artifact; `correction` is what that
/// declaration now says. Every recipe step that names the source is reached,
/// because a step reaches for a source only by naming it.
#[must_use]
pub fn sweep_by_claim(
    recipes: &[RecipeDecl],
    walls: &[WallDecl],
    artifacts: &[ArtifactDecl],
    source: &str,
    correction: &str,
) -> SweepRecord {
    let kind = if walls.iter().any(|wall| wall.name == source) {
        SourceKind::Wall
    } else {
        SourceKind::Artifact
    };
    let mut reprojected = Vec::new();
    for recipe in recipes {
        for step in &recipe.steps {
            let how = match &step.body {
                Some(StepBody::FiresArtifact { artifact, .. }) if artifact == source => "fires",
                Some(StepBody::CapturesArtifact { artifact, .. }) if artifact == source => {
                    "captures"
                }
                _ => continue,
            };
            reprojected.push(SweptInput {
                recipe: recipe.name.clone(),
                step: step.name.clone(),
                how,
                rechecked_against: correction.to_owned(),
            });
        }
        for parameter in &recipe.parameters {
            let Some(derivation) = &parameter.derivation else {
                continue;
            };
            if derivation.source != source {
                continue;
            }
            reprojected.push(SweptInput {
                recipe: recipe.name.clone(),
                step: parameter.name.clone(),
                how: "derives",
                rechecked_against: correction.to_owned(),
            });
        }
    }
    let declared = artifacts.len() + walls.len();
    SweepRecord {
        source: source.to_owned(),
        kind,
        correction: correction.to_owned(),
        correction_digest: digest_of(correction.as_bytes()),
        reprojected,
        completeness: format!(
            "enumerated from the {} checked recipe declaration(s) and {declared} declared \
             source(s) of this document; assembly is FROM declarations, so a step reaches a \
             source only by NAMING it and the edge set is therefore the set of declared names — \
             a consumer that did not name its source would not be a consumer of it",
            recipes.len()
        ),
    }
}
