# Docs staleness audit — 2026-08-01 (for task #133, Tom's Benji order)

Scout report, banked verbatim-in-substance by the seat before compaction.
Drives the #133 docs lane. Source: read-only Explore scout, very thorough.

## 🔴 TWO BRIEF PREMISES THE SCOUT REFUTED (bank these)

1. **A signal SENDER EXISTS in-language now**: `send name: Type` decl +
   `send name(payload) to target` statement — parser/send.rs, checker/
   sends.rs, mir/lower/send.rs → RuntimeFn::SignalSend. Docs must NOT say
   "no in-language send" (my old memory + task #24 are STALE).
2. **query/answer surface exists**: `query name: Type` + `answer` statement
   (parser/query.rs, QueryDecl).

## Assistant teaching content (Tom's explicit "assistant stuff")

Lives at `examples/assistant/resources/` — 5 files, embedded via
`include_str!` in `examples/agent-dev/worker/src/handlers/skill_resources.rs`
(SKILL_RESOURCES line 9), materialized to `<workspace>/.assistant/resources/`
by the assistant_provision activity. **Editing content = editing those 5 md
files; embedding needs the agent-dev WORKER rebuilt** (NOT aion-server, NOT
console — a rebuild needs its own clearance/pricing). Opening prompt text
duplicated: LIVE in `agent-dev/worker/src/handlers/opening_prompt.rs`
(67–132), DEAD duplicate in `examples/assistant/src/assistant/prompts.gleam`
(drift source — delete or mark).

Per-file: ENVIRONMENT.md stale-minor (no ~/.aion/AION_HOME; ground-truth
list omits awl_scaffold). AWL-AUTHORING.md stale (no --timeout at :161; :77
"AWL worker bodies do not exist" CONTRADICTS declared commands).
COMMANDS.md stale (:95 no --timeout — most user-visible gap; no scaffold,
no `aion query`, no `aion worker agent`, no archived-source mention).
AWL-REFERENCE.md stale — largest language gap: missing send, query/answer,
declared `run "…"` commands, `agent` marker, `advisory` config (§4 :137
wrong key list vs parser/workers.rs:290). CURRENT in it: subflows, layers,
multi-step distribute, value route payloads, timeout, visits, collect.
WORKERS.md stale (:20-22 "only shell" FALSE — `aion worker agent` shipped
945873a7; :16-18 misleading). prompts.gleam dead duplicate.
**Dead-Gleam-dialect check: CLEAN — all five refuse Gleam authoring.**

## Repo docs (ranked top gaps for Benji)

1. `docs/GETTING-STARTED.md` (THE landing page, README:16 "Start here") is
   100% Gleam, 0 AWL — teaches the loop the assistant is ordered to refuse.
2. NO user-readable AWL reference/tutorial exists anywhere in docs/ — the
   only complete one is filed as an example asset, unlinked.
3. `aion run --timeout` documented NOWHERE.
4. `examples/awl-hello/README.md` :59-67 teaches the RETIRED emit→gleam
   build→package→start loop (flagship AWL example, actively misleading).
5. Assistant reference missing 4 shipped features (send/query/commands/
   agent+advisory).
6. WORKERS.md "only shell" falsehood.
7. `crates/aion-cli/README.md` (crates.io front page) lists NO awl, NO run.
8. ~/.aion / AION_HOME only in CHANGELOG; operations.md (config reference)
   never mentions them.
9. `docs/authoring.md`:2-3 + `docs/guides/aion-authoring-dx.md`:29 assert
   "Gleam is the single source of truth… there is no DSL" — direct
   contradiction of current truth.
10. `docs/DECLARED-COMMANDS.md` is CURRENT + safety-critical ("THERE IS NO
    SANDBOX") and linked from NOTHING.
11. `docs/packaging.md` missing awl/document/+awl/schema/ entries.
12. `crates/aion-cli/templates/` has zero .awl templates — `aion new`
    always scaffolds Gleam.

Also: root README tagline omits AWL ("for Gleam, Rust, and the BEAM"),
"Two first-class surfaces" framing stale, no --timeout/scaffold; root
GETTING-STARTED.md routes to Gleam first; CHANGELOG Unreleased missing
every landing since 0700-roots (--timeout, archived source, subflows,
layers, value payloads, substeps); `aion --help` list in docs missing
awl/run/worker/dev/new/inspect/pause/resume/reopen; editor READMEs predate
agent/advisory grammar words; RELEASE-0.9.2.md two releases behind (0.10.0
current); design papers mis-shelved in user docs dir.

Grammar sources of truth for the fix lane: `crates/aion-awl/src/lexer/
tokens.rs` (Keyword::ALL 57 words, positional words :43-71),
`grammar_vocabulary_tests.rs` (drift gate), `crates/aion-cli/src/main.rs`
(Run{file,input,timeout} :285-301), `crates/aion-cli/src/awl.rs` (Check/
Fmt/Emit/Scaffold/Schema/Lsp).

## Lane plan (after leg-2 lands; docs-only fences)

Fence: docs/**, README.md, GETTING-STARTED.md, CHANGELOG.md, crate READMEs,
examples/*/README.md, examples/assistant/resources/*.md, templates (new
.awl template = code, weigh separately). NO builds without clearance
(assistant embed rebuild = agent-dev worker build — price it with Athena).
Truth rule: verify each claim against the code cited above; a doc-unfixable
finding goes to Tom, not papered.
