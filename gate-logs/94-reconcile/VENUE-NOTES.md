# venue at arming (run 4 — the real gate)

- box: Tom's Mac, 10 cores
- armed: 2026-08-20 20:04:28 +1000
- frozen HEAD: 2ee53285d66e8fe749c63f947c55e157f524dc40
- base: dfea2ec838e19993b493dc6e3545647194a581e9 (origin/main, = #83's merge dfea2ec83)
- disk at arming: 69 GiB free
- warm target carried over:  41G
- load at arming: 29.56 33.27 25.68
- deps: bun install --frozen-lockfile done earlier; node_modules untracked and untouched
- Tom's live server (pid 67116) is a permanent tenant on this box and is never a venue

## at completion

- battery pid: **59660** (detached `nohup`, ppid 1), exited 2026-08-20 20:52:40 +1000
- elapsed: 48 minutes, 24 legs, all exit 0
- HEAD at completion: `2ee53285d` — **unchanged** from arming
- disk: 69 GiB free at arming → 34 GiB at the last sample; peak load 56.49 on 10 cores
- the `battery.pid` file was folded into this line and removed. A `.pid` naming a
  dead process is not a record, it is a loaded gun: pids recycle, and the next
  reader to `kill` it hits whatever inherited the number.

**Both watches on this run were killed by the host's background-task reaper at
~35 minutes; the `nohup`'d battery itself sailed through.** They were plain
background bash, against my own standing rule to use Monitor for watches. What
made the loss legible was the notification saying `killed` rather than
`completed` — a reaped watch otherwise produces silence indistinguishable from a
healthy run. Re-armed as a Monitor, which is what reported the completion above.
⇒ On any watcher event, read the status field and check the watched process
directly before concluding anything about it.
