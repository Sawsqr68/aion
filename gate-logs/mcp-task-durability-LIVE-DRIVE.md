# MCP task durability — driven against a running binary that was killed

**2026-08-19 08:25–08:31Z.** The last thing owed on `bb9a98f1b`. The e2e test
`a_task_survives_a_server_that_never_saw_it` builds a second harness over the
same store, which is a fast regression guard. This is the decisive one: a real
process, killed with `SIGKILL`, restarted, and asked again.

## The venue, established rather than assumed

Own port (8391), own gRPC port (50391), own data dir under an owner-only path.
**Tom's server on 8080 was not touched** and its `./aion-data` was never opened.

The process answering on 8391 was identified through `lsof` and hashed — not
trusted from the version string, which would have been the same for a stale
binary:

```
port holder pid 4307 → /Users/tom/Developer/ablative/stack/aion/target/debug/aion
682c44e1f63a00ddc4d970da36006d9d29c7b3e4c9f0b97c251d48cf38839d75
```

That is the binary built from this branch at 0.17.0, byte for byte. The hash was
re-checked after every restart.

### Three refusals on the way in, each one the gate working

Worth recording because each is a control that the surface is not permissive:

1. `store.node_cache_budget is required and has no default` — the boot refused
   rather than picking a cache size.
2. `unsafe store.data_dir … ancestor /private/tmp is not owner-controlled:
   mode 1777` — the path-safety gate refused the scratchpad outright. The data
   dir was moved under an owner-only directory instead of the gate being
   worked around.
3. `observability.max_batch_events is required and has no default`.

And on the wire: `Mcp-Name header was sent but this request's body carries no
value for it` — the header/body mirror is enforced in BOTH directions, not just
when the header is missing.

## The sequence

```
tools/call start_run {workflow_type:"canary", await_completion:true}
  → resultType "task"
    taskId    run.v1:ad257c69-…:b630715e-…:default
    status    working
    ttlMs     null            ← unlimited, on the live wire
    createdAt 2026-08-19T08:29:12.287573+00:00

tasks/get   → working, createdAt 08:29:12.287573

kill -9 4307
  port released, ps shows nothing, curl exit 7 (connection refused)   ← the
  negative control: the port genuinely stops answering, so the next success
  cannot be the old process

restart → pid 8383, same data dir, same binary hash

tasks/get   → working, createdAt 08:29:12.287573   ← SAME task, SAME timestamp,
                                                     answered by a process that
                                                     never saw it minted
```

On the pre-change code this is where it ended: `TaskStore` was
`Arc<Mutex<HashMap<…>>>` and the handle was a freshly-minted UUID, so a restart
dropped every task while the runs carried on. That is the defect this change
closed, and this is the shape of its closure.

## The controls — because "always working" would look identical

A projection that answered `working` to anything would have produced exactly the
transcript above. So it was shown able to answer otherwise.

| Probe | Answer |
|---|---|
| Well-formed handle naming a run that does not exist | `-32602` |
| The REAL handle with the namespace swapped for `not-a-namespace` | `-32602` |
| `not-a-handle` | `-32602`, naming the format |

The refusal message is `task … is not a task this caller can read` — it does not
distinguish "no such task" from "not yours", which is the point: the handle is
derived from ids the caller already holds, so a distinguishing answer would turn
constructibility into a way to probe for another caller's work.

**And the status was watched change.** `tasks/cancel` was acknowledged and left
the run alone (`working` after it, as the extension permits — cancellation is
cooperative). Then the run was cancelled for real through the `cancel` TOOL:

```
tasks/get → status        "completed"
            createdAt     08:29:12.287573    ← held
            lastUpdatedAt 08:30:04.968063    ← advanced to the ending
            result.structuredContent.status  "Cancelled"
```

A cancelled run is a **completed task** carrying the run's terminal status —
the tool succeeded at its job of reporting how the run ended. `statusMessage`
changed with it, from the cancel-does-not-stop-the-run prose to "the run has
ended; its terminal status is in the result", because the message is projected
on every read rather than stored once at creation.

## The second kill, against a SETTLED task

The first restart proved a *running* task survives. A settled one carries a
result, which is rebuilt from history rather than remembered, so it was killed
again:

```
kill -9 8383  → curl 000 while dead
restart       → pid 10179
tasks/get     → completed, createdAt 08:29:12.287573,
                lastUpdatedAt 08:30:04.968063, ttlMs null, status "Cancelled"
```

Three distinct pids — 4307, 8383, 10179 — one task id, one unchanged answer.

## What this does not prove

Two callers were not exercised here; the authorization refusal on a VALID handle
held by a caller without the namespace is proven in
`a_valid_handle_is_refused_to_a_caller_who_does_not_hold_the_namespace`, on the
dev-token auth path, and that test was watched go red under mutation. This drive
ran as a single caller, so what it adds is durability, not entitlement.

Progress reporting is still not built. `notifications/tasks` is a MAY in the
extension and we implement neither it nor `subscriptions/listen`, which is
conformant — but it remains the piece that would answer "tell me how it is
going", as opposed to "tell me when it is done".

## Cleanup

The scratch server was stopped and its data directory removed. Nothing was left
running and nothing was left on Tom's box by this drive.
