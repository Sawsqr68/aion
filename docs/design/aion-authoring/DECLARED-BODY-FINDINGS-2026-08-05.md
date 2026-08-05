# Declared-body findings — 5 August 2026

Found while writing `examples/git-sweep/git_status_sweep.awl`, a three-step
document whose every action is a `run` body. Written at Tom's ask ("a workflow
that checks all the directories in a given directory, sees which ones are git
repositories, runs git status on each, and saves the output to a file"), so
these are an author's findings from actually building the thing, not a review.

The workflow itself works and is proven: run `4a4f7e83`, 7 repositories, the
report written to disk and read back. The findings below are what it cost to
get there.

## 1. 🔴 `$$` inside single quotes is a SILENT vacuous success

`CommandTemplate::parse` (`crates/aion-worker/src/shell/template.rs:188`) says
it plainly: *"Single quotes take their contents literally, including `$`."* So
inside `sh -c '…'` there is no `$$` escape — `$$1` reaches `sh` intact, where
`$$` is the shell's own process id. `$$1` becomes `<pid>1`.

What that did to a document that looked right:

- `for d in "$$1"/*/` globbed against a directory literally named `753901`,
  matched nothing, and printed nothing.
- `printf %s "$$2" > "$$1"` wrote a file **named `753901`** into the SERVER'S
  working directory — the repository root — containing `753902`.
- The run reported `outcome: swept, exit_code: 0, stdout: ""`. **Green.**

Three failures at once: the wrong answer, a stray file in a place the author
never named, and a success verdict over the top of both. Nothing in the run
output distinguishes it from the correct run — I only caught it because I
checked for the report file instead of trusting the exit code.

**The rule, stated the way an author needs it:** `$$` is the escape for a
literal dollar OUTSIDE single quotes. INSIDE single quotes write `$1`, `$d`
plainly — they are already literal. Getting this backwards cannot fail loudly,
because `$$` is valid shell.

**Gap:** `docs/workers/DECLARED-COMMANDS.md` documents `$$` as "a literal dollar
sign" and documents that single quotes are how you name a shell, but never says
the two interact — that inside single quotes the escape is not just unnecessary
but actively wrong. The one worked example that gets it right
(`DECLARED-COMMANDS.md:179`) uses bare `$1`, and a reader has no way to tell
whether that is deliberate or an oversight. That example is also not covered by
the doc example gate, so nothing would catch it if it drifted.

## 2. The ambiguity refusal's remedy cannot be followed

Deploying a changed body a second time earns:

> action `find_repositories` on task queue `local` declares 2 different bodies
> across retained package versions; refusing to guess which deploy this run
> meant — **redeploy so one body remains**

The refusal is right and the guard is good — guessing here would run the wrong
command. But redeploying is precisely what created the second body; following
the instruction literally makes a third. The route-active version also cannot be
unloaded (`RouteActive`), so the obvious first move is refused too.

**The order that actually works**, and what the message should say:

```
aion deploy FILE.awl          # the new version becomes route-active,
                              # naming the superseded hash in its output
aion unload <type> <old-hash> # now unloadable: no longer route-active
aion run FILE.awl --input …
```

`aion deploy` already prints `superseded_versions`, so the refusal has the hash
it needs to name. This bites on the SECOND iteration of every bodied document —
which is to say, on everyone who iterates.

**Closed.** The refusal now carries the declaring versions rather than a count,
and names the move as runnable `aion unload <type> <hash>` commands — full
hashes, because the deploy API refuses a short one. Three situations get three
different remedies, since one instruction cannot be right for all of them: a
superseded version left behind (retire it), nothing route-active at all (deploy
first, *then* retire), and two route-active packages that genuinely disagree
(nothing to retire — reconcile the documents). The printed hash is proven
runnable by parsing it back with the deploy API's own parser, and that test was
watched failing against a deliberately abbreviated hash
(`InvalidLength { found: 8 }`). `docs/workers/DECLARED-COMMANDS.md` gains the
deploy → unload → run order under *"Changing a body makes a NEW version"*.

**And then the refusal itself turned out to be mostly wrong (task #84).** Making
the remedy followable was the right fix for the message; it was not the fix for
*why the message appeared*. A body is a property of the run's own package
version, not of the task queue — content-hash namespacing pins each run to
exactly one version for its whole life. Resolution ignored that and read the
whole queue, so a second deploy killed work already in flight: a run started
under version A failed **terminally** at its next bodied activity because
version B — which that run would never execute — declared an edited command.
Nothing about the run had changed. That is a durability violation, not an
ergonomic one, and it is the same principle the ADVISORY class already honours
(`crates/aion/src/runtime/nif_activity_advisory.rs` resolves by
`(workflow_type, loaded_version)` so "a hot redeploy … never retroactively
reclasses an in-flight run's activities").

Resolution now takes the dispatching run's identity and reads its own version's
contract as the sole authority; it never borrows another version's body. The
queue-wide reading — and therefore the refusal above — survives only for the
case where the run's version cannot be named at all, because without the run's
identity, choosing IS the guess the refusal exists to prevent. Note what this
retires: `BRIEF-139-LEG-A-REWORK.md` R2.4 orders two documents to carry a
byte-identical body forever, a workaround for exactly this defect. **The brief
is a ratified work order and is not to be edited without Waffles.**

## 3. The authoring root is the home default, not the working directory

Tom's `~/.aion/config.toml` says `store.data_dir` and `authoring.workspace_dir`
are deliberately absent so the resolver picks the working-directory locations
`./aion-data` and `./aion-authoring`. **That held for the store and not for
authoring.** The server's own resolution line is unambiguous:

```
"aion-server authoring root resolved","authoring_root":"/Users/tom/.aion/authoring"
```

`./aion-data` exists, so it won; `./aion-authoring` does not exist, so the
resolver fell back to the home default. The comment states the rule as though
absence alone decides it, and an author who trusts it puts the document where
the console will never look. Measured, not assumed: after copying the document
to `~/.aion/authoring/`, `GET /awl/documents` lists it.

Not filed as a defect in the resolver — legacy-location-if-present is a
defensible rule. The defect is that the rule is written down somewhere that
disagrees with it.

## What is proven, and how

- `aion awl check` — ok, 3 steps.
- Run `4a4f7e83-79d5-4e7d-b2ab-1fe337a30e58` — `outcome: swept`.
- **The report file is the oracle, not the exit code.** Deleted before the run,
  so its existence afterwards is positive evidence: 2662 bytes, 7 `=== ` blocks,
  one per repository under `stack/`, each with real `git status --short
  --branch` output. The identical green payload from the BROKEN run is why the
  payload cannot be the oracle here.
- Visible in the console: `GET /awl/documents` lists `git_status_sweep.awl`;
  `GET /awl/deployed` carries the route-active version with
  `source: {state: "available"}`, so the deployed read-only view has the AWL too.

## Still open from this

Findings 1 and 2 are both closed — 1 as documentation (`483c933a`), 2 as the
refusal text itself.

`docs/workers/DECLARED-COMMANDS.md` is now inside the doc example gate, and it
paid immediately: **its opening example returned a type nothing declared.**
`RunOutcome` appeared twice in the document and was defined nowhere, so a reader
copying the very first block got `unknown type RunOutcome`. Watched failing
before the fix — `FAIL … 10:33: error: unknown type RunOutcome`, gate EXIT=1.
Both examples now declare what they return, and the document states the result
shape (`exit_code`, `stdout`, `stderr`, subsets allowed) it had never written
down.

What the gate holds, precisely: **two** of the document's seven fenced blocks.
The other five are annotated illustrations — argv arrows, the ✅/❌ contrast
pair, a shell transcript — and tagging them ` ```awl ` to raise the number would
report coverage of text that was never a document. And the honest limit stands:
the gate catches syntax and type drift. It would **not** have caught the `$$`
trap, which is a runtime semantic no checker sees — the ✅/❌ pair it cannot
check is exactly the pair whose two halves both check.
