# BRIEF #133 — Benji-facing docs current (Tom's order, 2026-08-01)

**Owner:** docs lane under Vesper's seat. **Authority:** Tom's direct order
05:34Z — "make sure that all the documentation's up to date… I would really
love it if the assistant stuff was all up to date as well… committed and
pushed." Benji starts on AWL/Aion imminently; every stale doc is a trap laid
for him personally.

## Fences (hard)

- **Files you may touch:** `docs/**` (excluding `docs/design/**` and
  `docs/evidence/**` — seat records, historical), root `README.md`, root
  `GETTING-STARTED.md`, `CHANGELOG.md`, `crates/*/README.md`,
  `examples/*/README.md`, `examples/assistant/resources/*.md`, editor
  extension READMEs.
- **NO builds.** No cargo, no bun, no gleam. Doc edits only. The assistant
  worker rebuild (needed before the running assistant SHOWS new resource
  content) is priced separately with Athena — out of this lane.
- **NO code.** Two named exceptions require their own verification:
  1. `examples/assistant/src/assistant/prompts.gleam` (dead duplicate of
     `opening_prompt.rs:67-132`): grep-prove zero imports/references first;
     if truly unreferenced, delete; if referenced, it is not dead — STOP and
     report, do not edit it.
  2. `.awl` template under `crates/aion-cli/templates/` — EXCLUDED entirely;
     filed as its own task.
- Derived commit list (`git status --porcelain` at end), commit + push to
  main. No formatter passes over anything you didn't edit.

## Truth rule (binding)

Verify EVERY claim you write against the code before writing it. Sources of
truth: `crates/aion-awl/src/lexer/tokens.rs` (Keyword::ALL, 57 words;
positional words :43-71), `grammar_vocabulary_tests.rs`,
`crates/aion-cli/src/main.rs` (Run{file,input,timeout} :285-301),
`crates/aion-cli/src/awl.rs` (Check/Fmt/Emit/Scaffold/Schema/Lsp),
`parser/workers.rs:290` (advisory config keys), `parser/send.rs` +
`checker/sends.rs` (send), `parser/query.rs` (query/answer). A claim you
cannot verify does not go in a doc. A doc-unfixable finding (needs a product
decision) goes in your report, not papered over.

**Two refuted premises — do not reintroduce:** in-language `send` EXISTS
(`send name: Type` decl + `send name(payload) to target`); `query`/`answer`
EXISTS. Any doc saying "no in-language send" is wrong.

## Work items (ranked; from DOCS-AUDIT-2026-08-01.md — read it first)

1. `docs/GETTING-STARTED.md` — THE landing page is 100% Gleam, 0 AWL.
   Rewrite AWL-first (check → deploy → run one-motion), Gleam second.
2. Promote a user-readable AWL reference into `docs/` (the only complete one
   is `examples/assistant/resources/AWL-REFERENCE.md`, an example asset).
   Fix its gaps as item 5 first, then place/link a docs-tree copy or link.
3. `aion run --timeout` documented (currently NOWHERE).
4. `examples/awl-hello/README.md:59-67` — retire the emit→build→package→
   start loop; teach `aion deploy` / `aion run`.
5. Assistant resources (Tom's explicit ask), all five files in
   `examples/assistant/resources/`: add send, query/answer, declared `run`
   commands, `agent` marker, `advisory` config (correct key list vs
   parser/workers.rs:290); fix AWL-AUTHORING.md:77 "AWL worker bodies do not
   exist" (contradicts declared commands); add --timeout to COMMANDS.md:95
   plus scaffold, `aion query`, `aion worker agent`, archived source;
   ENVIRONMENT.md: ~/.aion + AION_HOME + awl_scaffold in ground-truth list.
6. `docs/WORKERS.md:16-22` — "only shell" is FALSE (`aion worker agent`
   shipped 945873a7).
7. `crates/aion-cli/README.md` (crates.io front page) — add awl verbs + run.
8. ~/.aion + AION_HOME into `docs/operations.md` (config reference).
9. `docs/authoring.md:2-3` + `docs/guides/aion-authoring-dx.md:29` — retire
   "Gleam is the single source of truth… there is no DSL" (false).
10. Link `docs/DECLARED-COMMANDS.md` (safety-critical, "THERE IS NO
    SANDBOX") from WORKERS.md, AWL docs, and the reference — currently
    orphaned.
11. `docs/packaging.md` — add `awl/document/` + `awl/schema/` archive
    entries (landed 678ab99c) and the provenance rider (source excluded
    from package identity).
12. Root README — tagline includes AWL; two-surfaces framing current;
    CHANGELOG Unreleased catches up (--timeout, archived source, subflows,
    layers, value payloads, substeps, `aion worker agent`, ~/.aion).
13. Editor extension READMEs — grammar words `agent`/`advisory` current.
14. `aion --help` verb lists in docs — add awl/run/worker/dev/new/inspect/
    pause/resume/reopen where enumerated.

## Report back

Per-item: what changed, what code you verified it against, anything
doc-unfixable (for Tom). Derived file list. Commit hash after push.
