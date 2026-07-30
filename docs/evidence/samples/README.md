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

The pairing is the point. The log says the shutdown completed; the sample says the
runtime is still running. `run.rs`'s shutdown-signal arm never calls
`state.shutdown()`, so the engine is never stopped — see #72 for the read.

The observed process was pid 15202 on Tom's box. It was left **running** rather
than killed, so whoever takes #72 can attach to a real hung instance; it holds a
store writer lock on a throwaway proof data directory only, so killing it is safe
at any time and costs nothing but the live reproduction. It will not survive a
reboot.
