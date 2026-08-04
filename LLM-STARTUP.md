# Aion startup guide — for the AI assistant driving the setup

You are an AI assistant helping a person get **Aion** running on their
computer, with the ops console and the built-in authoring assistant, so they
can see and use everything. This document is addressed to you. Follow it in
order, verify each step's output before moving to the next, and never claim
a step succeeded without seeing its evidence.

**What Aion is, in one paragraph, so you can explain it:** a durable
workflow engine. Workflows written for it survive crashes, reboots, and
months of sleep, and resume exactly where they were — the engine records
every step and replays history instead of re-doing work. The person you are
helping will get: a running server, a web console (authoring, live runs,
history), and an AI authoring assistant they can talk to inside that
console.

## Ground rules for you

- **Verify, then proceed.** Every step below says what success looks like.
  If you did not see it, the step did not succeed — diagnose before moving
  on, and tell the person plainly what you see.
- **Never restart the server as a reflex.** If something looks wrong, read
  the logs first (`~/.aion/server.log`, `~/.aion/assistant-worker.log`).
  A durable engine's server holds live state; restarts are a deliberate
  act, not a troubleshooting twitch.
- **Do not guess syntax or flags.** Everything you need is in this file and
  the documents it links. If you are unsure, read the linked document.

## Step 0 — prerequisites (check all three before anything else)

Run each check; all three must pass.

| Requirement | Check | Success looks like |
|---|---|---|
| Rust toolchain | `cargo --version` | a version line, e.g. `cargo 1.8x…` |
| git + access to this repository | `git ls-remote https://github.com/ablative-io/aion.git HEAD` | one line: a commit hash and `HEAD` |
| norn (the assistant's agent runtime), logged in | `norn --version` | a version line |

Fixes when a check fails:

- **No cargo:** install Rust from <https://rustup.rs>, then restart the
  shell and re-check.
- **git ls-remote fails:** the repository is **private**. The person needs
  GitHub access to it (their GitHub account added to the repo, and git
  authenticated on this machine — `gh auth login` is the easiest path if
  the GitHub CLI is present). Do not proceed until this passes; every later
  step needs the clone.
- **No norn:** the assistant thinks with norn. The person needs it
  installed and signed in. If they don't have it, everything else in this
  guide still works — server, console, authoring — but the assistant will
  not answer. Say that honestly and continue with the rest if they want.

## Step 1 — clone and run the setup script

```sh
git clone --depth 1 https://github.com/ablative-io/aion.git
cd aion
bash scripts/setup.sh
```

The script is safe to re-run: every step detects work already done and
skips it. What it does, in order, announcing each step:

1. Checks prerequisites (same three as above).
2. Installs the `aion` CLI if missing (`cargo install aion-cli`). **A cold
   install compiles for ~25 minutes.** Warn the person, let it run. If
   `aion` is missing afterwards, `~/.cargo/bin` is not on PATH — add it.
3. Prepares `~/.aion` (the server's home) with owner-only permissions.
4. Starts the server and waits until it actually answers.
5. Deploys the assistant workflow.
6. Builds and starts the assistant worker, and waits for it to register.

**Success looks like** a final block printing:

```
==> Done
    Console:         http://127.0.0.1:8080
    Server build:    {"version":"0.11.0", …}
    …
```

(The port comes from the server's own startup banner — trust the printed
address, not this example.)

If the script fails, it says why and names the log to read. The two logs
that matter: `~/.aion/server.log` (server) and
`~/.aion/assistant-worker.log` (worker). Read the tail, fix the named
problem, re-run the script.

## Step 2 — verify for yourself, then hand over

Before telling the person it's ready, check both of these yourself:

```sh
curl -s http://127.0.0.1:8080/build     # → one JSON line with "version"
```

and in `~/.aion/assistant-worker.log`, a recent line containing
`connected and registered; serving dispatches`.

Then have the person open the printed console address in their browser.
They should see the Aion ops console: workflows, runs, and the authoring
surface.

## Step 3 — their first assistant session

In the console, pick the **assistant** workflow and start a run. It takes
exactly two inputs:

- **objective** — what they want, in their own words (must not be blank).
- **repo_path** — a git repository for the assistant to work against:
  a local path or clone URL. Empty is fine — that means a scratch
  workspace.

The session is a conversation: each message they send continues the same
assistant, with full memory, for as long as they keep it open — days or
weeks; the session survives server restarts. Ending it is explicit, in the
console. There are no round limits.

## If something is wrong — where to look

| Symptom | First look |
|---|---|
| `aion: command not found` after install | `~/.cargo/bin` missing from PATH |
| script fails at "Waiting for the server" | tail `~/.aion/server.log` — the last error line names the cause |
| worker "did not register" | `norn` missing or not logged in; tail `~/.aion/assistant-worker.log` |
| console loads but assistant never answers | worker log again — is the worker running? (`pgrep -fl agent-dev-worker`) |
| clone fails | repository access (Step 0, second check) |

## Going deeper (read these, in this order, when asked)

- [`GETTING-STARTED.md`](GETTING-STARTED.md) → `docs/GETTING-STARTED.md` —
  from nothing to a durable workflow, end to end, including the
  kill-the-server-watch-it-survive proof.
- [`docs/authoring/AWL.md`](docs/authoring/AWL.md) — AWL, the workflow
  language: every verb and the authoring loop.
- [`examples/assistant/resources/`](examples/assistant/resources/) — the
  assistant's own operating manual: environment, authoring method,
  commands, language reference, workers. What the assistant knows, you can
  read too.
- [`docs/workers/AGENT-WORKERS.md`](docs/workers/AGENT-WORKERS.md) — how
  agent-backed workers (like the assistant's) are wired.
