# #228 live acceptance — the working directory is a property of the RUN

Discharges the acceptance owed by commit `2066b3183` ("cwd from <parameter>").
A green test is not this proof; what follows is a live agent round, measured on
both sides of the seam.

## The world it ran in

Not Tom's server. Swaps and restarts of the production server sit on his word,
so this was proven in an isolated world with no blast radius, built for the
purpose and torn down after.

| | |
|---|---|
| binary | `target/release/aion`, SHA-256 `3b8e20ffadfbb3423717ba325a6eb34e7d5c3330bd154242d296d52e1056eeec` |
| server banner | `build 0.14.1+2066b3183c36392305657e955724e03bc928dc58` |
| home | `/Users/tom/.aion-228-accept` (removed at teardown) |
| ports | HTTP 18080 · gRPC 18090 · liminal 50161 |
| server pid | 80352 |
| worker pid | **95196 — one process for every run below** |
| worker launch dir | `/Users/tom/.aion-228-accept`, which holds **no `ANCHOR.txt`** |

The installed binary reports the same version string, `aion 0.14.1`, from
different bytes (SHA-256 `647dd2e4…`). Identity here is by hash, never by
version string.

## The fixture

Two trees, each holding an `ANCHOR.txt` whose contents exist nowhere else:

- `tree-alpha/ANCHOR.txt` → `ANCHOR-ALPHA-9f2c17`
- `tree-beta/ANCHOR.txt` → `ANCHOR-BETA-4d81ae`

`accept228.awl` declares `cwd from repo`, and `ask(repo: String, prompt: String)
-> Reply` carries the marker. The prompt names `ANCHOR.txt` by a **relative**
path and asks for its first line verbatim, nothing else.

The worker is deliberately launched from a directory holding no anchor. That is
the subtraction control: a harness that took its directory from the launch could
not answer either run correctly — it could only answer both wrongly.

## What was measured

| run | repo | expect | reply | outcome |
|---|---|---|---|---|
| `a38554f6` | `tree-alpha` | `ANCHOR-ALPHA-9f2c17` | `ANCHOR-ALPHA-9f2c17` | `matched` |
| `691cbb3d` | `tree-beta` | `ANCHOR-BETA-4d81ae` | `ANCHOR-BETA-4d81ae` | `matched` |
| control | `tree-beta` | `ANCHOR-ALPHA-9f2c17` | `ANCHOR-BETA-4d81ae` | **`mismatch`** |

One worker process, two directories, two different correct answers. A
launch-fixed directory cannot produce that pair.

The third row is the vacuity control. Asked to match alpha's anchor while
pointed at beta's tree, the run went **red** and returned beta's anchor: the
oracle can fail, and the answer tracks the tree rather than the expectation.

## Both sides of the seam

**Engine side** — the recorded dispatch input (`history-alpha.json`):

```json
"input": { "repo": "/Users/tom/.aion-228-accept/tree-alpha",
           "prompt": "Read the file ANCHOR.txt in your current working directory and reply with its first line exactly. …" }
```

An AWL activity input is a JSON **object keyed by parameter name**, even where
one parameter would do.

**Agent side** — what norn actually received (`norn-session-*.agent-side.jsonl`):

```json
{"type":"UserMessage","content":"Read the file ANCHOR.txt in your current working directory and reply with its first line exactly. …"}
```

The prompt **text**, alone. Not the `{"prompt":"…"}` envelope, not the object.
That is the pre-existing defect this task closed, measured at the boundary where
it did its damage — under the old `prompt_from_spec`, every AWL-authored agent
received the serialized JSON as its instructions.

**Directory, at the agent's own hand.** The agent was told only "your current
working directory". Its `read` tool call resolved the relative name against the
OS-supplied cwd:

- session `8bbd3c34` → `/Users/tom/.aion-228-accept/tree-alpha/ANCHOR.txt`
- session `9a882de8` → `/Users/tom/.aion-228-accept/tree-beta/ANCHOR.txt`

Two absolute paths, one worker process, neither one the launch directory.

## What this does not prove

The runs exercise the norn harness. The ACP adapter shares the
`HarnessWorkspace::PerRun` seam but was not driven live here.

The `mismatch` control surfaced task **#151** verbatim — `workflow 3 returned
error`, an internal counter naming nothing an operator can act on, while the
same response carried the workflow and run ids. That defect is unrelated to this
lane and remains open.

## Teardown

Server pid 80352 killed by recorded pid; `/Users/tom/.aion-228-accept` removed.
Tom's pids 8411 and 50822 were never touched and remain on the installed binary.
