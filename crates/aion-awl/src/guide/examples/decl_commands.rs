//! The command-declaration cluster's example documents.
//!
//! Two documents carry the whole surface, and they are the two faces of the
//! declared end-of-options property rather than two arbitrary samples. Every
//! word of the cluster points at whichever one demonstrates it, so an author
//! who follows any of the thirteen words arrives at a document that shows the
//! word doing its actual job beside the clauses it works with.
//!
//! Both are run through the crate's real parse and check pipeline by
//! `every_entry_example_passes_the_real_checker`, and both are pinned
//! CANONICAL against the printer, so neither can teach a layout `aion awl
//! fmt` would immediately rewrite.

/// The admitting face: `git tag -- <name> <sha>`.
///
/// The invocation declares that it admits the end-of-options marker, so the
/// emitter places `--` between the options and the operands unconditionally.
/// A tag name beginning with `-` is then a tag name, not an option — the
/// family closes by construction rather than by review.
pub(crate) const ADMITTING_COMMAND: &str = r#"//! Tag a commit by name, with the end-of-options marker its operands need.

workflow release_tagging
  outcome tagged: type String, route success

/// `git tag` admits the end-of-options marker, so a tag name beginning with a
/// dash is a name and never an option.
command tag_release(tag_name: String, commit: String)
  program "git" "tag"
  admits "--"
  operand name: String = "{{tag_name}}"
  operand sha: String = "{{commit}}"

step announce
  route tagged("tag declared")"#;

/// The refusing face: the diff census, plus the declared env, cwd, hardening
/// and timeout.
///
/// `git diff` must NOT get a `--` before a revision range: git would read
/// what follows as a pathspec and the census would silently measure something
/// else. So the invocation declares that it refuses the marker, and carries
/// the mandatory specimen that proves the leading-dash refusal fires.
pub(crate) const REFUSING_COMMAND: &str = r#"//! Census the files a flight changed, against a base pin it is given.

workflow gate_census
  outcome censused: type String, route success

/// `git diff` refuses the end-of-options marker: a `--` before a revision
/// range would reinterpret the range as a pathspec.
command diff_census(base_pin: String, cargo_bin: String)
  program "git" "diff"
  refuses "--"
  specimen "-x"
  option "--no-ext-diff"
  option "--name-only"
  operand range: String = "{{base_pin}}..HEAD"
  env GIT_PAGER = "cat"
  cwd "{workspace_root}"
  hardening
    path "{{cargo_bin}}"
    open_files 65536
  timeout 10m owner "the gate's author"

step census
  route censused("census declared")"#;
