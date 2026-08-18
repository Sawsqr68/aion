# Acceptance day 1 — declared expected-red set (2026-08-18)

Applies to every gates run on rocketfish measuring main at or after 66da344fb
UNTIL aion#57 lands; each entry names its row. A red outside this set is a
finding, never noise. This file exists so a red leg reads "expected-red: #57
pending" by name, not from memory (Waffles' requirement, 09:07Z).

## bun leg — 4 tests, all aion#57 (ICU platform glue; fix built, battery queued)
- `viewer-local rendering > a known instant renders exactly under an explicit timeZone option` (time.test.ts)
- `viewer-local rendering > the pinned system zone (Australia/Sydney) IS the default rendering zone` (time.test.ts)
- `formatInstantMs > renders a known epoch and refuses non-finite input` (time.test.ts)
- `InstantText > a parseable instant renders a <time> whose dateTime and title carry the raw UTC value` (shared-presentational.test.tsx)

Mechanism: CLDR 42 en-US date-time glue " at " (macOS ICU) vs ", " (Linux ICU);
exact-string pins can only match one platform until #57's deterministic
composition lands. The bun leg therefore exits 1 with exactly these 4 failures
(1033 pass) — any OTHER bun failure is out of set.

## cargo legs — expected reds until aion#59 lands (declared 09:45Z after run 5)
- clippy_workspace, clippy_server: RED on exactly one finding —
  `unnecessary_wraps` at `crates/aion-server/src/filesystem.rs:180`
  (backend_path's Linux cfg arm is infallible; genuine Linux finding, #59).
- gates_worker: RED on exactly one test —
  `an_absent_gate_tool_is_a_broken_measurement_not_a_result` (fixture assumes
  /usr/bin lacks cargo; false on Arch, #59).
- test: expected GREEN from run 6 on — its run-5 red was EMFILE under the
  box's soft nofile 1024; the worker now runs at the box's own hard limit
  (524288). A recurrence is a finding.
Any cargo-leg red OUTSIDE these named shapes is a finding.

## fmt, embed, write_manifest: NO expected reds

## Record corrections (amended in place, 09:45Z)
- RUN-1 FENCE CORRECTED: run 1's clippy red was fenced above as
  instrument-era (clippy 1.94 vs 1.97.1 drift). That attribution was WRONG —
  run 5, zram-free on pinned 1.97.1, reproduced the same lint. Run 1's clippy
  red is RE-ATTRIBUTED GENUINE, see #59. (Run 1's rustup gap was still real;
  it changed which clippy ran, not the verdict.)
- Run-2 reds (mixed-toolchain target artifacts), run-3 reds (registry-cache
  NUL corruption), run-4 reds (rustc SIGSEGV) stay fenced as box-era; root
  cause for runs 3-4's corruption class = zram swap failure
  ("Decompression failed! err=-22"), zram now masked at the unit level.
- RUN-5 "ZRAM-FREE" LABEL CORRECTED (10:05Z): root dmesg shows zram re-armed
  itself at 09:24:40Z (2 min after the session swapoff) and stayed live until
  the mask landed at 09:38:02Z — a window that covers run 5 entirely. Run 5
  was NOT zram-free. #59's three findings are unaffected: each stands on its
  own deterministic mechanism receipt (cfg-arm lint reasoning + post-fix Linux
  clippy_rc=0; nofile 1024 read from /proc; fixture logic), not on run 5's
  cleanliness. The first genuinely zram-free run is run 6.

## Run 6 (de74088c, 09:54:51Z, main e98693d7, gates$06018b87 route-success)
- STATUS = Completed WITH failing verdict as data — the aion#58 closure
  receipt: red gate results now route success and the workflow completes.
- In-set reds confirmed: bun (4 named tests), clippy_workspace + clippy_server
  (the one #59 lint), gates_worker (the one #59 fixture test).
- OUT-OF-SET finding (row #60): test leg RED — rustc ICE
  "incremental compilation error with exported_generic_symbols(beamr)"
  (aion-cli test awl_emit_beam) + rust-lld undefined-symbol where the
  suggested symbol EQUALS the missing one (symbol-hash mismatch,
  assistant_private_queue link). Corruption-shaped, first of its class in a
  genuinely zram-free window. Confound: the 108G workspace target (incl.
  incremental caches) spanned the zram + SIGSEGV eras and was never purged —
  purged 10:03Z. btrfs root corruption_errs baseline at purge = 52683
  (fresh error event logged 09:40:34Z). Run 7 on the purged target is the
  discriminating measurement: clean ⇒ stale-artifact era closed; any fresh
  corruption shape ⇒ RAM indicted ⇒ memtester + relocation per the staged
  decision tree.
