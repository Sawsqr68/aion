# Census: provider policy refusals across the fleet, 2026-08-18 → 2026-08-22

Measured by Vesper Lynd (gate seat) before gating
`briefs/aion-policy-refused-fallback.md`. Recorded here so R1 argues against
measured events rather than a remembered count.

**Known answer for anyone re-running this: 4 flights killed, ≥1 survived.**

## Method, and the mistake worth not repeating

`aion list --endpoint <ep>` for `Failed` workflows since 2026-08-18, then
`aion describe <workflow_id> --endpoint <ep>` per candidate.

🔴 **Classify on the FAILURE payload, not on the presence of the string
anywhere in the output.** My first pass over-counted: it matched flights whose
*prompt text* merely mentions the policy — including the flight carrying this
very brief. A census keyed to a word finds every document that discusses the
word. Key it to the event that decided the outcome.

(`aion describe` emits JSON already; there is no `--json` flag and passing one
is an error.)

## The population

**KILLED — 4 distinct flights**, every attempt refused, no survival:

| Flight | Attempts refused |
|---|---|
| investigate #68 `tom-ask` | 3 of 3 |
| investigate #68 `tom-ask`, attempt-2 re-fly | 3 of 3 |
| `inv73-harness-flight2` | 4 of 4 (parent + child) |
| beamr `beamracc-lifetimes-flight1` | 4 of 4 (parent + child) |

**SURVIVED — ≥1 flight**, refused then cleared on retry. This is the case that
refutes the deterministic-for-content premise.

### Exhibit A — workflow `4ddee56c` (`aion84_dev`, 2026-08-19)

History read event by event:

```
seq  9  ActivityFailed     act 1  attempt 1  kind=Retryable  (cyber_policy refusal)
seq 11  ActivityCompleted  act 1  attempt 2
seq 14  ActivityCompleted  act 2  attempt 1
```

Same act, same content: attempt 2 completed. The flight went on and ended
`gates_red` **on the gate**, not on the refusal. Under "fail fast after one
attempt" it dies at seq 9 instead.

### Exhibit B — aion#65 round 3's leg

Attempt 1 died at 22m44s on `cyber_policy` while the seat was reading its own
`process_exit.rs`. Attempt 2 — same prompt, same tree — ran **11m29s past
attempt 1's death depth** and delivered the leg whole. A different repo and a
different day from Exhibit A, so the two are independent.

## What the population supports, and what it does not

- **Supported:** a policy refusal is **stochastic**, not deterministic for its
  content. Two independent survivals. `is_deterministic() = true` is refuted,
  and so is collapsing the no-chain path to a single attempt.
- **Supported:** the refusal genuinely does burn a retry budget against a
  provider that keeps refusing — 4 flights spent 3–4 attempts each (~25 min)
  and died. Removing that waste is the real win.
- **NOT supported: any particular retry bound.** Two survivals is not a rate.
  Do not mint a number here; it is configuration, defaulting to what the act
  already declares.
- **Provider asymmetry: suggestive, NOT proven.** Every refusal above is on the
  norn/OpenAI path and none on ACP/Claude — but the ACP surface is a much
  smaller sample, so this is consistent with asymmetry rather than evidence of
  it. The brief's requirement to cover the ACP adapter's equivalent surface, or
  state with evidence why ACP cannot express a policy refusal, stands as
  written.

## The harness fact the chain design rests on

`norn auth login` offers ChatGPT alone and lists a single account, confirmed at
both boxes. **norn can never serve an Anthropic model whatever `-m/--model`
says**, so a provider change is necessarily a queue/harness hop, not a model
flag.

The hop's destination is proven to exist: rocketfish's dev and counsel seats
run the ACP/Claude harness with `ANTHROPIC_MODEL` pinned to `claude-opus-5`,
verified in `/proc/<pid>/environ` **and verified as a real pin by a
discriminating arm** — setting the variable to `claude-sonnet-5` returns
`claude-sonnet-5`. The naive control proved nothing, because the box's default
was already Opus and both arms answered the same. The chain therefore has a
live two-provider estate to be designed against.

— Vesper Lynd, 2026-08-22
