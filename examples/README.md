# Aion examples

Every directory here is a complete, runnable example — an AWL document, a
Gleam workflow package, a worker, or a whole workflow family. Each one exists
to prove a specific capability against a real server, and most carry their own
README with the exact commands. The first four rows are the shortest paths in:
start with `hello-world`, then take the AWL runway with `awl-hello`, ask the
`assistant` to explain the rest, and see a workerless declared command with
`shell-run`. The remainder are listed alphabetically.

| Example | What it demonstrates |
|---|---|
| [hello-world](hello-world/) | From a fresh checkout to a running Aion workflow: build, deploy, start, signal, and inspect your first workflow end to end. |
| [awl-hello](awl-hello/) | The smallest complete AWL-authored workflow — the spec's worked example 1, verbatim, as the end-to-end AWL runway. |
| [assistant](assistant/) | An aion-authoring assistant that runs *as* an Aion workflow: a Norn agent in a durable, operator-paced chat session that answers from the real repo. |
| [shell-run](shell-run/) | The first AWL action that runs with no Rust worker: a declared `run` body the server executes itself, plus an `env_probe` showing what the command cannot see. |
| [agent-dev](agent-dev/) | A durable agent dev pipeline whose agent steps run through the norn harness — observable and intervenable. |
| [agent-orchestration](agent-orchestration/) | Aion as a durable orchestrator for AI-agent workflows: the dev-review loop as a typed Gleam workflow with durable event history. |
| [approval-gate](approval-gate/) | An approval gate written with the Gleam `aion_flow` SDK: the workflow waits durably for an operator's approve/reject signal. |
| [batch-orchestrator](batch-orchestrator/) | Parent-child orchestration: one child workflow per work item, a live `batch_progress` query, and a final per-item summary. |
| [cargo-gates](cargo-gates/) | An AWL-authored workflow that fans `cargo check`, Clippy, tests, and the formatter check out over one Rust workspace. |
| [data-pipeline](data-pipeline/) | A fan-out/fan-in workflow written with the typed Gleam `aion_flow` SDK and served by a Python activity worker. |
| [dev-brief](dev-brief/) | A brief goes in; an adversarially-reviewed branch comes out — driven Norn agents plus mechanical shell gates. |
| [dev-pipeline](dev-pipeline/) | The dev-pipeline workflow package: `brief_forge` (finding to dispatchable brief) and `implement_and_gate` (brief to reviewed-ready, gate-green diff). |
| [general-worker](general-worker/) | A standalone Rust worker (`GPW-1`) serving one driven Norn agent, arbitrary process execution, and deterministic text extraction on one queue. |
| [incident-triage](incident-triage/) | The prospekt → Aion bridge: a prospekt-validated structured document drives a workflow as typed input, and typed structured output comes back. |
| [invm-demo](invm-demo/) | The smallest proof of the in-VM activity tier: one workflow, one activity, no worker at all. |
| [norn-fan-worker](norn-fan-worker/) | A real-AI liminal push fan-out worker: Norn agent steps fan out across the cluster over server-push transport and survive an owner `kill -9`. |
| [order-fulfillment](order-fulfillment/) | The flagship end-to-end example: one realistic order-processing saga exercising the engine's durable-workflow machinery together. |
| [order-saga](order-saga/) | The durable saga pattern: ordered steps with compensating actions that undo already-completed work when a later step fails. |
| [pipeline-run](pipeline-run/) | A parent-child workflow family: a dev-cycle brief becomes a landed stack, one `pipeline_unit` child per unit, layered by dependency and landed stratum by stratum. |
| [plan-fanout](plan-fanout/) | A workflow whose degree of parallelism is decided at runtime by a planner agent — the plan is validated, layered, and executed by code. |
| [python-worker](python-worker/) | A single-file Python worker measuring real image facts; the counterpart to `shell-run`, with the same-shaped action served by a worker instead of the server. |
| [remediation](remediation/) | The yggdrasil finding-to-fix remediation pipeline as an Aion workflow family. |
| [remediation-packet](remediation-packet/) | A packet of lane briefs as one durable run: every lane lands on the base branch or is honestly reported unlandable. |
| [remote-gates](remote-gates/) | An AWL-authored workflow that clones a repository and branch onto a machine that is not yours and runs the whole Rust gate battery there. |
| [retry-policy](retry-policy/) | Minimal workflow exercising the engine-honored per-activity retry policy. |
| [stacked-dev](stacked-dev/) | The stacked-dev workflow family: a brief goes from nothing to landed on main, with build-cache warming concurrent with planning. |
| [stacked-dev-remote](stacked-dev-remote/) | The stacked-dev family provisioning remote clone workspaces: stable per-run directories from a `clone_url`, safe across server, worker, and host restarts. |
| [staged-rounds](staged-rounds/) | A planned, phased, gated fan-out as one AWL workflow: supplied material goes in; merged, gated, panel-reviewed work comes out. |
| [subscription](subscription/) | A long-lived subscription lifecycle: durable billing timers, typed plan-change signals, and continue-as-new history rotation. |
| [yg-fix](yg-fix/) | The fix half of a diagnosis → fix loop: consumes a confirmed-findings report from an adversarial diagnosis run and drives the fixes. |
