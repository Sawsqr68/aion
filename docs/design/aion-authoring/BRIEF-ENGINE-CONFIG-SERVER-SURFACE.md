# BRIEF — engine knobs an operator cannot reach

**Boarded by:** Vesper, 2026-08-06
**Origin:** Waffles' ruling on the #7 completion-retry bound
(`RULING-COMPLETION-RETRY-BOUND-2026-08-06.md`), point 4 — a follow-on lane,
explicitly **not** a rider on #7.
**Status:** BOARDED, awaiting Waffles' next dispatch round.

> "Neither this knob nor `SignalDeliveryConfig` being reachable from `aion
> server` means the shipped answer to every such question is the default —
> worth its own brief so it gets a config surface **deliberately rather than as
> a bolt-on**." — Waffles, 2026-08-06T10:25:45Z

---

## THE GAP, SIZED

Not "a config surface is missing". **Exactly three knobs**, measured at my own
hand against the tree, not recalled.

`EngineBuilder` exposes these runtime knobs. Call-site counts are
`grep -rn "\.<knob>(" crates/aion-server/src crates/aion-cli/src`:

| Builder knob | Call sites in the product | Operator-reachable? |
|---|---|---|
| `scheduler_threads` | 27 | ✅ **YES** — CLI flag → `crates/aion-cli/src/server.rs:34,144` → `RuntimeConfig::scheduler_threads` → `state.rs:1173` |
| `owned_shards` | 3 | ✅ YES — `config/runtime.rs` `owned_shards`, applied at `state.rs:1197` |
| `outbox_enabled` | 1 | ✅ YES — `config/runtime.rs` `outbox.enabled`, applied at `state.rs:1174` |
| `query_timeout` | (via `assembly.query_timeout`) | ✅ YES — `config/runtime.rs` `query_timeout` |
| **`signal_delivery`** | **0** | 🔴 **NO** |
| **`completion_retry`** | **0** | 🔴 **NO** |
| **`visibility_reconciliation_interval`** | **0** | 🔴 **NO** |

**The three zeros are the brief.** Everything else in this family already has a
config path, which is what makes the omission a gap rather than a design stance.

### Why "0 call sites" is the right measurement here

The claim is not "the knob is unset" — a default is a setting. The claim is
**the operator cannot change it**, and that is exactly what zero call sites in
`aion-server` + `aion-cli` establishes: no code path from a config file, an
environment variable, or a CLI flag reaches the setter. The knob is reachable
only by an **embedder** calling `EngineBuilder` directly. Someone running
`aion server` — which is the product — has no lever at all.

---

## WHY IT MATTERS

**The shipped default is the whole answer.** For a knob with a config path, a
bad default is an annoyance: the operator overrides it. For these three, a bad
default is the permanent behaviour of every deployment, and the only remedy is a
new binary.

That is precisely how the #7 ruling became necessary. `CompletionRetryConfig`
shipped at an 8 ms ceiling — roughly 125 attempts per second, indefinitely,
against a store already unwell, on the single-worker engine-task executor. Had a
config path existed, that would have been a tuning note. Because none existed, it
needed a ruling from Waffles and a code change to fix.

🔴 **The precedent already exists in the same file.** `scheduler_threads` runs
the full path — CLI flag → `NonZeroUsize` override → server `RuntimeConfig` →
`EngineBuilder`. This lane is not inventing a mechanism; it is **extending a
mechanism that is already built and proven** to three knobs that were skipped.

---

## SCOPE

### In scope

- **R1** — `signal_delivery` (`SignalDeliveryConfig`: `ready_timeout`,
  `max_enqueue_attempts`, `initial_backoff`, `max_backoff`) reachable from the
  server config file.
- **R2** — `completion_retry` (`CompletionRetryConfig`: `initial_backoff`,
  `max_backoff`) reachable from the server config file. **The ruled 30 s ceiling
  is the default; this makes it overridable, it does not re-open the ruling.**
- **R3** — `visibility_reconciliation_interval` reachable from the server config
  file.
- **R4** — Each new setting **validated at the boundary**, refusing rather than
  clamping. `EngineBuilder` already refuses a zero `initial_backoff` (it is a hot
  loop that can never leave zero: `0 * 2 == 0` and `0 > ceiling` is false, and
  the completion retry has no attempt budget to stop it). That refusal must be
  reachable from the config path too, with a message naming the setting.
- **R5** — A gate that **fails when a builder runtime knob has no config path**,
  so the next knob added cannot silently repeat this. This is the load-bearing
  requirement: without it, the fix is a snapshot and the gap regrows.

### Out of scope

- Re-opening the 30 s ceiling or the no-budget decision. Both are ruled; see the
  ruling document. Tom holds override on the 30 s via Waffles' digest.
- CLI flags for these three. The config file is the surface; flags are a
  separate ergonomics question. `scheduler_threads` having a flag is history,
  not a pattern to match.

---

## THE TRAP THIS BRIEF MUST NOT FALL INTO

🔴 **A settings list that lives in two places has already drifted.** The moment
there is a server-config struct field *and* a builder setter *and* a defaults
table *and* a documentation page, the rule "these are the engine knobs" is known
in four places with nothing forcing agreement.

**R5 is the cure, and it must be subtraction-shaped, not a checker bolted on
top.** The strong form: derive the config surface from the builder's knobs so
there is one place, and the gate becomes structurally unnecessary. The weak
form — a test enumerating knobs — is itself a fifth copy and will drift from the
thing it guards.

Whoever takes this lane should decide between those two **before** writing code,
and record the choice. If the weak form is chosen, say why the strong form was
rejected; do not let it be the default because it is easier.

---

## ACCEPTANCE

- All three knobs settable from the server config file, each **proven by a live
  server observing the set value** — not by a unit test asserting the field
  round-trips. A round-trip test cannot tell a wired setting from an unwired one.
- Boundary validation refuses an unclimbable ladder **through the config path**,
  with the offending setting named in the refusal.
- R5's gate demonstrated **failing** on a deliberately unwired knob before being
  accepted. A gate never seen red has not been shown to work.
- Full unfiltered `cargo test -p aion-rs` and `-p aion-server`, plus
  `cargo clippy --workspace --all-targets -- -D warnings`, all green **at the
  final tree**.

---

## FILES LIKELY TOUCHED

- `crates/aion-server/src/config/runtime.rs` — struct fields (`RuntimeConfig` at
  line 47)
- `crates/aion-server/src/config/sections.rs`, `defaults.rs`, `file.rs` — parsing
  and defaults
- `crates/aion-server/src/state.rs` — the `EngineBuilder::new()` assembly chain
  at line 1168
- `crates/aion/src/engine/builder.rs` — only if R5 takes the strong form
- Server configuration documentation — whatever the R5 decision leaves standing
