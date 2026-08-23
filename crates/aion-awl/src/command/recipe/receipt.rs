//! Receipts: GENERATED records of a recipe run, never typed prose.
//!
//! # What a receipt is, and what makes one impossible to fake
//!
//! A receipt asserts what IS, and it names WHAT IT MEASURED — the tree, its
//! head, and whether that tree was clean. Those three are mandatory AT
//! CONSTRUCTION: [`MeasuredTree::new`] refuses a receipt that cannot say
//! what it measured, so "a receipt with no tree" is not a value this
//! language can build. That is the close-round receipt law expressed as a
//! type rather than as a review note.
//!
//! # Receipt-first is preserved as law
//!
//! A refused stage still yields its receipt. The receipt is constructed
//! before the stage's outcome is known and carries the refusal as one of the
//! things it measured, because a stage that refuses and produces nothing is
//! a stage nobody can audit — which is the exact failure "receipt first, no
//! receipt no staging" was written against
//! (`.fleet/fixtures/corpus/speech1.STAGING.md` at `1150e466…`, step 1).
//!
//! # Re-read before claiming
//!
//! [`claim`] re-reads an object from the committed state before anything
//! claims it, and refuses when the bytes have moved. A claim about bytes
//! that changed between the write and the claim is a claim about a tree that
//! no longer exists.

use std::fmt::Write as _;

use crate::Span;

use super::super::cause::CommandCause;
use super::super::error::CommandRefusal;
use super::census::{ArtifactSource, CensusRow, digest_of};
use super::compare::StageComparison;

/// A receipt could not be built because it could not say what it measured.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReceiptError {
    /// The head the receipt measured was not stated.
    #[error(
        "a receipt was built without naming the head it measured; a receipt that cannot say which \
         tree it read is a claim about no tree at all"
    )]
    HeadUnstated,
    /// The tree the receipt measured was not stated.
    #[error(
        "a receipt at head `{head}` was built without naming the tree it measured; two trees can \
         share a head, and a receipt names the one it read"
    )]
    TreeUnstated {
        /// The head the caller did supply.
        head: String,
    },
    /// The recipe the receipt is a receipt OF was not named.
    #[error("a receipt was built naming no recipe; a receipt of nothing records nothing")]
    RecipeUnstated,
}

/// The tree a receipt measured. Every field mandatory at construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasuredTree {
    head: String,
    tree: String,
    clean: bool,
}

impl MeasuredTree {
    /// Name the tree this receipt measured.
    ///
    /// # Errors
    ///
    /// Returns [`ReceiptError`] when the head or the tree is empty. There is
    /// no default and no placeholder: a receipt that cannot say what it
    /// measured is unrepresentable rather than optimistic.
    pub fn new(
        head: impl Into<String>,
        tree: impl Into<String>,
        clean: bool,
    ) -> Result<Self, ReceiptError> {
        let head = head.into();
        if head.trim().is_empty() {
            return Err(ReceiptError::HeadUnstated);
        }
        let tree = tree.into();
        if tree.trim().is_empty() {
            return Err(ReceiptError::TreeUnstated { head });
        }
        Ok(Self { head, tree, clean })
    }

    /// The head measured.
    #[must_use]
    pub fn head(&self) -> &str {
        &self.head
    }

    /// The tree measured.
    #[must_use]
    pub fn tree(&self) -> &str {
        &self.tree
    }

    /// Whether that tree was clean when it was measured.
    #[must_use]
    pub const fn clean(&self) -> bool {
        self.clean
    }

    /// The tree line a receipt opens with.
    #[must_use]
    pub fn rendered(&self) -> String {
        format!(
            "tree {} at head {} ({})",
            self.tree,
            self.head,
            if self.clean { "CLEAN" } else { "DIRTY" }
        )
    }
}

/// What one step of a fired recipe did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepOutcome {
    /// The step's declared name.
    pub step: String,
    /// What the step did, said in one line.
    pub did: String,
    /// The typed comparison the step made, when it made one.
    pub comparison: Option<StageComparison>,
    /// The refusal that stopped this step, when one did.
    pub refused: Option<String>,
}

/// A generated record of a recipe run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Receipt {
    tree: MeasuredTree,
    recipe: String,
    census: Vec<CensusRow>,
    steps: Vec<StepOutcome>,
}

impl Receipt {
    /// Open a receipt for `recipe`, measured against `tree`.
    ///
    /// # Errors
    ///
    /// Returns [`ReceiptError::RecipeUnstated`] when the recipe is unnamed.
    pub fn new(tree: MeasuredTree, recipe: impl Into<String>) -> Result<Self, ReceiptError> {
        let recipe = recipe.into();
        if recipe.trim().is_empty() {
            return Err(ReceiptError::RecipeUnstated);
        }
        Ok(Self {
            tree,
            recipe,
            census: Vec::new(),
            steps: Vec::new(),
        })
    }

    /// Record the census this run measured.
    pub fn measured(&mut self, census: Vec<CensusRow>) {
        self.census = census;
    }

    /// Record what one step did.
    pub fn stepped(&mut self, outcome: StepOutcome) {
        self.steps.push(outcome);
    }

    /// The tree this receipt measured.
    #[must_use]
    pub const fn tree(&self) -> &MeasuredTree {
        &self.tree
    }

    /// The recipe this is a receipt of.
    #[must_use]
    pub fn recipe(&self) -> &str {
        &self.recipe
    }

    /// The census rows this run measured, in the order it measured them.
    #[must_use]
    pub fn census(&self) -> &[CensusRow] {
        &self.census
    }

    /// What each step did, in the order the run ran them.
    #[must_use]
    pub fn steps(&self) -> &[StepOutcome] {
        &self.steps
    }

    /// The receipt as text: what it measured, then what it found.
    ///
    /// Generated, never typed. Every line is a fact the run established, in
    /// the order the run established it.
    #[must_use]
    pub fn rendered(&self) -> String {
        let mut out = format!(
            "receipt: recipe {} — {}\n",
            self.recipe,
            self.tree.rendered()
        );
        let _ = writeln!(out, "census: {} artifact(s) measured", self.census.len());
        for row in &self.census {
            let _ = writeln!(
                out,
                "  {} at {} — declared {}, observed {} ({})",
                row.artifact,
                row.address,
                row.declared.as_deref().unwrap_or("<none>"),
                row.observed,
                if row.agrees() { "agrees" } else { "DISAGREES" }
            );
        }
        let _ = writeln!(out, "steps: {}", self.steps.len());
        for outcome in &self.steps {
            let _ = writeln!(out, "  {} — {}", outcome.step, outcome.did);
            if let Some(comparison) = &outcome.comparison {
                let _ = writeln!(
                    out,
                    "    compared: {} ({})",
                    comparison.rendered(),
                    if comparison.matched() {
                        "matched"
                    } else {
                        "MISMATCH"
                    }
                );
            }
            if let Some(refused) = &outcome.refused {
                let _ = writeln!(out, "    REFUSED: {refused}");
            }
        }
        out
    }
}

/// Re-read an object from the committed state before claiming it.
///
/// `fleet-fanin.sh`'s discipline, generalized: what a builder claims about
/// an object is read back from where the object now lives, and a claim about
/// bytes that moved between the write and the claim is refused rather than
/// made.
///
/// # Errors
///
/// Returns [`CommandCause::ArtifactUnreadable`] when the object cannot be
/// re-read and [`CommandCause::ObjectMovedBeforeClaim`] when the bytes
/// re-read are not the bytes written.
pub fn claim(
    source: &dyn ArtifactSource,
    address: &str,
    written: &[u8],
    span: Span,
) -> Result<String, CommandRefusal> {
    let reread = source.read(address).map_err(|unreadable| {
        CommandRefusal::refuse(
            CommandCause::ArtifactUnreadable,
            span,
            address,
            format!(
                "`{address}` was written and could not be re-read before the claim: \
                 {unreadable}. A builder claims what the committed state says, so an object it \
                 cannot read back is an object it cannot claim"
            ),
        )
    })?;
    let written_digest = digest_of(written);
    let reread_digest = digest_of(&reread);
    if written_digest != reread_digest {
        return Err(CommandRefusal::refuse(
            CommandCause::ObjectMovedBeforeClaim,
            span,
            &reread_digest,
            format!(
                "`{address}` was written as `{written_digest}` and re-reads as `{reread_digest}` \
                 — it moved between the write and the claim. Every builder claim is re-read from \
                 the committed state before it is made, because a claim about bytes that have \
                 changed is a claim about a tree that no longer exists"
            ),
        ));
    }
    Ok(reread_digest)
}
