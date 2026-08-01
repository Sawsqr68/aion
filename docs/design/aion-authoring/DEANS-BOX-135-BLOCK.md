# Dean's-box block for #135 — full aion-server battery (post-#91-leg-2)

Staged 2026-08-01, seat Vesper. Send to Tom when he has a free moment (his
preference: HE runs the blocks, pastes back the number lines; I verify remote
HEAD over SSH and scp logs myself — remote-gates model proven on 5e03cf24).
Purpose: the 27 aion-server test targets Athena's cycle-arithmetic ruling
kept off the local box (landing record: SEAT-RESUME-2026-08-01-P.md).

Copy-paste blocks for Dean's box (`deanwhiting@100.68.29.10`, checkout at
`/Users/deanwhiting/Developer/stack/aion`):

```
# 1 — bring the checkout to the landing tree (fails-safe form)
cd /Users/deanwhiting/Developer/stack/aion \
  && git fetch origin \
  && git checkout -B main origin/main \
  && git log --oneline -1
```

```
# 2 — the battery (logs + honest exit)
cd /Users/deanwhiting/Developer/stack/aion \
  && cargo test -p aion-server --no-fail-fast > /tmp/gate-135-aion-server.log 2>&1; \
  echo "exit=$?"
```

```
# 3 — the summary lines to paste back
grep -c '^test result:' /tmp/gate-135-aion-server.log; \
grep -E '^test result:' /tmp/gate-135-aion-server.log \
  | awk '{p+=$4; f+=$6} END {print "targets="NR" passed="p" failed="f}'
```

Expected: 41 `test result:` lines, failed=0, exit=0 at tree ≥ 54ae9a43.
Caveats for the verdict (mine to apply, not Tom's): Dean's rustc is newer —
clippy/fmt skew is forward-compat signal, not noise; verify remote HEAD over
SSH before accepting numbers; scp the log to gate-logs/deans-box-135.log for
the evidence commit.
