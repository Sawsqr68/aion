"""Declarative AWL tutorial slides; kept separate from build orchestration."""

from tutorial_cli import MOTIONS, spelled
from tutorial_model import Slide

SLIDES_4: list[Slide] = [
Slide(
        eyebrow="Where next — deploy",
        title="Deploy the judged contract",
        cmd=[
            "aion --endpoint http://127.0.0.1:50099 --pretty \\",
            "     deploy repo_gate_judged.awl",
        ],
        points=[
            "Deploy records all five action shapes under queue "
            "<code>gate</code>.",
            "The four bodies are immediately servable by the server. "
            "<code>judge</code> is not: no process has offered it yet.",
        ],
    ),
Slide(
        eyebrow="Where next — pre-flight",
        title="Ask before you start a run",
        cmd=[
            "curl -sS http://127.0.0.1:8099/awl/workers/availability \\",
            "  -H 'content-type: application/json' \\",
            "  -d '{\"namespace\":\"default\",\"task_queue\":\"gate\"}'",
        ],
        out='{"available":false,"task_queue":"gate","connected_workers":0,'
        '"scaffold_hint":"No connected worker serves task queue `gate`. '
        'Scaffold and run this worker from Workers & Actions, then retry '
        'start."}',
        caption="Real output, one line, exactly as <code>curl</code> prints it.",
        points=[
            "<code>available: false</code> is an honest refusal before work can "
            "park: the one worker-owed action you just declared is "
            "<code>judge</code>.",
            "0.11.0 reports that absence at queue level: no connected worker "
            "serves <code>gate</code>, and it tells you to start one before "
            "retrying. Newer servers may add a per-action census; this chapter "
            "does not depend on it.",
        ],
    ),
Slide(
        eyebrow="Where next — connect",
        title="Start the worker in a second terminal",
        cmd=[
            "aion worker agent repo_gate_judged.awl \\",
            "  --liminal-address 127.0.0.1:50169 \\",
            "  --identity repo-gate-judge",
        ],
        bites=[
            "Declare the launch first, in <code>repo_gate_judged.awl</code>, "
            "indented under <code>worker gate</code> and above its actions: "
            "<code>harness</code>, then <code>kind norn</code>, "
            "<code>concurrency 4</code>, "
            "<code>reconnect_initial_backoff 1s</code>, "
            "<code>reconnect_max_backoff 30s</code>, "
            "<code>reconnect_max_attempts 100</code>, and "
            "<code>env_pass \"PATH\", \"HOME\"</code> — one setting per line.",
        ],
        points=[
            "The address is the tutorial config's <code>[outbox]</code> listener "
            "— not its HTTP port and not its gRPC port.",
            "The LAUNCH is declared in the document, in the "
            "<code>harness</code> section of the <code>worker</code> block: which "
            "harness, how many activities at once, how it reconnects, and which "
            "environment variables the agent subprocess is given. Every value is "
            "required operator policy — nothing anywhere invents one — and "
            "<code>aion awl check</code> names any you leave out.",
            "<code>env_pass</code> is the agent subprocess's WHOLE environment. "
            "The document names the variables, this machine supplies their "
            "values, and a variable you do not name is absent in the agent "
            "whatever your shell exported. Nothing is inherited.",
            "What is left on the command line is the connection: which document, "
            "where to dial, and who this worker says it is. There is no "
            "<code>--harness</code>, no <code>--concurrency</code> and no "
            "<code>--reconnect-*</code> flag — they were deleted when the section "
            "shipped, and passing one is an arg-parse error.",
            "The shipped binary carries the norn and ACP adapters, but does not "
            "expose its plain-CLI harness. This teachable path therefore declares "
            "<code>kind norn</code> and requires the <code>norn</code> CLI on "
            "<code>PATH</code> — which is also why <code>PATH</code> is in the "
            "<code>env_pass</code> list above.",
            "The one-block document derives queue <code>gate</code>; no "
            "<code>--task-queue</code> flag is needed.",
        ],
        note="If registration is refused, <b>you are told twice</b>: the server "
        "logs the refusal at WARN with the queue and reason, and this worker "
        "fails with the same diagnosis. Read either side; do not leave a "
        "rejected worker running and hope the queue becomes servable.",
    ),
Slide(
        eyebrow="Where next — pre-flight again",
        title="Now the same question says yes",
        cmd=[
            "curl -sS http://127.0.0.1:8099/awl/workers/availability \\",
            "  -H 'content-type: application/json' \\",
            "  -d '{\"namespace\":\"default\",\"task_queue\":\"gate\"}'",
        ],
        out='{"available":true,"task_queue":"gate","connected_workers":1,'
        '"scaffold_hint":null}',
        caption="Real output, one line, exactly as <code>curl</code> prints it.",
        points=[
            "Nothing in the document changed. The channel did: a worker now "
            "offers the one action the server cannot execute itself.",
            "The four command bodies remain server-run. The worker advertised "
            "only <code>judge</code>; its admission log names that action, while "
            "this 0.11.0 endpoint answers the queue-level question.",
        ],
    ),
Slide(
        eyebrow="Where next — run it",
        title="The evidence reaches the judge",
        cmd=[
            "aion --endpoint http://127.0.0.1:50099 --pretty \\",
            "     run repo_gate_judged.awl \\",
            '     --input "{\\"repo\\": \\"https://github.com/dtolnay/itoa\\", '
            '\\"workdir\\": \\"$HOME/gate-work-judged\\"}"',
        ],
        out='"result": {\n'
        '  "outcome": "passed",\n'
        '  "payload": {\n'
        '    "cloned": "Cloning into …",\n'
        '    "commit": "1577ed9",\n'
        '    "checks": "    Updating crates.io index … Finished `dev` profile …",\n'
        '    "tests": "… test result: ok. 11 passed; 0 failed; …",\n'
        '    "judgement": "GATE_APPROVED"\n'
        "  }\n"
        "}",
        caption="Real output, with the long middles elided at the <b>…</b> "
        "marks. Nothing was reworded.",
        points=[
            "Clone, head, checks and tests still run from the sealed command "
            "bodies. Their outputs are concatenated into the prompt in-flow.",
            "The worker returns one string. The workflow, not the worker, owns "
            "the exact-token routing and records the reply in its outcome.",
        ],
        note="A different reply ends at <code>failed</code> and carries "
        "<code>judge_reply</code> verbatim. A verdict is evidence about what one "
        "agent said after seeing the gate output — not proof that the repository "
        "is correct.",
    ),
Slide(
        eyebrow="Where next",
        title="You have the shape of it now",
        points=[
            "The AWL deck covers the rest of the language — waits, signals, "
            "children, parallel work, guards, retries, failure handling.",
            f"The same {spelled(len(MOTIONS))} motions scale up: "
            f"{', '.join(MOTIONS)}.",
            "A gate is only the first useful thing. Anything that is a "
            "sequence of jobs with a verdict at the end is the same shape.",
        ],
        note="The worker story has its own documents: "
        "<code>docs/workers/</code> for worker processes and admission (a "
        "worker is let in only if it offers every reachable worker-owed "
        "action on its queue), <code>docs/workers/DECLARED-COMMANDS.md</code> "
        "for the <code>run</code> bodies you used here.",
        big=False,
    )
]
