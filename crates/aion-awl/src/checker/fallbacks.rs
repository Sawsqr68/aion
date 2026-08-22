//! Validation for authored activity fallback chains.

use std::collections::BTreeMap;

use super::context::Ctx;

/// Reject duplicate queues while preserving the authored chain for runtime skips.
pub(super) fn run(ctx: &mut Ctx<'_>) {
    for action in ctx.doc.workers.iter().flat_map(|worker| &worker.actions) {
        let Some(fallback) = action
            .config
            .as_ref()
            .and_then(|config| config.fallback.as_ref())
        else {
            continue;
        };
        let mut first: BTreeMap<&str, crate::Span> = BTreeMap::new();
        for entry in &fallback.entries {
            if let Some(first_span) = first.get(entry.name.as_str()) {
                ctx.error(
                    entry.span,
                    format!(
                        "duplicate fallback task queue `{}`; its first occurrence starts at byte {}",
                        entry.name, first_span.start
                    ),
                );
            } else {
                first.insert(entry.name.as_str(), entry.span);
            }
        }
    }
}
