# Server restart procedure

The one procedure for stopping, starting, and replacing the production
`aion server` process, and for reading what killed one that died on its own.
Written 2026-08-17 from the 2026-08-16 double-death incident (pids 52844 and
2681, both anonymous at the time); merges the `~/.aion/logs/` discipline
proven that day with the detached-launch posture from the #262 dig and the
death note shipped by the same branch as this document.

Every step is a command you run and a check you read. Nothing here is
optional; the checks are what make the next death attributable.

## Standing facts

- **Home**: `$AION_HOME`, default `~/.aion`. Config at `~/.aion/config.toml`.
- **Data root**: until the home migration completes, the production store is
  the LEGACY CWD-relative directory (`<repo>/aion-data`), so the server MUST
  be started from the directory the store lives under — a wrong CWD starts a
  healthy-looking server over an EMPTY store. The startup log states both
  (`config_source`, `data_root`): read them every start, and treat an
  unexpected `data_root` as a failed start.
- **Logs**: `~/.aion/logs/<name>-<UTC date>.log` (e.g.
  `aion-server-20260816.log`), append; pid file `~/.aion/logs/<name>.pid`
  containing the bare pid.
- **Death note**: `<AION_HOME>/logs/aion-server.death.log`, written by the
  server itself (see `crates/aion-server/src/death_note.rs`). The startup
  banner names it.

## Stopping a server

1. Identify what you are stopping — never a pid from memory:
   `cat ~/.aion/logs/aion-server.pid`, then `ps -p <pid> -o pid,command` and
   confirm the command is `aion server`. If the pid file and the live process
   disagree, stop and reconcile first; the pid file is stale evidence of an
   earlier corpse.
2. `kill -TERM <pid>` — one TERM. The graceful drain owns shutdown; the log
   gains "shutdown signal received; beginning graceful drain", and the death
   note gains a `SIGNAL SIGTERM observed` line plus a `DISARMED` line when
   the drain completes.
3. Wait for the process to exit (`ps -p <pid>` empty). In-flight agent
   activities can legally outlive the drain window — a "parked" drain is a
   SUCCESS (recovery re-adopts on the next boot); do not escalate just
   because the drain took its configured time.
4. A second TERM forces immediate exit (exit 130) — use it only when the
   drain is genuinely wedged, and say so wherever you report the restart.
   Never `kill -9` a server that is answering its log; SIGKILL is the one
   death the note cannot record.

## Starting a server

1. From the store's directory (see Standing facts), start detached — its own
   session, immune to the terminal's death and the session reaper (the #262
   exposure was a server left inside a killable process group):

   ```sh
   cd /Users/tom/Developer/ablative/stack/aion   # the store's parent, today
   UTC_DATE=$(date -u +%Y%m%d)
   nohup aion server >> ~/.aion/logs/aion-server-"$UTC_DATE".log 2>&1 &
   echo $! > ~/.aion/logs/aion-server.pid
   ```

   (`gate-logs/detached-relaunch.py` is the proven scripted form of the same
   posture; either is acceptable, the log+pid discipline is not optional.)
2. Read the startup log before calling it up:
   - `config_source` and `aion_home` are the ones you expect;
   - `data_root` is the production store, not a fresh directory;
   - the startup banner line carries `version`, `build`, `commit` — record
     them where you announce the restart (a version string alone cannot
     distinguish two builds; the commit can);
   - the banner's `death_note` path exists and its file now ends with an
     `ARMED pid=<new pid>` line.
3. Verify liveness from outside: `curl -fsS http://127.0.0.1:8080/health/live`
   answers 200, and `aion versions` answers non-empty (an empty catalog over
   the production store means the wrong CWD/store — kill it and start over
   from the right directory).
4. Confirm the pid file matches the live process (`cat` it, `ps -p` it).
   Workers re-register on their own; confirm the queues you rely on show
   their workers before dispatching anything that matters.

## Replacing the binary under a live server

`cp` onto a running image truncates the running binary. Swap by rename:
build/install to a new path, `mv` the new file over the old name (rename is
atomic; the running process keeps its old image), then restart per this
procedure. Identify binaries by SHA-256, never by version string.

## Cutting a release: the changelog embed

The embedded changelog is a second copy of the root `CHANGELOG.md` and the
test that guards them (`changelog_embed_matches_the_authored_document`)
names its own remedy in the assertion message — the remedy IS the cp:

```
cp CHANGELOG.md crates/aion-server/changelog-embed/CHANGELOG.md
```

Run it in the same motion as every root-changelog edit at a release cut.
Missed twice now (the 0.15.0 cut, then the 0.16.0 cut at f27c1faf2 —
caught by a branch battery, fixed at main 15bb089da): each miss leaves
main red on that test for every tree branched after it.

## When a server is found dead

Read in this order, and write down what each said before forming a theory:

1. **The death note** (`~/.aion/logs/aion-server.death.log`) — the partition:

   | Note ends with                       | Meaning                                                        |
   |--------------------------------------|----------------------------------------------------------------|
   | `DISARMED clean run-loop exit…`      | ordinary shutdown; the main log has the drain story            |
   | `PANIC …` then `DISARMED …`          | died by panic; the entry has thread, location, backtrace       |
   | `SIGNAL <name> received; terminating`| killed by that signal from outside; go find the sender         |
   | `SIGNAL SIGTERM observed` then nothing | drain started and never finished — read the main log tail    |
   | `ARMED …` and nothing after          | SIGKILL / raw exit / fatal fault — see below                   |

   `BREADCRUMB` entries are not endings — they name work as it starts
   (today: each declared action body, with action name, workflow, run,
   activity, and attempt). Whatever the note ends with, the LAST breadcrumb
   above it names what was in flight when the process died; the third
   2026-08-16 death (pid 25680) died mid-declared-action and only an
   ordinary log line said so.

2. **The main log tail** — the last timestamp bounds the death; WARN/ERROR
   lines in the final minute are the face.
3. **OS evidence** — `~/Library/Logs/DiagnosticReports/` for crash reports
   (their ABSENCE is evidence against the SIGSEGV/SIGABRT class, which the
   note cannot record by itself), `log show --last <window>` for jetsam or
   kernel kills.
4. Only then attribute. "Armed, nothing after, no crash report, no jetsam"
   is the SIGKILL-or-raw-exit class — the 2026-08-16 pair's face; suspect an
   external reaper or an in-process `exit()` and hunt the sender, not the
   server.

## Reporting

Every stop, start, or swap gets a line wherever the estate reports state:
UTC time, old pid → new pid, version + commit from the banner, and which of
the checks above you actually read. A restart nobody can reconstruct is the
first half of the next anonymous death.
