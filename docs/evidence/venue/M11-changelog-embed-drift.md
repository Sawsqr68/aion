# M11 — the v0.20.0 cut renamed a release heading and left the packaged copy stale

Second red the workspace suite reached, after M10. Like M9 this one predates
the flight; unlike M9 it is *inside* the wall the gate runs, so it is fixed
rather than reported.

## The red, verbatim

```
test changelog::tests::changelog_embed_matches_the_authored_document ... FAILED
test result: FAILED. 1159 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
```

The assertion (`crates/aion-server/src/changelog.rs`) requires the packaged
`crates/aion-server/changelog-embed/CHANGELOG.md` — the copy `include_str!`
compiles into the server's changelog panel — to be byte-identical to the
authored `CHANGELOG.md` at the repository root.

## Provenance: pre-existing at the rig base

```
$ git show 'c590712f:CHANGELOG.md'                                  | md5sum
22c3f8e54faca524fbd949df58474ca9
$ git show 'c590712f:crates/aion-server/changelog-embed/CHANGELOG.md'| md5sum
156502aa904922aac347875ed9e391f3

$ git diff --name-only c590712f..HEAD | grep -i changelog
(no output — neither file was touched by this flight)
```

The rig base `c590712f` **is** the release commit `release: v0.20.0 — the
beamr 0.20.0 floor`, and it is where the drift was introduced. The gate runs
`cargo test --workspace` unconditionally, so this is the flight's red to
clear regardless of who wrote it — the same ruling M9 records for `fmt`.

## Cause: one commit, two defects

`git show c590712f -- CHANGELOG.md` is a single hunk, and it is a **rename**,
not an insertion:

```diff
 ## Unreleased
 
-## 0.19.0 — 2026-08-21
+## 0.20.0 — 2026-08-23
+
+**The beamr 0.20.0 floor.** A dependency-floor release, one motion across
+the estate: beamr 0.19.3 → 0.20.0 (the accessor-lifetimes release ...
```

1. **The embed was never synced.** That is what the test catches.
2. **The `## 0.19.0 — 2026-08-21` heading was deleted.** No test catches
   this. Its body did not move, so 0.19.0's release notes came to sit under
   the 0.20.0 heading.

Defect 2 makes the 0.20.0 section contradict itself. It opens with

> aion's own public surfaces do not move

and then, with no intervening heading, continues

> **Breaking.** This release moves public surfaces in six crates
> (cargo-semver-checks: 23 findings against 0.18.1) ...

The "against 0.18.1" comparison base dates that paragraph to the 0.19.0 cut
exactly. Confirmed against the tree at the 0.19.0 release commit:

```
$ git show '65ddcc52:CHANGELOG.md' | sed -n '8,10p'
## 0.19.0 — 2026-08-21

**Breaking.** This release moves public surfaces in six crates
```

## Fix

Both defects, in the order that makes the second one safe:

1. Restore `## 0.19.0 — 2026-08-21` immediately before its own body — the
   line the release diff deleted, verbatim, at the boundary git records.
2. `cp CHANGELOG.md crates/aion-server/changelog-embed/CHANGELOG.md` — the
   remedy the failing assertion names in its own message.

Step 1 is not optional given step 2. Syncing alone would have taken a
document that misattributes a breaking API change to the wrong release and
compiled it into the server panel operators read to decide whether a version
bump will break them. The heading is recovered from git, not reconstructed:
no wording was authored here.

Result:

```
$ cmp CHANGELOG.md crates/aion-server/changelog-embed/CHANGELOG.md
(identical)

$ grep -n '^## ' CHANGELOG.md | head -4
6:## Unreleased
8:## 0.20.0 — 2026-08-23
28:## 0.19.0 — 2026-08-21
87:## 0.18.1 — 2026-08-19
```

The 0.20.0 section is now the floor prose alone, and every release from
0.19.0 down is byte-unchanged from the rig base.

## Note for the release owner

The heading restoration is one line and is separable from the sync. It is
called out here rather than buried because it edits a published release
document — if the intent at the cut really was to merge the two sections,
revert that line alone and re-run the `cp`; the gate is satisfied either way.
