"""Move the authoring tutorial's worker slide onto the document-declared launch."""

import pathlib

p = pathlib.Path("docs/authoring/build-tutorial.py")
s = p.read_text()

old = '''        cmd=[
            "aion worker agent repo_gate_judged.awl \\\\",
            "  --harness norn \\\\",
            "  --liminal-address 127.0.0.1:50169 \\\\",
            "  --identity repo-gate-judge --concurrency 4 \\\\",
            "  --reconnect-initial-backoff 1 \\\\",
            "  --reconnect-max-backoff 30 \\\\",
            "  --reconnect-max-attempts 100",
        ],
        points=[
            "The address is the tutorial config's <code>[outbox]</code> listener "
            "— not its HTTP port and not its gRPC port.",
            "Identity, concurrency and all three reconnect values are required "
            "operator policy on 0.11.0; the worker refuses to invent them.",
            "The installed 0.11.0 binary carries the norn and ACP adapters, but "
            "does not expose its plain-CLI harness. This teachable path therefore "
            "uses norn and requires the <code>norn</code> CLI on "
            "<code>PATH</code>.",
            "The one-block document derives queue <code>gate</code>; no "
            "<code>--task-queue</code> flag is needed.",
        ],'''

new = '''        cmd=[
            "# First, in repo_gate_judged.awl, under `worker gate`:",
            "#   harness",
            "#     kind norn",
            "#     concurrency 4",
            "#     reconnect_initial_backoff 1s",
            "#     reconnect_max_backoff 30s",
            "#     reconnect_max_attempts 100",
            '#     env_pass "PATH", "HOME"',
            "",
            "aion worker agent repo_gate_judged.awl \\\\",
            "  --liminal-address 127.0.0.1:50169 \\\\",
            "  --identity repo-gate-judge",
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
        ],'''

if old not in s:
    raise SystemExit("tutorial slide context not found")
s = s.replace(old, new, 1)
p.write_text(s)
print("ok")
