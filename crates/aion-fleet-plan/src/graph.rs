//! The normalized typed graph — the run's ordering authority.
//!
//! Design §5.3, final ¶, verbatim: *"The normalized typed graph returned by
//! successful `check_plan`, not a later parse of the working tree, is the
//! run's ordering authority."* Every consumer in this crate — the chain
//! decision function included — reads ordering from a [`NormalizedGraph`]
//! and never re-parses plan text.

use serde::{Deserialize, Serialize};

use crate::ids::conclusion_path;

/// `advance` policy for a stage (design §2.1, §5.2 clause 16).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Advance {
    /// Dispatch-time preauthorization to accept a clear, green, verified
    /// stage result as the next base.
    Auto,
    /// An explicit human approval/adaptation seam. The conclusion is
    /// verified and recorded BEFORE the hold is offered or released
    /// (design §2.6) — no hold can approve a stage whose conclusion has not
    /// verified.
    Hold,
}

/// The comparison an `admit` condition makes.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Comparison {
    /// `==`
    Equals,
    /// `!=`
    NotEquals,
}

impl Comparison {
    /// The canonical operator text, as authored.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Equals => "==",
            Self::NotEquals => "!=",
        }
    }
}

/// A stage's admission condition (design §2.1).
///
/// Free-form report text is never a branch condition (design §4). The only
/// admissible shapes are `always` and a comparison against an EARLIER
/// stage's recorded conclusion.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Admit {
    /// `admit=always`
    Always,
    /// `admit=<stage-id>.conclusion==<value>` or `!=`
    Conclusion {
        /// The earlier stage whose recorded conclusion is compared.
        stage_id: String,
        /// Which comparison.
        comparison: Comparison,
        /// The value compared against, which must belong to that stage's
        /// declared closed vocabulary (§5.2 clause 14).
        value: String,
    },
}

impl Admit {
    /// The canonical authored text of this condition.
    ///
    /// A skip record must name the condition (design §4), and it names it
    /// in this canonical form rather than by quoting the plan line, so the
    /// record is a function of the normalized graph.
    #[must_use]
    pub fn canonical(&self) -> String {
        match self {
            Self::Always => "always".to_owned(),
            Self::Conclusion {
                stage_id,
                comparison,
                value,
            } => format!("{stage_id}.conclusion{}{value}", comparison.as_str()),
        }
    }
}

/// One normalized stage.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Stage {
    /// The stage id. Charset-closed; interpolates into `conclusion_path`.
    pub id: String,
    /// The dispatch-fixed leg ids that run in parallel inside this stage.
    pub legs: Vec<String>,
    /// The predecessor stage id; `None` for the single root.
    pub after: Option<String>,
    /// The admission condition.
    pub admit: Admit,
    /// The stage's closed conclusion vocabulary.
    pub conclusions: Vec<String>,
    /// The one leg responsible for committing the conclusion token.
    pub conclusion_leg: String,
    /// The advance policy.
    pub advance: Advance,
}

impl Stage {
    /// The canonical conclusion path this stage's `conclusion_leg` must
    /// commit its single declared token at (design §2.1, §5.1).
    #[must_use]
    pub fn conclusion_path(&self) -> String {
        conclusion_path(&self.id)
    }

    /// Is `value` a member of this stage's declared closed vocabulary?
    #[must_use]
    pub fn declares(&self, value: &str) -> bool {
        self.conclusions.iter().any(|v| v == value)
    }
}

/// The normalized typed graph returned by a successful `check_plan`.
///
/// `order` is the single chain in execution order, root first. Because v1
/// admits exactly one root, at most one successor per stage, and exactly one
/// terminal (design §2.1, §5.2 clause 8), that order is total and
/// unambiguous — which is what lets the chain decision function be a pure
/// function of this graph plus recorded history.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NormalizedGraph {
    /// The block version this graph was normalized from.
    pub version: String,
    /// The stages in chain order, root first.
    pub order: Vec<Stage>,
}

impl NormalizedGraph {
    /// The root stage — the first in chain order.
    #[must_use]
    pub fn root(&self) -> Option<&Stage> {
        self.order.first()
    }

    /// Look a stage up by id.
    #[must_use]
    pub fn stage(&self, id: &str) -> Option<&Stage> {
        self.order.iter().find(|s| s.id == id)
    }

    /// The stage that follows `id` in chain order, if any.
    #[must_use]
    pub fn successor(&self, id: &str) -> Option<&Stage> {
        let at = self.order.iter().position(|s| s.id == id)?;
        self.order.get(at + 1)
    }

    /// The chain position of `id`, root being 0.
    #[must_use]
    pub fn position(&self, id: &str) -> Option<usize> {
        self.order.iter().position(|s| s.id == id)
    }

    /// How many stages the chain declares.
    #[must_use]
    pub fn len(&self) -> usize {
        self.order.len()
    }

    /// Does the chain declare no stage at all?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }
}
