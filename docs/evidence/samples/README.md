# Captured process evidence

Raw artefacts kept because the finding they support is about a **live** process
state that no test observes. Each is named for the task it belongs to.

## `aion-server-hang-after-drain-15202.*`

Task #72 — `aion server` never exits after a signal-initiated graceful drain.

- `.log` — the server's own output. Its **last line** is
  `activity drain completed cleanly` at `2026-07-29T23:22:15.645368Z`. Nothing
  follows it, ever.
- `.sample` — a macOS `sample` of the same process taken 2026-07-30, over two
  hours later, with the process still alive. It shows
  `beamr::scheduler::execution::park_thread`,
  `beamr::scheduler::run_queue::RunQueue::steal_half_from`, and
  `beamr::io::thread_pool` receivers all live and parked: a fully-booted idle VM
  that was never stopped, in a process whose transports had already exited and
  which held no listening TCP socket.

The pairing is the point, and it corrected a wrong reading. The log says the
shutdown completed; the sample says the runtime is still running.

My first diagnosis was that `run.rs`'s shutdown-signal arm never calls
`state.shutdown()`. **That was wrong** — `shutdown.rs:145-150` calls it for every
non-`Forced` outcome. The sample is what disproved it, and it names the real cause:

- the **main thread** is inside `drop_glue<BlockingPool>` → `BlockingPool::shutdown`
  → `Receiver::wait`. `main` has already RETURNED; the process is stuck in tokio's
  runtime `Drop`, waiting for the blocking pool to drain.
- another thread shows why it never drains: a `spawn_blocking` dispatch parked in
  `queue_service::wait::select_worker_or_refuse` → `park_for_worker` →
  `block_on(registry.wait_for_worker())` with **no timeout and no shutdown in the
  race** (`wait.rs:186` passes `None` whenever no schedule-to-start deadline is
  configured, which is the default).

So the drain is honest about what it tracks, `state.shutdown()` succeeds, and the
hang happens *after* both — in runtime teardown, after the last log line. See #72.

The binary sampled predates HEAD (its symbol is `park_for_workers`, plural, which no
longer exists), but the hazard is unchanged in today's `park_for_worker(None)`. This
is not a stale-binary artefact.

The observed process was pid 15202 on Tom's box. It was left **running** rather
than killed, so whoever takes #72 can attach to a real hung instance; it holds a
store writer lock on a throwaway proof data directory only, so killing it is safe
at any time and costs nothing but the live reproduction. It will not survive a
reboot.
