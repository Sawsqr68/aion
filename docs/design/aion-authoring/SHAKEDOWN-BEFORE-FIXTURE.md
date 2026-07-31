# The shakedown before-fixture, read before anything re-runs over it

Read 2026-07-31 ~09:52–09:58Z, before standing the rig up. **The fixture is not
what the brief says it is, in both directions.** This is the whole reason
fixture-first was the right spine.

## Where it lives

The 07-29 rig store is **`/Users/tom/.aion/remote-gates-proof/`** (4.2M; `data/`
plus the run's logs). Its `data/` mtime was `2026-07-29T18:20:26+1000` and is
**unchanged** by this read.

It is **not** Tom's live store. Tom's server (pid 99844) stores into
`/Users/tom/Developer/ablative/stack/aion/aion-data` and holds 21 runs, none of
them the fixture, none of them `Running`.

**Read method:** the store was **copied** to `$HOME/.aion-fixture-read-vesper/`
and a server was pointed at the copy. The original was never opened by a
writer. A fixture you open with a writer to inspect is a fixture you have
already risked.

## What is actually in the rig store — 13 runs, all `remote_gates`

| run | status | started |
|---|---|---|
| 48aad5cc | Failed | 08:24:05 |
| bdb309d2 | Completed | 08:24:05 |
| 3a894965 | Cancelled | 08:32:59 |
| e6fc9d83 | Failed | 08:46:35 |
| dca75b24 | Failed | 09:09:05 |
| f795e134 | Cancelled | 09:12:55 |
| be861a16 | Cancelled | 09:12:55 |
| 2c3b6227 | Failed | 09:43:10 |
| d2474722 | Cancelled | 09:43:10 |
| abbd1861 | Cancelled | 09:43:10 |
| **ae0273fd** | **Cancelled** | 10:10:32 |
| **539f2363** | **Cancelled** | 10:10:33 |
| **0756ecd5** | **Running** | 12:34:58 |

## 🔴 The brief is inverted, both ways

The banked ruling says the harvest evidence is the **zombie-`Running` pair
`ae0273fd` / `539f2363`, deliberately left uncancelled**, and that `0756ecd5`
is "a partial run record that *may* still sit in rig state".

Measured, the opposite is true on both counts:

1. **`ae0273fd` and `539f2363` are `Cancelled`.** They are not zombies and they
   are not uncancelled. Either they were cancelled after the record was
   written, or the record was wrong when written. Whichever it is, **the
   preservation instruction is protecting something that is not there.**
2. **`0756ecd5` is the one actually sitting `Running`** — started **12:34:58**,
   *two hours after* the 10:22 incident, so it is not the wreckage of Hit A′ at
   all. It is a later run, and it is the live zombie.

Nobody should re-run believing the pair is the before-state. **The zombie to
protect is `0756ecd5`.**

## `0756ecd5` is also an exhibit already preserved in the tree

Independently of the store, `0756ecd5` is cited as the wedge exhibit in
`docs/design/DELIVERY-CONTRACT-SCOPING.md` ("preserved untouched") and pinned as
a named regression in `crates/aion-server/tests/outbox_delivery_contract_e2e.rs`
(`wedge_shape_regression_0756ecd5`). So its *shape* survives in source whatever
happens to the row. That is a real mitigation, and it is also why the id turns
up in a repo grep with no rig log behind it.

## What the log says the failure actually was

From `server-remote.log`, the full arc of `ae0273fd` — three distinct events,
21 lines:

```
10:10:38.601  INFO  activity completed
10:11:08.812  WARN  outbox dispatch failed; scheduling retry with backoff
10:22:10.824  WARN  outbox dispatch exhausted retry budget; dead-lettering row
```

and the recorded reason, on every dead-lettered row:

```
worker dispatch failed for namespace liminal,
activity type aion.dispatch.default.remote_gates.shell:
no liminal worker registered for the row's pool
```

**The activity COMPLETED. What failed was delivering its result back.** The
work was done at 10:10:38 and thrown away twelve minutes later.

This sharpens the brief's framing. The banked wording is "worker lease expired
10:22:04Z mid-cargo"; the store's own record says the dispatcher found **no
liminal worker registered for the row's pool** at delivery time. Those may be
the same event seen from two ends (lease expiry → deregistration → nothing to
dispatch to), but **10:22:04 is when the retry budget ran out, not when the
work died** — the delivery had already been failing since 10:11:08.

That matters for what green means: a leg that never has to deliver a completion
across a worker gap proves nothing about this.

## 🔴 #113 fired again, live, during this read — on the pre-fix binary

Standing the reader up the first time, from the repository root, with
`AION_HOME` set:

```
WARN  AION HOME MIGRATION REQUIRED: using legacy store data directory
      `/Users/tom/Developer/ablative/stack/aion/aion-data` instead of
      `/Users/tom/.aion-fixture-read-vesper/data`
INFO  data root resolved  data_root=/Users/tom/Developer/ablative/stack/aion/aion-data
ERROR aion-server failed: data dir is locked by another live writer
      (writer lock held at .../aion-data/writer.lock)
```

**The binary at `target/release/aion` is the 30 Jul 23:13Z build — it predates
`8d731436`, so it still has the defect.** It ignored `AION_HOME` and went
straight for Tom's live store. What stopped it was **haematite's writer lock**,
not any decision of mine.

That is the second time in two days that this defect was caught by an unrelated
guard: first a missing feature flag, now a writer lock. Tom's server was
verified unharmed afterwards (health 200, count still 21).

**Two standing consequences:**

1. **The fix is in the tree, not in the binaries.** Every tool built before
   `8d731436` still inverts `AION_HOME`. Until the release binary is rebuilt,
   treat `target/release/aion` as unsafe to run from the repository root.
2. **My own error, plainly:** I ran a pre-fix binary from the one directory that
   triggers the defect I had documented that morning, trusting `AION_HOME` to
   isolate it. Knowing a finding is not the same as applying it. The working
   rule is now: **run from a directory with no legacy `aion-data`, or use a
   post-`8d731436` binary** — and the second read did exactly that, resolving
   correctly to the copy.

## Still open before the re-run

- **The worker's home.** Dean's box is off. My box or Tom's — my mechanics call,
  and it must be named in the green report.
- **Whether to cancel `0756ecd5`.** It is the live zombie and also the preserved
  exhibit. It should be read in full and its disposition decided deliberately,
  not incidentally by a re-run.
- The evidence copies are at the session scratchpad under `fixture/`:
  `ae0273fd.log`, `539f2363.log`, `server-remote.log`, `rig-workflows.json`.
  **These are the only copies outside the rig store — they belong somewhere
  durable if the fixture matters.**
