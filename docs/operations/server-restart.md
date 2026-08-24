# Server restart procedure

The one procedure for stopping, starting, and replacing the production
`aion server` process, and for reading what killed one that died on its own.
Written 2026-08-17 from the 2026-08-16 double-death incident; rewritten when
`aion server stop` / `aion server status` shipped — the verbs are now the
primary path, and the by-hand ritual they replaced is kept below as the
fallback it becomes (a server started by a pre-verb binary wrote no pid
record for the verbs to find).

Every step is a command you run and a check you read. Nothing here is
optional; the checks are what make the next death attributable.

## Standing facts

- **Home**: `$AION_HOME`, default `~/.aion`. Config at `~/.aion/config.toml`.
- **Pid record**: `<AION_HOME>/run/aion-server.pid`, written by the server
  itself after BOTH listeners bind — one JSON object carrying pid, start
  instant, binary SHA-256, version, commit, and the bound addresses. This is
  an incarnation identity, not a bare number: the verbs verify the recorded
  start instant against the live process table before touching anything, so
  a recycled pid is a refusal, never a kill.
- **Logs**: `~/.aion/logs/<name>-<UTC date>.log` (e.g.
  `aion-server-20260816.log`), append.
- **Death note**: `<AION_HOME>/logs/aion-server.death.log`, written by the
  server itself (see `crates/aion-server/src/death_note.rs`). The startup
  banner names it. **The note is not single-tenant**: test builds run from
  worktrees against the same home write their own ARMED/DISARMED brackets
  into the same file. When reading it by hand, match on pid AND the build
  string in the ARMED line, never on recency alone. (The verbs bind to the
  recorded pid's bracket, so they are not confused by other tenants.)

## Stopping a server

```sh
aion server status     # read what is actually running before acting
aion server stop       # SIGTERM the verified incarnation, wait, report
```

- `stop` waits up to `--patience <seconds>` (absent, the config's own
  `drain.timeout_seconds`) and then prints the drain outcome from the death
  note by name: clean, or parked work by worker and task, or the honest
  absence when no record was written.
- A **parked** drain is a SUCCESS — in-flight agent activities can legally
  outlive any sane drain window; recovery re-adopts them on the next boot.
  Do not escalate just because the drain took its configured time.
- **still draining** at patience is not a failure: the server owns its
  drain. Run `aion server stop` again ONLY when the drain is genuinely
  wedged — the server treats the second termination signal as "force
  immediate exit" (exit 130) — and say so wherever you report the restart.
- A refusal is final until you resolve it by hand: `no pid file` means no
  server claimed this home through the verb-aware binary (see the fallback
  ritual below); a `stale incarnation` refusal means the recorded pid now
  belongs to a different process — verify with `ps -p <pid>`, remove the
  named file once satisfied, and never signal the number the file carried.
- Never `kill -9` a server that is answering its log; SIGKILL is the one
  death the note cannot record, and the next `aion server status` will
  report exactly that shape (`bracket never closed`).

**Respawner interaction — HOLD FIRST.** On an estate where a watcher or
respawner restarts dead servers, stopping the server without holding the
watcher first starts a race: the respawner sees the exit and boots the old
binary back before your replacement starts. Hold/disable the respawner,
`aion server stop`, do your work, start, then release the hold.

## Starting a server

1. Start detached — its own session, immune to the terminal's death and the
   session reaper (the #262 exposure was a server left inside a killable
   process group):

   ```sh
   UTC_DATE=$(date -u +%Y%m%d)
   nohup aion server >> ~/.aion/logs/aion-server-"$UTC_DATE".log 2>&1 &
   ```

   (If the production store is still the LEGACY CWD-relative directory
   (`<repo>/aion-data`), start from the directory the store lives under — a
   wrong CWD starts a healthy-looking server over an EMPTY store. The
   startup log states both `config_source` and `data_root`: read them every
   start.)
2. `aion server status` — the record layer shows the new pid, version,
   commit, and bound addresses; the build layer answers from the server's
   own listener; ready answers YES once the store is up. The doors bind in
   seconds even over a large owed-timer backlog (the catch-up runs BEHIND
   the open doors since 0.23.0); "bound but catch-up still draining" is a
   normal serving state, visible in the log as
   `startup catch-up running behind open doors`.
3. Read the startup log before calling it up: `config_source`, `aion_home`,
   `data_root`, and the banner's `version`/`build`/`commit` — record them
   where you announce the restart. The banner's `death_note` path exists and
   its file now ends with an `ARMED pid=<new pid>` line.
4. `aion versions` answers non-empty (an empty catalog over the production
   store means the wrong store resolution — stop it and start over).
   Workers re-register on their own; confirm the queues you rely on show
   their workers before dispatching anything that matters.

## Replacing the binary under a live server

`cp` onto a running image truncates the running binary. Swap by rename:
build/install to a new path, `mv` the new file over the old name (rename is
atomic; the running process keeps its old image), then restart per this
procedure. Identify binaries by SHA-256, never by version string — the pid
record carries the serving binary's hash for exactly this comparison.

## Cutting a release: the changelog embed

The embedded changelog is a second copy of the root `CHANGELOG.md` and the
test that guards them (`changelog_embed_matches_the_authored_document`)
names its own remedy in the assertion message — the remedy IS the cp:

```
cp CHANGELOG.md crates/aion-server/changelog-embed/CHANGELOG.md
```

Run it in the same motion as every root-changelog edit at a release cut.
Missed three times now (0.15.0, 0.16.0 at f27c1faf2, and the 0.22.0 cut):
each miss leaves main red on that test for every tree branched after it.

## Cutting a release: the tag-tree census

Before ANY tag is pushed, the tag's tree must pass the mechanized census
(docs repo, `tooling/tip-clean.sh` — added after the 2026-08-24 finding
that v0.22.0 published `.fleet/` flight residue and `gate-logs/` carried a
live credential into five public tags):

```
tip-clean.sh <repo> <tag-tree>   # must exit 0
```

rc=1 (DIRTY) blocks the push; rc=2 (REFUSE) blocks louder — the instrument
could not measure, which is never a pass. Run the census against
**remote-peeled shas**, never local tag refs: local tags can silently hold
a different commit than the remote (two did on 2026-08-24, both refused by
fetch as clobbers), and a census over a stale local ref measures the wrong
object.

The census population is every PUBLIC ref, not the tag alone: main, all
remote-peeled tags, and all remote branches — `fleet/*` refs above all,
because fleet work is exactly what mints session transcripts, and a
transcript proved to carry a live token on nine public `fleet/*` branches
that no tag-scoped census would ever have read (2026-08-24, second
credential). A pattern-scoped sweep is only as wide as its pattern; the
population must be as wide as what is fetchable.

## When a server is found dead

`aion server status` first: a dead recorded server is reported with the
death note's account for that pid (including the drain outcome record when
one was written). Then read in this order, and write down what each said
before forming a theory:

1. **The death note** (`~/.aion/logs/aion-server.death.log`) — the partition
   (match pid AND build string; the file is shared by every build that runs
   against this home):

   | Note ends with                       | Meaning                                                        |
   |--------------------------------------|----------------------------------------------------------------|
   | `DISARMED clean run-loop exit…`      | ordinary shutdown; the `OUTCOME` entry above it has the drain story |
   | `PANIC …` then `DISARMED …`          | died by panic; the entry has thread, location, backtrace       |
   | `SIGNAL <name> received; terminating`| killed by that signal from outside; go find the sender         |
   | `SIGNAL SIGTERM observed` then nothing | drain started and never finished — read the main log tail    |
   | `ARMED …` and nothing after          | SIGKILL / raw exit / fatal fault — see below                   |

   `OUTCOME <json>` entries are the drain's own record — which face the
   drain ended on, what was parked, which managed workers stopped — written
   before the disarm; `aion server stop` renders the same record.
   `BREADCRUMB` entries are not endings — they name work as it starts.
   Whatever the note ends with, the LAST breadcrumb above it names what was
   in flight when the process died.

2. **The main log tail** — the last timestamp bounds the death; WARN/ERROR
   lines in the final minute are the face.
3. **OS evidence** — `~/Library/Logs/DiagnosticReports/` for crash reports
   (their ABSENCE is evidence against the SIGSEGV/SIGABRT class, which the
   note cannot record by itself), `log show --last <window>` for jetsam or
   kernel kills.
4. Only then attribute. "Armed, nothing after, no crash report, no jetsam"
   is the SIGKILL-or-raw-exit class; suspect an external reaper or an
   in-process `exit()` and hunt the sender, not the server.

## Fallback: the by-hand ritual (pre-verb binaries only)

A server started by a binary older than the verbs wrote no pid record;
`aion server stop` refuses with `no pid file` rather than guessing. For
that server, one last time:

1. Find it: `ps aux | grep 'aion server'`, confirm the command line and the
   home it serves before trusting any pid.
2. `kill -TERM <pid>` — one TERM; the drain owns shutdown. Wait for
   `ps -p <pid>` to empty; a second TERM forces (exit 130).
3. Start the replacement per this procedure — the verb-aware binary writes
   its own record, and this ritual is never needed again for it.

## Reporting

Every stop, start, or swap gets a line wherever the estate reports state:
UTC time, old pid → new pid, version + commit from the banner (or from
`aion server status`), and which of the checks above you actually read. A
restart nobody can reconstruct is the first half of the next anonymous
death.
