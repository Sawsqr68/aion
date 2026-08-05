# Documentation command gate — findings

**Generated** by `docs/authoring/doc_command_gate.py --write`. Do not hand-edit.

Checked against `aion 0.11.0`.

## What a PASS means, and what it does not

A `PASS` means the binary resolves the subcommand path and accepts every
flag the sentence writes. It does **not** mean the flag does what the
surrounding prose says it does — no automated check can establish that, and
claiming it would make this gate a mood rather than a measurement.

Each `PASS` was re-checked with its deepest subcommand and each of its flags
replaced by names the binary has never heard of. A claim whose broken form
still passed would be reported `UNMEASURED`, never `PASS`.

Positional arguments are not checked: the documents write them as stand-ins
(`WORKFLOW_ID`, `FILE.awl`) that no run could resolve. The count is reported
so the size of what went unchecked is visible.

Each of the gate's own refusals is re-provable on demand with
`--self-check`, which breaks the extractor, the absence declaration, and
the subcommand walk in turn and requires each break to be caught.

## Verdicts

| Verdict | Count | Meaning |
|---|---|---|
| FAIL | 0 | the binary contradicts what the document writes |
| UNMEASURED | 0 | the broken form passed too, so the check never read the claim |
| UNVERIFIABLE | 0 | not a command string the binary could ever be asked to resolve |
| FAMILY | 0 | names a command family rather than a runnable command; the family exists |
| BINARY-ONLY | 1 | names the binary alone |
| ABSENT | 1 | the document says it does not exist, and the binary agrees |
| PASS | 92 | the subcommand path resolves and every flag written is accepted |

| Verdict | Command | Established | Appears |
|---|---|---|---|
| BINARY-ONLY | `aion` | names the binary alone — nothing beyond its existence to establish | docs/GETTING-STARTED.md:91<br>docs/GETTING-STARTED.md:93<br>docs/GETTING-STARTED.md:193<br>examples/assistant/resources/AWL-REFERENCE.md:11 |
| ABSENT | `aion worker python` | the binary does not offer it, as claimed — `examples/assistant/resources/WORKERS.md` names it under 'Layer 2: not available' to state that no script host ships | examples/assistant/resources/WORKERS.md:201 |
| PASS | `aion --endpoint HOST:PORT --namespace NAME cancel WORKFLOW_ID --reason TEXT` | `aion cancel` resolves; accepts `--endpoint`, `--namespace`, `--reason`; 1 positional arg(s) not checked | examples/assistant/resources/COMMANDS.md:250 |
| PASS | `aion --endpoint HOST:PORT --namespace NAME deploy FILE.awl` | `aion deploy` resolves; accepts `--endpoint`, `--namespace`; 1 positional arg(s) not checked | examples/assistant/resources/COMMANDS.md:102 |
| PASS | `aion --endpoint HOST:PORT --namespace NAME describe WORKFLOW_ID` | `aion describe` resolves; accepts `--endpoint`, `--namespace`; 1 positional arg(s) not checked | examples/assistant/resources/COMMANDS.md:214 |
| PASS | `aion --endpoint HOST:PORT --namespace NAME inspect WORKFLOW_ID` | `aion inspect` resolves; accepts `--endpoint`, `--namespace`; 1 positional arg(s) not checked | examples/assistant/resources/COMMANDS.md:215 |
| PASS | `aion --endpoint HOST:PORT --namespace NAME list` | `aion list` resolves; accepts `--endpoint`, `--namespace` | examples/assistant/resources/COMMANDS.md:213 |
| PASS | `aion --endpoint HOST:PORT --namespace NAME pause WORKFLOW_ID --reason TEXT` | `aion pause` resolves; accepts `--endpoint`, `--namespace`, `--reason`; 1 positional arg(s) not checked | examples/assistant/resources/COMMANDS.md:248 |
| PASS | `aion --endpoint HOST:PORT --namespace NAME query WORKFLOW_ID QUERY_NAME` | `aion query` resolves; accepts `--endpoint`, `--namespace`; 2 positional arg(s) not checked | examples/assistant/resources/COMMANDS.md:236 |
| PASS | `aion --endpoint HOST:PORT --namespace NAME reopen WORKFLOW_ID` | `aion reopen` resolves; accepts `--endpoint`, `--namespace`; 1 positional arg(s) not checked | examples/assistant/resources/COMMANDS.md:251 |
| PASS | `aion --endpoint HOST:PORT --namespace NAME resume WORKFLOW_ID` | `aion resume` resolves; accepts `--endpoint`, `--namespace`; 1 positional arg(s) not checked | examples/assistant/resources/COMMANDS.md:249 |
| PASS | `aion --endpoint HOST:PORT --namespace NAME run FILE.awl --input '{"name":"Ada"}'` | `aion run` resolves; accepts `--endpoint`, `--namespace`, `--input`; 1 positional arg(s) not checked | examples/assistant/resources/COMMANDS.md:126 |
| PASS | `aion --endpoint HOST:PORT --namespace NAME signal WORKFLOW_ID SIGNAL_NAME --payload '{"approved":true}'` | `aion signal` resolves; accepts `--endpoint`, `--namespace`, `--payload`; 2 positional arg(s) not checked | examples/assistant/resources/COMMANDS.md:226 |
| PASS | `aion --endpoint HOST:PORT --namespace NAME versions --workflow-type WORKFLOW_NAME` | `aion versions` resolves; accepts `--endpoint`, `--namespace`, `--workflow-type` | examples/assistant/resources/COMMANDS.md:216 |
| PASS | `aion --endpoint HOST:PORT run FILE.awl --input '{"name":"Ada"}' --timeout 5m` | `aion run` resolves; accepts `--endpoint`, `--input`, `--timeout`; 1 positional arg(s) not checked | examples/assistant/resources/COMMANDS.md:140 |
| PASS | `aion --endpoint HOST:PORT worker shell --manifest worker.toml --awl FILE.awl` | `aion worker shell` resolves; accepts `--endpoint`, `--manifest`, `--awl` | examples/assistant/resources/COMMANDS.md:169 |
| PASS | `aion --endpoint HOST:PORT worker shell --manifest worker.toml --awl your_document.awl` | `aion worker shell` resolves; accepts `--endpoint`, `--manifest`, `--awl` | examples/assistant/resources/WORKERS.md:117 |
| PASS | `aion --endpoint http://127.0.0.1:50051 worker shell --manifest worker.toml --awl my_flow.awl` | `aion worker shell` resolves; accepts `--endpoint`, `--manifest`, `--awl` | docs/workers/SHELL-WORKER-MANIFEST.md:7<br>docs/workers/SHELL-WORKER-MANIFEST.md:23 |
| PASS | `aion --help` | `aion` accepts `--help` | docs/GETTING-STARTED.md:199 |
| PASS | `aion --version` | `aion` accepts `--version` | docs/GETTING-STARTED.md:198<br>examples/assistant/resources/COMMANDS.md:4 |
| PASS | `aion awl check` | `aion awl check` resolves (subcommand name only) | docs/authoring/AWL.md:113<br>examples/assistant/resources/AWL-REFERENCE.md:6<br>examples/assistant/resources/AWL-REFERENCE.md:306<br>examples/assistant/resources/AWL-REFERENCE.md:701<br>examples/assistant/resources/AWL-REFERENCE.md:703<br>examples/assistant/resources/AWL-REFERENCE.md:705<br>examples/assistant/resources/WORKERS.md:93 |
| PASS | `aion awl check <FILE.awl>` | `aion awl check` resolves (subcommand name only); 1 positional arg(s) not checked | docs/authoring/AWL.md:49 |
| PASS | `aion awl check FILE.awl` | `aion awl check` resolves (subcommand name only); 1 positional arg(s) not checked | examples/assistant/resources/AWL-AUTHORING.md:255<br>examples/assistant/resources/COMMANDS.md:40<br>examples/assistant/resources/COMMANDS.md:19 |
| PASS | `aion awl check awl_hello.awl` | `aion awl check` resolves (subcommand name only); 1 positional arg(s) not checked | examples/assistant/resources/AWL-AUTHORING.md:36 |
| PASS | `aion awl check my_flow.awl` | `aion awl check` resolves (subcommand name only); 1 positional arg(s) not checked | docs/GETTING-STARTED.md:272 |
| PASS | `aion awl check my_flow.awl                      # 1. until it is clean` | `aion awl check` resolves (subcommand name only); 1 positional arg(s) not checked | docs/authoring/AWL.md:72 |
| PASS | `aion awl emit` | `aion awl emit` resolves (subcommand name only) | examples/assistant/resources/AWL-REFERENCE.md:706 |
| PASS | `aion awl emit <FILE.awl> [--output <P>] [--target gleam\|beam]` | `aion awl emit` resolves; accepts `--output`, `--target`; 1 positional arg(s) not checked | docs/authoring/AWL.md:53 |
| PASS | `aion awl emit FILE.awl --output target/awl/workflow.gleam` | `aion awl emit` resolves; accepts `--output`; 1 positional arg(s) not checked | examples/assistant/resources/COMMANDS.md:67 |
| PASS | `aion awl emit FILE.awl --target beam --output target/awl/workflow.beam` | `aion awl emit` resolves; accepts `--target`, `--output`; 1 positional arg(s) not checked | examples/assistant/resources/COMMANDS.md:57 |
| PASS | `aion awl fmt <FILE.awl>` | `aion awl fmt` resolves (subcommand name only); 1 positional arg(s) not checked | docs/authoring/AWL.md:50 |
| PASS | `aion awl fmt FILE.awl` | `aion awl fmt` resolves (subcommand name only); 1 positional arg(s) not checked | examples/assistant/resources/AWL-AUTHORING.md:258<br>examples/assistant/resources/COMMANDS.md:35 |
| PASS | `aion awl fmt my_flow.awl` | `aion awl fmt` resolves (subcommand name only); 1 positional arg(s) not checked | docs/GETTING-STARTED.md:285 |
| PASS | `aion awl fmt my_flow.awl                        # 2. canonicalise, check again` | `aion awl fmt` resolves (subcommand name only); 1 positional arg(s) not checked | docs/authoring/AWL.md:73 |
| PASS | `aion awl lsp` | `aion awl lsp` resolves (subcommand name only) | docs/GETTING-STARTED.md:469<br>docs/authoring/AWL.md:54<br>docs/authoring/AWL.md:137 |
| PASS | `aion awl scaffold` | `aion awl scaffold` resolves (subcommand name only) | docs/authoring/AWL.md:122<br>examples/assistant/resources/WORKERS.md:22 |
| PASS | `aion awl scaffold <FILE.awl> --out <DIR> [--worker <name>] [--aion-crates <DIR>]` | `aion awl scaffold` resolves; accepts `--out`, `--worker`, `--aion-crates`; 1 positional arg(s) not checked | docs/authoring/AWL.md:52 |
| PASS | `aion awl scaffold FILE.awl --out ./workers` | `aion awl scaffold` resolves; accepts `--out`; 1 positional arg(s) not checked | examples/assistant/resources/AWL-AUTHORING.md:110<br>examples/assistant/resources/WORKERS.md:219<br>examples/assistant/resources/COMMANDS.md:77 |
| PASS | `aion awl schema <FILE.awl> [--type <T>] [--queries]` | `aion awl schema` resolves; accepts `--type`, `--queries`; 1 positional arg(s) not checked | docs/authoring/AWL.md:51 |
| PASS | `aion awl schema FILE.awl` | `aion awl schema` resolves (subcommand name only); 1 positional arg(s) not checked | examples/assistant/resources/COMMANDS.md:45 |
| PASS | `aion awl schema FILE.awl --queries` | `aion awl schema` resolves; accepts `--queries`; 1 positional arg(s) not checked | examples/assistant/resources/AWL-REFERENCE.md:663<br>examples/assistant/resources/AWL-AUTHORING.md:245<br>examples/assistant/resources/COMMANDS.md:242 |
| PASS | `aion awl schema FILE.awl --type Report` | `aion awl schema` resolves; accepts `--type`; 1 positional arg(s) not checked | examples/assistant/resources/COMMANDS.md:46 |
| PASS | `aion awl schema my_flow.awl` | `aion awl schema` resolves (subcommand name only); 1 positional arg(s) not checked | docs/GETTING-STARTED.md:294 |
| PASS | `aion awl schema my_flow.awl --type Greeting` | `aion awl schema` resolves; accepts `--type`; 1 positional arg(s) not checked | docs/GETTING-STARTED.md:314 |
| PASS | `aion cancel` | `aion cancel` resolves (subcommand name only) | docs/GETTING-STARTED.md:452 |
| PASS | `aion deploy` | `aion deploy` resolves (subcommand name only) | docs/GETTING-STARTED.md:100<br>docs/GETTING-STARTED.md:366<br>docs/GETTING-STARTED.md:370<br>docs/GETTING-STARTED.md:832<br>docs/authoring/AWL.md:84<br>examples/assistant/resources/AWL-REFERENCE.md:706 |
| PASS | `aion deploy <FILE.awl>` | `aion deploy` resolves (subcommand name only); 1 positional arg(s) not checked | docs/authoring/AWL.md:60 |
| PASS | `aion deploy FILE.awl` | `aion deploy` resolves (subcommand name only); 1 positional arg(s) not checked | examples/assistant/resources/COMMANDS.md:158 |
| PASS | `aion deploy my_flow.awl` | `aion deploy` resolves (subcommand name only); 1 positional arg(s) not checked | docs/GETTING-STARTED.md:412 |
| PASS | `aion deploy my_flow/my-flow.aion` | `aion deploy` resolves (subcommand name only); 1 positional arg(s) not checked | docs/GETTING-STARTED.md:894 |
| PASS | `aion describe` | `aion describe` resolves (subcommand name only) | examples/assistant/resources/AWL-REFERENCE.md:665 |
| PASS | `aion describe <workflow-id> --pretty` | `aion describe` resolves; accepts `--pretty`; 1 positional arg(s) not checked | docs/GETTING-STARTED.md:442<br>docs/GETTING-STARTED.md:947 |
| PASS | `aion generate` | `aion generate` resolves (subcommand name only) | docs/GETTING-STARTED.md:707<br>docs/authoring/AWL.md:7 |
| PASS | `aion generate .` | `aion generate` resolves (subcommand name only) | docs/GETTING-STARTED.md:717<br>docs/GETTING-STARTED.md:697 |
| PASS | `aion generate . --check` | `aion generate` resolves; accepts `--check`; 1 positional arg(s) not checked | docs/GETTING-STARTED.md:718 |
| PASS | `aion inspect` | `aion inspect` resolves (subcommand name only) | docs/GETTING-STARTED.md:453 |
| PASS | `aion list` | `aion list` resolves (subcommand name only) | docs/GETTING-STARTED.md:422<br>docs/authoring/AWL.md:65 |
| PASS | `aion new <name>` | `aion new` resolves (subcommand name only); 1 positional arg(s) not checked | docs/GETTING-STARTED.md:207 |
| PASS | `aion package` | `aion package` resolves (subcommand name only) | docs/GETTING-STARTED.md:414<br>docs/GETTING-STARTED.md:666<br>docs/authoring/AWL.md:7<br>examples/assistant/resources/COMMANDS.md:107<br>examples/assistant/resources/COMMANDS.md:160 |
| PASS | `aion package .` | `aion package` resolves (subcommand name only) | docs/GETTING-STARTED.md:726 |
| PASS | `aion package . --build` | `aion package` resolves; accepts `--build`; 1 positional arg(s) not checked | docs/GETTING-STARTED.md:753 |
| PASS | `aion pause` | `aion pause` resolves (subcommand name only) | docs/GETTING-STARTED.md:451 |
| PASS | `aion query` | `aion query` resolves (subcommand name only) | docs/GETTING-STARTED.md:955 |
| PASS | `aion query <workflow-id> <query-name>` | `aion query` resolves (subcommand name only); 2 positional arg(s) not checked | examples/assistant/resources/AWL-REFERENCE.md:662 |
| PASS | `aion query <workflow-id> status` | `aion query` resolves (subcommand name only); 2 positional arg(s) not checked | docs/GETTING-STARTED.md:918 |
| PASS | `aion query WORKFLOW_ID QUERY_NAME` | `aion query` resolves (subcommand name only); 2 positional arg(s) not checked | examples/assistant/resources/AWL-AUTHORING.md:244 |
| PASS | `aion reopen` | `aion reopen` resolves (subcommand name only) | docs/GETTING-STARTED.md:453 |
| PASS | `aion resume` | `aion resume` resolves (subcommand name only) | docs/GETTING-STARTED.md:452 |
| PASS | `aion route my_flow <content-hash>   # roll back / forward to a loaded version` | `aion route` resolves (subcommand name only); 2 positional arg(s) not checked | docs/GETTING-STARTED.md:967 |
| PASS | `aion run` | `aion run` resolves (subcommand name only) | docs/GETTING-STARTED.md:315<br>docs/GETTING-STARTED.md:366<br>docs/GETTING-STARTED.md:370<br>docs/authoring/AWL.md:77<br>examples/assistant/resources/AWL-REFERENCE.md:706<br>examples/assistant/resources/COMMANDS.md:268 |
| PASS | `aion run --timeout` | `aion run` resolves; accepts `--timeout` | docs/authoring/AWL.md:88 |
| PASS | `aion run --timeout 5m` | `aion run` resolves; accepts `--timeout` | examples/assistant/resources/AWL-AUTHORING.md:265 |
| PASS | `aion run <FILE.awl> --input '<json>' [--timeout <dur>]` | `aion run` resolves; accepts `--input`, `--timeout`; 1 positional arg(s) not checked | docs/authoring/AWL.md:61 |
| PASS | `aion run FILE.awl --input 'JSON'` | `aion run` resolves; accepts `--input`; 1 positional arg(s) not checked | examples/assistant/resources/AWL-AUTHORING.md:260 |
| PASS | `aion run FILE.awl --input ...` | `aion run` resolves; accepts `--input`; 1 positional arg(s) not checked | examples/assistant/resources/COMMANDS.md:159 |
| PASS | `aion run my_flow.awl --input '{"name":"Ada"}'` | `aion run` resolves; accepts `--input`; 1 positional arg(s) not checked | docs/GETTING-STARTED.md:400 |
| PASS | `aion run my_flow.awl --input '{"name":"Ada"}'   # 3. compile, deploy, start, await` | `aion run` resolves; accepts `--input`; 1 positional arg(s) not checked | docs/authoring/AWL.md:74 |
| PASS | `aion server` | `aion server` resolves (subcommand name only) | docs/GETTING-STARTED.md:195<br>docs/GETTING-STARTED.md:217 |
| PASS | `aion server --config aion.toml` | `aion server` resolves; accepts `--config` | docs/GETTING-STARTED.md:386<br>docs/GETTING-STARTED.md:429<br>docs/GETTING-STARTED.md:879<br>docs/GETTING-STARTED.md:935 |
| PASS | `aion signal <workflow-id> approval --payload '{"approver":"ada"}'` | `aion signal` resolves; accepts `--payload`; 2 positional arg(s) not checked | docs/GETTING-STARTED.md:441<br>docs/GETTING-STARTED.md:946 |
| PASS | `aion start` | `aion start` resolves (subcommand name only) | examples/assistant/resources/COMMANDS.md:160 |
| PASS | `aion start my_flow --input '{"name":"Ada"}'` | `aion start` resolves; accepts `--input`; 1 positional arg(s) not checked | docs/GETTING-STARTED.md:413<br>docs/GETTING-STARTED.md:905 |
| PASS | `aion unload my_flow <content-hash>  # remove a non-routed, unpinned version` | `aion unload` resolves (subcommand name only); 2 positional arg(s) not checked | docs/GETTING-STARTED.md:968 |
| PASS | `aion versions                       # every loaded version, route flags` | `aion versions` resolves (subcommand name only) | docs/GETTING-STARTED.md:966 |
| PASS | `aion worker --help` | `aion worker` resolves; accepts `--help` | examples/assistant/resources/WORKERS.md:27 |
| PASS | `aion worker agent` | `aion worker agent` resolves (subcommand name only) | docs/authoring/AWL.md:123<br>examples/assistant/resources/COMMANDS.md:180 |
| PASS | `aion worker agent <FILE.awl>` | `aion worker agent` resolves (subcommand name only); 1 positional arg(s) not checked | examples/assistant/resources/AWL-REFERENCE.md:210 |
| PASS | `aion worker agent <FILE.awl> --harness <name> --liminal-address <addr>` | `aion worker agent` resolves; accepts `--harness`, `--liminal-address`; 1 positional arg(s) not checked | docs/authoring/AWL.md:63 |
| PASS | `aion worker agent FILE.awl` | `aion worker agent` resolves (subcommand name only); 1 positional arg(s) not checked | examples/assistant/resources/AWL-AUTHORING.md:105<br>examples/assistant/resources/WORKERS.md:20<br>examples/assistant/resources/WORKERS.md:34 |
| PASS | `aion worker agent FILE.awl --harness norn --liminal-address HOST:PORT --identity NAME --concurrency N --reconnect-initial-backoff S --reconnect-max-backoff S --reconnect-max-attempts N` | `aion worker agent` resolves; accepts `--harness`, `--liminal-address`, `--identity`, `--concurrency`, `--reconnect-initial-backoff`, `--reconnect-max-backoff`, `--reconnect-max-attempts`; 1 positional arg(s) not checked | examples/assistant/resources/COMMANDS.md:170 |
| PASS | `aion worker shell` | `aion worker shell` resolves (subcommand name only) | docs/GETTING-STARTED.md:218<br>docs/authoring/AWL.md:121<br>docs/workers/SHELL-WORKER-MANIFEST.md:32<br>docs/workers/SHELL-WORKER-MANIFEST.md:167<br>examples/assistant/resources/AWL-AUTHORING.md:108<br>examples/assistant/resources/WORKERS.md:158<br>examples/assistant/resources/COMMANDS.md:176 |
| PASS | `aion worker shell --manifest worker.toml --awl <FILE.awl>` | `aion worker shell` resolves; accepts `--manifest`, `--awl` | docs/authoring/AWL.md:62 |
| PASS | `aion worker shell --manifest worker.toml --awl FILE.awl` | `aion worker shell` resolves; accepts `--manifest`, `--awl` | examples/assistant/resources/WORKERS.md:19 |
| PASS | `aion worker shell --manifest worker.toml --awl my_flow.awl` | `aion worker shell` resolves; accepts `--manifest`, `--awl` | docs/GETTING-STARTED.md:392 |
