//! Key-encoding scheme for the single-node haematite-backed store.
//!
//! The store keeps workflow-history events in haematite's native event-stream
//! keyspace and every other durable record (timers, packages, routes, and
//! outbox rows) in the general KV keyspace. The two keyspaces are disjoint by
//! construction: event streams are addressed through [`haematite::EventStore`]
//! (which encodes `stream_key || 0x00 || seq`), while the KV records here all
//! carry a single-byte region tag that is never `0x00` and never collides with
//! an event stream key. Workflows are enumerated directly from the event
//! streams (see [`workflow_id_from_event_stream_key`]), so there is no separate
//! workflow-id index.
//!
//! # Region tags
//!
//! | tag  | region                | key layout                                  |
//! |------|-----------------------|---------------------------------------------|
//! | `E`  | event stream          | `E || workflow_uuid_bytes` (16 bytes)       |
//! | `t`  | durable timers        | `t: || workflow_id_text || 0x1f || timer`   |
//! | `p`  | deployed packages     | `p: || type || 0x1f || content_hash`        |
//! | `r`  | package routes        | `r: || workflow_type`                       |
//! | `o`  | outbox rows           | `o: || dispatch_key`                        |
//! | `n`  | namespace registry    | `n: || namespace_name`                      |
//!
//! Range scans over a tag prefix use the half-open upper bound produced by
//! [`prefix_upper_bound`]. Because the single-node store runs haematite with
//! `shard_count == 1`, every key lives in one shard and a prefix range scan is
//! globally complete (haematite's `range` is shard-local, routed from `from`).

use aion_core::{RunId, WorkflowId};
use uuid::Uuid;

/// Field separator inside composite KV keys (ASCII unit separator).
///
/// `0x1f` sorts below every printable byte used in a workflow type, content
/// hash, or timer-id token, so a composite key never lets one field's tail bleed
/// into the next field's head when range-scanning a prefix.
pub(crate) const FIELD_SEP: u8 = 0x1f;

/// Event-stream key for `workflow_id`: the tag byte `E` followed by the raw
/// 16-byte UUID.
///
/// This is handed to [`haematite::EventStore::append_batch`]/`read`, which append
/// their own `0x00 || seq` suffix. The `E` tag keeps the stream key out of the
/// `b"_:"` KV regions below.
pub(crate) fn event_stream_key(workflow_id: &WorkflowId) -> Vec<u8> {
    let mut key = Vec::with_capacity(1 + 16);
    key.push(b'E');
    key.extend_from_slice(workflow_id.as_uuid().as_bytes());
    key
}

/// Recover the [`WorkflowId`] from an event-stream key, if `key` is one.
///
/// The inverse of [`event_stream_key`]: a stream key is the tag byte `E`
/// followed by the raw 16-byte UUID, so a valid key is exactly 17 bytes long
/// and starts with `E`. Returns `None` for any other key (e.g. a KV-region key
/// or a malformed stream key), so callers enumerating workflows from the event
/// streams can defensively skip non-stream keys.
pub(crate) fn workflow_id_from_event_stream_key(key: &[u8]) -> Option<WorkflowId> {
    if key.len() != 17 || key[0] != b'E' {
        return None;
    }
    Uuid::from_slice(&key[1..17]).ok().map(WorkflowId::new)
}

/// Prefix for the durable-timer region.
pub(crate) const TIMER_PREFIX: &[u8] = b"t:";

/// Timer key for `(workflow_id, timer_id_token)`.
pub(crate) fn timer_key(workflow_id: &WorkflowId, timer_id_token: &str) -> Vec<u8> {
    composite(
        TIMER_PREFIX,
        &[
            workflow_id.to_string().as_bytes(),
            timer_id_token.as_bytes(),
        ],
    )
}

/// Prefix for the deployed-package region.
pub(crate) const PACKAGE_PREFIX: &[u8] = b"p:";

/// Package key for `(workflow_type, content_hash)`.
pub(crate) fn package_key(workflow_type: &str, content_hash: &str) -> Vec<u8> {
    composite(
        PACKAGE_PREFIX,
        &[workflow_type.as_bytes(), content_hash.as_bytes()],
    )
}

/// Prefix for the package-route region.
pub(crate) const ROUTE_PREFIX: &[u8] = b"r:";

/// Route key for `workflow_type`.
pub(crate) fn route_key(workflow_type: &str) -> Vec<u8> {
    composite(ROUTE_PREFIX, &[workflow_type.as_bytes()])
}

/// Decode a route key back into its workflow type.
pub(crate) fn workflow_type_from_route_key(key: &[u8]) -> Option<String> {
    let suffix = key.strip_prefix(ROUTE_PREFIX)?;
    String::from_utf8(suffix.to_vec()).ok()
}

/// Prefix for the outbox region.
pub(crate) const OUTBOX_PREFIX: &[u8] = b"o:";

/// Outbox key for `dispatch_key`.
pub(crate) fn outbox_key(dispatch_key: &str) -> Vec<u8> {
    composite(OUTBOX_PREFIX, &[dispatch_key.as_bytes()])
}

/// Prefix for the namespace-registry region (Control-Plane Phase 1).
///
/// A registry record names a durable, minted-on-use namespace so the live set of
/// namespaces is listable and survives owner-node death. The single-byte tag `n`
/// is disjoint from every other region tag (`E`, `t`, `p`, `r`, `o`, `d`) and
/// from the `0x00`-suffixed event-stream keyspace, so a namespace record never
/// collides with any other durable record. Records are written through the
/// quorum-replicated fenced path ([`crate::HaematiteStore::register_namespace_record`])
/// in distributed mode so the registry travels with its shard on failover.
pub(crate) const NAMESPACE_PREFIX: &[u8] = b"n:";

/// Registry key for `name`: `n: || namespace_name`.
pub(crate) fn namespace_key(name: &str) -> Vec<u8> {
    composite(NAMESPACE_PREFIX, &[name.as_bytes()])
}

/// Decode a namespace-registry key back into its namespace name.
///
/// The inverse of [`namespace_key`]: strips the `n:` prefix and re-reads the
/// remaining bytes as UTF-8. Returns `None` for a key that is not in the
/// namespace region or whose suffix is not valid UTF-8, so a prefix scan can
/// defensively skip a malformed key rather than fail the whole enumeration.
///
/// The list path decodes each record from its VALUE (which carries the name), so
/// this key inverse is not yet exercised by production code; it is the declared
/// seam for the S4 boot-wiring / key-driven enumeration that follows this slice.
/// Gated to `cfg(test)` (rather than an `#[allow(dead_code)]`) so the production
/// build carries no unused code per the zero-production-allow policy, while its
/// round-trip stays provable; S4 promotes it out of `cfg(test)` when a real
/// caller lands.
#[cfg(test)]
pub(crate) fn namespace_from_key(key: &[u8]) -> Option<String> {
    let suffix = key.strip_prefix(NAMESPACE_PREFIX)?;
    String::from_utf8(suffix.to_vec()).ok()
}

/// Prefix for durable worker-deployment records.
pub(crate) const WORKER_DEPLOYMENT_PREFIX: &[u8] = b"w:";

/// Worker-deployment key for an operator-chosen deployment name.
pub(crate) fn worker_deployment_key(name: &str) -> Vec<u8> {
    composite(WORKER_DEPLOYMENT_PREFIX, &[name.as_bytes()])
}

/// Decode a worker-deployment key back into its operator-chosen name.
pub(crate) fn worker_deployment_from_key(key: &[u8]) -> Option<String> {
    let suffix = key.strip_prefix(WORKER_DEPLOYMENT_PREFIX)?;
    String::from_utf8(suffix.to_vec()).ok()
}

/// Prefix for the shard-owner directory region (SS-3).
///
/// A directory record names the node that currently OWNS (or has adopted) a
/// distribution shard, so a non-owning node's request-routing edge can resolve
/// the shard's *current* owner — including a survivor that adopted the shard
/// after the declared owner died — rather than mis-resolving to the dead
/// declared owner (gap #2). The record is written through the quorum-replicated
/// fenced path ([`crate::HaematiteStore::publish_shard_owner`]) so only the true
/// fenced owner can publish it and every reachable member durably receives it.
pub(crate) const SHARD_OWNER_PREFIX: &[u8] = b"d:";

/// The directory-record key for `shard`, derived so it deterministically HASHES
/// to `shard` itself under the supplied `shard_for` routing.
///
/// Co-locating the record on its own shard means the record write is subject to
/// the SAME epoch fence the publisher just won for that shard: only the node
/// that elected itself owner of `shard` can replicate-write the record, so two
/// survivors racing to adopt cannot both publish — exactly one (the election
/// winner) does. The key is found by probing a `u64` suffix counter until
/// `shard_for(key) == shard`. The probe uses the live database's OWN
/// `shard_for` (passed in) rather than a reimplementation, so the key can never
/// drift from haematite's routing. The search is deterministic (every node
/// computes the identical key for a given `shard`) and terminates quickly
/// (`1/shard_count` of suffixes match).
pub(crate) fn shard_owner_key(shard: usize, shard_for: impl Fn(&[u8]) -> usize) -> Vec<u8> {
    let mut suffix: u64 = 0;
    loop {
        let mut key = SHARD_OWNER_PREFIX.to_vec();
        key.extend_from_slice(&suffix.to_be_bytes());
        if shard_for(&key) == shard {
            return key;
        }
        // A u64 counter cannot realistically be exhausted (every shard has many
        // matching suffixes among 2^64), so this loop always returns.
        suffix = suffix.wrapping_add(1);
    }
}

/// Region tag for the observability (`O`) keyspace — NOI-5's transcript spine.
///
/// Uppercase `O` (`0x4F`) is byte-disjoint from EVERY other region: it is not
/// `0x00` (the event-stream `seq` separator), not `E` (`0x45`, the event-stream
/// tag), and not any of the lowercase KV tags (`t`, `p`, `r`, `o` = `0x6F`, `n`,
/// `d`). An `O`-region key can therefore never collide with a workflow-history
/// event-stream key nor any other durable record — which is precisely what makes
/// the observability transcript "durable but non-replay-authoritative" a
/// structural guarantee: the replay decoder only ever scans `E`-tagged streams
/// (17-byte keys starting with `0x45`), so an `O`-tagged record is invisible to
/// replay and, being a different schema besides, undecodable as an `Event`.
pub(crate) const OBSERVABILITY_TAG: u8 = b'O';

/// Total encoded width of one `O`-region observability stream key.
///
/// `O(1) || workflow_uuid(16) || run_uuid(16) || activity_seq_be(8) ||
/// attempt_be(4)`. Named because three places must agree on it: the encoder, the
/// decoder's length gate, and the disjointness test.
pub(crate) const OBSERVABILITY_KEY_LEN: usize = 1 + 16 + 16 + 8 + 4;

/// Width of the `O || workflow_uuid || run_uuid` scan prefix.
pub(crate) const OBSERVABILITY_RUN_PREFIX_LEN: usize = 1 + 16 + 16;

/// The observability stream key for `(workflow_id, run_id, activity_seq, attempt)`.
///
/// Layout (fixed [`OBSERVABILITY_KEY_LEN`] bytes): `O(1) || workflow_uuid(16) ||
/// run_uuid(16) || activity_seq_be(8) || attempt_be(4)`. This is handed to
/// [`haematite::EventStore::append_batch`]/`read_from`, which append their own
/// `0x00 || seq` suffix — so within one attempt's stream the records order by
/// their server-allocated `store_seq`. The fixed-width big-endian components make
/// the key order lexicographically identical to the tuple order and keep the key
/// unambiguously parseable, and the leading `O` tag keeps it disjoint from the
/// `E`-stream and the `_:`-prefixed KV regions (see [`OBSERVABILITY_TAG`]).
///
/// # The run axis sits SECOND, deliberately
///
/// It is placed immediately after the workflow so `O || workflow || run` is a
/// contiguous prefix, which is what makes a single-run enumeration one range
/// scan rather than a workflow-wide scan with a post-filter. It also makes the
/// lexicographic key order `(workflow, run, activity, attempt)`, matching the
/// order the in-memory reference store sorts its summaries in.
///
/// # There is no decode arm for the pre-run 29-byte key
///
/// Transcript records are observability, never replay authority, so the estate's
/// no-backwards-compatibility rule applies without qualification: a record
/// written under the old run-ambiguous key is simply unreachable through this
/// keyspace, and [`decode_observability_stream_key`] rejects its key on length.
/// Reading one back would mean answering a run-scoped question with data that
/// cannot say which run it came from — the exact ambiguity being removed.
///
/// # Those records are also UNRECLAIMED, and that is not fixable here
///
/// Unreachable is not the whole cost: a database written by a build that
/// predates the run axis keeps its 29-byte `O` records on disk forever. Nothing
/// reaps them — `observability_list_blocking` skips whatever fails to decode,
/// and there is no purge verb or migration. This is stated rather than fixed
/// because it CANNOT be fixed from this crate: haematite exposes no
/// stream-deletion primitive at all (`append_with_ttl` is its only expiry
/// surface — there is no `delete`/`truncate` for a sequence stream), so a reaper
/// would have to begin as a haematite change, and the records share one database
/// file with the `E` event streams so they cannot be discarded selectively
/// either. The leak is bounded and non-growing: no writer emits that shape any
/// more, and nothing depends on the bytes. Reclaiming them means discarding the
/// store directory — which discards workflow history too, so it is an operator's
/// explicit choice, never a maintenance step.
pub(crate) fn observability_stream_key(
    workflow_id: &WorkflowId,
    run_id: &RunId,
    activity_seq: u64,
    attempt: u32,
) -> Vec<u8> {
    let mut key = Vec::with_capacity(OBSERVABILITY_KEY_LEN);
    key.push(OBSERVABILITY_TAG);
    key.extend_from_slice(workflow_id.as_uuid().as_bytes());
    key.extend_from_slice(run_id.as_uuid().as_bytes());
    key.extend_from_slice(&activity_seq.to_be_bytes());
    key.extend_from_slice(&attempt.to_be_bytes());
    key
}

/// The [`OBSERVABILITY_RUN_PREFIX_LEN`]-byte `O || workflow_uuid || run_uuid`
/// prefix every observability stream key of ONE RUN shares (see
/// [`observability_stream_key`] for the full layout).
///
/// Handed to a stream-metadata scan predicate (`starts_with`) to enumerate one
/// run's retained transcript streams; the fixed-width layout means the prefix
/// can never partially match a foreign region's key, and including the run means
/// a sibling generation of the same continue-as-new chain is excluded by the
/// scan itself rather than by a filter someone could forget.
pub(crate) fn observability_run_prefix(workflow_id: &WorkflowId, run_id: &RunId) -> Vec<u8> {
    let mut prefix = Vec::with_capacity(OBSERVABILITY_RUN_PREFIX_LEN);
    prefix.push(OBSERVABILITY_TAG);
    prefix.extend_from_slice(workflow_id.as_uuid().as_bytes());
    prefix.extend_from_slice(run_id.as_uuid().as_bytes());
    prefix
}

/// Decode a full [`OBSERVABILITY_KEY_LEN`]-byte `O`-region stream key into
/// `(activity_seq, attempt)`. Returns `None` for any other region/length (scan
/// safety) — including the pre-run-axis 29-byte key, which is not decodable
/// here by design.
pub(crate) fn decode_observability_stream_key(key: &[u8]) -> Option<(u64, u32)> {
    if key.len() != OBSERVABILITY_KEY_LEN || key[0] != OBSERVABILITY_TAG {
        return None;
    }
    let activity_seq = u64::from_be_bytes(
        key.get(OBSERVABILITY_RUN_PREFIX_LEN..OBSERVABILITY_RUN_PREFIX_LEN + 8)?
            .try_into()
            .ok()?,
    );
    let attempt = u32::from_be_bytes(
        key.get(OBSERVABILITY_RUN_PREFIX_LEN + 8..OBSERVABILITY_KEY_LEN)?
            .try_into()
            .ok()?,
    );
    Some((activity_seq, attempt))
}

/// Build a composite key: `prefix` then each field joined by [`FIELD_SEP`].
fn composite(prefix: &[u8], fields: &[&[u8]]) -> Vec<u8> {
    let mut key = prefix.to_vec();
    for (index, field) in fields.iter().enumerate() {
        if index > 0 {
            key.push(FIELD_SEP);
        }
        key.extend_from_slice(field);
    }
    key
}

/// The exclusive upper bound for a half-open `[prefix, upper)` range scan that
/// returns exactly the keys starting with `prefix`.
///
/// Increments the last byte; if every byte is `0xff` (no finite successor)
/// returns `None`, and callers treat that as an unbounded tail. A `0xff`-only
/// prefix never occurs for the ASCII region tags this store uses.
pub(crate) fn prefix_upper_bound(prefix: &[u8]) -> Option<Vec<u8>> {
    let mut upper = prefix.to_vec();
    while let Some(last) = upper.last_mut() {
        if *last < 0xff {
            *last += 1;
            return Some(upper);
        }
        upper.pop();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use aion_core::{RunId, WorkflowId};
    use uuid::Uuid;

    /// A run identifier whose big-endian UUID bytes order with `value`, so a key
    /// ordering assertion states the tuple order it means.
    fn run(value: u128) -> RunId {
        RunId::new(Uuid::from_u128(value))
    }

    #[test]
    fn workflow_id_from_event_stream_key_round_trips() {
        let workflow_id = WorkflowId::new(Uuid::from_u128(42));
        let key = event_stream_key(&workflow_id);
        assert_eq!(workflow_id_from_event_stream_key(&key), Some(workflow_id));
    }

    #[test]
    fn workflow_id_from_event_stream_key_rejects_non_stream_keys() {
        // A KV-region key (wrong tag / wrong length) is not a stream key.
        assert_eq!(
            workflow_id_from_event_stream_key(
                timer_key(&WorkflowId::new(Uuid::from_u128(1)), "t").as_slice()
            ),
            None
        );
        // A 17-byte key with the wrong tag byte is rejected.
        let mut wrong_tag = vec![b'X'];
        wrong_tag.extend_from_slice(Uuid::from_u128(2).as_bytes());
        assert_eq!(workflow_id_from_event_stream_key(&wrong_tag), None);
    }

    #[test]
    fn route_key_round_trips() {
        let key = route_key("checkout");
        assert_eq!(
            workflow_type_from_route_key(&key).as_deref(),
            Some("checkout")
        );
    }

    #[test]
    fn namespace_key_round_trips() {
        let key = namespace_key("orders");
        assert_eq!(namespace_from_key(&key).as_deref(), Some("orders"));
    }

    #[test]
    fn namespace_key_is_tagged_and_disjoint_from_other_regions() {
        let key = namespace_key("default");
        assert_eq!(&key[..2], NAMESPACE_PREFIX);
        // Disjoint from every other single-byte region tag.
        assert_ne!(&key[..2], TIMER_PREFIX);
        assert_ne!(&key[..2], PACKAGE_PREFIX);
        assert_ne!(&key[..2], ROUTE_PREFIX);
        assert_ne!(&key[..2], OUTBOX_PREFIX);
        assert_ne!(&key[..2], WORKER_DEPLOYMENT_PREFIX);
        assert_ne!(&key[..2], SHARD_OWNER_PREFIX);
        // Not the `E`-tagged event-stream keyspace either.
        assert_ne!(key[0], b'E');
    }

    #[test]
    fn namespace_from_key_rejects_foreign_keys() {
        // A timer-region key is not a namespace key.
        assert_eq!(
            namespace_from_key(timer_key(&WorkflowId::new(Uuid::from_u128(3)), "t").as_slice()),
            None
        );
    }

    #[test]
    fn prefix_upper_bound_increments_last_byte() {
        assert_eq!(prefix_upper_bound(b"t:"), Some(b"t;".to_vec()));
    }

    #[test]
    fn prefix_upper_bound_unbounded_when_all_ff() {
        assert_eq!(prefix_upper_bound(&[0xff, 0xff]), None);
    }

    #[test]
    fn observability_key_is_o_tagged_and_disjoint_from_every_region() {
        let workflow_id = WorkflowId::new(Uuid::from_u128(11));
        let key = observability_stream_key(&workflow_id, &run(1), 3, 2);
        // Uppercase `O` (0x4F) tag.
        assert_eq!(key[0], OBSERVABILITY_TAG);
        assert_eq!(key[0], 0x4F);
        // Disjoint from the event-stream tag `E` (0x45) and the seq separator.
        assert_ne!(key[0], b'E');
        assert_ne!(key[0], 0x00);
        // Disjoint from every lowercase KV region tag — including `o` (0x6F, the
        // OUTBOX tag), which is a DIFFERENT byte from uppercase `O` (0x4F).
        assert_ne!(&key[..2], TIMER_PREFIX);
        assert_ne!(&key[..2], PACKAGE_PREFIX);
        assert_ne!(&key[..2], ROUTE_PREFIX);
        assert_ne!(&key[..2], OUTBOX_PREFIX);
        assert_ne!(&key[..2], NAMESPACE_PREFIX);
        assert_ne!(&key[..2], SHARD_OWNER_PREFIX);
        assert_ne!(&key[..2], WORKER_DEPLOYMENT_PREFIX);
        assert_ne!(key[0], OUTBOX_PREFIX[0]);
        // Fixed-width layout: O(1) + workflow(16) + run(16) + activity_seq(8)
        // + attempt(4). The run axis is what widened this from 29 to 45.
        assert_eq!(key.len(), OBSERVABILITY_KEY_LEN);
        assert_eq!(key.len(), 45);
        // The uuid + fixed-width suffix means it can NEVER be mistaken for a
        // 17-byte `E`-stream key (the only key shape the replay decoder decodes).
        assert_ne!(key.len(), 17);
        assert_eq!(workflow_id_from_event_stream_key(&key), None);
    }

    #[test]
    fn worker_deployment_keyspace_is_tagged_and_disjoint() {
        let key = worker_deployment_key("shells");
        assert_eq!(&key[..2], WORKER_DEPLOYMENT_PREFIX);
        assert_ne!(&key[..2], TIMER_PREFIX);
        assert_ne!(&key[..2], PACKAGE_PREFIX);
        assert_ne!(&key[..2], ROUTE_PREFIX);
        assert_ne!(&key[..2], OUTBOX_PREFIX);
        assert_ne!(&key[..2], NAMESPACE_PREFIX);
        assert_ne!(&key[..2], SHARD_OWNER_PREFIX);
        assert_ne!(key[0], b'E');
    }

    #[test]
    fn observability_run_prefix_is_a_strict_prefix_of_the_full_key() {
        let workflow_id = WorkflowId::new(Uuid::from_u128(21));
        let prefix = observability_run_prefix(&workflow_id, &run(1));
        let full = observability_stream_key(&workflow_id, &run(1), 7, 2);
        assert_eq!(prefix.len(), OBSERVABILITY_RUN_PREFIX_LEN);
        assert!(full.starts_with(&prefix));
        assert!(full.len() > prefix.len(), "the prefix is strict");
        // A different workflow's key does not share the prefix.
        let other = observability_stream_key(&WorkflowId::new(Uuid::from_u128(22)), &run(1), 7, 2);
        assert!(!other.starts_with(&prefix));
        // Nor does a SIBLING GENERATION of the same workflow: the run is inside
        // the prefix, so one run's scan cannot reach another's records at all.
        let sibling = observability_stream_key(&workflow_id, &run(2), 7, 2);
        assert!(
            !sibling.starts_with(&prefix),
            "a sibling continue-as-new generation must fall outside the scan"
        );
    }

    #[test]
    fn observability_stream_key_encode_decode_round_trips() {
        let workflow_id = WorkflowId::new(Uuid::from_u128(23));
        let key = observability_stream_key(&workflow_id, &run(9), u64::MAX - 3, u32::MAX - 1);
        assert_eq!(
            decode_observability_stream_key(&key),
            Some((u64::MAX - 3, u32::MAX - 1))
        );
    }

    #[test]
    fn decode_observability_stream_key_rejects_foreign_and_truncated_keys() {
        // A 17-byte `E`-tagged event-stream key is not an `O`-region key.
        let event_key = event_stream_key(&WorkflowId::new(Uuid::from_u128(24)));
        assert_eq!(decode_observability_stream_key(&event_key), None);
        // A truncated `O`-region key (wrong length) decodes to nothing.
        let mut truncated =
            observability_stream_key(&WorkflowId::new(Uuid::from_u128(24)), &run(1), 1, 1);
        truncated.pop();
        assert_eq!(decode_observability_stream_key(&truncated), None);
        // A full-length key with the wrong tag byte is rejected.
        let mut wrong_tag =
            observability_stream_key(&WorkflowId::new(Uuid::from_u128(24)), &run(1), 1, 1);
        wrong_tag[0] = b'X';
        assert_eq!(decode_observability_stream_key(&wrong_tag), None);
    }

    /// The pre-run-axis 29-byte key shape is NOT decodable, by design: no compat
    /// arm exists, so a record written under the run-ambiguous key is simply
    /// unreachable rather than silently answering a run-scoped read.
    #[test]
    fn the_pre_run_axis_key_shape_is_not_decodable() {
        let workflow_id = WorkflowId::new(Uuid::from_u128(31));
        let mut legacy = Vec::with_capacity(29);
        legacy.push(OBSERVABILITY_TAG);
        legacy.extend_from_slice(workflow_id.as_uuid().as_bytes());
        legacy.extend_from_slice(&7u64.to_be_bytes());
        legacy.extend_from_slice(&2u32.to_be_bytes());
        assert_eq!(legacy.len(), 29);
        assert_eq!(decode_observability_stream_key(&legacy), None);
    }

    #[test]
    fn observability_keys_order_by_workflow_then_run_then_activity_then_attempt() {
        let workflow = WorkflowId::new(Uuid::from_u128(5));
        // Same workflow+run+activity, ascending attempt ⇒ ascending key.
        let a0 = observability_stream_key(&workflow, &run(1), 7, 0);
        let a1 = observability_stream_key(&workflow, &run(1), 7, 1);
        assert!(a0 < a1, "ascending attempt must sort ascending");
        // Ascending activity_seq ⇒ ascending key (big-endian width preserved).
        let act_low = observability_stream_key(&workflow, &run(1), 7, 5);
        let act_high = observability_stream_key(&workflow, &run(1), 8, 0);
        assert!(act_low < act_high, "ascending activity must sort ascending");
        // The run outranks BOTH: a later run's lowest activity still sorts above
        // an earlier run's highest, so each run's streams form one contiguous
        // range and a range scan over the run prefix is complete.
        let run_one_high = observability_stream_key(&workflow, &run(1), u64::MAX, u32::MAX);
        let run_two_low = observability_stream_key(&workflow, &run(2), 0, 0);
        assert!(run_one_high < run_two_low, "the run axis outranks the rest");
    }

    #[test]
    fn event_stream_key_is_tagged_and_disjoint_from_kv_regions() {
        let workflow_id = WorkflowId::new(Uuid::from_u128(7));
        let key = event_stream_key(&workflow_id);
        assert_eq!(key[0], b'E');
        assert_ne!(&key[..2], TIMER_PREFIX);
        assert_ne!(&key[..2], OUTBOX_PREFIX);
    }

    /// `shard_owner_key(shard, shard_for)` returns a key that the SAME
    /// `shard_for` routes to `shard` — for every shard, and deterministically
    /// (two calls yield the identical key). This is the load-bearing property:
    /// every node computes the identical directory key for a shard and that key
    /// co-locates on the shard itself (so the publish is fenced by the shard's
    /// ownership). Uses a real `Database::shard_for` so the probe can never drift
    /// from haematite's routing.
    #[test]
    fn shard_owner_key_lands_on_its_shard_for_every_shard() -> Result<(), haematite::DatabaseError>
    {
        let dir = std::env::temp_dir().join(format!(
            "aion-keyspace-downer-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
        ));
        let shard_count = 4;
        let database = haematite::Database::create(haematite::DatabaseConfig {
            data_dir: dir,
            shard_count,
            executor_threads: None,
            distributed: None,
        })?;
        for shard in 0..shard_count {
            let key = shard_owner_key(shard, |bytes| database.shard_for(bytes));
            assert_eq!(
                database.shard_for(&key),
                shard,
                "directory key for shard {shard} must route to that shard"
            );
            // Deterministic: a second derivation yields the identical key.
            let again = shard_owner_key(shard, |bytes| database.shard_for(bytes));
            assert_eq!(key, again, "directory key derivation must be deterministic");
            // Tagged into the directory region, disjoint from other KV regions.
            assert_eq!(&key[..2], SHARD_OWNER_PREFIX);
        }
        Ok(())
    }
}
