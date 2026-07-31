# An unrecoverable run cannot be cancelled, and stays `Running` forever

Found 2026-07-31 ~10:05Z while executing Waffles' ruling to cancel the rig's
live zombie `0756ecd5` through the sanctioned cancel path. **The ruling could
not be executed: the sanctioned path refuses the row.** This is the report of
why, measured rather than inferred.

## The chain, end to end

1. `0756ecd5` is pinned to package version
   `13958627cbadec5b9c762dff5f985061aa8d5442bb4601a058a9081ee3a73bf0` — the
   hash the `remote_gates` archive was minted under on 07-29.
2. That archive was hashed under the `.v4` worker-contract identity domain.
   Commit **`ad2fca44`** (30 Jul, *after* the rig build `782b8e60`) moved
   package identity to `.v5`, folding a declared action body into the hash.
   Its own message states the consequence: *"Archives hashed under `.v4` refuse
   to load … and must be redeployed; this build carries no compatibility shims
   by policy."*
3. So on today's binary the archive fails reload:
   `expected 13958627…, computed b367d725…`.
4. The engine therefore skips the run at startup recovery — *"active workflow
   failed startup recovery; skipping it and continuing"* — so it is **never
   resident**.
5. `lifecycle::terminate::cancel` opens with `registered_handle(...)` and then
   takes `handle.recorder()`. **Cancellation must append through the run's one
   Recorder, and that Recorder lives on the resident handle.**
6. No residency ⇒ no Recorder ⇒ no sanctioned append ⇒ `WorkflowNotFound`.

**A run pinned to an unloadable package can never be recovered and can never
be cancelled. Its status is `Running` permanently.**

Step 5 is not a defect in itself. It is load-bearing invariant #3 — single
writer per workflow — working exactly as designed. The hole is that the
invariant has no branch for a run that can never obtain a writer.

## The control plane and the read plane disagree about existence

Measured on one server, one run, one namespace, within seconds:

| verb | result |
|---|---|
| `describe 0756ecd5 --namespace default` | **rc=0** — full history, `Running` |
| `resume 0756ecd5 --namespace default` | rc=1 — *"run 522fa865 is `Running`, not `Paused`; only a Paused run can be resumed"* |
| `cancel 0756ecd5 --namespace default` | **`WorkflowNotFound`** |
| `pause 0756ecd5 --namespace default` | **`not found`** |

`resume` is the decisive one: it **found the run and read its status** before
refusing on a legitimate state check. So the run is not unreachable. Two verbs
resolve existence from the store; two resolve it from engine residency.

Controls that make this non-vacuous:

- `--namespace liminal` returns not-found *for that namespace*, and the run's
  own `SearchAttributesUpdated` records `aion.namespace = "default"` — so the
  refusal is not a namespace mismatch.
- A bogus workflow id returns rc=1, so `not found` is a discriminating signal
  rather than a universal answer.

An operator sees a `Running` row in the console and in `describe`, and every
action that would resolve it reports the row does not exist.

## The second finding: the message blames the wrong party

`crates/aion/src/loader/persistence.rs:116`:

```
persisted deployed package failed validation on reload
(store corruption or manual tampering); skipping it —
runs pinned to it will fail recovery loudly
```

The actual cause here is neither corruption nor tampering. It is a **deliberate,
documented identity-domain bump** whose correct remedy is `aion deploy` again.
The fixture proves the bytes are innocent: the rig's `data/` mtime has not moved
since `2026-07-29T18:20:26+1000`, and **none** of the six 07-29 rig server logs
contains a single integrity-mismatch line. The same bytes loaded clean that day.
What changed is the verdict, not the data.

An operator who reads "store corruption or manual tampering" against a
production store will open a data-loss incident. The correct action is a
redeploy. `ad2fca44` deliberately put the contract-bearing hash in the report so
the two cases *could* be told apart — and the message one layer up still names
only corruption and tampering.

This is the **fourth** instance of the same pattern in three days, after #83,
#113 and #114: *the information existed and was discarded one layer above the
decision that needed it.* Here it is not even discarded — it is present in the
report and contradicted by the sentence wrapped around it.

## The message is fixed — `6b9252ad`, live-proven

The misattributing sentence is gone. The skip now states the disjunction, says
it cannot tell the two causes apart, and names the remedy. Proven on the real
binary against the exact store that produced the false claim:

```
LEVEL:  ERROR
REMEDY: redeploy this workflow
ERROR:  package integrity mismatch: expected `13958627…`, computed `b367d725…`
MESSAGE: persisted deployed package failed validation on reload; skipping it —
  runs pinned to it will fail recovery loudly. Cause is EITHER an archive minted
  under a superseded package-identity domain (an aion upgrade moved package
  identity; this build carries no encoder for a superseded domain, by policy) OR
  corrupted / tampered stored bytes. This build cannot tell them apart. The
  remedy is the same for both: redeploy this workflow.
```

The fix turned out narrower and more clear-cut than first framed. I had intended
to *identify* the superseded-domain case; `aion-package` states plainly that this
build carries no `.v4` encoder and the two causes are therefore
indistinguishable. So the defect was not a missing diagnosis — it was **asserting
a diagnosis the code is documented as unable to make**. Refusing to guess costs
nothing here, because both remedies are the same redeploy.

## Remedy shape — needs a ruling, not a quiet build

The narrow fix that respects invariant #3: when a run is **non-resident and
non-respawnable**, mint a Recorder for it directly against the store for the
single append that records cancellation. Non-residency is precisely the
condition under which no other Recorder exists, so "exactly one writer" still
holds.

The machinery is nearly there already — `resume_paused_workflow` respawns a
crashed-while-paused run through the reopen recovery path to obtain a Recorder.
That path cannot be reused here, because respawn requires loading the package,
which is the very thing that fails.

I am **not** building this off my own bat. It introduces a second way to mint a
Recorder, and the single-writer invariant is the one thing in this engine that
must never be loosened casually. It wants a ruling and a written argument first.

Two smaller pieces are unambiguous and can land without a semantics ruling:

1. **Fix the misattributing message** — name the identity-domain refusal, and
   say `redeploy`.
2. **Make the degraded state queryable.** A run skipped at startup recovery is
   currently visible only as one ERROR line in a boot log. This is Cally's
   `enforce_contract` finding A, now witnessed live on a real store.

## Status of Waffles' ruling

Not executed. Conditions (2) and (3) were met before the attempt — the evidence
is banked twice, and the cancellation reason carrying who/when/why and citing
his 09:59:56Z ruling was composed and passed to the sanctioned verb. Condition
(1) is what failed: the sanctioned path itself refuses, and row surgery is
forbidden, so there is no remaining authorized route.

`0756ecd5` therefore **remains `Running`** in the rig store, untouched. Nothing
was written. The shakedown's before-state is unchanged from the `ecf2050e`
fixture read.
