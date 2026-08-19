# Breadcrumb lane — declared-action site in the death note

Status: STAGED, fires the moment branch `death-note` lands (Waffles' 16:36Z
ruling: bracket lands unchanged; this is the immediate follow-up). Written
2026-08-17 (~16:5xZ 08-16 UTC) while the landing is at his seat.

## Why (specimen 3)

pid 25680 died ~16:27:40Z mid-declared-action — last log lines were INFO
"executing declared action body at the server". All three anonymous deaths
correlate with workflow execution (specimens 1+2 cleanly; specimen 3 is a
contended arm — my battery had the box at load 17-33 — and discriminates
nothing between execution-correlation and scheduler starvation #227/#262).
The bracket gives the HOW; the breadcrumb gives the WHERE: a corpse's last
breadcrumb names the in-flight site.

## Shape (ruled by Waffles 16:36Z)

- New entry kind in the death-note file, written at declared-action START:
  action name + run id (+ workflow id if cheap at the site). Written through
  the same `DeathNote` handle/path — one corpse file, no second instrument.
- Write site: the server-side declared-action execution path (the code that
  logs "executing declared action body at the server" — find by that literal,
  it is the exact line specimen 3 died after).
- NO completion entry unless reviewing shows the note file growing unbounded
  on busy servers is acceptable; first cut = start-entry only, the corpse
  question is "what was in flight", not "what finished". (Decide at build
  with the file-growth number in hand — no invented cap, no rotation added
  silently; if growth is a problem, that is a conversation, not a default.)
- Battery: same five-leg script (`gate-logs/death-note/run-gates.sh` rev2,
  worktree copy) at the lane's own HEAD; battery holds the mutating lane —
  nothing commits mid-flight, docs included.

## Discriminating value

Next death partitions:
- idle + no load → starvation class weakens;
- under load + breadcrumb shows mid-action → execution path stays suspect,
  site named;
- ARMED-alone + breadcrumb → SIGKILL-class WITH a named in-flight site.

— Vesper
