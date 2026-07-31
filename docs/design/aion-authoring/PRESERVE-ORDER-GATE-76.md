# A behaviour-changing dependency feature now has a name (#76)

**Built 2026-08-01. The generalisation of the `preserve_order` incident, measured
rather than recalled.**

## What could happen, and did

`serde_json`'s `preserve_order` feature swaps `Map` from a `BTreeMap` (iterates
key-sorted) to an `IndexMap` (iterates in insertion order). Several emitters here
*walk* a `Map` to build something order-sensitive — generated record field order,
decoder field sequence, the MIR literal pool. That is program **structure**, not
layout, so the swap changes a compiled package's bytes and therefore its content
hash.

It cost two red goldens once: the same `.awl` source compiled to a **different
package version** depending on an unrelated crate sharing the build.

## 🔴 Why it is structural and not a one-off

**No crate in this workspace asks for the feature.** Measured (see the re-run at
the foot of this file), and the negative is the whole finding — the grep across
every `Cargo.toml` is empty while the feature is on.

It arrives entirely through cargo's feature unification, from
`agent-client-protocol-schema` sharing the build graph. Two consequences:

1. **You cannot find it by reading our manifests.** Proven above — the grep is
   empty while the feature is on.
2. **It can arrive with no commit of ours at all.** A dependency bump anywhere in
   the tree flips it, and our diff is a lockfile line.

## What was already immune — and it held

The emitters, by construction: `aion-package/src/canonical.rs` makes canonicality
a TYPE (`CanonicalJson`, whose `Serialize` sorts at every depth), so no call site
can forget. Load-Bearing Invariant 5 is independently immune —
`content_hash_with_contract` routes through `PackageContract::canonical_bytes`,
hand-rolled and explicitly sorted, never touching serde's `Serialize`.

Re-measured this session under the ON resolution: **`aion-package` and `aion-awl`
both green, goldens unmodified.** The claim is true. What was missing was
anything that would notice if it stopped being true.

## 🔴 The detection was an ACCIDENT of resolution, not a gate

The ON case did already run — but only because two invocations disagree:

```
cargo test -p aion-package    -> preserve_order OFF   (measured)
cargo test --workspace        -> preserve_order ON    (measured)
```

That disagreement is the only reason the original incident was caught. Nothing
stated that **both** resolutions were required, so the detection evaporates the
day the acp crate stops being a workspace member, or the day someone reads a
per-crate green and stops. It was tacit knowledge in one seat's memory file.

## 🔴 And under OFF, every ordering test in the workspace is VACUOUS

This is the sharper half, and it is why the ON run had to be pinned by name.

With `preserve_order` off, `Map` is a `BTreeMap`. So a fixture built
`alpha, beta, gamma` and one built `gamma, beta, alpha` **are not two objects —
they are the same object.** Every
`*_independent_of_insertion_order` test then compares a value with itself:

| test | under OFF |
|---|---|
| `canonical_bytes_are_independent_of_insertion_order` | compares a map with itself |
| `beam_set_order_is_independent_of_insertion_order` | same |
| `content_hash_is_independent_of_insertion_order` | same |

A vacuous pass and a real one render identically. Nothing in the name, the body,
or the output tells them apart.

## What was built

### 1. The posture, detected by BEHAVIOUR

`crates/aion-package/src/build_coverage.rs` asks a `Map` what it does:

```rust
fn map_is_insertion_ordered() -> bool {
    let mut probe = Map::new();
    probe.insert("beta".to_owned(), Value::Null);
    probe.insert("alpha".to_owned(), Value::Null);
    probe.keys().next().map(String::as_str) == Some("beta")
}
```

**🔴 A `cfg` would have been actively wrong here, and this is the reusable part.**
Every other posture announcement in this workspace keys off `cfg(feature = …)`.
A crate can only `cfg` on its OWN features, so the obvious construction — a
forwarding `preserve-order = ["serde_json/preserve_order"]` feature announced via
`cfg(feature = "preserve-order")` — reports correctly when someone enables it
explicitly and reports **`false` while the feature is ON** in the unification
case. Which is the entire hazard. It would be a label that lies in exactly the
situation it exists to detect.

The announcement also says *what* the build is and never *how it got that way*:
the probe reads a `Map`, so it cannot distinguish an explicit `--features` from
unification, and claiming either would invent a cause it never observed.

### 2. The non-vacuity link, asserted as a biconditional

`insertion_order_is_observable_exactly_when_the_map_preserves_it` asserts that
the fixtures are distinguishable **if and only if** the probe reports insertion
ordering. That is a real claim in BOTH resolutions: under OFF it pins *why* the
sibling tests are vacuous, under ON it pins that they are not.

### 3. Three named gates in `scripts/battery.sh`

| gate | asserts |
|---|---|
| `posture:serde-json-sorted` | `-p aion-package` ALONE announces `key_sorted`, and never `insertion_ordered` |
| `posture:serde-json-insertion` | with `--features serde_json/preserve_order`, the opposite |
| `rust:preserve-order-pins` | every pin in the two Map-walking crates, under the resolution where they are not vacuous |

`-- --nocapture` is load-bearing: a cfg-selected announcement is printed as a test
NAME whether or not output is captured; a runtime-detected one is printed by the
running test and cargo swallows it otherwise.

`-p aion-package` **ALONE** is load-bearing for the reason the proto posture gates
already carry: the resolution depends on which packages are named, so a run that
names a neighbour is measuring a different program.

## 🔴 The inverted control — measured, and it is the whole lane

Everything above is a claim about what the gate *would* catch. This is what it
does catch. `sorted_entries` was mutated to return entries **unsorted** — the
emitters' entire ordering defence deleted — and both resolutions were run against
that same broken tree:

| resolution | command | result |
|---|---|---|
| **OFF** | `cargo test -p aion-package -p aion-awl` | 🔴 **0 failed, exit 0** |
| **ON** | same, `--features serde_json/preserve_order` | ✅ **11 failed, exit 101** |

**Same code, same mutation, opposite verdict.** The per-crate run — the one a
person types, the one that has been green all along — cannot see the deletion of
the entire mechanism this workspace relies on for reproducible package bytes.

The eleven that died, and nothing else did:

```
canonical::tests::canonical_bytes_are_independent_of_insertion_order
canonical::tests::canonical_bytes_are_key_sorted_at_every_depth
canonical::tests::canonical_encoding_matches_a_key_sorted_expectation
canonical::tests::serialize_value_canonicalises_a_field_of_a_derived_type
canonical::tests::sorted_entries_is_key_order_whatever_the_insertion_order
mir::tests::lowers_cover_and_golden
authored_key_order_cannot_change_the_emitted_bytes
derived_schemas_serialize_key_sorted
folded_schema_text_is_key_sorted_in_the_emitted_source
projected_record_fields_are_emitted_in_key_order
the_sidecar_matches_a_key_sorted_expectation
```

`canonical.rs` restored and verified byte-identical afterwards — sha256
`667b24dd…` before and after, and `git diff` clean on the file.

So the vacuity is not an argument about `BTreeMap` semantics. It is a measured
fact: **eleven tests whose names all say "sorted" or "order" pass a build in
which nothing sorts.**

## What was deliberately NOT built

A general "enumerate every resolved feature and fail on an unknown one" xtask.
That is a policy with a maintenance cost and a false-positive rate, and CLAUDE.md
forbids inventing that kind of limit unilaterally. It needs a ruling first.

## The reusable shape, worth more than the fix

> **"The identity digest is safe" is not the same claim as "the output bytes are
> safe."** The first was true and properly verified; the second was never
> examined. Complete for one class of question, silently incomplete for the
> other — and the sentence looks the same either way.

And a survey trap found alongside it. Re-measured at my own hand on 2026-08-01,
because the figures in the original finding had drifted with the tree and a stale
number reads exactly like a current one:

```
grep -rn  'cfg(feature'                 crates/ --include='*.rs'   ->  236
grep -rnE 'cfg(_attr)?\([^)]*feature'   crates/ --include='*.rs'   ->  350
```

A **32.6% blind spot** (the original finding said 233/344, same ratio), because
`cfg(all(feature = …))`, `cfg(any(…))`, `cfg(not(…))` and `cfg_attr` are all
invisible to the obvious grep. Use the second form for any feature survey on this
tree.

The load-bearing negative was re-run too, and is still empty:

```
$ grep -rnE 'preserve_order|arbitrary_precision|float_roundtrip|unbounded_depth|raw_value' \
    --include='Cargo.toml' .
(no matches)
```
