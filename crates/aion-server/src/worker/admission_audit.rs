//! Whether a contract refusal is NEW INFORMATION, and therefore worth saying.
//!
//! A refused worker redials forever. Task #94 measured the liminal transport
//! re-logging the identical full diagnosis on every dial — 2/second, 12 MB/hour,
//! *"observability buried"*. Naming the refusal on the gRPC path too (#147 R1)
//! would have given that defect a second host, so the naming and the decision
//! to repeat it land together.
//!
//! **The rule is a transition, not a cap.** The full diagnosis is emitted when
//! the refusal for a `(task_queue, node)` is new or different; an identical
//! repeat carries nothing the first line did not, so it is silence. There is no
//! interval, no count, and no configured value anywhere in this module —
//! suppressing a message that says exactly what the last one said is
//! correctness, not rate limiting.
//!
//! # Why the key is what it is
//!
//! The obvious key is `(identity, task_queue, node)` — and **`identity` is a
//! client-supplied string**. A refused worker that varies its identity per dial
//! would grow this map forever, and since a refused connection never enters the
//! registry, nothing else would ever evict it. "Bounded by construction" would
//! have been false while the code looked entirely right.
//!
//! ⇒ **A map keyed on anything the refused party controls cannot be bounded by
//! construction.** So the key is `(task_queue, node)`, both server-derived and
//! both already enumerated by the gate from the deployed catalog — the map
//! cannot hold more entries than the catalog has queue/node pairs — and the
//! client's part rides in the VALUE, where it can inform but never allocate.
//!
//! One consequence is accepted rather than capped: two DIFFERENTLY broken
//! workers alternating on one queue flip the digest on every dial, so both are
//! named every time. In that shape the refusal reason genuinely changes each
//! dial, so every message carries information the previous one did not. Any
//! mechanism quiet enough to suppress it would suppress new information by
//! design, which is the defect this whole change exists to kill.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Mutex;

/// The server-derived identity of a refusal site.
type SiteKey = (String, Option<String>);

/// Remembers the last contract refusal per `(task_queue, node)`, so an
/// unchanged one can be met with silence.
///
/// Shared by every registration transport through the connected-worker
/// registry: the registry knows who was admitted, this knows who was turned
/// away, and both callers of the admission gate already hold it. One shared
/// record is what stops the two transports drifting apart again — the whole
/// finding behind #147 was one rule maintained in two places.
#[derive(Debug, Default)]
pub struct AdmissionAudit {
    /// Last refusal digest per site. Bounded by the catalog's queue/node pairs.
    last: Mutex<HashMap<SiteKey, u64>>,
}

impl AdmissionAudit {
    /// A fresh audit remembering nothing, so the first refusal always speaks.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether this refusal should be named, recording it as the site's latest.
    ///
    /// Returns `true` when the site has no remembered refusal, or when this
    /// refusal differs from the remembered one in either the worker build
    /// identity or the diagnosis itself.
    ///
    /// A poisoned lock **still names the refusal**. Declining to log because an
    /// unrelated thread panicked would turn one failure into exactly the silent
    /// refusal this module exists to prevent, so the guard is recovered and the
    /// call proceeds. Failing to speak is never the safe direction here.
    pub fn should_name(&self, task_queue: &str, node: Option<&str>, digest_of: &str) -> bool {
        let key = (task_queue.to_owned(), node.map(ToOwned::to_owned));
        let mut hasher = DefaultHasher::new();
        digest_of.hash(&mut hasher);
        let digest = hasher.finish();
        let mut last = match self.last.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        last.insert(key, digest) != Some(digest)
    }

    /// Forget this site's refusal because a worker was just admitted for it.
    ///
    /// A worker that is fixed, admitted, and then breaks again must be heard
    /// immediately rather than silenced by its own history.
    pub fn clear_admitted(&self, task_queue: &str, node: Option<&str>) {
        let key = (task_queue.to_owned(), node.map(ToOwned::to_owned));
        let mut last = match self.last.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        last.remove(&key);
    }

    /// Forget every site on a queue that no longer holds any reachable contract.
    ///
    /// This is hygiene, not the bound: the catalog already bounds the map, so
    /// the map stays finite even if this is never called. It exists so a queue
    /// whose last version is unloaded does not keep a fossil refusal that would
    /// silence the first refusal after it is redeployed.
    pub fn clear_queue(&self, task_queue: &str) {
        let mut last = match self.last.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        last.retain(|(queue, _), _| queue != task_queue);
    }

    /// How many sites are remembered. For tests asserting the bound holds.
    #[must_use]
    pub fn remembered_sites(&self) -> usize {
        match self.last.lock() {
            Ok(guard) => guard.len(),
            Err(poisoned) => poisoned.into_inner().len(),
        }
    }
}

#[cfg(test)]
#[path = "admission_audit_tests.rs"]
mod admission_audit_tests;
