# Live proof: an AWL action runs a real command, with no worker

2026-07-30, on `origin/main` at `a22369bd`. Recorded because the claim
"AWL is a working language" is only worth the run that shows it.

## What was wrong

An AWL action could declare its name, its parameters, and its result type —
and nothing about what it DID. `ActionDecl` carried no body. Every action was
therefore a requirement on a hand-written Rust crate, and a `.awl` document
alone could not do work. The command text a compiler did produce was dropped
at packaging time: `CompiledWorkflow.action_bodies` had zero consumers, so the
`.aion` archive carried only the action's name and JSON schemas.

## The document

`examples/shell-run/shell_run.awl`, in full:

```
workflow shell_run
  input text: String
  outcome stamped: type RunOutcome, route success

type RunOutcome { exit_code: Int, stdout: String, stderr: String }

worker shell_box
  action stamp(text: String) -> RunOutcome
    run "echo $text"

step stamp_it
  text |> stamp |> route stamped
```

No Rust crate. No worker process. No `.gleam` file.

## Run 1 — it works

```
$ aion run examples/shell-run/shell_run.awl --input '{"text":"AWL runs real commands now"}'
{"result":{"outcome":"stamped","payload":{"exit_code":0,"stderr":"","stdout":"AWL runs real commands now"}},
 "run_id":"3082a0d8-07f7-4607-bd73-da37d3cfcf8f",
 "workflow_id":"3e8ee1e0-50b3-4085-8680-3b97388225ee",
 "workflow_type":"shell_run"}
```

The server's own log for that run, confirming it executed the body rather than
routing to a worker:

```
"message":"executing declared action body at the server"  "activity_name":"stamp"
```

No worker was ever registered on the `shell_box` queue. Under the previous
behaviour this dispatch would have parked on `NoLivePollers` indefinitely.

## Run 2 — the security property, live

The same document, with a hostile value in the parameter:

```
$ aion run examples/shell-run/shell_run.awl \
    --input '{"text":"hello; rm -rf /tmp/PROOF_OF_INJECTION; echo PWNED"}'
{"result":{"outcome":"stamped","payload":{"exit_code":0,"stderr":"",
 "stdout":"hello; rm -rf /tmp/PROOF_OF_INJECTION; echo PWNED"}}, ...}

$ ls /tmp/PROOF_OF_INJECTION
ls: /tmp/PROOF_OF_INJECTION: No such file or directory
```

The whole value came back as one literal argument. `echo PWNED` never ran —
`stdout` is exactly the input, with no second line. `rm` never ran.

That is structural, not filtered. The declared command is parsed ONCE into an
argv whose elements are literal text or named parameter holes; a value is
substituted as one WHOLE argv element and never re-enters a parser. There is
no shell between the declaration and `execve`, so a value has nothing to
escape into. Nothing is blacklisted, because nothing needs to be.

## What the run exercised

| Piece | Commit |
|---|---|
| `run "…"` body through lexer, parser, checker, printer, compile | `a22369bd` |
| Body sealed into package identity (`.v5` contract domain) | `ad2fca44` |
| Server reads the deployed body and executes it, no worker | `69f5514e` |
| The executor itself (argv substitution, process-group containment) | `95426218` |
| Registration of a declared command as an activity | `6db1f6a0` |

## Gates at the landing tree

- `cargo nextest run --workspace --no-fail-fast` → `Summary [ 188.491s] 3157 tests run: 3157 passed (5 slow), 6 skipped`
- `cargo clippy --workspace --all-targets -- -D warnings` → clean, exit 0
- `cargo fmt --all` → applied
- BC-4 covered differential, including the new declared-body fixture:
  `50 succeeded, 29 failed-path, 0 cancelled, 3 timer-parked, 2 excluded, 0 refused, 0 DIVERGENCES, 0 INFRA`

## Three pins moved, each verified rather than blessed

1. **BC-4 covered ratchet 83 → 84** and its success inventory. Not accepted on
   the reasoning that "a body is a contract property, not a flow one" — the
   differential was RUN, and the declared-body fixture completes on both the
   AWL-direct and Gleam-reference backends with byte-identical normalized
   trails and zero divergences.
2. **`retry_three_every_1s.hex` contract golden.** Verified at the bytes: two
   record length prefixes moved `0x208→0x209` and `0x1eb→0x1ec`, with one
   trailing `00` — exactly one body discriminant per action record, nothing
   else.
3. **`FIXED_INPUT_ARCHIVE_SHA256`.** The archive embeds the canonical contract
   record and binds it into identity, so it moved with the archive FORMAT under
   the `.v5` domain. `assembling_twice_yields_byte_identical_archives` still
   passes, so determinism held across the change.

## One failure that was NOT ours, diagnosed rather than repinned

`sdk_closure_freshness::committed_closure_matches_a_fresh_oracle_harvest` failed
with a `gleam build` File IO error against
`/Users/tom/Developer/ablative/aion/examples/awl-hello/src/awl_hello.gleam` —
note `ablative/aion`, not `ablative/stack/aion`. The test is untouched by this
work, and the stale absolute path lived in a gitignored gleam build cache
created before the repository moved under `stack/`. Clearing that cache
(`examples/awl-hello/build`, 3.3 MB, regenerated on demand) returned it to
green. No source change, and no repin, because nothing about the pin was wrong.
