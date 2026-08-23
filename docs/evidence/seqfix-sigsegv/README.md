# seqfix-flight1 — the SIGSEGV in `recovery_declared_body_e2e`

Read `CAUSE.md` first. Everything else here is one of its receipts.

| file | what it is |
|---|---|
| `CAUSE.md` | **the named cause, the branch taken (F3), and why the measurements forced it** |
| `RED-FIRST.md` | the reproduction O1 ordered, **with its true rate stated in both directions** |
| `RED-FIRST-ordered-command-prefix-tree.log` | raw transcript of `cargo test -p aion-server --test recovery_declared_body_e2e` on the pre-fix tree |
| `D1-armA-preserved-binary.txt` | arm A redone on the preserved crashing artifact — **20/20 SIGSEGV** |
| `D2-armB-base-957de0a1.txt` | arm B at base — **0/20** |
| `D3-build-mode-discrimination.txt` | the same source built two ways — **0/20 and 0/20** |
| `D4-split-942c80e3.txt` | the ordered split of the two linked perturbations — **0/20, 0/20, 0/20** |
| `D5-isolation-vs-load.txt` | load and `ulimit -n` are not the variable |
| `D6-faulting-frame.md` | **the backtrace.** beamr's JIT compile path; M12 retired as this crash's lead |
| `D7-build-draw-lottery.txt` | **the rung that decides the flight** — 20 relinks of one tree, 20 different binaries |
| `TEN-GREENS.log` | the ten consecutive greens O1 orders, and what they are worth |
| `O2-verification-runs.log` | every O2 row that needed a command |
| `V4-r7-emit-identity-recaptured.tsv` | R7's 424-document observable, re-captured live from this tree's `aion` binary |
| `GATE-RUN-1.log`, `GATE-RUN-2.log` | `.fleet/gate-entry.sh`, unedited, on the final tree |

The venue-level finding the next round should read before it starts:
`docs/evidence/venue/M13-beamr-jit-compile-sigsegv.md`.
