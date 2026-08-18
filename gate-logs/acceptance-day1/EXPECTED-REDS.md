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

## Every other leg: NO expected reds
fmt, clippy_workspace, clippy_server, test, embed, gates_worker, write_manifest
are expected green on the pinned instruments (rustup 1.97.1 per repo pin,
clean target). Run-1/run-2 reds were instrument-era (no rustup; mixed-toolchain
target artifacts) and are recorded in their run histories, not carried forward.
