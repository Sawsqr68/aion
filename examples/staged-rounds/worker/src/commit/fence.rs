//! The staging fence: which UNTRACKED files a mechanical commit may adopt.
//!
//! WHY IT EXISTS, measured. Every mechanical commit used to stage with
//! `git add -A`. A gate battery runs the operator's real commands, `cargo
//! build` creates an untracked `target/`, and `-A` swept the whole of it into
//! the item's commit. On the live run of 2026-07-29 the three mechanical
//! commits on one item branch carried 98, 97 and 94 files, of which 96, 96 and
//! 94 were under `target/`. Both review lenses then correctly rejected the
//! item, both rounds, for breaching its scope fence — the review layer worked
//! perfectly, and no repository whose build output is not already gitignored
//! could ever converge.
//!
//! THE RULE THE FENCE IMPLEMENTS. Tracked modifications and deletions are
//! ALWAYS staged, everywhere in the worktree, fence or no fence: a change to a
//! file that already exists is a real change to real source, and hiding it
//! would make the branch disagree with what the panel reviews. Catching a
//! tracked out-of-fence change is the REVIEWERS' job, not the machinery's to
//! conceal. It is only NEW files where junk and legitimate new source are
//! indistinguishable without a declared intent — and a work item already
//! carries one, in [`crate::types::WorkItem::scope_in`].
//!
//! NO GLOBBING. Containment is exact match or directory prefix, on
//! repo-root-relative paths. An entry that does not match simply does not
//! admit the file, and the exclusion is RECORDED by name (see
//! [`super::exclusion_note`]) — which is honest, and loud enough for an
//! operator to fix the plan. Glob semantics would also be unsafe further down:
//! the admitted paths are handed to `git add` as `:(literal)` pathspecs
//! precisely so a filename containing `*` can never match a second file.

/// The set of repo-root-relative paths a mechanical commit may adopt an
/// UNTRACKED file under. Built from a work item's `scope_in`; the empty fence
/// ([`ScopeFence::tracked_only`]) adopts nothing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScopeFence {
    entries: Vec<String>,
}

impl ScopeFence {
    /// The fence a work item's `scope_in` declares.
    ///
    /// Entries are normalised — and ONLY normalised, never interpreted:
    ///
    /// - surrounding whitespace is trimmed, and blank entries are dropped
    ///   (a blank entry can only ever be a plan-authoring slip, and left as-is
    ///   it would admit nothing while looking like a declared fence);
    /// - a leading `./` or `/` is stripped, because both spell the same
    ///   repo-root-relative path git's listings print without either;
    /// - trailing `/` is stripped, because a planner writes a directory as
    ///   `src/` at least as often as `src` (this package's own prompt tests
    ///   use `"src/"`), and `src/` + `/` would match nothing at all.
    ///
    /// Nothing else is inferred. In particular `.` is NOT read as "the whole
    /// repository": a fence that admits everything is not a fence, and the
    /// honest outcome of such an entry is that its files are excluded and
    /// named in the record.
    #[must_use]
    pub fn new(entries: &[String]) -> Self {
        Self {
            entries: entries
                .iter()
                .map(|entry| {
                    let trimmed = entry.trim();
                    let trimmed = trimmed.strip_prefix("./").unwrap_or(trimmed);
                    let trimmed = trimmed.strip_prefix('/').unwrap_or(trimmed);
                    trimmed.trim_end_matches('/').to_owned()
                })
                .filter(|entry| !entry.is_empty())
                .collect(),
        }
    }

    /// The empty fence: tracked changes only, no untracked file is ever
    /// adopted. This is what the integration-side steps commit under — see
    /// [`super::commit_integration_fix`] and [`super::conclude_merge`] for why
    /// a brand-new file appearing there is not the machinery's to take.
    #[must_use]
    pub fn tracked_only() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Whether the fence admits `path` (repo-root-relative, as git's `-z`
    /// listings print it): exact match on an entry, or the path lying inside
    /// an entry read as a directory.
    #[must_use]
    pub fn admits(&self, path: &str) -> bool {
        self.entries.iter().any(|entry| {
            // `strip_prefix` + the separator check, rather than
            // `starts_with(entry)`, so `src` does not admit `srcery/x.rs`.
            path == entry
                || path
                    .strip_prefix(entry)
                    .is_some_and(|rest| rest.starts_with('/'))
        })
    }

    /// How the fence reads in a recorded exclusion note, so an operator (and a
    /// review lens) can see WHY a path was not staged without going to the
    /// plan for it.
    #[must_use]
    pub fn describe(&self) -> String {
        if self.entries.is_empty() {
            return "tracked-only (this step adopts no untracked file at all)".to_owned();
        }
        format!("scope_in = [{}]", self.entries.join(", "))
    }
}
