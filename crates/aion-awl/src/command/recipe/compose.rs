//! Composition: binding values to a recipe's declared parameters, and the
//! slot refusal that stops an unfilled one from travelling.
//!
//! # The behaviour absorbed here, at the bytes
//!
//! `.fleet/fixtures/corpus/sleepreap.build-input.py` (`032ee3d7…`) refuses
//! with exit 2 when the ground it reads still carries `RECORD-RUN-PENDING`,
//! and again when it is given too few arguments.
//! `.fleet/fixtures/corpus/liminal22.build-input.py` (`b51f60f7…`) carries
//! the first guard over `TBD-LINUX-TESTS` and NOT the second: at line 15 it
//! indexes `sys.argv[1]` with no length check, so the same mistake that
//! earns a named refusal in one composer raises an `IndexError` in the
//! other. Both behaviours become one typed refusal here, and that
//! normalization is the point rather than a rounding: a slot nothing filled
//! refuses BY NAME, whichever composer's shape it came from.
//!
//! # Compose is its own layer
//!
//! Nothing has run when a composition refuses, and nothing will. That is a
//! different claim from a fire refusal, where the world was reached for, and
//! the two get different layers so a reader can tell them apart without
//! reading the code that produced them.

use std::collections::BTreeMap;

use crate::Span;

use super::super::cause::CommandCause;
use super::super::error::CommandRefusal;
use super::check::is_unfilled_sentinel;
use super::decl::{ParameterKind, RecipeDecl};

/// The values a composition binds to a recipe's parameters.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Composition {
    values: BTreeMap<String, String>,
}

impl Composition {
    /// An empty composition.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Bind one parameter, returning the composition so bindings chain.
    #[must_use]
    pub fn with(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.values.insert(name.into(), value.into());
        self
    }

    /// The value bound to `name`, if any.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }
}

/// A recipe composed: every parameter that has a value at compose time, and
/// the re-derive parameters that deliberately do not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Composed {
    /// The values every non-re-derive parameter resolved to.
    pub resolved: BTreeMap<String, String>,
    /// The parameters that stay unresolved until the fire minute, in source
    /// order. A re-derive parameter is READ THEN, never now.
    pub deferred: Vec<String>,
}

/// Compose `recipe` against `composition`, refusing every unfilled slot.
///
/// Every parameter is examined — the function does not stop at the first
/// refusal — because an author filling one slot at a time is an author
/// running the composer five times to learn five things it already knew.
///
/// # Errors
///
/// Returns every compose-layer refusal the composition earns:
/// [`CommandCause::SlotUnfilled`] for a slot nothing filled or a bound value
/// still carrying its sentinel, and [`CommandCause::LiteralWhereDerived`]
/// for a hand value bound to a re-derive slot.
pub fn compose(
    recipe: &RecipeDecl,
    composition: &Composition,
) -> Result<Composed, Vec<CommandRefusal>> {
    let mut resolved = BTreeMap::new();
    let mut deferred = Vec::new();
    let mut refusals = Vec::new();

    for parameter in &recipe.parameters {
        let bound = composition.get(&parameter.name);
        if parameter.kind == ParameterKind::ReDerive {
            if let Some(value) = bound {
                refusals.push(CommandRefusal::refuse(
                    CommandCause::LiteralWhereDerived,
                    parameter.span,
                    value,
                    format!(
                        "recipe `{}`'s slot `{}` is declared `{}` and was composed with the hand \
                         value `{value}`. A re-derive slot is read from its declared source at \
                         the FIRE minute — not the compose minute, because they drift — so a \
                         value supplied here is a value that was true when the composer ran and \
                         nothing re-checks at the minute it matters",
                        recipe.name,
                        parameter.name,
                        ParameterKind::ReDerive
                    ),
                ));
                continue;
            }
            deferred.push(parameter.name.clone());
            continue;
        }

        let declared = parameter
            .default
            .as_ref()
            .map(|literal| literal.text.as_str());
        let Some(value) = bound.or(declared) else {
            // The bytes observed are the SLOT'S OWN NAME: nothing was seen,
            // and the thing that was missing is what a reader must be handed.
            refusals.push(unfilled(
                parameter.span,
                &parameter.name,
                format!(
                    "recipe `{}`'s slot `{}` was not filled and declares no default, so nothing \
                     says what it is. A slot nothing filled refuses BY NAME here rather than \
                     travelling into a dispatch as an empty string or as the index error one of \
                     the two composers this behaviour was read from raises instead",
                    recipe.name, parameter.name
                ),
            ));
            continue;
        };
        if is_unfilled_sentinel(value) {
            refusals.push(unfilled(
                parameter.span,
                value,
                format!(
                    "recipe `{}`'s slot `{}` still carries the sentinel `{value}`, which is what \
                     a skeleton writes where a value must go. The sentinel is the composer's own \
                     refusal condition, typed: a template copied and not filled must stop here, \
                     not at the venue",
                    recipe.name, parameter.name
                ),
            ));
            continue;
        }
        resolved.insert(parameter.name.clone(), value.to_owned());
    }

    if refusals.is_empty() {
        Ok(Composed { resolved, deferred })
    } else {
        Err(refusals)
    }
}

/// A slot refusal. The message always names the slot; the observed bytes are
/// what was actually seen there — the slot's own name when nothing was, the
/// sentinel when a skeleton's placeholder was.
fn unfilled(span: Span, observed: &str, message: String) -> CommandRefusal {
    CommandRefusal::refuse(CommandCause::SlotUnfilled, span, observed, message)
}
