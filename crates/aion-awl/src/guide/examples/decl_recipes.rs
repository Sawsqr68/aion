//! The recipe cluster's example documents.
//!
//! Three documents carry the whole assembly surface, and they are the three
//! things an assembly declares rather than three arbitrary samples: what is
//! reached for outside the document (a wall and an artifact), what is
//! supplied to it (parameter kinds, including the one that is re-derived at
//! the fire minute), and what it does in order (steps and their dependency
//! edges). Every word of the cluster points at whichever one demonstrates it
//! doing its actual job.
//!
//! All three run through the crate's real parse and check pipeline and are
//! pinned CANONICAL against the printer, so none can teach a layout `aion awl
//! fmt` would immediately rewrite.

/// The declarations an assembly reaches for outside itself.
///
/// One wall, declared once, projected into every rendering; one artifact,
/// declared with the content digest the reference census measures it
/// against, and with the sentinel that says it was never filled.
pub(crate) const ASSEMBLY_SOURCES: &str = r#"//! Declare the wall a round enforces and the artifact its census measures.

workflow staging_sources
  outcome staged: type String, route success

command diff_census(base_pin: String)
  program "git" "diff"
  refuses "--"
  specimen "-x"
  option "--name-only"
  operand range: String = "{{base_pin}}..HEAD"

/// One wall, one home: this declaration is projected into the prompt a role
/// reads AND into the mechanical census, so the two cannot disagree.
wall changed_paths
  allows "crates/aion-awl"
  allows ".fleet/"
  excludes "mutation vocabulary"

/// The gate the flight fires, declared with the digest the census measures
/// its deployed bytes against.
artifact gate_entry ".fleet/gate-entry.sh"
  digest "1e1e3ad2ae66bb0bef5b82a8e154491032ee3d7598ac361a731a5f0917b47a6b"
  excludes "RECORD-RUN-PENDING"

recipe measure_the_tree
  parameter base_pin: String per_item
  step census
    runs command diff_census

step announce
  route staged("sources declared")"#;

/// The four parameter kinds, and the derivation the fourth one carries.
///
/// `skeleton` is fixed across every dispatch of the rig, `per_item` is
/// supplied per dispatch, `both` is partly each — and `re_derive` is read
/// from its declared source AT THE FIRE MINUTE, not the compose minute,
/// because the two drift.
pub(crate) const RECIPE_PARAMETERS: &str = r#"//! Declare what a rig supplies once, what it supplies per dispatch, and what
//! it re-derives.

workflow investigation_rig
  outcome composed: type String, route success

command open_clone(node: String)
  program "git" "clone"
  admits "--"
  operand target: String = "{{node}}"

recipe compose_investigation
  parameter tracker: String = "github" skeleton
  parameter issue: String per_item
  parameter investigate_prompt: String both
  parameter control_expect: String re_derive derives "origin/main:workflows/investigate/investigate_worker.awl" extract "line 22"
  step clone
    runs command open_clone

step announce
  route composed("input declared")"#;

/// Steps, their dependency edges, their at-fire preconditions, and the two
/// step bodies that reach an artifact.
///
/// The edges are a DAG and the order a document happens to write the steps
/// in is not the order they run: the edges are. A `fires` step compares a
/// declared demonstration artifact against a known answer; a `captures` step
/// reads one into a typed result.
pub(crate) const RECIPE_STEPS: &str = r#"//! Order the acts of a staging assembly, and say what each one does.

workflow staging_assembly
  outcome fired: type String, route success

command push_branch(branch: String)
  program "git" "push"
  admits "--"
  operand target: String = "{{branch}}"

artifact known_answer_red ".fleet/gate-entry.sh"
  digest "77f0333fcea03ada0c017af9fdea725231c3e2aecce8ade71781571daff9115d"
  restores ".fleet/gate-entry.sh"

artifact operating_ground "dispatch-rigs/sleepreap/OPERATING-GROUND.md"
  digest "032ee3d7598ac361a731a5f0917b47a6b1ea7c32c3cc3b9670b3c11c32db23b7"

recipe stage_the_flight
  parameter branch: String per_item
  step read_ground
    captures ground: String of artifact operating_ground
  step self_test
    after read_ground
    requires "workers serving the queue"
    fires artifact known_answer_red expecting "deliverable_absent"
  step push
    after self_test
    runs command push_branch

step announce
  route fired("stage declared")"#;
