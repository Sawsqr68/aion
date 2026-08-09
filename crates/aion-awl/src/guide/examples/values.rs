//! Examples for the value vocabulary: collection combinators, predicates,
//! and the boolean connectives.

/// `filter`: a field predicate, and a compound one.
pub(crate) const FILTER: &[&str] = &[
    r"//! Keep only the findings the predicate accepts.
workflow keep_blockers
  input findings: [Finding]
  outcome done: type [Finding], route success

type Finding { title: String, blocking: Bool }

step sift
  findings |> filter(.blocking) -> blockers
  route done(blockers)",
    r#"//! A filter stage inside a longer pipe, feeding a tally.
workflow measure_noise
  input findings: [Finding]
  outcome done:   type String, route success
  outcome failed: type String, route failure

type Finding { resolved: Bool }

step measure
  findings |> filter(.resolved) |> count -> settled
  findings |> count -> total
  outcome all_settled: when settled == total, route done("all settled")
  outcome open_items: otherwise, route failed("unresolved findings remain")"#,
];

/// `map`: a field projection, and a projection chained after a filter.
pub(crate) const MAP: &[&str] = &[
    r"//! Rewrite each element through a deterministic expression.
workflow list_titles
  input findings: [Finding]
  outcome done: type [String], route success

type Finding { title: String, blocking: Bool }

step rename
  findings |> map(.title) -> titles
  route done(titles)",
    r"//! Combinators chain: filter first, then project the titles.
workflow blocker_titles
  input findings: [Finding]
  outcome done: type [String], route success

type Finding { title: String, blocking: Bool }

step summarize
  findings |> filter(.blocking) |> map(.title) -> titles
  route done(titles)",
];

/// `sort`: ranking by a key, and taking the front of the ordered list.
pub(crate) const SORT: &[&str] = &[
    r"//! Order a collection by a comparable key.
workflow rank_findings
  input findings: [Finding]
  outcome done: type [Finding], route success

type Finding { title: String, severity: Int }

step rank
  findings |> sort(.severity) -> ranked
  route done(ranked)",
    r"//! Sort by a key, then take the front of the ordered list.
workflow pick_leader
  input candidates: [Candidate]
  outcome done: type Candidate, route success

type Candidate { name: String, score: Int }

step choose
  candidates |> sort(.score) -> ranked
  ranked[0] -> leader
  route done(leader)",
];

/// `count`: one tally, and two tallies compared in a guard.
pub(crate) const COUNT: &[&str] = &[
    r"//! Count a collection's elements.
workflow tally_findings
  input findings: [Finding]
  outcome done: type Int, route success

type Finding { title: String }

step tally
  findings |> count -> total
  route done(total)",
    r#"//! Two tallies, compared in a guard.
workflow judge_sweep
  input receipts: [Receipt]
  outcome done:   type String, route success
  outcome failed: type String, route failure

type Receipt { ok: Bool }

step judge
  receipts |> count -> attempted
  receipts |> filter(.ok) |> count -> succeeded
  outcome full: when succeeded == attempted, route done("all landed")
  outcome partial: otherwise, route failed("some failed")"#,
];

/// `any`: a field predicate, and a compound one.
pub(crate) const ANY: &[&str] = &[
    r#"//! True when at least one element satisfies the predicate.
workflow spot_blocker
  input findings: [Finding]
  outcome done: type String, route success

type Finding { blocking: Bool }

step look
  findings |> any(.blocking) -> has_blocker
  outcome blocked: when has_blocker, route done("blocked")
  outcome clear: otherwise, route done("clear")"#,
    r#"//! The predicate can be compound.
workflow flag_hard_blockers
  input findings: [Finding]
  outcome done: type String, route success

type Finding { blocking: Bool, severity: String }

step look
  findings |> any(.blocking and .severity == "blocker") -> has_hard_blocker
  outcome blocked: when has_hard_blocker, route done("hard blocker present")
  outcome clear: otherwise, route done("no hard blockers")"#,
];

/// `all`: a field predicate, and a disjunctive one.
pub(crate) const ALL: &[&str] = &[
    r#"//! True when every element satisfies the predicate.
workflow verify_receipts
  input receipts: [Receipt]
  outcome done: type String, route success

type Receipt { ok: Bool }

step verify
  receipts |> all(.ok) -> all_ok
  outcome good: when all_ok, route done("every receipt landed")
  outcome partial: otherwise, route done("at least one failed")"#,
    r#"//! Universal claims read well with `or`: passed, or explicitly waived.
workflow verify_waivers
  input checks: [Check]
  outcome done: type String, route success

type Check { passed: Bool, waived: Bool }

step verify
  checks |> all(.passed or .waived) -> acceptable
  outcome good: when acceptable, route done("release may proceed")
  outcome bad: otherwise, route done("unwaived failure present")"#,
];

/// `is`: emptiness, and presence.
pub(crate) const IS: &[&str] = &[
    r#"//! `is` opens a predicate: `is empty`, `is present`, or `is absent`.
workflow check_queue
  input items: [String]
  outcome done: type String, route success

step look
  outcome idle: when items is empty, route done("idle")
  outcome busy: otherwise, route done("busy")"#,
    r"//! `is present` proves an optional before it is read.
workflow read_manifest
  input manifest: Manifest
  outcome done: type String, route success

type Manifest { name: String, notes: String? }

step read
  outcome noted: when manifest.notes is present, route done(manifest.notes)
  outcome bare: otherwise, route done(manifest.name)",
];

/// `empty`: an input list, and a filtered list.
pub(crate) const EMPTY: &[&str] = &[
    r#"//! The predicate for a collection with nothing in it.
workflow judge_review
  input findings: [String]
  outcome clean:   type String, route success
  outcome blocked: type String, route failure

step judge
  outcome pass: when findings is empty, route clean("no findings")
  outcome fail: otherwise, route blocked("findings remain")"#,
    r#"//! Emptiness after filtering: no blockers left means clear.
workflow clear_to_land
  input findings: [Finding]
  outcome done:   type String, route success
  outcome failed: type String, route failure

type Finding { blocking: Bool }

step decide
  findings |> filter(.blocking) -> blockers
  outcome clear: when blockers is empty, route done("clear to land")
  outcome blocked: otherwise, route failed("blockers remain")"#,
];

/// `present`: one proof, and a chained deep proof.
pub(crate) const PRESENT: &[&str] = &[
    r"//! Prove the optional present; the proof narrows `T?` to `T` in the arm.
workflow read_note
  input finding: Finding
  outcome done: type String, route success

type Finding { title: String, note: String? }

step read
  outcome noted: when finding.note is present, route done(finding.note)
  outcome bare: otherwise, route done(finding.title)",
    r"//! Proofs chain: the first makes the record real, the second its field.
workflow read_deep_note
  input report: Report
  outcome done: type String, route success

type Detail { note: String? }
type Report { title: String, detail: Detail? }

step read
  outcome noted: when report.detail is present and report.detail.note is present,
    route done(report.detail.note)
  outcome bare: otherwise, route done(report.title)",
];

/// `absent`: a field never written, and a timed wait that lapsed.
pub(crate) const ABSENT: &[&str] = &[
    r#"//! Prove the optional absent, which is what omission looks like.
workflow chase_signoff
  input review: Review
  outcome done: type String, route success

type Review { change: String, verdict: String? }

step chase
  outcome pending: when review.verdict is absent, route done("chase " + review.change)
  outcome settled: otherwise, route done(review.change)"#,
    r#"//! Expiry IS absence: a lapsed timed wait proves the binding absent.
workflow chase_reply
  input ticket: String
  signal reply: Reply
  outcome done:      type String, route success
  outcome escalated: type String, route failure

type Reply { text: String }

step hold
  wait reply timeout 1d -> response
  outcome silent: when response is absent, route escalated("no reply: " + ticket)
  outcome answered: otherwise, route done("answered: " + ticket)"#,
];

/// `not`: a guard negation, and negation inside a collection predicate.
pub(crate) const NOT: &[&str] = &[
    r#"//! Negate a boolean expression.
workflow gate_ship
  input checks_passed: Bool
  outcome held:    type String, route failure
  outcome shipped: type String, route success

step decide
  outcome hold: when not checks_passed, route held("checks failed")
  outcome go: otherwise, route shipped("shipped")"#,
    r#"//! Negation inside a quantifier's predicate.
workflow find_open_items
  input findings: [Finding]
  outcome done: type String, route success

type Finding { resolved: Bool }

step look
  findings |> any(not .resolved) -> has_open
  outcome open: when has_open, route done("open items remain")
  outcome settled: otherwise, route done("all resolved")"#,
];

/// `and`: a proof-carrying chain, and a plain conjunction.
pub(crate) const AND: &[&str] = &[
    r#"//! Both conjuncts must hold; each proof carries to the operands after it.
workflow admit_report
  input report: Report
  outcome done: type String, route success

type Report { blocking: Bool, detail: String? }

step admit
  outcome real: when report.blocking and report.detail is present, route done(report.detail)
  outcome noise: otherwise, route done("nothing blocking")"#,
    r#"//! Plain conjunction: both flags must hold.
workflow admit_release
  input build: Build
  outcome done:   type String, route success
  outcome failed: type String, route failure

type Build { tests_green: Bool, signed: Bool }

step admit
  outcome ship: when build.tests_green and build.signed, route done("shipped")
  outcome hold: otherwise, route failed("not ready")"#,
];

/// `or`: a plain disjunction, and the absence-narrowing false edge.
pub(crate) const OR: &[&str] = &[
    r#"//! Either disjunct admits; absence proofs hold on the false edge.
workflow accept_change
  input change: Change
  outcome done: type String, route success

type Change { approved: Bool, urgent: Bool, title: String }

step judge
  outcome go: when change.approved or change.urgent, route done(change.title)
  outcome no: otherwise, route done("declined: " + change.title)"#,
    r#"//! On the false edge of `is absent`, the right side reads the value safely.
workflow honor_waiver
  input request: Request
  outcome done: type String, route success

type Waiver  { approved: Bool }
type Request { title: String, waiver: Waiver? }

step judge
  outcome waved: when request.waiver is absent or request.waiver.approved,
    route done("proceed: " + request.title)
  outcome held: otherwise, route done("held: " + request.title)"#,
];
