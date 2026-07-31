# The SDK-closure freshness gate is byte-blind (#115)

Found 2026-07-31 while closing [#83](AWLFAILED-CARRIES-NOTHING.md). The
blindness was documented in the gate's own module comment, so this was a
decision to revisit, not an oversight to correct.

## ✅ BUILT AND CLOSED — `439d6a88`

Landed 2026-07-31. Three deviations from the design below, each deliberate:

1. **Direct chunk comparison, not a digest.** Same detection, no new
   dependency, and it *names the chunk that moved* instead of returning a
   boolean — a `Code`-only divergence reads as a source edit, a whole-set
   divergence reads as a toolchain move.
2. **An allowlist, not an exclude list.** The design named chunks to exclude.
   An exclude list must be extended every time a toolchain introduces a new
   volatile chunk, and the failure mode of forgetting is a gate that flaps. An
   allowlist ignores anything unrecognised by construction.
3. **The double-harvest stability check was not built as specified.** The
   design asked for two consecutive fresh harvests compared against each other.
   The allowlist makes that check structurally unnecessary — volatility can only
   enter through a named chunk — and
   `a_changed_metadata_chunk_does_not_diverge` pins the property directly by
   proving a perturbed `CInf` does *not* trip the gate. Stated here rather than
   quietly dropped.

**Watched failing, twice**, because a green pin proves nothing otherwise:

- Byte axis disabled at its source → the pin fails and **both** controls stay
  green, so they are not passing merely because everything passes.
- A genuinely stale committed beam → the gate reports *"committed closure BYTES
  lag the harvest while the wire surface matches … `["aion@awl@error (StrT)"]`"*,
  and the export axis passed on that same run. That sentence is the finding.

**One invalid attempt, recorded because it nearly became evidence.** The first
end-to-end check perturbed the `Code` chunk and the gate failed with
`DecodeError("unsupported opcode 252")` — the export-set *parser* crashed before
the byte assertion was reached. That is not the finding; it is a broken file. A
stale closure has bytes that are **valid but different**, so the fixture moved to
`StrT` — a changed string table, which is exactly the changed-error-message case
the design names. A fixture that fails for the wrong reason is not a fixture.

Measured while there: **the committed closure is currently byte-fresh** against
this toolchain, so the axis lands as a guard rather than as a repair. The
07-24 Erlang drift described below did not leave a residue.

The three mechanism pins mutate a committed beam rather than harvesting a fresh
one, so they need no `gleam` toolchain and run in **every** battery, not only
where the oracle project can be built.

Gates: `cargo test -p aion-awl-package` exit 0, 36 passed / 0 failed across 7
targets; `cargo clippy -p aion-awl-package --all-targets -D warnings` exit 0.

## What the gate does

`crates/aion-awl-package/tests/sdk_closure_freshness.rs` rebuilds the oracle
Gleam project (`examples/awl-hello` — the exact tree
`scripts/regen-awl-sdk-closure.sh` harvests) and compares the fresh harvest
against the committed `sdk-closure/` beams on three axes:

1. **Version stamp** — committed stamp equals the oracle's locked `aion_flow`.
2. **Module set** — same modules, modulo the script's test-only exclusions.
3. **Per-module export set** — `(function, arity)` pairs, via
   `beamr::loader::load_beam_chunks`.

It never compares the bytes.

## Why that is a hole

The gate's stated purpose, from its own doc comment:

> A stale committed bundle (an SDK edit without a closure regeneration) fails
> here in CI instead of failing at runtime in a deployed direct archive.

And its stated reason for comparing exports instead of bytes:

> Comparison is by export SET, not bytes: a compiler upgrade may legitimately
> change beam bytes without changing the wire surface, and the surface is what
> generated code links against.

**The exclusion swallows the inclusion.** An SDK edit that changes a function
*body* — a bug fix, a changed error message, a corrected branch — changes
neither the module set nor any export set. It is invisible to all three axes.
That edit is *exactly* "an SDK edit without a closure regeneration", the case
the gate was written to catch, and the case it cannot see.

The wire surface is what generated code **links against**. It is not what
**runs**. The closure ships as bytes; the bytes are what execute in a deployed
direct archive.

## Measured, not theorised

Closing #83 I edited `gleam/aion_flow/src/aion/awl/error.gleam` — added a
variant, changed three mapper bodies — then ran
`scripts/regen-awl-sdk-closure.sh`. It rewrote **all 46 tracked beams**, and
`crates/aion-awl-package/tests/awl_assemble.rs` had to be repinned:

| pin | before | after |
|---|---|---|
| `BUNDLE_CHECKSUM` | `2b1cc2af…` | `a04dc8ab…` |
| `FIXED_INPUT_ARCHIVE_SHA256` | `96b3f8ce…` | `dc3fc5fd…` |

The freshness gate was **green across that entire change**. The bundle-checksum
pin caught it — but that pin only fires once the regeneration has already been
run. It answers "did you commit what you generated", never "should you have
generated". Nothing in the suite asks the second question.

There is a second, older data point in the same direction: the toolchain moved
Erlang 9.0.6 → 10.0.2 on 2026-07-24 and the committed closure's bytes went
stale against it. That drift was found by hand, not by this gate.

## The fix

Add a fourth axis: a **semantic byte digest** per module, compared fresh
against committed.

Whole-file equality is the wrong instrument — BEAM files carry a `CInf` chunk
holding compile time, source path, and compiler options, so two builds of
identical source differ. That volatility is almost certainly why export-set
comparison was chosen, and discarding whole-file comparison was right. What
does not follow is discarding byte comparison altogether.

Digest over the chunks that decide behaviour:

- **Include:** `Code`, `LitT`, `StrT`, `ImpT`, `ExpT`, `LocT`, `FunT`, and the
  atom table (`AtU8`/`Atom`).
- **Exclude:** `CInf` (timestamps, source paths, options), `Dbgi`, `Docs`,
  `Line`.

`export_set` already walks `load_beam_chunks`; the digest belongs beside it,
reusing that parse rather than opening a second BEAM reader.

## The objection, answered

*"A compiler upgrade will now fail this gate."* Yes — and it should.

The remedy for every failure this gate can report is the same single action:
run `scripts/regen-awl-sdk-closure.sh` and commit the result. That is true for
an SDK source edit and equally true for a toolchain upgrade. So the
false-positive worry never bought anything: it traded away detection of the
gate's own target case in exchange for silence on a case whose correct response
is identical.

And a toolchain upgrade that changes the bytes is not a nuisance to be tolerated
— **it is a change to what ships and runs.** The committed closure is the
artifact; if the compiler now produces different code for the same source, the
committed artifact is stale by definition. The 07-24 Erlang move is the proof:
it changed the shipped bytes and nothing told us.

The failure message should name which axis diverged, so an operator can tell an
SDK edit from a toolchain move at a glance, even though the command they run is
the same either way.

## Tests this needs

- A module whose **body** changes with its exports held constant must fail the
  gate. This is the pin, and it must be watched failing — the current gate
  passes it today, which is the whole finding.
- Export-set and module-set divergences must still fail, with their own
  messages (the new axis must not swallow the old ones).
- A byte-identical fresh harvest must pass. Without this control, a gate that
  failed unconditionally would satisfy every assertion above.
- The digest must be stable across two consecutive fresh harvests from an
  unchanged tree — otherwise the excluded-chunk list is wrong and the gate is a
  coin flip. Compute both in-test rather than pinning a constant, so the test
  encodes the property instead of one harness's snapshot.

## Note for whoever picks this up

Do not replace the export-set comparison. It produces a **better diagnostic**
than a digest can — it names the functions gained and lost. The digest is an
additional axis that answers a different question, and losing the export diff
would trade a precise message for a boolean.
