# Brief: wire the ACP harness into the `aion` binary

Task #29's remaining half. The ACP adapter crate exists and is tested, but **no
operator can serve it**, so "ACP worker shipped out of the box" is not yet true.

## The gap, stated precisely

`aion worker agent` (landed `945873a7`) serves *the* compiled-in harness, singular:

- `crate::harness::default_dyn_agent_harness()` hard-returns `Arc::new(NornHarness::new())`
  under `#[cfg(all(feature = "norn", feature = "liminal-transport"))]`.
- `crate::harness::compiled_harness_capabilities()` returns
  `aion_integration_norn::registration_capabilities()` under `#[cfg(feature = "norn")]`
  and the empty set otherwise.
- `aion-integration-acp` has **no `registration_capabilities()` analogue at all**, so
  even if it were composed there would be nothing to advertise from.

Harness identity is therefore a compile-time constant with exactly one possible
value. Adding a second harness is not a matter of another `cfg` arm — it needs
*selection*, and ACP needs *configuration*, because an ACP agent is a subprocess
whose launch command is not inferable.

## R1 — `aion_integration_acp::registration_capabilities()`

Publish `{Cancel}` — matching the adapter's own ruling that `Cancel` is the only
primitive ACP v1 can actually deliver.

Mirror `aion-integration-norn`'s treatment exactly, because the reasoning is
identical and already written down there: the published set is a fact about the
**adapter's translation ceiling**, not a prediction about a binary; a
registration-time set and a per-session handshake set can disagree in both
directions; and both disagreements surface as an attempt-scoped
`CapabilityNotSupported` naming the primitive rather than silently.

Port the bidirectional pin too — the norn test drives **all five** neutral
primitives and asserts published-set ⇔ translatable-set, so adding one to either
side alone fails. The ACP crate needs its own copy of that test, not a comment
promising the same thing.

## R2 — harness selection, derived not defaulted

Add `--harness <name>` to `aion worker agent`, resolving over the harnesses
**compiled into this binary**:

- Exactly one compiled in → derive it. Nothing to choose is unambiguous
  derivation, not an assumed default.
- Several compiled in, none selected → refuse, naming every choice.
- A name this build does not carry → refuse, naming what it does carry.

This is deliberately the same shape as `worker_surface::select_worker`'s queue
resolution, whose reasoning is already justified in that module. Reuse the
argument; do not invent a second one.

`compiled_harness_capabilities()` and `default_dyn_agent_harness()` both become
functions **of the resolved harness** rather than of `cfg` alone. Current callers
to update: `worker_agent/surface.rs` (the `--check` report), `worker_agent/serve.rs`
(`compose`), and `harness::announce_composed_harness`.

## R3 — ACP configuration, nothing invented

ACP launch commands are genuinely not standardised. Verified against primary
sources 2026-07-30 (and recorded in the crate README): Claude Code is
`npx @agentclientprotocol/claude-agent-acp` with no args of its own, Gemini CLI is
`npx @google/gemini-cli --acp`, OpenCode is `opencode acp`; others use `acp serve`,
`--acp=true`, `agent stdio`, or `exec --output-format acp-daemon`. Nothing may be
inferred from an agent's name.

So `AcpHarness::new(command, args, cwd, permission_policy)`'s four required
arguments each need a source, following `worker_agent/args.rs`'s existing contract
to the letter — flag wins over environment variable, blank environment value counts
as missing, and **every** missing setting is reported at once rather than one per
run:

| Flag | Required | Notes |
| --- | --- | --- |
| `--acp-command` | yes | the executable, verbatim |
| `--acp-arg` | no | repeatable, order preserved |
| `--acp-cwd` | yes | validated **absolute**; `session/new` requires it |
| `--acp-permission` | yes | `deny` \| `allow-once`; no default — see below |
| `--acp-env KEY=VALUE` | no | repeatable |

Two hard rules:

1. **`--acp-permission` has no default.** `PermissionPolicy` deliberately has no
   `Default` impl. A default here is an assumed default on a security decision: it
   either silently approves an agent's file writes and shell commands, or silently
   blocks work the operator meant to allow. The operator states it out loud.
2. **`--acp-cwd` is not defaulted to the worker's own directory.** That would
   silently scope an agent somewhere nobody chose.

An ACP flag supplied when the resolved harness is not ACP must be **refused**, not
ignored — a flag that silently does nothing is how an operator comes to believe a
policy is in force when it is not.

## R4 — the norn binary path gap, while the seam is open

`default_agent_harness()` resolves `norn` from `PATH`, so an operator whose `norn`
lives elsewhere cannot use the verb at all. The previous lane declined to add
`--norn-bin` because naming a concrete adapter inside a harness-neutral verb is
wrong — correctly. R2/R3 create the right shape for it: per-harness configuration
resolved after selection. Add the norn binary path there, as harness
configuration rather than as a special case in the verb.

## Acceptance

- `aion worker agent <doc> --harness acp --acp-command … --acp-cwd … --acp-permission deny --check`
  reports the surface, the resolved harness, and `{Cancel}` — without connecting.
- Omitting `--acp-permission` refuses and names it, alongside any other missing
  setting in the same message.
- A relative `--acp-cwd` refuses.
- `--acp-command` with `--harness norn` refuses.
- A build with both harnesses and no `--harness` refuses, naming both.
- A build with neither still derives and prints the action surface, then refuses to
  serve, naming the missing piece — the existing `unservable` behaviour must survive.
- Workspace `cargo clippy --all-targets -- -D warnings` and `cargo fmt` clean; no
  `#[allow]`/`#[expect]`/`#[ignore]`/`_var`/`#[cfg(any())]` anywhere.

## Live proof required

Registration + `--check` are provable without an agent binary. A **dispatched turn**
through ACP needs a real agent on the box; there was none when the adapter was
written, so its live path is entirely unexercised. State honestly which of the two
was proven — do not let `--check` passing stand in for a turn that never ran.
