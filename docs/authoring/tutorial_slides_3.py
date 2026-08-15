"""Declarative AWL tutorial slides; kept separate from build orchestration."""

from tutorial_model import Excerpt, Slide

SLIDES_3: list[Slide] = [
Slide(
        eyebrow="Step 8 — run the gate",
        title="A green gate",
        cmd=[
            "aion --endpoint http://127.0.0.1:50099 --pretty \\",
            "     run repo_gate.awl \\",
            '     --input "{\\"repo\\": \\"https://github.com/dtolnay/itoa\\", '
            '\\"workdir\\": \\"$HOME/gate-work-5\\"}"',
        ],
        out='"outcome": "passed",\n'
        '"payload": {\n'
        '  "cloned": "Cloning into \'/Users/tom/gate-work-5\'...",\n'
        '  "commit": "1577ed9",\n'
        '  "checks": "    Updating crates.io index\\n     Locking 53 packages '
        "to latest compatible versions\\n…\\n    Checking criterion v0.8.2\\n"
        "    Finished `dev` profile [unoptimized + debuginfo] target(s) in "
        '9.61s",\n'
        '  "tests": "\\nrunning 0 tests\\n\\ntest result: ok. 0 passed; 0 '
        "failed; …\\n\\n\\nrunning 11 tests\\ntest test_i16_0 ... ok\\n…\\n"
        "test result: ok. 11 passed; 0 failed; …\\n\\n\\nrunning 2 tests\\n"
        "test src/lib.rs - Buffer (line 67) ... ok\\ntest src/lib.rs - (line "
        "20) ... ok\\n\\ntest result: ok. 2 passed; 0 failed; 0 ignored; 0 "
        'measured; 0 filtered out; finished in 0.87s\\n"\n'
        "}",
        caption="Real output, with the long middles elided at the <b>…</b> "
        "marks. Nothing was reworded.",
        points=[
            "<code>tests</code> holds <b>three</b> verdicts, because "
            "<code>cargo test</code> runs three harnesses — the library's own "
            "tests, the integration tests, and the documentation examples. A "
            "gate that read only the first one would call a repository green "
            "while its doc examples were broken.",
        ],
        note="It fetched 53 crates and compiled them with no environment but "
        "<code>PATH</code> — a declared command inherits nothing else. That "
        "is worth knowing before you write one that needs a variable set.",
    ),
Slide(
        eyebrow="Step 8 — run the gate",
        title="A red gate",
        lede="Point it at a repository whose tests fail, and this is the whole "
        "of it.",
        out='error: workflow run failed: workflow 8 returned error\n'
        '  details: {"tag":"AwlOutcomeFailure","outcome":"failed",'
        '"payload":"{\\"reason\\":\\"the gate did not pass\\"}"}\n'
        "\n"
        "$ echo $?\n"
        "1",
        points=[
            "That is your <code>on failure</code> block, and your own "
            "<code>reason</code>, coming back.",
            "🔴 <b>Ignore the number in <code>workflow 8</code>.</b> It is an "
            "internal counter, not the workflow id — you cannot look it up. "
            "A known defect; the run's real id is in the console, and in the "
            "output of a run that succeeds.",
            "<b>The command exits 1.</b> So a gate written this way drops "
            "straight into anything that already runs your builds — no "
            "parsing, no glue.",
            "The run is recorded as failed, with everything it did before it "
            "failed still in its history.",
        ],
        note="Contrast a failure with no <code>on failure</code> to catch it "
        "— cloning into a folder that already exists: <code>details: "
        "{\"tag\":\"AwlActivityFailed\",\"message\":\"activity failed "
        "(retryable): the declared command `git` exited 128: fatal: "
        "destination path '/Users/tom/gate-work-5' already exists and is not "
        "an empty directory.\"}</code>. No outcome — the run just faults, and "
        "hands you the command's own words. Both were measured; neither was "
        "reworded.",
    ),
Slide(
        eyebrow="Look at it",
        title="In the console",
        points=[
            "<b>Workflows</b> lists every run: type, status, when it started "
            "and ended.",
            "Open one for its history — every event, in order, exactly as it "
            "was recorded.",
            "That history <i>is</i> the run. It is not a log <i>about</i> the "
            "run.",
        ],
    ),
Slide(
        eyebrow="What just happened",
        title="Why any of this is worth the trouble",
        lede="The workflow never did the work. It decided, and every decision "
        "and every answer was written down before it moved on.",
        points=[
            "Kill the server mid-run and start it again: it picks up from the "
            "recorded history rather than starting over.",
            "The clone that already happened is not done a second time — its "
            "recorded answer is handed back instead.",
            "That is the whole point of Aion. Everything else is detail.",
        ],
        note="Worth doing next, on your own, once the gate works: start it "
        "against something slow, <code>kill -9</code> the server halfway, "
        "start it again, and watch. That exercise is not part of this "
        "tutorial's proven path — try it and see for yourself.",
    ),
Slide(
        eyebrow="When it goes wrong",
        title="Four things, and what they mean",
        table=[
            (
                "<code>destination path … already exists</code>",
                "the <code>workdir</code> from a previous run. Give each run "
                "its own, or remove the old one yourself — the gate will "
                "never remove it for you.",
            ),
            (
                "an empty <code>checks</code> or <code>tests</code>",
                "you read the wrong stream. <code>cargo</code>'s progress is "
                "on stderr; test verdicts are on stdout.",
            ),
            (
                "<code>exited 127</code> or <i>no such file</i>",
                "the command is not on the server's <code>PATH</code>. A "
                "declared command inherits <code>PATH</code> and nothing "
                "else — no <code>HOME</code>, none of your shell's variables.",
            ),
            (
                "your runs seem to have vanished",
                "you started the server from a different folder and "
                "<code>data_dir</code> is relative. Check the "
                "<code>data_root</code> line at startup.",
            ),
        ],
    ),
Slide(
        eyebrow="Where next — perform it",
        title="Declare the judge on the same queue",
        lede="Keep the four command bodies. Declare the outcome record the "
        "seam returns, then add one plain agent action inside the same "
        "<code>worker gate</code> block.",
        excerpt=Excerpt(
            "repo_gate_judged.awl",
            start="type Reply",
            end="agent, timeout",
            dedent=False,
        ),
        caption="One queue, two ways to serve it: the server owns the four "
        "<code>run</code> bodies; a worker owns <code>judge</code>, which "
        "returns the declared <code>Reply</code> record.",
        note="The rest of the document builds the prompt from the clone, commit, "
        "check and test outputs. Only the exact reply "
        "<code>GATE_APPROVED</code> takes the success route. Every other reply "
        "takes a failure outcome that carries the reply unchanged. That stops "
        "free-form commentary being mistaken for approval; it does not make an "
        "agent's opinion true.",
    )
]
