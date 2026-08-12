# Brief: the directory an agent stands in is declared, not inherited

Task #222. An ACP agent's working directory is a required, gate-tested `cwd` in
the worker document. A Norn agent has no way to say it at all — the adapter never
calls `current_dir`, so every spawned `norn` inherits whichever directory the
worker process happened to be started from. That value appears in no document, no
argv, and no log. Two adapters serving the same purpose disagree about whether the
most consequential setting an agent has is worth writing down.

## Why this is a correctness defect and not an ergonomics gap

The working directory decides which tree the agent reads and which files it
edits. A worker started from a shell that had `cd`'d somewhere else runs its
agents against a different repository than the one the operator believes is under
the run — and nothing in the run's record says so, because the value was never
captured anywhere. There is no artifact to check the behaviour against.

## Scope

1. **`cwd` becomes a required, absolute field of the `norn` harness section.**
   It moves out of the checker's `foreign(...)` set — where it was refused as a
   setting norn does not have — and into `absolute_path(...)` beside ACP's, with
   a refusal that names why the value is load-bearing.
2. **`CompiledHarnessKind::Norn` carries it**, resolved through the same
   `required_path` backstop the ACP arm uses, so a document that reaches the
   compiler without one is refused there rather than silently launching.
3. **`NornHarness` takes it as a CONSTRUCTOR PARAMETER** and `impl Default` is
   deleted. This is the load-bearing design call: an `Option<PathBuf>` set
   through a builder method has exactly two possible behaviours, and both are
   worse. Falling back to inheriting leaves the defect intact under a new name.
   Refusing at spawn compiles everywhere and fails at runtime — the
   "accepted, then does nothing" shape. A positional parameter makes the
   compiler ask every composition root the question, once, at the place where a
   launch setting is actually decided.
4. **The invocation preview carries it**, because the preview documents itself as
   what the spawn will be, and a preview that omits the directory is a preview of
   a different process.
5. **Every composition root passes one.** Where a repository or workspace root is
   already in scope it is used; otherwise the root reads `std::env::current_dir()`
   explicitly. The second case is behaviour-identical to what happened before —
   the point is that the inheritance is now written down at the top of the
   program instead of happening invisibly at the bottom.

Every existing `norn` worker document must now declare `cwd`. That is a breaking
document change, taken deliberately under this build's no-backwards-compatibility
rule: a compatibility default would be the inherited directory, which is the
defect. Document owners are told before the landing, not by a red `awl check`
after it.

## Scope amendment — the ACP retry classification

**Added after the brief was ratified. Recorded here because an amendment belongs
in the artifact, not in a conversation.**

Building the Norn refusal meant reading ACP's, and ACP's is misclassified.
`require_absolute` raised a relative `cwd` as `HarnessError::protocol`, which
`is_deterministic` reports as **retryable**. The path comes from the worker
document; the next attempt reads the same document and finds the same relative
path. So the worker re-spent its entire attempt budget re-confirming a value that
could not change, and told the operator a transient-transport story about a fault
that was neither transient nor transport.

The error type's own documentation names this exact failure as the reason
`Configuration` exists:

> deterministic by definition — the launch was refused by a value in the
> document, and the next attempt reads the same document. A refusal that
> presented as a transient transport failure would tell the operator the wrong
> story AND spend the whole attempt budget confirming it.

The amendment was ruled to ride this task rather than docket separately, on the
reasoning that **parity with a call site is not parity with the contract** — the
error type's documentation *is* the contract, and the ACP call site violated it in
the type's own words. Leaving ACP misclassified while making Norn correct would
create a deliberate divergence inside the very task that exists to remove one.

Two conditions came with it, both discharged:

- **Its own commit**, separate from the Norn work, so the classification change
  can be read, reverted, or bisected on its own.
- **Its own test pinning the classification.** ACP's existing test asserted
  `HarnessError::Protocol { .. }` — the wrong behaviour was pinned by a test that
  defended it, which is why it survived. The replacement asserts both the variant
  and `is_deterministic()`, because the variant alone would pass on a future edit
  that kept the name and moved the classification, and `is_deterministic` is the
  property that actually decides whether the worker retries. The
  `additionalDirectories` refusal — the same helper, the same document source —
  gets the determinism assertion too, so the pin cannot hold on one path while
  the other silently stays retryable.

## Scope amendment 2 — `{workspace_root}` in a path-valued setting

**Added after the brief was ratified, on a ruling. Recorded here for the same
reason as the first amendment.**

Making `cwd` required and absolute made one shipped document inexpressible.
`crates/aion-server/assistant-embed/assistant.awl` is compiled into the server
binary and served on whatever machine runs it, so no absolute literal it could
write is right on the second box. The fork — fold in the minimal slice of #210,
or let the composition root choose a directory, or park the document — went up
for a ruling.

**Ruled: fold in the minimal slice**, on the reasoning that it is the
already-ratified direction arriving early. The alternative where the server picks
a `cwd` at compose time was disqualified in terms worth keeping: *configuration
nobody wrote down deciding what actually ran — the precise shape this task exists
to kill, wearing a helpful face.* Parking the document was disqualified because it
parks a fix that live workers dodge only by luck.

What that adds, and nothing more: placeholder expansion for the harness section's
PATH-VALUED SETTINGS. The five conditions the ruling attached, and how each is
discharged:

1. **One truth, mechanically.** The expansion calls the server's own
   `WorkspaceRoot::resolve()` rather than reimplementing it — no second
   implementation, no copied rule. The dependency direction made the *spelling*
   part of that impossible to satisfy by re-export (aion-awl cannot depend on
   aion-server), so it was satisfied by SUBTRACTION instead: the placeholder
   literal moved DOWN into `aion_awl::WORKSPACE_ROOT_PLACEHOLDER`, which
   aion-server now `pub use`s. It is a token of the language documents are
   written in, and it now lives in the language crate. Nothing has two copies.
2. **The accept-set is exactly two forms at the checker** — an absolute path, or
   a value carrying the placeholder — stated once in
   `aion_awl::harness_path_form` and read by both the checker and the compiler's
   backstop. The composition root expands and THEN demands absolute, so a
   placeholder expanding to something non-absolute, or a box with no resolvable
   root, refuses loudly at the worker and never spawns.

   **One narrowing, made deliberately and recorded here rather than assumed:**
   the accepted form is LEADING with the placeholder, not merely CARRYING it.
   `agents/{workspace_root}` cannot expand to an absolute path under any root, so
   accepting it at the checker would hand the composition root a value it must
   refuse — green at check time, red at launch. The narrowing moves that refusal
   to where the value is written. It widens refusal, never permission.
3. **Scope stays minimal and the remainder stays named.** This is placeholder
   expansion for path-valued harness settings, nothing more. #210's outstanding
   half — per-run bindable `cwd`, and per-box binding for a harness ARGUMENT — is
   still open, and is named as open in this brief, in `assistant.awl`'s own
   narration, and in `docs/workers/AGENT-WORKERS.md`. The shorthand does not
   retire it.
4. **Three pins.** A document shipping `{workspace_root}` in `cwd` round-trips
   checker → composition root → resolved absolute path; a bare relative `cwd` is
   still refused, witnessed by a fixture written AFTER the new accept-form
   existed rather than assumed to have survived it; and the no-resolvable-root arm
   refuses with a `Configuration`-class error, deterministic, on the same
   discipline as amendment 1.
5. **The shipped document's narration moves with the truth.** `assistant.awl`'s
   "wrong on every machine but one" paragraph now records the placeholder as the
   answer for `cwd`, and claims exactly that. The `--workspace-root`/`-C`
   confinement pair is `args`, which this landing did NOT give expansion to, so
   the narration still names it absent and says why — a document claiming one step
   beyond the build is absence wearing a green tick.

## Scope amendment 3 — the third channel

**Added during self-review, before landing. Found by reading this brief's own
first paragraph back against the built diff.**

The defect is stated as three channels: the directory "appears in no document, no
argv, and no log." The scope above closes the document. Argv stays open and is
named as #210's half. **The log was still silent, and nothing said so.**

That gap is worst exactly where amendment 2 lands. A document writing
`{workspace_root}` is deliberately silent about the path — that silence is the
point, because the path differs per box — so the document can no longer answer
"where did my agent actually run". Only the worker that resolved it can.

So the agent worker's startup line carries the working directory, and carries the
RESOLVED one: `ResolvedHarness::launch_directory()` reads through the same value
the adapter is handed rather than re-deriving it from the written setting. That
distinction is the whole content of the fix. A report the child can disagree with
is worse than no report, because it is believed. It is pinned as
`the_reported_launch_directory_is_the_one_the_child_will_stand_in`, whose
assertion is that the reported path and the spawned path are one value, and that
the value is the expanded directory rather than the placeholder.

The accessor is common to both harness kinds, so ACP's directory — required and
gate-tested since before this task — now appears in the same line. It was the
declared half of the asymmetry and was equally unsaid at runtime.

## Scope amendment 4 — a missing directory must not be reported as a missing binary

**Also found during self-review. This one is a defect the task itself created,
which is why it belongs to the task rather than to a follow-up.**

Before this work, the Norn adapter set no working directory, so a failed spawn
could only ever be about the binary — and the adapter's message says exactly
that, by name, as a RETRYABLE transport error. Correct, then.

Setting `current_dir` changes what that message can mean. `Command::spawn`
answers a missing working directory with the same `No such file or directory` it
answers a missing binary with, so a declared `cwd` that is not on this box would
have been re-attempted to the end of the retry budget while pointing the operator
at the wrong setting — the exact pairing of faults amendment 1 corrected one crate
over.

So the pre-spawn gate checks both properties of `cwd`: absolute, and there.
Both are `Configuration`, both terminal, for the reason amendment 1 states — the
value comes from the document, and the next attempt reads the same document.

The directory is deliberately NOT created. `cwd` names the tree the agent is
meant to read and edit; conjuring an empty one would turn a typo into an agent
working against nothing, which is the failure the whole setting exists to make
impossible. `{workspace_root}` is the one exception and it is not one in spirit:
the launching box creates its OWN clones root, and only that root, one layer up.

## What this task does not do

`cwd` remains a fixed value declared per worker document. Binding it per run —
and the same for ACP's `command` — is a separate, ratified piece of work (#210),
as is letting a run supply the anchor phrase, and as is binding a harness
ARGUMENT to a per-box value. Nothing here expands `{workflow_id}` or any other
RUN-identity placeholder inside `cwd`: a process cannot be started in a directory
that does not exist yet, and a per-run clone does not exist until the run makes
it. `{workspace_root}` is the one exception, and it is not a run-identity value —
it names a property of the BOX, resolvable before any run exists, and the
expansion creates the directory it names.
