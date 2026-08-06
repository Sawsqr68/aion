//! Whether a contract refusal is NEW INFORMATION, and therefore worth saying.
//!
//! A refused worker redials forever. Task #94 measured the liminal transport
//! re-logging the identical full diagnosis on every dial — 2/second, 12 MB/hour,
//! *"observability buried"*. Naming the refusal on the gRPC path too (#147 R1)
//! would have given that defect a second host, so the naming and the decision
//! to repeat it land together.
//!
//! **The rule is a transition, not a cap.** The full diagnosis is emitted when
//! the refusal for a [`RefusalSite`] is new or different; an identical
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
//! construction.** The client's part rides in the VALUE, where it can inform but
//! never allocate.
//!
//! ## The correction of 2026-08-06, and why it was needed
//!
//! The first cut of this module keyed on `(task_queue, node)` and its own doc
//! called both *"server-derived"*. **They are not.** Both arrive on the wire in
//! the worker's registration request, on both transports
//! (`worker_grpc.rs:652-653`, `liminal_transport.rs:1571-1572`). Moving
//! `identity` out of the key while leaving `node` in it defeated exactly half of
//! the trap the design had named, and the stated bound was false while — again —
//! the code looked entirely right.
//!
//! `task_queue` turned out to be bounded anyway, but by a mechanism nobody had
//! written down: a queue with no reachable deployed contract demands nothing, so
//! it ADMITS ([`aion::Engine::worker_contracts_for_admission`] returns an empty
//! admission) and never reaches this map. `node` had no such gate at all —
//! `dispatch_can_reach` demands every UNPINNED action of every node, so a
//! refused worker varying its advertised node grew the map without limit.
//!
//! ⇒ The site is now [`RefusalSite`], which the gate constructs from the
//! CATALOG rather than from the request. A node names a site only when some
//! demanded action is pinned to it; otherwise the refusal is node-independent —
//! every node owes the same actions and fails identically — and it is one site.
//! The bound is the catalog's: queues × (their pinned nodes + 1), plus one for
//! an unreadable catalog. **Nothing a refused party can say allocates an entry.**
//!
//! That collapse is not merely a bound; it is more truthful. Two connections
//! owing an identical action set and failing identically are the same fault, and
//! reporting them separately was #94 volume for no information.
//!
//! One consequence is accepted rather than capped: two DIFFERENTLY broken
//! workers alternating on one queue flip the value on every dial, so both are
//! named every time. In that shape the refusal reason genuinely changes each
//! dial, so every message carries information the previous one did not. Any
//! mechanism quiet enough to suppress it would suppress new information by
//! design, which is the defect this whole change exists to kill.

use std::collections::HashMap;
use std::sync::Mutex;

/// Where a refusal happened, in terms **the server controls**.
///
/// This is a type rather than a tuple so that a caller cannot pass the node it
/// read off the wire by accident: the distinguishing node is derived from the
/// deployed catalog by [`crate::worker::contracts::refusal_site`], and a raw
/// `Option<&str>` from a registration request looks exactly like a correct
/// argument at the call site. The first cut of this module had that bug.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RefusalSite {
    /// A queue's contract gate, at the locality the demanded actions
    /// distinguish. `node` is `Some` only when some action demanded of this
    /// connection is PINNED to that node; a node no demanded action mentions
    /// cannot change the verdict, so it does not name a site of its own.
    Queue {
        /// The queue whose deployed contracts produced the refusal.
        task_queue: String,
        /// The pinned node this refusal is specific to, `None` when the
        /// refusal holds identically for every locality.
        node: Option<String>,
    },
    /// The catalog itself could not be read.
    ///
    /// One site, not one per queue: the failure is server-wide and identical
    /// whatever queue was dialled, so keying it by the requested queue would
    /// let a client-chosen string allocate — the very thing this enum exists to
    /// prevent — and would re-log one server fault once per queue name tried.
    CatalogUnreadable,
}

/// The refusal a site is currently sitting in.
///
/// The whole refusal is kept rather than a hash of it because two callers need
/// it: the transition rule needs to know whether this refusal differs from the
/// last, and the start-time availability hint needs to be able to SAY what the
/// refusal was. A digest can answer the first question and nothing else, and a
/// hint that can only say "something was refused" is the guess-in-the-grammar-
/// of-a-diagnosis this change exists to delete.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RememberedRefusal {
    /// The pinned node this refusal is specific to, `None` when it holds
    /// identically for every locality.
    ///
    /// This is the site's node — derived from the catalog — and deliberately
    /// **not** the string the connection advertised. A refusal caused by an
    /// unpinned action is not about the node it happened to arrive on, and
    /// naming that node would send an operator to fix the wrong thing.
    pub node: Option<String>,
    /// The refused worker's build identity, as it advertised itself.
    pub identity: String,
    /// The gate's full rendering of why it refused.
    pub reason: String,
}

/// Remembers the last contract refusal per [`RefusalSite`], so an unchanged one
/// can be met with silence.
///
/// Shared by every registration transport through the connected-worker
/// registry: the registry knows who was admitted, this knows who was turned
/// away, and both callers of the admission gate already hold it. One shared
/// record is what stops the two transports drifting apart again — the whole
/// finding behind #147 was one rule maintained in two places.
#[derive(Debug, Default)]
pub struct AdmissionAudit {
    /// Last refusal per site. Bounded by the catalog: its queues, times their
    /// pinned nodes plus one, plus one for an unreadable catalog. Every
    /// component is server-derived — see [`RefusalSite`].
    last: Mutex<HashMap<RefusalSite, RememberedRefusal>>,
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
    pub fn should_name(&self, site: &RefusalSite, identity: &str, reason: &str) -> bool {
        let refusal = RememberedRefusal {
            node: match site {
                RefusalSite::Queue { node, .. } => node.clone(),
                RefusalSite::CatalogUnreadable => None,
            },
            identity: identity.to_owned(),
            reason: reason.to_owned(),
        };
        let mut last = match self.last.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        last.insert(site.clone(), refusal.clone()) != Some(refusal)
    }

    /// Every refusal currently remembered for `task_queue`, node-ordered.
    ///
    /// Read by the start-time availability hint. A queue with no connected
    /// worker and a refusal on record is not an unstarted queue — it is a queue
    /// whose worker is running and being turned away — and the two cannot be
    /// told apart from the connected-worker count, because a refused connection
    /// never reaches the registry.
    #[must_use]
    pub fn refusals_on_queue(&self, task_queue: &str) -> Vec<RememberedRefusal> {
        let last = match self.last.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let mut refusals = last
            .iter()
            .filter_map(|(site, refusal)| match site {
                RefusalSite::Queue {
                    task_queue: queue, ..
                } if queue == task_queue => Some(refusal.clone()),
                RefusalSite::Queue { .. } | RefusalSite::CatalogUnreadable => None,
            })
            .collect::<Vec<_>>();
        refusals.sort_by(|left, right| left.node.cmp(&right.node));
        refusals
    }

    /// Forget this site's refusal because a worker was just admitted for it.
    ///
    /// A worker that is fixed, admitted, and then breaks again must be heard
    /// immediately rather than silenced by its own history.
    pub fn clear_admitted(&self, site: &RefusalSite) {
        let mut last = match self.last.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        last.remove(site);
    }

    /// Forget every site on a queue that no longer holds any reachable contract.
    ///
    /// Called by the gate when a queue's admission demands nothing of anyone: a
    /// remembered refusal was recorded when the catalog still demanded
    /// something, so once it demands nothing that refusal is a fossil, and a
    /// fossil would silence the FIRST refusal after the queue is redeployed.
    ///
    /// Pruning is lazy and costs one pass on a call the gate was making anyway
    /// — no timer, no sweeper. It cannot reach a queue nobody dials again,
    /// which is why the bound is structural rather than resting on this.
    pub fn clear_queue(&self, task_queue: &str) {
        let mut last = match self.last.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        last.retain(|site, _| match site {
            RefusalSite::Queue {
                task_queue: queue, ..
            } => queue != task_queue,
            RefusalSite::CatalogUnreadable => true,
        });
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
