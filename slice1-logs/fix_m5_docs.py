"""M5: the enumeration is THREE, and norn-dev's runbook teaches the new shape.

Also the one missing sentence the reviewer asked for: why the SERVER's managed-worker
spawn environment is deliberately not cleared.
"""

import pathlib

# ------------------------------------------------------- M5: the enumeration
p = pathlib.Path("docs/workers/AGENT-WORKERS.md")
s = p.read_text()

old = """Neither document this repository ships with an agent action —
`crates/aion-server/assistant-embed/assistant.awl` and
`workflows/norn-fleet/norn_fleet.awl` — carries a `harness` section yet, so
serving either with this verb means adding one first."""
new = """None of the three documents this repository ships with an agent action —
`crates/aion-server/assistant-embed/assistant.awl`,
`workflows/norn-fleet/norn_fleet.awl` and `workflows/norn-dev/norn_dev.awl` —
carries a `harness` section yet, so serving any of them with this verb means
adding one first. (The census is every document with an `agent`-marked action:
`grep -rln 'agent,\\|^    agent$' workflows/ crates/aion-server/assistant-embed/`.)"""
if old not in s:
    raise SystemExit("AGENT-WORKERS enumeration context not found")
s = s.replace(old, new, 1)

# ------------------------------------- the missing sentence about the server's spawn
anchor = "The rules it lives by, and why:"
addition = """### Why the SERVER's own spawn environment is not cleared

`env_pass` governs the AGENT SUBPROCESS — the `claude-code-acp` or `norn` a
worker launches. It does not govern the worker PROCESS itself, and the managed
-worker supervisor (`crates/aion-server/src/worker/supervisor/`) still hands a
supervised worker the server's own environment, inherited. That is deliberate,
and it is not a gap left in the fence:

- The worker process's own settings are read from that environment BY DESIGN.
  The ops console's deploy form documents it in as many words — a setting left
  blank "must come from the server process's environment
  (`AION_WORKER_CONCURRENCY`, `AION_RECONNECT_*`)" — so clearing it would break
  every deployed `aion worker awl` worker that relies on it, silently, at the
  next supervised restart.
- Clearing it would not add safety where the incident happened. `CLAUDECODE`
  reaching a worker process harms nothing; it broke a live run by reaching the
  AGENT the worker spawned. That boundary is now closed at the spawn site,
  which is downstream of every launcher — a hand-run CLI, a console click, a
  supervised restart — so one mechanism covers all three and none of them can
  opt out.
- The two are independent by construction: whatever the worker process
  inherits, the agent child gets `env_pass` and nothing else.

Declaring the worker process's own environment is a separate decision on a
separate surface, and it needs the operator's word on what a supervised worker
is allowed to see. It is not this change.

The rules it lives by, and why:"""
if anchor not in s:
    raise SystemExit("AGENT-WORKERS rules anchor not found")
s = s.replace(anchor, addition, 1)
p.write_text(s)
print("M5 enumeration + server-env rationale:", "norn_dev.awl" in s)

# ------------------------------------------------------- M5: norn-dev's runbook
p = pathlib.Path("workflows/norn-dev/README.md")
s = p.read_text()
print("norn-dev README bytes:", len(s))
