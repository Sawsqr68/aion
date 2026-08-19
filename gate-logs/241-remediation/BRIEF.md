# #241 remediation — discharge the Opus review of record (VERDICT: FAIL)

You are remediating an existing build, not starting one. The branch
`241-final-message` in this worktree carries three commits (red pin
cccef3c2d → carry d0571f176 → contract enforcement 4ca454d9c) that add
`final_message` as a distinct field on the canonical AgentOutcome record
with REPLACE semantics. An Opus review of record (verdict FAIL,
2026-08-15 02:40Z) found the core work good and bypass-free but
incomplete on contract completeness. The FULL review text follows this
brief in your prompt — every finding carries file:line. Your charter is
the review's own "What must happen before this lands" list plus the
dispatcher's addendum, restated here with dispatch rulings applied.

## The charter

1. **Fix the three Gleam constructor sites** in the example packages
   still passing three fields (Gleam constructors are total; both
   packages resolve aion_flow by path, so `gleam test` in
   examples/assistant and examples/agent-dev is currently a compile
   error). Run `gleam test` for all three packages; commit the logs.

2. **Build the Gleam gate.** The review offered two arms (gate in
   battery.sh, or Gleam-ungated stated unmissably). The dispatch takes
   the GATE arm — the disclaimer arm is not available to this
   remediation. Add an announced Gleam gate to the battery so the
   CRITICAL-1 breakage class is observable by the record, prove it red
   on the pre-fix tree state or by reintroducing one three-field
   constructor (commit the red log), then green after the fix.

3. **Fix every stale-contract site the review names** (IMPORTANT-1
   through IMPORTANT-4 and IMPORTANT-6, file:line in the review):
   aion-integrations/src/error.rs:82 rustdoc; build-deck.py:261-264 and
   build-tutorial.py:690-693 PLUS regenerating both HTML outputs;
   examples/agent-dev/worker/src/main.rs:10; both activities.gleam;
   examples/agent-dev/README.md:24; harness.gleam:27; the
   valid/agent_seam.awl and invalid/agent_returns_the_wrong_record.awl
   fixture headers; retired_doctrine_gate.rs:195's husk sample.

4. **Restore the corpus-gate assertion** (IMPORTANT-5):
   awl_corpus_gate.rs:249 asserts the full four-field phrase the
   diagnostic actually emits, not a one-field substring.

5. **Assert `final_message` directly** in acp_live_agent.rs:226-237
   (the only real-agent E2E) and in norn translate/tests/outcomes.rs
   (`outcome.final_message == outcome.text` at record level)
   (SUGGESTION-1/-2 — taken as required by the dispatch).

6. **Document the in-flight replay consequence** (SUGGESTION-3, taken
   as required): RULING-AGENT-ACTION-WIRE-FORM.md must state that a
   three-field agent result already recorded in a run's history
   hard-fails decode after the swap — the "no longer decodes" line
   gains the operational consequence.

7. **Decode census (dispatcher addendum, mandatory):** census every
   site that DECODES a historical AgentOutcome payload — replay,
   resume, describe, console read paths. State precisely which runs a
   binary swap breaks: in-flight only, or completed-history reads too.
   Pin the deploy constraint in RULING-AGENT-ACTION-WIRE-FORM.md: the
   binary must not swap while any run's history holds a three-field
   agent result unless the read path for it is proven unused. The
   census is enumerated in a committed record (file list, one verdict
   per site), not asserted in prose.

8. **One complete workspace battery on the final tree**, output to a
   file, manifest read. Charter item 8 (file three battery-defect
   tickets) is ALREADY DISCHARGED by the dispatcher — tasks #251
   (announce anchor race), #252 (TS proto stub regen), #253 (bun test
   vs declared vitest) exist; do not re-file. Those three blockers are
   dated and attributed: if the battery shows a FOURTH red, it belongs
   to this diff and must be fixed, not attributed.

## House laws

Worktree-only (this tree), commit-as-you-go on branch
241-final-message, never push, no lint bypasses (#[allow]/#[expect]/
#[ignore]/_var renames are refusals, not fixes), ≤500 LOC per file,
clippy ×2 (--workspace AND -p per touched crate) + fmt clean, all
battery/gate output to files under gate-logs/241-remediation/ in this
worktree, box load recorded beside any red. Do not claim completion or
verification without direct evidence: a claim of a green run names its
log file.
