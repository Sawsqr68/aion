# libsql retirement R0 — live-specimen memory diagnosis

Authorized by Waffles 2026-08-17 ~11:39+1000 as READ-ONLY diagnosis (no store
writes, no code changes, never TERM the server). Specimen: the live serving
`aion server`, pid 23923, launched 11:28:51+1000, listener 127.0.0.1:8080.
Evidence artifact for aion#32 and the retirement brief's R0.

## Samples (raw files beside this document)

- `sample1-footprint.txt` / `sample1-vmmap.txt` at 11:40:09+1000
  (uptime 11m18s): **Physical footprint 39.2 GB** (peak 39.2).
- Second footprint at 11:41:04+1000: **40 GB** — ~1 GB growth in 55 s.

## Findings

1. **Rate confirmed and worse than the brief's figure.** 39.2 GB at 11m18s
   uptime = ~3.4 GB/min average since boot; the 11:40→11:41 delta ran ~1 GB/min
   at the 40 GB mark. The brief's "~3 GB/min while merely serving" is
   corroborated on a fresh specimen.

2. **The growth is one malloc zone of small allocations.**
   `DefaultMallocZone`: 38.5 GB region footprint, **5,522,368 live
   allocations, 37.9 GB allocated, avg ~7.2 KB each, 1% fragmentation.**
   MALLOC_SMALL: 38.3 GB dirty across 9,805 regions. Everything else is
   noise (MALLOC_LARGE 924 MB, TINY/NANO < 1 MB). This is millions of
   page-cache-sized allocations retained, not a few big buffers.

3. **The leaked memory is write-once, never re-read.** 37.6 GB of the
   MALLOC_SMALL dirty total is SWAPPED OUT (97%); resident is only ~480 MB.
   A live cache would be touched and stay resident; this is allocated,
   dirtied once, and abandoned — the signature of a true leak.

4. **🔴 The memory gate is blind to this leak.** `ps` RSS for the specimen
   reads **1.6 GB** while physical footprint is 39–40 GB, because the leak
   swaps out. The specimen was at ~39 GB footprint when the :37 gate window
   passed and it was NOT restarted. Either the gate's meter reads
   RSS/phys-mem (which swap hides) or an in-flight-run hold applied at :37.
   If the meter is RSS, the 20 GB bound never fires on exactly the memory
   it exists to bound.

5. **🔴 Box-level hazard: swap exhausted.** `vm.swapusage` at 11:41:
   total 38.9 GB, **used 38.3 GB, free 642 MB**. macOS will grow swap into
   the 74 GiB of free disk at the leak's rate (~1 GB/min at the 40 GB mark).
   This is a live threat to everything on the box (builders, the re-fly),
   not just to the specimen.

## Allocation sites — SETTLED by malloc_history (supersedes the earlier
## per-poll libsql hypothesis, which was WRONG)

Second specimen: pid 16337, booted 11:42:59+1000 with `MallocStackLogging=1`
on the fixed (#33-swapped) binary at Waffles' hand; ~2.9 GB footprint at
2 min uptime when sampled. `malloc_history 16337 -allBySize` captured 145
distinct stacks → `malloc-history-allBySize.txt` beside this document.

**The accumulator is haematite, not libsql.** The two dominant stacks:

1. **119,789 allocations, 1.215 GB** —
   `aion_server::state::connect_haematite_store` → `build_haematite_store`
   → `HaematiteStore::materialize_all_shards` →
   `haematite::db::Database::read_value` → `ShardRouter::handle_for_shard`
   → `ShardHandle::spawn` → beamr `Scheduler::spawn_native` →
   `ShardNativeHandler::make_factory` → `stream_index::rebuild::<DiskStore>`
   → `DiskStore::read_node` → **`LeafNode::deserialise`** → malloc.
2. **55,889 allocations, 692 MB** — same tail via
   `WalRecovery::recover_path` / `recover_with_store` →
   `DiskStore::read_node` → **`LeafNode::deserialise`**.

Six stacks in total terminate in `LeafNode::deserialise`. libsql appears
nowhere in the top stacks. The earlier hypothesis in this file (per-poll
libsql lifecycle, #85-adjacent) is refuted by this measurement.

**Decisive arithmetic:** `~/.aion/data` is **2.0 MB on disk** while
specimens reach 39 GB. A 2 MB store cannot yield 38 GB of live nodes in
one pass — the same nodes are re-deserialised in an unbounded loop and
retained. The sites are boot-shape paths (shard materialization, WAL
recovery) still allocating long after boot on a merely-serving process.

## Consequences

- **libsql retirement does NOT kill the aion#32 accumulator.** The
  retirement brief's acceptance premise breaks; R0 fired exactly as
  designed, before anything was deleted on that theory.
- The loop mechanism (why materialize/recovery re-runs unboundedly) is
  haematite-internal and NOT proven here. Candidates: CORE-008
  (wedged-actor recycle) and #227 (six coexisting beamr VMs). The
  `ShardHandle::spawn` frame on a read path is suggestive, unproven.
- Guarding must use physical footprint, never RSS — the leak swaps out
  (97%) and RSS reads ~1.6 GB against a 39 GB footprint.

## Disposition

- Verdict DM'd to Waffles and posted on aion#32 as R0's evidence
  artifact (2026-08-17 ~14:0x+1000):
  https://github.com/ablative-io/aion/issues/32#issuecomment-5310888620
- Box hazards from the first pass both remedied at Waffles' hand:
  specimen cycled (swap released 38G→10G) and the starved session-cron
  gate replaced with a continuous 60s guard (footprint > 20 GB or
  swap-used > 26 GB).
- Read-only wall held throughout: no store writes, no schema pokes, no
  code changes, server never touched by me.
