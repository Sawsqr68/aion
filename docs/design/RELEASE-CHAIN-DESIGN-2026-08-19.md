# The release chain — growing one ceremony into an ordered wave

**2026-08-19, Vesper.** For Waffles the Terrible. Design only; nothing built.
Sits inside `ESTATE-DISPATCH-LAYOUT-2026-08-19.md` — the layout is the building,
this is a tenant.

The ask: the held publish chain (Athena's crate wave, Hermes's liminal-rs 0.6.1,
the beamr release, and future cuts) becomes an aion workflow, each leg's receipt
recorded in run history, so the release stops being seats hand-running batteries
and becomes a run Waffles can describe from his seat.

---

## 1. What already exists, and what it already solves

`workflows/release_ceremony/release_ceremony.awl` is not a seed. It is a
complete, careful single-repository ceremony, and it already solves the problem
that looks like the hard one:

```
cut_witness → changelog_gate → semver_gate → open
            → [sequence crate in publish_order: publish_and_confirm]
            → publish_verdict → tag_step → install_step → verdict
```

**Ordering between crates in one repository is done.** `publish_order` is the
operator's declared topological order; the sequence region publishes and
index-confirms each crate before the next begins; cargo itself refuses a publish
whose dependency is not yet at the index, so a wrong order is a loud red rather
than a silent reorder. It is idempotent — a crate already at the index records
`skipped` — so a ceremony that died halfway re-runs on the same input and
resumes where the index says it stopped.

And it carries the design move worth copying, the one that makes this whole
document short:

> **`tag_step` takes the full per-crate confirmation set as its input.**
> Tag-without-publish is not refused at runtime. It is **not writable**. The #76
> drift — 0.16 and 0.17 tagged and never published — became inexpressible in the
> language rather than forbidden by a check somebody has to remember to run.

## 2. What is actually missing

Not ordering. **Crossing a repository boundary.**

Athena's crates, Hermes's liminal-rs, and beamr are separate trees with real
dependencies between them. Two things happen at a boundary that never happen
inside one:

**(a) The downstream tree must CHANGE before it can be released.** Once
liminal-rs 0.6.1 is at the index, aion's manifests have to be bumped to depend
on it, committed, and re-gated. That is not a publish leg. It produces a *new
tree*, and every gate that ran on the old one measured a world that no longer
exists — the cut witness, the semver surface, the battery. A chain that bumps
after gating publishes a tree nobody gated, which is precisely the delta the
existing ceremony flags about cut-versus-landing-gate order, one level up.

**(b) A downstream can be published against an upstream that is not there
yet.** This is the cross-repo twin of tag-without-publish, and it is worse:
tagging early leaves a wrong label, whereas publishing early leaves a *broken
package on the index that cannot be withdrawn*. The haematite 0.8.3 lesson
already in the ceremony's own header is this failure in miniature.

## 3. The proposal, which is one idea applied twice

### Make the upstream's receipt an INPUT to the downstream

Do for the chain exactly what `tag_step` does for the tag.

```awl
workflow release_chain
  input links: [Link]          // declared dependency order, operator's word
  input chain_message: String
  outcome released:      type ChainReleased,   route success
  outcome attempted:     type ChainAttempted,  route failure
  outcome unchainable:   type Unchainable,     route failure
```

Each link runs `release_ceremony` as a **child workflow**, and the child's input
carries the confirmation sets of every link it depends on:

```awl
type Link {
  repo: String,
  tree: String,
  version: String,
  depends_on: [String],           // repo names, upstream
  publish_order: [String],
}

/// What one completed link hands to the links below it. This type is the
/// whole design: a downstream ceremony cannot be STARTED without one of
/// these per upstream, so "published against something not yet at the
/// index" is not a mistake the chain can express.
type Receipt {
  repo: String,
  version: String,
  checksums: [CrateProbe],        // the index's own confirmation, not ours
  head_commit: String,
}
```

A link's child start takes `upstream: [Receipt]`. There is no route to a
downstream ceremony that does not pass through its upstreams' receipts. The
`sequence` region over `links` gives the ordering, the same way `publish_order`
gives it inside a repo — and for the same reason: the index is the enforcer, the
declaration is the operator's.

**Not** a `parallel` region across independent links, even where the graph
allows it. The whole value here is a run Waffles can describe from his seat; two
links failing at once in different repos is a worse story than one link failing,
and the wave is minutes, not hours. If that ever stops being true it is a
one-word change, and it should be made deliberately rather than inherited.

### The bump is a LEG, not a side effect

Between an upstream's receipt and a downstream's ceremony sits a step that
nothing today performs:

```awl
step bump_downstream after upstream_receipts
  // Rewrite the downstream tree's dependency on each upstream to the version
  // the receipt names, commit it, and hand back the new head. Refuses if the
  // manifest does not already depend on that upstream (a bump that ADDS a
  // dependency is not a bump), or if the tree is dirty before it starts.
  route regate_downstream

step regate_downstream after bump_downstream
  // The battery, on the BUMPED tree. Not the one that ran before.
  outcome green: when gate.green, route ceremony_child
  outcome red: otherwise, route unchainable(stage: "regate", repo: link.repo, ...)
```

`bump_downstream` writes the version from the **receipt**, never from the link's
declaration. Those two agreeing is the ordinary case; when they disagree the
receipt is the fact and the declaration is a hope, and taking the declaration
would publish against a version the index does not hold.

`regate_downstream` is the leg I most expect to be argued about, because it is
slow and it feels redundant — the tree was gated an hour ago. It is not
redundant. It is the only thing standing between us and a wave whose every gate
measured a tree that no longer exists.

## 4. What the run history gives Waffles

Every leg's receipt lands in the run's event history, so `describe_run` answers
the questions that currently require a seat to be awake:

- which links are done, which is in flight, which have not started
- for a done link, the index's own checksums — not our assertion that we
  published
- for a failed link, the named refusing stage (`cut_witness`, `changelog`,
  `semver`, `publish`, `regate`) and the partial confirmation set
- resumption: because each child ceremony is already idempotent against the
  index, and the chain's ordering comes from receipts rather than from a
  cursor, **re-running the chain with the same input resumes it.** Links already
  released re-confirm from the index and record `skipped`; the first unfinished
  link picks up where the index says it stopped.

That last property is not new work. It falls out of the existing ceremony's
idempotence plus receipts-as-input, which is the argument for building it this
way rather than with a chain-level progress cursor that could disagree with the
index.

## 5. Where it runs

From the layout document: namespace `release`, `Pinned` to the release box. A
worker on the wrong box cannot register for the release pool at all, so a
misplaced release worker fails at sign-up rather than three links into a wave.

Queues: `release/quick` for the bump and the probes, `release/long` for the
publishes and the re-gates. This split is doing real work here, not tidiness — a
re-gate is minutes and a publish waits on the index, both far past the 30-second
liveness window, while a cut witness is seconds.

One worker per queue, `max_concurrency = 3` for two in flight and a spare. **The
spare permit is load-bearing on the release box specifically**: a release worker
saturated mid-publish stops answering the liveness ping and gets withdrawn for
doing its job, and a withdrawal mid-publish is the most expensive one available
to us. This is the hazard Waffles asked to have designed for, and this is the
line that does it.

## 6. What I am NOT proposing

- **No automatic version selection.** Versions stay the operator's declared
  word. The chain enforces order and evidence, never what to release.
- **No rollback.** A crates.io publish cannot be withdrawn. `attempted` carrying
  a partial confirmation set is the honest terminal state, and the remedy is a
  new version, never an un-publish. Any "rollback" leg would be a lie in code.
- **No ranking between links.** Ordering is the declared graph, first-come
  first-serve inside a queue, per the layout's Fulcrum boundary.
- **No generalisation of the changelog embed face yet.** The ceremony's header
  already flags `crates/aion-server/changelog-embed` as aion-shaped. The chain
  makes that *more* pressing, since Hermes's and beamr's links will hit it — but
  it is its own wave, and pretending otherwise would put a non-aion link into a
  gate that only understands aion.

## 7. Build order, when the hold lifts

1. `Receipt` type + `release_ceremony` accepting `upstream: [Receipt]` and
   refusing when a declared upstream is absent. **Ceremony stays runnable
   standalone with an empty set** — the first link's real state, not a special
   case.
2. `bump_downstream` + `regate_downstream` as a worker action pair, tested
   against a two-repo fixture where the upstream version deliberately differs
   from the declared one, so the receipt-wins rule is *witnessed* rather than
   asserted.
3. `release_chain` over `sequence link in links`, children per link.
4. Drive it on the held wave with `--dry-run` semantics on the publish action
   only, so the ordering, the bumps and the re-gates all really happen and
   nothing reaches the index.

Step 4 is the one that matters. A chain whose ordering has never been watched go
wrong is a chain nobody has tested — and the cheapest way to watch it go wrong is
to declare an order that is deliberately incorrect and confirm cargo refuses it,
before ever declaring a correct one.
