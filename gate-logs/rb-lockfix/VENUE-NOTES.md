# Venue — rb-lockfix landing battery

**Frozen HEAD:** `2141daf479aa6bd3be4eb1617a3555734e49947d`
**Branch:** `fleet/rb-lockfix-result` (fleet delivery `113401a7b` + `origin/main` merged in)
**Merge parents:** 113401a7b2c00d0f56acd0425f731cc0e009959d f576fba4b4f0f15f40f0dbf1996e3158bba63f2c
**origin/main at arming:** f576fba4b4f0f15f40f0dbf1996e3158bba63f2c

Delivery base was `cdde313b3` — 20 commits / 3 merges behind main. Merged main
in at the desk so the gate runs at a real base. Both sides verified present
after the merge (my `eligibility_patience` from #112; their
`required_lock_acquisition_ruling`), because the branch adds a REQUIRED
parameter to `HaematiteStore::open`/`open_or_create` and main has been adding
callers — a clean text merge is exactly where that breaks.

**Pre-gate compile checks (both clean, both verified by the `Finished` line,
not by an exit code):**
- `cargo check --workspace --all-targets` — 0 errors, 0 warnings.
- `cargo check --workspace --all-targets --all-features` — 0 errors, **2
  warnings**, `unused_mut` at `api/http/awl_deployed.rs:414` and
  `api/http/managed_workers.rs:513`. **Pre-existing, not this branch's**:
  neither file appears in the delivery's diffstat. They live in the all-features
  resolution, which `rust:clippy` (`--workspace --all-targets`, no
  `--all-features`) never lints — the gap already recorded for the gates brief.

**Venue:** 10 cores, load 6.48 / 8.01 / 8.49 at arming, **99 GiB free**.
`bun install --frozen-lockfile` — 504 installs / 608 packages, no changes.
Tom's live server is up and is NEVER a venue for anything here.

**Disk:** `target/debug` was reclaimed before this run (79G, stale under any
profile change and #112 had landed, so nothing in flight depended on it).
🔴 `.worktrees/port-247` and `.worktrees/247-leg3` are OFF LIMITS at any pressure.

**Build profile:** this branch introduces `[profile.dev]` / `[profile.test]`
with `debug = 0, incremental = false`. Measured at the desk before arming and
left EXACTLY as delivered — see `PROFILE-MEASUREMENT.md`. This battery is
therefore the first full-workspace run in the new profile world, which is also
why the flake register's base rates need re-stating rather than re-citing.

**What this battery can and cannot settle:** it proves the delivery breaks
nothing at this venue. It does NOT re-price the 250ms lock-acquisition ruling's
population — that was fitted on a 16-core Linux VM (3 samples, 93–150ms window),
and macOS reaches process creation by a different path. Carrier 4's 1/24 base
rate WAS measured here, so this is a legitimate world for the race; whether my
reds share the fleet's mechanism is UNMEASURED.
🔴 Pre-committed before firing: **if it reds here I do not raise 250.**

---
## Run 1 ABORTED DELIBERATELY at ~8 minutes — not a failure

Log preserved as `battery-20260821-ABORTED-for-template-fix.log` (2 legs done,
`rust:fmt` and `rust:clippy` both exit 0, no reds).

I stopped it because I found a defect that had to be in the attested commit:
the shipped operator template said **"RAISE THE PATIENCE if you see that"** for
haematite's *"data dir is locked by another live writer"* — a number raised at a
symptom, given as operator guidance, with no diagnostic step. The common cause of
that message in production is **another live server on the same data directory**,
where a longer patience only delays a refusal that is protecting the WALs.

Both templates are `include_str!`d into `aion-server` and `aion-cli`
(`scaffold.rs:31`, `new/template.rs:61,73`), so it is a **build input, not a
docs-only change** — it cannot ride in a post-gate records commit. Measured
rather than assumed; my first grep for the embed site returned nothing because it
used the `crates/*/src` pathspec that matches nothing in this repo.

One warm battery beats two cold ones: aborting at 8 minutes costs less than
landing known-bad operator guidance and gating a second time.

## Run 2 — the attested run

**Frozen HEAD:** `efc62b55a98e25f8ae5991f8cc34f5aad0429473` (adds `efc62b55a`, the guidance fix)
**Pre-checked:** `config::scaffold` — 4 passed, 1169 filtered out, read off the
individual test lines, including the byte-identity pin
`the_embedded_template_matches_the_shared_cli_template`.

---
## Run 2 ALSO ABORTED (~7 min, 2 legs green) — second truth defect found

Log: `battery-20260821-ABORTED-2-for-readme-truth.log`.

All five `aion new` scaffold READMEs said *"state persists in `./aion-data`"*.
False for every project they scaffold: the scaffolded `aion.toml` pins no
`data_dir` (`new::template_tests` asserts it must not), so the home default
applies — `$AION_HOME/data`, deriving to `~/.aion/data`. Same defect class
that nearly returned a production server on an EMPTY store during the 0.18.1
restart. READMEs are `include_str!`d, so it is a build input and cannot ride
post-gate.

**Swept before aborting so this is the last one:** every data-location claim in
both template trees is now accounted for — 5 READMEs (fixed) and
`shared/gitignore`'s `aion-data/` entry, kept deliberately because it asserts
nothing about where data lives and still protects the legacy case.

## Run 3 — the attested run

**Frozen HEAD:** `31cf6cf7ec75b3d6395ee488ba4cdddfc4edb0ba`
Adds `efc62b55a` (writer-lock guidance: diagnose before tuning) and
`31cf6cf7e` (scaffold README data directory).
**Pre-checked:** `config::scaffold` 4 passed / 1169 filtered;
`aion-cli --bins new::` 24 passed / 270 filtered. Both read off result lines.

🔴 **`.fleet/` is still present at this HEAD and that is deliberate.** It is
stripped in a visible commit AFTER the gate, on the desk's own census: 0 of 24
declared legs mention fleet, gates.json has no `.fleet` reference, no crate
source or test reads it, it is not a workspace member — with positive controls
of 20 and 1363 files on the same pathspecs. The only readers are
`workflows/fleet-dev/bin/*.sh`, the fleet rig, which no declared leg invokes.
