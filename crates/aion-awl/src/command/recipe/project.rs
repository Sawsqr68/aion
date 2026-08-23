//! One wall, one home: the two renderings a wall declaration projects into.
//!
//! # The family this closes
//!
//! One intent written in two languages, only one of them executing. A rig's
//! prompts said "every changed path stays inside the allowlist" and a gate
//! script enforced a regular expression, and the two were kept in agreement
//! by whoever last read both. At the pinned bytes of
//! `.fleet/fixtures/corpus/haem-wall.INPUT-FINAL.json` (`77f0333f…`) the
//! same allowlist stands VERBATIM twice — in `fix_prompt` and in
//! `legs[0].prompt` — is referred to without restatement twice more
//! (`fix_prompt` again, `judge_prompt`), and is enforced a fifth time by the
//! gate script the rig names. Five renderings, one intent, no mechanism.
//!
//! Both renderings here come from ONE declaration, so they cannot disagree.
//! The builder owns the PROJECTION and never the CONTENT: which paths a
//! lane's wall admits stays authored, in the declaration, exactly as the
//! per-lane wall sets, the briefs, the judge duties and the FINDINGS stay
//! authored.
//!
//! # The negative half
//!
//! A wall's `excludes` are terms a round MEASURED OUT. A projection that
//! would emit one refuses, with the term as its observed bytes, because the
//! way a measured-out term comes back is that somebody writes it again in
//! good faith and nothing is watching.

use super::super::cause::CommandCause;
use super::super::error::CommandRefusal;
use super::decl::{ParamLiteral, WallDecl};

/// The two renderings one wall declaration produces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WallProjection {
    /// The prompt rendering: what a role is told, in words.
    pub prompt: String,
    /// The mechanical rendering: an anchored alternation a census matches
    /// changed paths against.
    pub enforcement: String,
}

/// Project a wall into both of its renderings.
///
/// # Errors
///
/// Returns [`CommandCause::ExcludedTerm`] when a pattern, or the wall's own
/// name, carries a term the wall declares excluded.
pub fn project(wall: &WallDecl) -> Result<WallProjection, CommandRefusal> {
    for excluded in &wall.excludes {
        let term = excluded.text.trim();
        if term.is_empty() {
            continue;
        }
        if let Some(carrier) = carrier_of(wall, term) {
            return Err(CommandRefusal::refuse(
                CommandCause::ExcludedTerm,
                excluded.span,
                term,
                format!(
                    "projecting wall `{}` would emit `{term}`, which this wall declares excluded \
                     — it stands in {carrier}. An excluded term is a term a round MEASURED out, \
                     declared beside the wall so the next projection meets the measurement rather \
                     than someone's memory of it",
                    wall.name
                ),
            ));
        }
    }
    Ok(WallProjection {
        prompt: prompt_of(wall),
        enforcement: enforcement_of(wall),
    })
}

/// Where an excluded term stands, if it stands anywhere the projection would
/// emit.
fn carrier_of(wall: &WallDecl, term: &str) -> Option<String> {
    let folded = term.to_lowercase();
    if wall.name.to_lowercase().contains(&folded) {
        return Some(format!("the wall's own name `{}`", wall.name));
    }
    wall.allows
        .iter()
        .find(|pattern| pattern.text.to_lowercase().contains(&folded))
        .map(|pattern| format!("the allowed pattern `{}`", pattern.text))
}

/// The prompt rendering: the sentence a role reads.
fn prompt_of(wall: &WallDecl) -> String {
    let patterns: Vec<&str> = wall
        .allows
        .iter()
        .map(|pattern| pattern.text.as_str())
        .collect();
    format!(
        "⛔ Wall `{}`: every changed path stays inside the allowlist ({}). A path outside it is \
         moved or removed, never admitted by widening the wall.",
        wall.name,
        patterns.join(", ")
    )
}

/// The mechanical rendering: the anchored alternation a paths census greps
/// changed files against.
///
/// The patterns are escaped for the one metacharacter class a path pattern
/// carries by accident rather than by intent — `.`, `+`, `(`, `)`, `[`,
/// `]`, `{`, `}`, `^`, `$`, `?`, `|`, `\` — while `*` is left alone,
/// because a rig writes `known-carries*` meaning "and what follows". A
/// declaration whose patterns were escaped into literals would enforce a
/// wall narrower than the one its prompt describes, which is the exact
/// disagreement one declaration exists to prevent.
fn enforcement_of(wall: &WallDecl) -> String {
    let alternatives: Vec<String> = wall
        .allows
        .iter()
        .map(|pattern| escape_pattern(&pattern.text))
        .collect();
    format!("^({})", alternatives.join("|"))
}

fn escape_pattern(pattern: &str) -> String {
    let mut escaped = String::with_capacity(pattern.len());
    for character in pattern.chars() {
        if matches!(
            character,
            '.' | '+' | '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' | '?' | '|' | '\\'
        ) {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

/// What changed between two states of one wall declaration.
///
/// The hand-built refire artifact, mechanized: a round that re-fires with a
/// corrected wall produces this rather than a person re-reading two prompts
/// side by side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WallDiff {
    /// Patterns the later state allows and the earlier did not.
    pub allowed_added: Vec<String>,
    /// Patterns the earlier state allowed and the later does not.
    pub allowed_removed: Vec<String>,
    /// Terms the later state excludes and the earlier did not.
    pub excluded_added: Vec<String>,
    /// Terms the earlier state excluded and the later does not.
    pub excluded_removed: Vec<String>,
    /// Whether the prompt rendering changed.
    pub prompt_changed: bool,
    /// Whether the mechanical rendering changed.
    pub enforcement_changed: bool,
}

impl WallDiff {
    /// Whether the two states project the same thing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.allowed_added.is_empty()
            && self.allowed_removed.is_empty()
            && self.excluded_added.is_empty()
            && self.excluded_removed.is_empty()
            && !self.prompt_changed
            && !self.enforcement_changed
    }

    /// Every changed projection, named. This is what a refire artifact says.
    #[must_use]
    pub fn changed_projections(&self) -> Vec<&'static str> {
        let mut changed = Vec::new();
        if self.prompt_changed {
            changed.push("prompt");
        }
        if self.enforcement_changed {
            changed.push("enforcement");
        }
        changed
    }
}

/// The structured difference between two states of one wall declaration.
///
/// # Errors
///
/// Returns the projection refusal of whichever state cannot be projected: a
/// diff between a state and one that would emit an excluded term is not a
/// diff, it is the refusal.
pub fn diff(before: &WallDecl, after: &WallDecl) -> Result<WallDiff, CommandRefusal> {
    let before_projection = project(before)?;
    let after_projection = project(after)?;
    Ok(WallDiff {
        allowed_added: only_in(&after.allows, &before.allows),
        allowed_removed: only_in(&before.allows, &after.allows),
        excluded_added: only_in(&after.excludes, &before.excludes),
        excluded_removed: only_in(&before.excludes, &after.excludes),
        prompt_changed: before_projection.prompt != after_projection.prompt,
        enforcement_changed: before_projection.enforcement != after_projection.enforcement,
    })
}

fn only_in(present: &[ParamLiteral], absent_from: &[ParamLiteral]) -> Vec<String> {
    present
        .iter()
        .filter(|item| !absent_from.iter().any(|other| other.text == item.text))
        .map(|item| item.text.clone())
        .collect()
}
