# Issue-investigation dispatch rig

The reusable skeleton for composing an `investigate` run input. Copy
`skeleton.input.json`, fill every `<<PLACEHOLDER>>`, validate, dispatch.

**Operator handbook** (the laws): `onboarding/cally-ray-issue-investigations.md`.
This directory is the *artifact* those laws describe; when the two disagree the
handbook wins and this file is the bug.

## ⛔ Why this lives here and not in a run directory

A file under `/home/aion/fleet/runs/<id>/` is a **dispatch record** — the record
of what a live run was *actually started with*. **Never edit one.** Mutating a
run's input destroys the evidence of what that run was, and a run whose input
cannot be recovered cannot be audited, re-derived, or defended.

⭐ The failure this prevents is subtle and was made for real on 2026-08-20: a
file *used as* a template gets reasoned about *as* a template. **Its role in the
instruction ("copy from this") silently overwrites its nature (an immutable
record).** Ask what an artefact **is** before proposing to change it.

## Which fields are skeleton, which are per-issue

| field | kind | note |
|---|---|---|
| `node_label`, `repo_url`, `workspace_root` | **skeleton** | `workspace_root` must match the harness's static value — do not change it |
| `control_prompt` | **skeleton** | hardened no-preamble wording; see below |
| `control_expect` | ⚠️ **neither** | a value to **re-derive**, never to copy — see law 4 |
| `issue`, `threshold` | per-issue | `threshold` is the *truthful* trigger |
| `investigate_prompt` | **both** | step 0, HARD RULES and DELIVERABLE are skeleton; the numbered asks 1–3 and the issue body are per-issue |

## ⚠️ `control_expect` is re-derived, never copied

The control asks the agent to quote a known literal from its clone. **That
literal lives in a file that can change.** A skeleton carrying a copied
`control_expect` goes stale silently the moment someone edits the target line,
and every operator who copies it inherits a control that fails for a reason
having nothing to do with the agent.

⇒ **At the dispatch minute** (not the compose minute — they drift), read the
literal from `origin/main`, not from a local checkout:

```sh
gh api repos/ablative-io/aion/contents/workflows/investigate/investigate_worker.awl \
  --jq .content | base64 -d | sed -n '22p'
```

Set `control_expect` to exactly what that prints. If it no longer matches what
the `control_prompt` asks for, fix the prompt — do not weaken the control.

## The control prompt earns its wording

Preamble is the failure mode: an agent that answers `I'll read that file — worker
investigate_ops` fails a strict exact-match control while having done the work
correctly. The skeleton's wording forbids narration across **every** message the
agent emits, not just the last one. **Do not soften it**, and do not copy a
control prompt out of an older run dir — the hardening was added on 2026-08-20
after a flight-2 control failure and older records predate it.

## Compose checklist

1. `gh issue view <n> -R ablative-io/aion --json title,body`.
2. Fill placeholders. **Paste the issue body byte-for-byte** — do not retype or
   reflow it.
3. Re-derive `control_expect` (above) at the dispatch minute.
4. `jq -e . input.json` — and prove the body is verbatim rather than asserting
   it: extract it back out from between the markers and `diff` against
   `gh issue view`.
5. Enumerate what is **actually running** before claiming a slot — never count
   from memory. ⚠️ `list_runs` over MCP is unusable from the Claude Code client
   (decode failure even with `namespace` alone); enumerate on the VM instead:
   `aion --endpoint http://192.168.50.40:50051 list --status running --namespace default`
6. `scp` to the VM and start per the handbook's law 2 (`--input-file`; never an
   object input through MCP `start_run`).

## ⚠️ A repro ask must not require what the fence denies

Name the **minimal** workload that exercises the mechanism, and forbid heavier
shapes until it fails. A defect in the **engine** — dispatch, correlation,
scheduling, retry — reproduces with trivial no-op activities and no agent legs.

⭐ Asking for a *realistic* workload is the trap: it sends the agent after
runners, credentials and tokens that the loopback fence rightly denies. **The
task then contradicts the fence, and the agent must break one of them** — either
it breaches the fence or it fails the task, and neither outcome reads as the
composer's error. Require the agent to state attempts `N` per shape before
escalating, so "it didn't fire" arrives with its denominator attached.
