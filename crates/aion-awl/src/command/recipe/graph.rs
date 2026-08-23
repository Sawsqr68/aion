//! The dependency DAG between a recipe's steps, and the cycle refusal.
//!
//! A recipe's steps are ordered by their `after` edges, never by the order
//! an author happened to write them in. Two properties follow and both are
//! mechanical here: every edge names a step that exists, and the edges close
//! no cycle. A cycle is refused WITH A SPAN — the span of the edge that
//! closes it — because "your recipe has a cycle" without a place to look is
//! a diagnostic that makes an author read the whole declaration.
//!
//! The traversal is an explicit-stack depth-first search rather than
//! recursion: a deeply chained recipe must produce a refusal, never a stack
//! overflow, and an overflow is not a refusal a caller can handle.

use std::collections::{BTreeMap, BTreeSet};

use super::decl::{RecipeDecl, RecipeStep};

/// A cycle the edges close, named by the steps on it in visit order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyCycle {
    /// The steps on the cycle, in the order the traversal met them, with the
    /// first name repeated at the end so a reader sees it close.
    pub path: Vec<String>,
    /// The step whose edge closes the cycle.
    pub closed_by: String,
}

impl DependencyCycle {
    /// The cycle as a reader follows it: `a -> b -> c -> a`.
    #[must_use]
    pub fn rendered(&self) -> String {
        self.path.join(" -> ")
    }
}

/// The topological order the edges imply, or the cycle that makes one
/// impossible.
///
/// Steps with no edge between them keep their SOURCE order, so a recipe that
/// declares no dependency at all runs in the order it was written and the
/// order is still a fact about the declaration rather than about a hash.
///
/// # Errors
///
/// Returns the [`DependencyCycle`] the edges close.
pub fn topological_order(recipe: &RecipeDecl) -> Result<Vec<&RecipeStep>, DependencyCycle> {
    let index: BTreeMap<&str, usize> = recipe
        .steps
        .iter()
        .enumerate()
        .map(|(at, step)| (step.name.as_str(), at))
        .collect();

    let mut ordered: Vec<&RecipeStep> = Vec::with_capacity(recipe.steps.len());
    let mut done: BTreeSet<usize> = BTreeSet::new();

    for root in 0..recipe.steps.len() {
        if done.contains(&root) {
            continue;
        }
        // (step, next edge to walk). `on_path` is the set of steps currently
        // open on the stack — a repeat inside it is the cycle.
        let mut stack: Vec<(usize, usize)> = vec![(root, 0)];
        let mut on_path: Vec<usize> = vec![root];
        while let Some((at, edge)) = stack.pop() {
            let step = &recipe.steps[at];
            if let Some(reference) = step.after.get(edge) {
                stack.push((at, edge + 1));
                let Some(&target) = index.get(reference.name.as_str()) else {
                    // An edge to nowhere is `reference_undeclared`, raised by
                    // the checker with its own span. The graph walks past it
                    // rather than inventing a node, so one declaration does
                    // not earn two refusals for one mistake.
                    continue;
                };
                if done.contains(&target) {
                    continue;
                }
                if let Some(opened) = on_path.iter().position(|open| *open == target) {
                    let mut path: Vec<String> = on_path[opened..]
                        .iter()
                        .map(|node| recipe.steps[*node].name.clone())
                        .collect();
                    path.push(recipe.steps[target].name.clone());
                    return Err(DependencyCycle {
                        path,
                        closed_by: step.name.clone(),
                    });
                }
                stack.push((target, 0));
                on_path.push(target);
                continue;
            }
            // Every edge of this step is walked: it may run now.
            if done.insert(at) {
                ordered.push(step);
            }
            if on_path.last() == Some(&at) {
                on_path.pop();
            }
        }
    }
    Ok(ordered)
}
