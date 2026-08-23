# AWL-CUT1 — dispatch rig (first AWL command-builder implementation flight)

Staged by Waffles 2026-08-23. Brief: `briefs/awl-builder-cut1-implementation.md`
at docs `9ec0a8f` (Calliope, my three amendments folded at `0da4852`, OWNER
GRILL PASSED WITH FOLDS by Vesper at `9ec0a8f`). Sources govern the brief;
the brief governs this rig.

## Venue and lane

- **Venue: rocketfish** (Arch Linux). Workflow type **`fleet_dev_rf`**
  (deployed type verified by run history at the engine; queues `dev_rf` /
  `counsel_rf` / ops, served by the workers Vesper relaunched 2026-08-23
  08:33:42Z under the CURED PATH — `~/.cargo/bin` first, exported by the
  fixed babysitter so resurrections inherit it).
- **Workspace root:** `/home/rocketfish/fleet/runs`.
- **Repo:** `https://github.com/ablative-io/aion.git`.
- **BASE (pinned): `5e2fb5a3ed32b53d48dbc585aa914acd056458c6`** —
  post-seqfix-relanding main, ls-remote verified; Vesper's gate on the landed
  tree: check 0, clippy -D warnings 0, nextest 5342/5342.
- **Landing lane: Vesper (owner).** Landing re-gate at Waffles' desk first.
- Single leg `build` — the surface lands whole (no-shims wall); leg
  boundaries inside one crate family would manufacture merge conflicts.

## Toolchain ruling (the grill's staging note, binding)

The venue ran 1.94.0 against a 1.97.1 pin SILENTLY until 2026-08-23 (system
cargo ignores `rust-toolchain.toml`; the defect was a dropped `~/.cargo/bin`
in the babysitter's PATH export, fixed at source). The gate therefore:

1. requires rustc, cargo, AND rustup on PATH (`toolchain_absent`);
2. requires the repo pin present (`toolchain_unpinned`) and the active
   version equal to it (`toolchain_mismatch`);
3. asserts the SOURCE: `rustup show active-toolchain` must name
   `rust-toolchain.toml` as the override origin — a version that matches by
   coincidence is `pin_unhonored`, red. Today the discrimination is free on
   this venue: 1.97.1-via-pin vs 1.98.0-via-stable-default are different
   numbers.

⛔ NO fmt gate (Tom's ruling, docs `2ab9689`): format is autoformat
discipline before commit, never a gate leg.

## Gate

`gate-entry.sh` (this directory; carried in the base branch at
`.fleet/gate-entry.sh`). Order: toolchain → tree receipt → paths census →
clippy → new-test control → nextest (zero-state first). Wall:

```
^(crates/aion-awl|apps/aion-ops-console/src/features/authoring/assets/awl-guide\.json|Cargo\.toml|Cargo\.lock|\.fleet/)
```

`crates/aion-awl` is a prefix by design: it admits `aion-awl`,
`aion-awl-lsp` (R7 LSP rows), `aion-awl-package`, and the
`aion-awl-command` sibling-crate fallback the brief names as revisable.
Positive control: at least one changed file under `crates/aion-awl/` or the
census red-fires `deliverable_absent`.

## Fixture specimens (my amendment 3, repo-qualified per the grill fold)

The four primary rigs' gate scripts, carried into the flight tree at
`.fleet/fixtures/gate-scripts/` as pinned bytes — R6's fixtures cite these,
never remembered text:

| specimen file | source repo | source path | taken at | sha256 |
|---|---|---|---|---|
| `haem-wall.gate-entry.sh` | haematite | `.fleet/gate-entry.sh` | `87de93b6` | `1e1e3ad2d189b5b2c8fb07b6e302e5cc3505ac7fa7f84ac5561020e30ff10a1e` |
| `haem-wall-r2.gate-entry.sh` | haematite | `.fleet/gate-entry.sh` | `125cbaac` | `ae66bb0b6819f36f46515017640b8bd6eea3e5009add725a3d70ab4209256832` |
| `seqlegs-impl.gate-entry.sh` | aion | `.fleet/gate-entry.sh` | `957de0a1` | `ef5b82a83eaead5a2ac42e6a026df99bb6bc0acdb9f2917ab4c787283c6f1804` |
| `speech-1.KIT-gate-entry.sh` | docs | `dispatch-rigs/speech-1-python-wire/KIT-gate-entry.sh` | `e98ca99` | `e1544910aee3378d3fbab17856235f8bb1bd7a7b6ca0dec54b497a2babaa7a5a` |

## Probe battery

`probe-arms.sh` — 12 arms, red-first, on rocketfish, against the real repo at
BASE, under the cured worker PATH. Two shadowings recorded honestly:
`toolchain_mismatch` is probed via the venue's own historical defect shape
(system rustc ahead of rustup) because a working rustup makes any other
specimen unconstructible; `suite_empty` is shadowed-by-reality (the
workspace names thousands of tests) — the check stays, its red arm is
unreachable without gutting the repo.

## Carrier laws (folded into every prompt in the INPUT)

- Report = a COMMITTED FILE at `.fleet/reports/build.md`; payload copy is a
  courtesy. Finish with a CLEAN tree — the gate attests the working tree.
- Judge's FINAL message = EXACTLY the verdict token (`judge_pass` /
  findings then flag); a judge that dies mid-duty is recorded identically to
  one that objected — never die mid-duty.
- Spec-vs-implementation discrepancies are reported WITH BYTES, never
  patched silently; any spawned process is killed only by its own recorded
  pid.

## Acceptance path (after terminal green)

1. Waffles: landing re-gate at my desk on the pushed result branch.
2. Vesper: owner review + landing (her gate at landing, per her grill
   signature); item-5 placement pricing decision is hers at landing.
3. Calliope: conformance read of the landed surface against her design brief
   (fixture provenance + refusal vocabulary + teaching rows).
