//! Unit tests for the refusal-transition rule.
//!
//! Two properties carry the design and both are asserted here directly rather
//! than inferred from behaviour that happens to look right:
//!
//! 1. **The rule is a transition.** A refusal speaks when it is new or
//!    different and is silent only when it says exactly what the last one said.
//!    Every silence in these tests is paired with a demonstration that the same
//!    site speaks again the moment anything about the refusal changes.
//! 2. **The map is bounded by construction.** Not "bounded in practice", not
//!    "bounded because nothing does that" — the refused party is given a
//!    thousand distinct identities and the map still holds one entry, because
//!    the key contains nothing the refused party supplies.

use super::AdmissionAudit;

const QUEUE: &str = "payments";
const OTHER_QUEUE: &str = "billing";

#[test]
fn the_first_refusal_at_a_site_is_always_named() {
    let audit = AdmissionAudit::new();
    assert!(
        audit.should_name(QUEUE, None, "worker-a", "charge/2 arity"),
        "a site with no remembered refusal has nothing to have already said"
    );
}

#[test]
fn an_identical_repeat_carries_nothing_new_so_it_is_silence() {
    let audit = AdmissionAudit::new();
    let (worker, reason) = ("worker-a", "charge/2 arity");

    assert!(
        audit.should_name(QUEUE, None, worker, reason),
        "the first must speak"
    );
    for dial in 1..=5 {
        assert!(
            !audit.should_name(QUEUE, None, worker, reason),
            "redial {dial} repeated the first line verbatim; #94 measured that \
             shape at 2/second and 12 MB/hour of identical diagnosis"
        );
    }
}

#[test]
fn a_changed_reason_at_the_same_site_speaks_again() {
    let audit = AdmissionAudit::new();
    let worker = "worker-a";
    let arity = "charge/2 arity";
    let return_type = "charge/2 return type";

    assert!(audit.should_name(QUEUE, None, worker, arity));
    assert!(!audit.should_name(QUEUE, None, worker, arity));
    assert!(
        audit.should_name(QUEUE, None, worker, return_type),
        "the operator fixed one disagreement and hit the next; silence here \
         would read as 'the fix worked' while the queue stayed unservable"
    );
    assert!(
        !audit.should_name(QUEUE, None, worker, return_type),
        "and the new reason then settles into silence in its turn"
    );
}

#[test]
fn a_changed_build_identity_speaks_again_even_on_the_same_reason() {
    let audit = AdmissionAudit::new();
    let reason = "charge/2 arity";

    assert!(audit.should_name(QUEUE, None, "build-1", reason));
    assert!(
        audit.should_name(QUEUE, None, "build-2", reason),
        "a REDEPLOYED worker failing the same way is news: it says the rebuild \
         did not take, which is a different fact from the original refusal"
    );
}

#[test]
fn alternating_refusals_both_speak_every_time_and_the_map_stays_at_one() {
    let audit = AdmissionAudit::new();
    let (broken_one, its_reason) = ("worker-a", "charge/2 arity");
    let (broken_other, other_reason) = ("worker-b", "refund/1 missing");

    for round in 0..20 {
        assert!(
            audit.should_name(QUEUE, None, broken_one, its_reason),
            "round {round}: the reason genuinely changed since the last dial"
        );
        assert!(
            audit.should_name(QUEUE, None, broken_other, other_reason),
            "round {round}: and changed back"
        );
    }
    assert_eq!(
        audit.remembered_sites(),
        1,
        "two differently broken workers on one queue are still ONE site — the \
         accepted consequence is log volume, never memory growth"
    );
}

#[test]
fn nodes_on_one_queue_are_separate_sites() {
    let audit = AdmissionAudit::new();
    let (worker, reason) = ("worker-a", "charge/2 arity");

    assert!(audit.should_name(QUEUE, Some("node-1"), worker, reason));
    assert!(
        audit.should_name(QUEUE, Some("node-2"), worker, reason),
        "a second node refusing for the same reason is a second broken \
         deployment, and an operator who only heard about node-1 would fix \
         one and believe the queue recovered"
    );
    assert!(!audit.should_name(QUEUE, Some("node-1"), worker, reason));
    assert!(!audit.should_name(QUEUE, Some("node-2"), worker, reason));
    assert_eq!(audit.remembered_sites(), 2);
}

#[test]
fn an_unpinned_refusal_is_a_different_site_from_a_node_pinned_one() {
    let audit = AdmissionAudit::new();
    let (worker, reason) = ("worker-a", "charge/2 arity");

    assert!(audit.should_name(QUEUE, None, worker, reason));
    assert!(
        audit.should_name(QUEUE, Some("node-1"), worker, reason),
        "`None` is not a wildcard that swallows every node's first refusal"
    );
    assert_eq!(audit.remembered_sites(), 2);
}

#[test]
fn queues_are_separate_sites() {
    let audit = AdmissionAudit::new();
    let (worker, reason) = ("worker-a", "charge/2 arity");

    assert!(audit.should_name(QUEUE, None, worker, reason));
    assert!(audit.should_name(OTHER_QUEUE, None, worker, reason));
    assert!(!audit.should_name(QUEUE, None, worker, reason));
    assert_eq!(audit.remembered_sites(), 2);
}

#[test]
fn admission_forgets_the_site_so_a_later_break_is_heard_immediately() {
    let audit = AdmissionAudit::new();
    let (worker, reason) = ("worker-a", "charge/2 arity");

    assert!(audit.should_name(QUEUE, None, worker, reason));
    assert!(!audit.should_name(QUEUE, None, worker, reason));

    audit.clear_admitted(QUEUE, None);
    assert_eq!(audit.remembered_sites(), 0);
    assert!(
        audit.should_name(QUEUE, None, worker, reason),
        "the queue was served and then broke again the same way; silencing \
         that by its own history is the defect wearing a memory"
    );
}

#[test]
fn admission_on_one_node_leaves_the_other_nodes_remembering() {
    let audit = AdmissionAudit::new();
    let (worker, reason) = ("worker-a", "charge/2 arity");

    assert!(audit.should_name(QUEUE, Some("node-1"), worker, reason));
    assert!(audit.should_name(QUEUE, Some("node-2"), worker, reason));

    audit.clear_admitted(QUEUE, Some("node-1"));
    assert!(
        audit.should_name(QUEUE, Some("node-1"), worker, reason),
        "node-1 was served, so its next refusal is news again"
    );
    assert!(
        !audit.should_name(QUEUE, Some("node-2"), worker, reason),
        "node-2 was never served and is still saying what it already said"
    );
}

#[test]
fn clearing_a_site_that_was_never_refused_is_harmless() {
    let audit = AdmissionAudit::new();
    audit.clear_admitted(QUEUE, Some("node-1"));
    audit.clear_queue(OTHER_QUEUE);
    assert_eq!(audit.remembered_sites(), 0);
}

#[test]
fn clearing_a_queue_takes_all_its_nodes_and_leaves_other_queues_alone() {
    let audit = AdmissionAudit::new();
    let (worker, reason) = ("worker-a", "charge/2 arity");

    assert!(audit.should_name(QUEUE, None, worker, reason));
    assert!(audit.should_name(QUEUE, Some("node-1"), worker, reason));
    assert!(audit.should_name(QUEUE, Some("node-2"), worker, reason));
    assert!(audit.should_name(OTHER_QUEUE, Some("node-1"), worker, reason));
    assert_eq!(audit.remembered_sites(), 4);

    audit.clear_queue(QUEUE);
    assert_eq!(
        audit.remembered_sites(),
        1,
        "unloading a queue's last version drops every fossil refusal on it, \
         node-pinned or not, and touches no other queue"
    );
    assert!(
        !audit.should_name(OTHER_QUEUE, Some("node-1"), worker, reason),
        "the surviving entry is the OTHER queue's, still remembered"
    );
}

#[test]
fn the_refused_party_cannot_grow_the_map() {
    let audit = AdmissionAudit::new();

    for dial in 0..1_000 {
        assert!(
            audit.should_name(QUEUE, None, &format!("worker-{dial}"), "charge/2 arity"),
            "dial {dial} advertised a new identity, which is genuinely a \
             different worker and so genuinely speaks"
        );
    }

    assert_eq!(
        audit.remembered_sites(),
        1,
        "a thousand client-chosen identities on one queue are one site. The \
         key holds only what the SERVER derives; the client's contribution \
         rides in the value where it can inform but never allocate"
    );
}

#[test]
fn a_queue_with_no_refusal_on_record_reports_none() {
    let audit = AdmissionAudit::new();
    assert!(audit.should_name(OTHER_QUEUE, None, "worker-a", "charge/2 arity"));
    assert!(
        audit.refusals_on_queue(QUEUE).is_empty(),
        "a queue nobody was refused on must report nothing, or the availability \
         hint would blame this queue for another queue's outage"
    );
}

#[test]
fn the_refusals_on_a_queue_carry_the_reason_and_the_node() {
    let audit = AdmissionAudit::new();
    assert!(audit.should_name(QUEUE, Some("node-2"), "worker-b", "refund/1 missing"));
    assert!(audit.should_name(QUEUE, Some("node-1"), "worker-a", "charge/2 arity"));
    assert!(audit.should_name(OTHER_QUEUE, None, "worker-c", "unrelated"));

    let refusals = audit.refusals_on_queue(QUEUE);
    assert_eq!(refusals.len(), 2, "both nodes' refusals: {refusals:?}");
    assert_eq!(
        refusals
            .iter()
            .map(|refusal| refusal.node.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("node-1"), Some("node-2")],
        "node-ordered, so an operator reading two refusals reads them the same \
         way twice: {refusals:?}"
    );
    assert_eq!(refusals[0].identity, "worker-a");
    assert_eq!(
        refusals[0].reason, "charge/2 arity",
        "the REASON is what makes the hint a diagnosis instead of a guess in \
         the grammar of one: {refusals:?}"
    );
}

#[test]
fn a_served_queue_stops_reporting_the_refusal_it_recovered_from() {
    let audit = AdmissionAudit::new();
    assert!(audit.should_name(QUEUE, None, "worker-a", "charge/2 arity"));
    assert_eq!(audit.refusals_on_queue(QUEUE).len(), 1);

    audit.clear_admitted(QUEUE, None);
    assert!(
        audit.refusals_on_queue(QUEUE).is_empty(),
        "the worker was fixed and admitted; a hint still reporting the old \
         refusal would send the operator after a problem that is over"
    );
}

#[test]
fn a_poisoned_lock_still_names_the_refusal() {
    let audit = AdmissionAudit::new();
    let (worker, reason) = ("worker-a", "charge/2 arity");
    assert!(audit.should_name(QUEUE, None, worker, reason));

    // An unwind IS the fixture here: std poisons a `Mutex` only when a thread
    // dies holding its guard, and exposes no other way to reach that state. The
    // unwind is produced by an assertion WRITTEN TO FAIL — the harness's own
    // failure mechanism — rather than by a hand-rolled panic, so the test needs
    // no exemption from the crate's ban on panicking library code.
    let died = std::thread::scope(|scope| {
        scope
            .spawn(|| {
                // Bound, not discarded: the guard must still be alive when the
                // unwind starts, or the lock is never poisoned.
                let held = audit.last.lock();
                assert!(
                    held.is_err(),
                    "deliberate: this assertion is written to FAIL. The lock was \
                     healthy and is held right now, so failing here unwinds \
                     through the live guard and poisons it — which is the state \
                     the assertions after the join are about"
                );
            })
            .join()
    });

    assert!(
        died.is_err(),
        "the fixture thread must actually have panicked — if it returned \
         normally the lock is healthy and every assertion below is vacuous"
    );
    assert!(
        audit.last.is_poisoned(),
        "and the panic must have poisoned the lock, or this test is measuring \
         the ordinary path under a dramatic name"
    );

    assert!(
        audit.should_name(QUEUE, None, "worker-a", "refund/1 missing"),
        "a changed reason after an unrelated thread died must still be heard: \
         declining to log on a poisoned lock would turn one panic into exactly \
         the silent refusal this module exists to prevent"
    );
    assert!(
        !audit.should_name(QUEUE, None, "worker-a", "refund/1 missing"),
        "and the transition rule still holds through the poison — recovery is \
         not amnesia"
    );
    audit.clear_admitted(QUEUE, None);
    assert_eq!(
        audit.remembered_sites(),
        0,
        "every method recovers the guard, not just the one on the hot path"
    );
}
