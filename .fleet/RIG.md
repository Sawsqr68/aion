# AWL-CUT2 — dispatch rig (recipes / staging assembly implementation flight)

Staged by Waffles 2026-08-23. Brief: `briefs/awl-recipes-cut-implementation.md`
at docs `85f2748` (Calliope; my review CLEAN; OWNER GRILL PASSED WITH FOLDS by
Vesper — folds A/B/C in place, ratchet ruling RATIFIED). Sources govern the
brief; the brief governs this rig.

## Venue and lane

- **Venue: rocketfish** (Arch Linux). Workflow type **`fleet_dev_rf`**;
  queues `dev_rf` / `counsel_rf` / ops. NOTE: rf's liminal workers dial the
  LAN address and are healthy natively (the liminal-over-tailscale defect on
  Vesper's docket does not touch this venue).
- **Workspace root:** `/home/rocketfish/fleet/runs`.
- **Repo:** `https://github.com/ablative-io/aion.git`.
- **BASE (pinned, the staging duty named exactly):
  `1bab95477cb86d498d07bb8da445f00351b87972`** — aion main whose HEAD is
  cut-1's landing commit (`land: awl-cut1`, contains `fleet/awl-cut1-result`
  = `2262c8b8d`, base `5e2fb5a3e`), ls-remote verified at staging. This
  satisfies the brief's binding clause: the base CONTAINS cut-1's landed
  surface, which recipes consume.
- **Landing lane: Vesper (owner).** Landing re-gate at Waffles' desk first.
- Single leg `build` — the surface lands whole (no-shims wall), same
  reasoning as cut-1.

## First acts bound by the brief (the flight's, restated for the record)

1. **Conformance read** of the landed cut-1 surface against `9ec0a8f` before
   building — divergence = reportable finding + amendment, never silent.
2. **Has-this-already-happened census** over THIS cut's constructs, fresh,
   pathspec stated.

## Gate

`gate-entry.sh` (this directory; carried in the base branch at
`.fleet/gate-entry.sh`). Byte-derived from cut-1's LANDED gate (the
F-RIG-1-amended script that survived the flight and both re-gate passes),
changed in exactly three places: BASE → `1bab954…`, WALL gains
`crates/aion-cli` (executing verb per the ratified placement, fold B), and
the cut-1 amendment comment block replaced by the cut-2 wall rationale.
Wall:

```
^(crates/aion-awl|crates/aion-cli|apps/aion-ops-console/src/features/authoring/assets/awl-guide\.json|examples/assistant/resources/AWL-REFERENCE\.md|Cargo\.toml|Cargo\.lock|\.fleet/)
```

Twin surfaces admitted up front — cut-1's F-RIG-1 (wall omitted a
byte-pinned twin) is the lesson; the reference twin and the executing-verb
crate are in the wall from staging, not discovered mid-flight. Positive
control: at least one changed file under `crates/aion-awl/` or the census
red-fires `deliverable_absent`. Toolchain gate carried unchanged from cut-1
(pin-honored assertion incl. `pin_unhonored` source check). ⛔ NO fmt gate
(Tom's ruling, docs `2ab9689`).

## R7 corpus exhibits (the grill's staging duty: digest-pinned in the rig)

Carried into the flight tree at `.fleet/fixtures/corpus/` as pinned bytes;
BRIEF-R7's fixtures cite these, never remembered text. All taken from the
docs repo at `3c7cbb6`:

| exhibit | brief clause | source path (docs repo) | sha256 |
|---|---|---|---|
| `sleepreap.build-input.py` | R7(a) | `dispatch-rigs/sleepreap/build-input.py` | `032ee3d7598ac361a731a5f0917b47a6b1ea7c32c3cc3b9670b3c11c32db23b7` |
| `liminal22.build-input.py` | R7(a) | `dispatch-rigs/liminal22/build-input.py` | `b51f60f780e542dffbaa6879db2294ccd3fa050e8138b33e87e1ef3cd0d06351` |
| `skeleton.input.json` | R7(b) | `dispatch-rigs/issue-investigations/skeleton.input.json` | `4e90bc3bf2e3c2f74546c794cc4b87b183fcf60672065668720552edb2eb1218` |
| `issue-investigations.README.md` | R7(b) field-kind rows | `dispatch-rigs/issue-investigations/README.md` | `b1db6ae3dfa3d261a29923ed091d56657c3978fc2bf453d3a3f6ae12d973fc65` |
| `speech1.STAGING.md` | R7(c) | `dispatch-rigs/speech-1-python-wire/STAGING.md` | `1150e466b0c3264da1ac6ce93d9301287d89a492c60615b88b4b9b420f7e5f4a` |
| `haem-wall.INPUT-FINAL.json` | R7(d) allowlist | `dispatch-rigs/haem-wall/INPUT-FINAL.json` | `77f0333fcea03ada0c017af9fdea725231c3e2aecce8ade71781571daff9115d` |
| `speech1.KIT-input-template.json` | R7(e) sed-fill slots | `dispatch-rigs/speech-1-python-wire/KIT-input-template.json` | `1a95315217b431ae7d1956183da51098e02ddc671d3e7ab737d0593df429f5ed` |

Notes recorded at staging: the two `build-input.py` copies DIFFER (measured);
both are pinned and the fixture set's completeness claim must name both.
R7(d)'s allowlist is embedded in the haem-wall INPUT (fix/judge prompts) —
the exhibit is the carrying file. Cut-1's four gate-script specimens ALSO
travel (unchanged from cut-1's rig, at `.fleet/fixtures/gate-scripts/`) since
BRIEF-R5/R7 fixtures may cite them.

## Probe posture (recorded honestly)

The gate is byte-derived from cut-1's landed script with three named deltas.
Cut-1's 12-arm battery (all arms fired at staging 2026-08-23, plus the flight
and two re-gate passes on the same script family) stands as the evidence for
the unchanged arms — cited, not re-fired. Delta arms probed fresh at staging:
census admit/deny against the NEW wall (aion-cli admitted; an out-of-wall
path refused), BASE reachability on the venue clone. `suite_empty` remains
shadowed-by-reality as recorded in cut-1's rig.

## Carrier laws (folded into every prompt in the INPUT)

- Report = a COMMITTED FILE at `.fleet/reports/build.md`; finish with a
  CLEAN tree — the gate attests the working tree.
- Judge's FINAL message = EXACTLY the verdict token; never die mid-duty.
- Spec-vs-implementation discrepancies reported WITH BYTES, never patched
  silently; any spawned process killed only by its own recorded pid.
- BRIEF-Rn vs DESIGN-Rn disambiguation at every citation (fold C).
- Control-token replies: display artifacts (line numbers, prefixes) are NOT
  file content (hardening from speech2-r2's control flake, same day).

## Acceptance path (after terminal green)

1. Waffles: landing re-gate at my desk on the pushed result branch (run the
   flight's amended gate if it amends; verify any amendment against F-RIG-1
   discipline — exact delta named).
2. Vesper: owner review + landing (her gate at landing).
3. Calliope: conformance read of the landed surface against her brief.
4. The strike list becomes the CONTRACT OF THE CUTOVER — a staged act after
   landing, owned by staging (me), retire-in-the-same-motion-as-convert,
   never another lane's in-flight material.
