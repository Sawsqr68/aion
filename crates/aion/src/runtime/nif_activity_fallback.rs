//! History-derived activity fallback routing.

use aion_core::{ActivityId, Event};

/// Decode the authored fallback queue chain from one activity dispatch config.
///
/// The config is emitted by the BEAM module bound to this run, not read from the
/// currently deployed package. Every `.aion` version is an immutable,
/// content-hash-named module and recovery re-executes workflow code from that
/// bound module, so the authored list is stable for the run. Reading a current
/// deployment instead would agree in ordinary tests but break replay for a
/// long-lived workflow when a different list is deployed mid-hop.
///
/// Routing metadata never makes dispatch decoding fail. An absent or explicit
/// `null` field means no chain; malformed JSON or a present value that is not an
/// array of strings is warned about and treated as no chain.
pub(super) fn fallback_chain_from_config(config: &str) -> Vec<String> {
    let value: serde_json::Value = match serde_json::from_str(config) {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "malformed activity dispatch config; ignoring fallback routing");
            return Vec::new();
        }
    };
    let Some(fallback) = value.get("fallback") else {
        return Vec::new();
    };
    if fallback.is_null() {
        return Vec::new();
    }
    let Some(entries) = fallback.as_array() else {
        tracing::warn!(fallback = %fallback, "malformed activity fallback chain; expected an array of queue names");
        return Vec::new();
    };
    let chain = entries
        .iter()
        .map(|entry| entry.as_str().map(str::to_owned))
        .collect::<Option<Vec<_>>>();
    if let Some(chain) = chain {
        chain
    } else {
        tracing::warn!(fallback = %fallback, "malformed activity fallback chain; every queue name must be a string");
        Vec::new()
    }
}

/// Select the first authored queue at or after `consumed` that differs from the
/// queue currently serving the activity, preserving its original authored index.
pub(super) fn next_hop(
    chain: &[String],
    initial_queue: &str,
    current_queue: &str,
    consumed: usize,
) -> Option<(usize, String)> {
    let effective_current = if current_queue.is_empty() {
        initial_queue
    } else {
        current_queue
    };
    chain
        .iter()
        .enumerate()
        .skip(consumed)
        .find(|(_, queue)| queue.as_str() != effective_current)
        .map(|(index, queue)| (index, queue.clone()))
}

/// Recorded fallback hops for one activity, in durable history order.
pub(super) fn recorded_hops<'a>(history: &'a [Event], activity_id: &ActivityId) -> Vec<&'a Event> {
    history
        .iter()
        .filter(|event| {
            matches!(
                event,
                Event::ActivityFallbackRouted {
                    activity_id: id,
                    ..
                } if id == activity_id
            )
        })
        .collect()
}

/// The latest durably selected fallback queue for one activity.
pub(super) fn recorded_hop_queue(history: &[Event], activity_id: &ActivityId) -> Option<String> {
    recorded_hops(history, activity_id)
        .into_iter()
        .rev()
        .find_map(|event| match event {
            Event::ActivityFallbackRouted { to_task_queue, .. } => Some(to_task_queue.clone()),
            _ => None,
        })
}

/// The next authored position after all durably recorded hops.
pub(super) fn consumed_fallback_position(history: &[Event], activity_id: &ActivityId) -> usize {
    recorded_hops(history, activity_id)
        .into_iter()
        .filter_map(|event| match event {
            Event::ActivityFallbackRouted { fallback_index, .. } => {
                usize::try_from(*fallback_index)
                    .ok()
                    .and_then(|index| index.checked_add(1))
            }
            _ => None,
        })
        .max()
        .unwrap_or(0)
}

/// Initial queue followed by each durably recorded hop destination.
pub(super) fn refused_queue_sequence(
    history: &[Event],
    activity_id: &ActivityId,
    initial_queue: &str,
) -> Vec<String> {
    let mut queues = vec![initial_queue.to_owned()];
    queues.extend(recorded_hops(history, activity_id).into_iter().filter_map(
        |event| match event {
            Event::ActivityFallbackRouted { to_task_queue, .. } => Some(to_task_queue.clone()),
            _ => None,
        },
    ));
    queues
}

/// Return a trailing nonterminal policy refusal that recovery must dispose of
/// before recording or dispatching another execution.
pub(super) fn trailing_policy_refusal(
    history: &[Event],
    activity_id: &ActivityId,
) -> Option<(u32, String)> {
    for event in history.iter().rev() {
        match event {
            Event::ActivityFailed {
                activity_id: id,
                error,
                attempt,
                ..
            } if id == activity_id => {
                return (error.kind == aion_core::ActivityErrorKind::PolicyRefused)
                    .then(|| (*attempt, error.message.clone()));
            }
            Event::ActivityStarted {
                activity_id: id, ..
            }
            | Event::ActivityCompleted {
                activity_id: id, ..
            }
            | Event::ActivityCancelled {
                activity_id: id, ..
            } if id == activity_id => return None,
            Event::ActivityFallbackRouted {
                activity_id: id, ..
            } if id == activity_id => return None,
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{fallback_chain_from_config, next_hop};

    #[test]
    fn fallback_config_accepts_only_string_arrays() {
        assert_eq!(
            fallback_chain_from_config(r#"{"fallback":["norn","claude"]}"#),
            ["norn", "claude"]
        );
        assert!(fallback_chain_from_config(r#"{"fallback":null}"#).is_empty());
        assert!(fallback_chain_from_config(r#"{"fallback":["norn",2]}"#).is_empty());
    }

    #[test]
    fn next_hop_skips_current_queue_but_keeps_authored_index() {
        let chain = vec!["norn".to_owned(), "claude".to_owned()];
        assert_eq!(
            next_hop(&chain, "norn", "norn", 0),
            Some((1, "claude".to_owned()))
        );
        assert_eq!(next_hop(&chain, "norn", "claude", 2), None);
    }
}
