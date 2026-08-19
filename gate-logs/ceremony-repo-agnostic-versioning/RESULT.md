# Three ceremony legs made repo-agnostic — per-crate versioning, and absence that stopped reading as a value

**Vesper, 2026-08-20.** Commit `a5c996123`. Found live: the beamr 0.19.3 ceremony
(`3d4028b3`) refused at `cut_witness` on Tom's engine, minutes after I had
redeployed the document.

## The refusal, and the sentence that hid the cause

```
unreleasable(stage: "cut_witness")
"workspace version  does not wear the declared release version 0.19.3"
```

**Two spaces where a number belongs.** The empty string was rendered as though it
were a value, so *"the key is absent"* and *"the key says something else"* wore
one sentence. Same defect I had caught in my own gate file twenty minutes
earlier (`CLIPPY_RC=`) — absence dressed as a measurement.

## 🔴 My first diagnosis was a phantom, and Artemis caught it

I reported two causes. The first was wrong: I read
`/Users/tom/Developer/ablative/stack/beamr`, her **working copy**, on
`pr20-numeric-eq` @ 58a7e5d and dirty, and concluded beamr had not been bumped.
The ceremony was never given that tree.

**The evidence was inside the run I was already holding.** Its own durable
history records the probe's result:

```json
"result": { "ok": false,
  "head_commit": "ae8b3d5e589ab16b54ff6ebb64537a16502be145",
  "workspace_version": "" }
```

`head_commit` is **ae8b3d5** — her pinned, clean worktree, with `crates/beamr` at
0.19.3. It also got *past* the dirty-tree check, which is itself proof the tree
was clean. **The run under diagnosis carried the identity of what it measured,
and I went to the path I expected instead of reading it.** Adjacent-instance
trap; she hit the same shape the same night reading `tomWhiting/aion` for
`ablative-io/aion`.

One cause, mine, and sufficient on its own.

## The one cause, in three faces

All three assumed a repository declares **one version at the workspace root**.
beamr versions **per crate**; its `[workspace.package]` carries no version key.

### 1. `probe_release_tree` — an absent key is a repo SHAPE, not a cut error

Resolves `[workspace.package].version`, then `[package].version`. When **neither**
declares one, it reports `(per-crate) …` naming what it did and did not vouch
for, and passes that face. A key that exists and disagrees still refuses, now
naming which section it read.

### 2. `publish_and_confirm` — the replacement safety, in the stronger place

Relaxing the cut witness without replacing the check would have opened a real
hole. **Artemis made the case better than my own reasoning:** `publish_one` hands
**every** crate in `publish_order` the **single** ceremony version, so the set the
cut witness would vouch for and the set actually pushed are the same set **by
construction**. A subset check would leave exactly that gap open.

Before any index read the guard resolves the crate's own manifest — including
`version.workspace = true` inheritance — and refuses **rc 94** naming the crate,
its manifest version, and the declared one. Without it, cargo publishes what the
manifest says while the index confirmation looks for something else.

### 3. `verify_install` — an inapplicable probe is not a red

It ran `cargo install <crate> --version <ceremony version>` and demanded a
binary. **A lib-only crate can never satisfy that.** For this release `beamr`
0.19.3 is lib-only and `beamr-cli` carries the binary at 0.6.0, so
`install_green: false` was **structurally guaranteed** whatever happened —
Artemis flagged it before firing rather than letting it surprise anyone. The leg
now names the lib-only and declared-none cases as **not-applicable** in its
refusal text.

## 🔴 The transcription trap — only driving the SEALED string found it

These bodies are authored multi-line and joined into one AWL string literal.
Joining on a bare space produced:

```
m="" for f in $(find ...); do ... done
```

— a **`sh` syntax error**. `aion awl check` reports the document **ok**, because
the checker checks the AWL and not the shell inside the string. My first splice
shipped that for two of the three bodies.

It survived a check *and* a byte-identity witness: I compared the lexer's string
to my joined string and they matched — **both were equally broken**. What caught
it was executing the sealed composite, which failed with
`syntax error near unexpected token 'm=""'`.

★ **A round-trip proves transcription, not execution.** The joined form is the
thing that runs, and until it runs, nothing has been tested.

`join.py` now appends `;` to every line that does not end in `do`, `then`,
`else`, `{`, `(`, `;`, `&&`, `||` or `|` — `;` is required between two commands
and is itself a syntax error after the openers.

## 🔴 Two more of my own traps, both caught by the harness rather than by eye

1. **The battery reported five PASS(silent) that were all non-runs.** Quoting
   collapsed three arguments into one, `$2` was unbound, `set -u` killed the
   script, and empty output read as "passed". Fixed by demanding **exit 0 AND
   empty stderr** for a pass, and by a **positive control** — a deliberately
   broken invocation that must read as NON-RUN.
2. **Fixing a JSON-quoting hazard introduced a shell command-execution one.** A
   refusal message contained `name = "$c"`, which breaks JSON. I replaced the
   quotes with **backticks** — which `sh` executes: the arm died with
   `not-a-crate: command not found`. Only the non-run detector saw it; before
   that fix it read as PASS(silent). Now `<$c>`.

## Evidence

Every arm driven **in its joined form** — the form that runs:

| body | arm | result |
|---|---|---|
| `probe_release_tree` | beamr @ae8b3d5, per-crate, clean | `ok: true`, `(per-crate) …` |
| | aion tree, version mismatch | `ok: false`, names `[workspace.package]` and both versions |
| | missing tree / dirty tree | refuses, unchanged |
| `manifest_guard` | `beamr` 0.19.3 == declared | passes (silent) |
| | `beamr-cli` 0.6.0 vs 0.19.3 | rc 94, names crate + both versions |
| | crate absent from tree | rc 94 |
| | `aion-core`, `version.workspace = true`, match | passes — **the inheritance branch is exercised** |
| | same, mismatch | rc 94 |
| `verify_install` | `install_probe_crate` empty | `declared-none`, no cargo run |
| | **`serde` 1.0.200 from the real index** | `not-applicable: ships no binary target` |

Every refusal was parsed as JSON by the harness, so no arm can produce a wire
decode failure.

**Witnessed at aion's own lexer** (`worker-check-surface.json`): the sealed
command for `probe_release_tree` and `verify_install` is **byte-identical** to
the joined body driven above, and the guard sits **verbatim, before** the publish
work in `publish_and_confirm`. The composite was then driven **from the sealed
string itself** — with a crate absent from the tree, which is unpublishable even
had the guard failed entirely — and refused rc 94 with valid JSON and no stderr.

## Landing

- Only the **worker document** changed (3 lines: the three bodies). The workflow
  document is untouched, so **no redeploy was needed** — the deployed contract is
  unchanged.
- `aion awl check` clean on both; `aion awl fmt` applied.
- Worker restarted: **pid 87647**, exact original argv, cwd
  `/Users/tom/Developer/ablative/stack/aion`. The served file's SHA-256 matches
  the committed one — the address is **pinned, not assumed**.
- Preconditions re-checked: one non-terminal run (`canary`), nothing on
  `release_ops`.

## What this did NOT gate

0.19.3 published tonight by beamr's own `scripts/release.sh --publish` under
Waffles' ruling. This generalization is the **next-release path**, not tonight's
gate.

Also on the record: Artemis **refused** to add an inert `version = "0.19.3"` to
beamr's `[workspace.package]` to satisfy my probe. It would change nothing real —
crates with explicit versions never inherit it — and would have permanently
blinded that check for every future beamr release. Correct refusal, clock or no
clock.
