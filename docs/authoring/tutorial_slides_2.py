"""Declarative AWL tutorial slides; kept separate from build orchestration."""

from tutorial_model import Excerpt, Slide
from tutorial_cli import FINISHED_LINE_COUNT
from tutorial_keywords import KEYWORD_COUNT, KEYWORD_GROUP_COUNT, KEYWORD_TABLE

SLIDES_2: list[Slide] = [
Slide(
        eyebrow="Step 4 — the first real job",
        title="Add a worker and give it something to do",
        lede="Two new blocks and one new type. Add them to what you already "
        "have.",
        excerpt=Excerpt("repo_gate-3-clone.awl", start="/// What a `run` body"),
        caption="Your document is now this, on top of the contract you already "
        "wrote.",
    ),
Slide(
        eyebrow="Step 4 — the first real job",
        title="What each of those lines is",
        table=[
            (
                "<code>worker</code>",
                "a name for whoever does real work — in practice, a queue. "
                "Actions are declared inside it.",
            ),
            (
                "<code>action</code>",
                "one job, with its parameters, their types, and the shape it "
                "returns. This one takes a repository and a folder.",
            ),
            (
                "<code>run</code>",
                "the implementation, right here. The server executes this "
                "command itself — there is no worker process to write or "
                "start. <code>$repo</code> and <code>$into</code> are this "
                "action's own parameters.",
            ),
            (
                "<code>step</code>",
                "a stage of the workflow: the plan, in order.",
            ),
            (
                "<code>-&gt;</code> in a step",
                "give the result a name. <code>cloned</code> now holds what "
                "the command handed back.",
            ),
            (
                "<code>route</code>",
                "finish, this way, with this. Here it ends at "
                "<code>passed</code>, filling in the one field "
                "<code>Gate</code> declares.",
            ),
        ],
        note="Three rules for a <code>run</code> body and you can write your "
        "own. <b>One:</b> it always produces <code>exit_code</code>, "
        "<code>stdout</code> and <code>stderr</code>, so the action returns "
        "those — or a named subset, which is what <code>Report</code> is. "
        "Claim anything else and the checker refuses. <b>Two:</b> the body "
        "goes on its own line under the action, after any config line, and "
        "arguments interpolate as <code>$name</code> or "
        "<code>${name}</code>. <b>Three:</b> it is <i>not</i> a shell — no "
        "pipes, no redirects, no <code>&amp;&amp;</code>. A program and its "
        "arguments. If you want a pipe, write a script and call that.",
    ),
Slide(
        eyebrow="Step 4 — the first real job",
        title="Run it",
        lede="One verb: compile, deploy, start, wait for the answer. Point it "
        "at any repository — a URL, or a path to one you already have.",
        cmd=[
            "aion --endpoint http://127.0.0.1:50099 --pretty \\",
            "     run repo_gate.awl \\",
            '     --input "{\\"repo\\": \\"https://github.com/dtolnay/itoa\\", '
            '\\"workdir\\": \\"$HOME/gate-work-3\\"}"',
        ],
        out='{\n'
        '  "workflow_type": "repo_gate",\n'
        '  "workflow_id": "cdf7fd30-a5d5-4946-bec7-e0417e073c7d",\n'
        '  "run_id": "f4c72e7c-9ac9-457d-91ae-99eeced0d874",\n'
        '  "result": {\n'
        '    "outcome": "passed",\n'
        '    "payload": {\n'
        '      "cloned": "Cloning into \'/Users/tom/gate-work-3\'..."\n'
        "    }\n"
        "  }\n"
        "}",
        caption="The real output of this deck's own run of that document. A "
        "repository was cloned onto the machine and the evidence came back "
        "through the workflow.",
        note="🔹 <b>Give every run its own <code>workdir</code>.</b> "
        "<code>git</code> refuses to clone into a folder that already exists, "
        "and that refusal is a real failure you will meet later on purpose. "
        "The clone's chatter is on <i>standard error</i>, not standard output "
        "— that is why the route reads <code>cloned.stderr</code>. Nothing is "
        "wrong; that is simply where <code>git</code> writes it.",
    ),
Slide(
        eyebrow="Step 5 — a second job",
        title="Say which commit you judged",
        lede="A verdict nobody can repeat is not a verdict. Add a second "
        "action, call it in the same step, and grow the shape you hand back.",
        excerpt=Excerpt(
            "repo_gate-4-commit.awl",
            start="/// Which commit",
            end="route passed",
        ),
        caption="The new action, and the step that now calls both.",
        note="Three things changed together, and they always will. The "
        "<code>action</code> declares the job. The <code>step</code> calls it "
        "and names its result <code>at</code>. And <code>type Gate</code> "
        "gained a <code>commit</code> field, because a workflow may only hand "
        "back a shape it has declared. Miss one and the checker names the one "
        "you missed.",
    ),
Slide(
        eyebrow="Step 5 — a second job",
        title="Two calls, one step",
        points=[
            "Calls inside a step happen in the order they are written. "
            "<code>head</code> runs after <code>clone</code>, which is what "
            "makes it safe to look inside the folder.",
            "<code>workdir</code> is an input, so any call can use it. "
            "<code>at.stdout</code> is a result, so only lines after it exist "
            "can.",
            "Run it again — a fresh <code>workdir</code> — and the payload "
            "now carries <code>\"commit\": \"1577ed9\"</code> beside the "
            "clone.",
        ],
        out='{\n'
        '  "outcome": "passed",\n'
        '  "payload": {\n'
        '    "cloned": "Cloning into \'/Users/tom/gate-work-4\'...",\n'
        '    "commit": "1577ed9"\n'
        "  }\n"
        "}",
        caption="Real output, trimmed to the result. Every run of this "
        "tutorial is in the deck exactly as it came back.",
    ),
Slide(
        eyebrow="Step 6 — the gate itself",
        title="The checks and the tests",
        lede="Two more actions. These are the ones that make it a gate.",
        excerpt=Excerpt(
            "repo_gate.awl",
            start="/// cargo writes",
            end='run "cargo test',
            # Keep the indentation: these actions live INSIDE `worker gate`,
            # and dedenting them to column zero would show a reader something
            # the checker rejects.
            dedent=False,
        ),
        caption="Real commands, run by the server, sealed into the document.",
        note="The commands are literal, and they have to be: a caller can "
        "never choose which program runs — <code>run \"$program arg\"</code> "
        "is refused at check time. That is the whole reason a gate written "
        "this way can be trusted. Notice also which stream each one is read "
        "from below: <code>cargo</code> writes its progress to <i>standard "
        "error</i>, while the test binaries write their verdicts to "
        "<i>standard output</i>. Read the wrong one and your report is empty "
        "with nothing to say why.",
    ),
Slide(
        eyebrow="Step 6 — the gate itself",
        title="A second step, and what has to finish first",
        excerpt=Excerpt("repo_gate.awl", start="step fetch"),
        caption="The last two blocks of the finished document.",
        table=[
            (
                "<code>after</code>",
                "names what must finish first. <code>verify</code> waits for "
                "<code>fetch</code>, so the folder is really there before "
                "anything looks in it.",
            ),
            (
                "the route moved",
                "the ending belongs to the step that ends the work. It was in "
                "<code>fetch</code>; now <code>verify</code> owns it.",
            ),
            (
                "<code>on</code> <code>failure</code>",
                "what to do when the step faults. It must itself end in a "
                "<code>route</code> — quietly swallowing a failure is a check "
                "error, not an option.",
            ),
        ],
    ),
Slide(
        eyebrow="Step 6 — the gate itself",
        title="How a gate says no",
        lede="There is no <i>if the tests failed</i> line, and there does not "
        "need to be.",
        points=[
            "A command that exits non-zero <b>fails its action</b>. The "
            "failure carries the command's own words.",
            "A failed action <b>faults its step</b>.",
            "A faulted step runs its <code>on failure</code> block — and that "
            "block routes to <code>failed</code>, which you declared as "
            "<code>route failure</code>.",
        ],
        note="So the exit status of <code>cargo test</code> is what decides "
        "the outcome, without a single line of comparison. The corollary is "
        "worth knowing: <code>fetch</code> has no <code>on failure</code>, so "
        "a clone that fails does not become a red gate — it faults the run "
        "outright. Both are shown two slides on.",
    ),
Slide(
        eyebrow="Step 7 — the whole thing",
        title="The finished document",
        lede=f"{FINISHED_LINE_COUNT} lines. This is every block you have "
        "added, in order.",
        excerpt=Excerpt("repo_gate.awl"),
        caption="A complete file — checked, deployed and run exactly as it "
        "appears here.",
    ),
Slide(
        eyebrow="Step 7 — the whole thing",
        title="Every word in both documents",
        lede=f"{KEYWORD_COUNT} words, in {KEYWORD_GROUP_COUNT} groups. Each "
        "group answers one question.",
        keywords=True,
        table=KEYWORD_TABLE,
        note="Every language word used by the command gate and the judged gate "
        "is in that table, and the build refuses if that stops being true. The "
        "agent and policy words are the only ones you have not used yet; the "
        "performed worker chapter below puts them to work. It is not the whole "
        "language — waits, signals, children and parallel work have their own "
        "deck.",
    ),
Slide(
        eyebrow="Step 8 — run the gate",
        title="Deploy it on its own",
        lede="<code>run</code> deploys for you. Doing it separately is worth "
        "seeing once, because of what comes back.",
        cmd=[
            "aion --endpoint http://127.0.0.1:50099 --pretty \\",
            "     deploy repo_gate.awl",
        ],
        out='{\n'
        '  "workflow_type": "repo_gate",\n'
        '  "content_hash": "9926c2fe0c205d11ceae7799c132f98cc5a46610127b963'
        'f7d5f2318b66fb5b0",\n'
        '  "deployed_entry_module": "repo_gate$9926c2fe0c205d11ceae7799c132f'
        '98cc5a46610127b963f7d5f2318b66fb5b0",\n'
        '  "entry_function": "run",\n'
        '  "freshly_loaded": true,\n'
        '  "route_changed": true,\n'
        '  "superseded_versions": [\n'
        '    "0d5f38c454c4dbdf049e5ea720977a88711036d4645151628f3852bde2b30f'
        '96",\n'
        '    "24d3b951e83b516079d6688dbe3c234a6a36d1e8e16b5404a07e1397f1f63c'
        '9f"\n'
        "  ]\n"
        "}",
        points=[
            "The hash is the version's <b>real name</b>. It is taken from the "
            "content, commands and all — the same file deployed to a "
            "different server gets the same hash.",
            "<code>deployed_entry_module</code> is that hash bolted onto the "
            "workflow's name. That is the module the server actually loaded, "
            "and it is why two versions of <code>repo_gate</code> can be live "
            "at once without colliding.",
            "<code>freshly_loaded: true</code> because these exact bytes had "
            "not been deployed before. Deploy the unchanged file again and it "
            "is <code>false</code> — change one character and it is a "
            "different version.",
            "<code>superseded_versions</code> are the two earlier stages you "
            "deployed while building this up. They are kept, not deleted: a "
            "run already going keeps the version it started on, to the end, "
            "even if that is a month from now.",
        ],
    )
]
