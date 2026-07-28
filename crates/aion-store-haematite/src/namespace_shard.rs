//! Which distribution shard a namespace-registry record lives on.
//!
//! The namespace registry is keyed by `n: || namespace_name`
//! ([`crate::keyspace::namespace_key`]), so a namespace record hashes to a shard
//! of its OWN — independent of the shard any workflow in that namespace lands
//! on. Until this accessor existed the shard was computed only inside
//! [`crate::HaematiteStore::register_namespace_record`]'s failure arms, where it
//! stamps a `NotOwner` refusal; nothing could ask the question BEFORE attempting
//! the write.
//!
//! Asking beforehand is what lets the mint be routed to the record's owning node
//! instead of being fenced forever on a node that does not own that shard (the
//! defect proven in `docs/evidence/failover-start-path-investigation.md`).
//!
//! A sibling module rather than another `impl` block in `store.rs`: that file is
//! already 4000+ lines and does not grow.

use crate::HaematiteStore;
use crate::keyspace;

impl HaematiteStore {
    /// The distribution shard that owns `name`'s namespace-registry record.
    ///
    /// Computes `shard_for(namespace_key(name))` — the SAME hashing
    /// [`Self::register_namespace_record`] routes its quorum CAS through, so the
    /// answer here and the shard named in that path's `NotOwner` refusal are the
    /// same number by construction.
    ///
    /// Note this is deliberately NOT
    /// [`Self::shard_for_workflow`](HaematiteStore::shard_for_workflow): a
    /// namespace's registry record and a workflow started in that namespace hash
    /// different keys and routinely land on different shards. Routing decisions
    /// about the two are independent.
    #[must_use]
    pub fn shard_for_namespace(&self, name: &str) -> usize {
        self.event_store()
            .database()
            .shard_for(&keyspace::namespace_key(name))
    }
}

#[cfg(test)]
#[path = "namespace_shard_tests.rs"]
mod tests;
