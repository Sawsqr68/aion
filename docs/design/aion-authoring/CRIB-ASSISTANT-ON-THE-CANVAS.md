# Crib sheet: the AWL assistant, on the canvas, right now

Everything below was checked against the running server at 13:2xZ on
2026-07-30. Nothing here needs a rebuild, a restart, or a command from you
before it works.

## What is running

| | |
|---|---|
| Server | pid 99844, `http://127.0.0.1:8080` |
| Worker | pid 35591, serving queue `default` |
| Console | the same address — open it in a browser |

Leave both alone. Restarting the server is what took the queue down last time.

## Open it on the canvas

1. Go to `http://127.0.0.1:8080`.
2. Open the authoring studio. Three documents are there:
   `assistant.awl`, `assistant_spike.awl`, `staged_rounds.awl`.
3. Open **`assistant.awl`**. That is the full session workflow, authored in
   AWL — not the spike, not a stub.

It checks clean: `ok: true`, `deploys_green: true`, zero diagnostics, two
steps. The canvas has no saved positions yet, so it lays itself out from the
document's own structure; drag it about and it will remember.

## What you are looking at

Two steps. The first provisions a workspace — it clones the aion repo and
composes the opening prompt. The second is the session: an agent round, then
it parks and waits for you, then the next round, until you end it or a budget
runs out.

The waiting is the point. It parks on one signal and holds there. A run of the
older version of this workflow sat parked for twenty-eight days and answered a
live query instantly when asked.

## Drive it by hand

Start a session — from the console's start form, or:

```
POST /workflows/start
{"namespace":"default","workflow_type":"assistant","task_queue":"default",
 "input":{"content_type":"application/json","bytes":[…UTF-8 bytes of…]}}
```

where the input JSON is:

```json
{"objective":"<what you want it to do>",
 "repo_path":"/Users/tom/Developer/ablative/stack/aion"}
```

Talk to it — this is the continue signal:

```json
{"message":"<your next message>","end":false}
```

End it cleanly:

```json
{"message":"","end":true}
```

**Both fields are required.** `{"end":true}` on its own will fail the round —
that is a real gap in the language today, it is written up, and it is being
fixed.

## What it did when I drove it

Two rounds. I asked it to name the file that defines AWL loop lowering. It
answered `crates/aion-awl/src/emitter/loops.rs`, correctly, and volunteered a
detail I had not asked for about counting loops. I then asked it — without
repeating anything — what file it had just named. It answered from memory.
Then I ended it, and it completed with `OperatorEnded`, `rounds: 2`, and the
last reply, every field exact.

Full record, with the recorded bytes: `ASSISTANT-AWL-LIVE-PROOF.md`.

## Two things it cannot do yet, so you are not surprised

- **You cannot ask a running session what it is doing.** The old Gleam version
  answered a status query; AWL has no query surface at all yet. That is being
  built right now.
- **The prompt goes over the wire wrapped in JSON** rather than as plain text.
  The agent copes, but it is wrong and it is ruled to be fixed.
