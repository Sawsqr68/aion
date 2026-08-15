"""Declarative AWL tutorial slides; kept separate from build orchestration."""

from tutorial_model import Excerpt, Slide
from tutorial_cli import CLI_CRATE, CLI_RUST_MIN, CLI_VERSION, COMMAND_COUNT, STAGE_COUNT, config_heredoc, spelled

SLIDES_1: list[Slide] = [
Slide(
        eyebrow="Aion — from nothing",
        title="A gate that judges a repository",
        lede="One workflow, built up a block at a time. By the end it clones a "
        "repository, runs its checks, runs its tests, and keeps the verdict "
        "for as long as you keep the run.",
        big=True,
        caption="It is called <code>repo_gate</code> from the first slide to "
        "the last. Every command and every line of it was run end to end "
        "before it was written down.",
    ),
Slide(
        eyebrow="What you end up with",
        title="A verdict you can point at",
        points=[
            "A command gate you wrote yourself, in a language you can read.",
            f"{spelled(COMMAND_COUNT).capitalize()} real commands, run by the "
            "server — no worker process needed for that first verdict.",
            "Then one agent action on the same queue, served through a real "
            "worker channel and judged on the gate's recorded evidence.",
            "Completed runs whose whole histories are on disk, naming the "
            "commit and carrying the reply that judged it.",
        ],
        # Says only what is true of ALL of them. The first two are never run —
        # the skeleton is shown, and the contract reaches `aion awl check` and
        # stops — so any promise about running every one is false, which is the
        # defect this sentence carried before. Checking clean IS uniform, and
        # the build's own document gate proves it for every staged file on every
        # render. No second number here: a count of how many get run would be a
        # fresh literal with nothing holding it true.
        note=f"You will build it up through {spelled(STAGE_COUNT)} complete "
        "documents. Each one does a little more than the last, and each one "
        "checks clean before you add the next block.",
    ),
Slide(
        eyebrow="Before you start",
        title="One binary. Get it, check it.",
        lede=f"If you have Rust {CLI_RUST_MIN} or newer, this installs the "
        "server, client and authoring tools used throughout the tutorial.",
        cmd=[f"cargo install {CLI_CRATE}", "", "aion --version"],
        out=f"aion {CLI_VERSION}",
        points=[
            "That one binary is the server, the client, and the authoring tools.",
            "The final agent-worker chapter also uses the norn CLI; that "
            "dependency is called out again where the worker starts.",
        ],
        note=f"🔴 <b>The crate is <code>{CLI_CRATE}</code>; the command it "
        "installs is <code>aion</code>.</b> Do not reach for "
        "<code>cargo install aion</code> — on crates.io that name belongs to "
        "an unrelated crate, a small datetime library with no program in it, "
        "so the install stops with <i>there is nothing to install</i>. "
        "Measured 2026-08-07. If you would rather build from a clone of the "
        "repository, <code>cargo install --path crates/aion-cli</code> does "
        "the same job.",
    ),
Slide(
        eyebrow="Step 1 — the server",
        title="Pick a folder and write the config",
        lede="One file tells the server which ports to use and where to keep "
        "its data. Paste this whole block — your shell fills in your own home "
        "folder, so there is nothing to edit.",
        cmd=config_heredoc(),
        note="🔴 <b><code>data_dir</code> must be a full path, and "
        "<code>$HOME</code> is how you get one without typing your username.</b> "
        "A relative path is resolved against whatever folder you happened to "
        "start the server from — start it from somewhere else and it looks "
        "like every run you ever did has vanished. Nothing is lost; you are "
        "just looking at a different store. And <code>~</code> does <i>not</i> "
        "work here: the config file is not a shell, so a <code>~</code> is "
        "taken literally and you get a folder actually named "
        "<code>~</code>. Inside the <code>&lt;&lt;EOF</code> block above your "
        "shell expands <code>$HOME</code> before the file is written, so what "
        "lands on disk is already a full path.",
    ),
Slide(
        eyebrow="Step 1 — the server",
        title="Start it",
        cmd=[
            "cd ~/my-first-flow",
            "AION_HOME=~/my-first-flow/aion-home \\",
            "  aion server --config aion.toml",
        ],
        points=[
            "Leave it running. Open a second terminal for everything else.",
            "The console is now at <code>http://127.0.0.1:8099</code>.",
            "<code>AION_HOME</code> keeps this tutorial's workflows in their "
            "own folder. Leave it off and they go to <code>~/.aion</code>, "
            "alongside anything else you have ever made.",
        ],
        note="If it refuses to start saying an ancestor <i>is not "
        "owner-controlled</i>, your folder is somewhere the whole machine can "
        "write to — <code>/tmp</code>, typically. Move it under your home "
        "folder. The refusal is the store protecting itself, and it names the "
        "exact folder at fault.",
    ),
Slide(
        eyebrow="Step 1 — the server",
        title="Where things land",
        table=[
            (
                "your workflow documents",
                "<code>~/my-first-flow/aion-home/authoring/</code> — that is "
                "<code>$AION_HOME/authoring/</code>, and the server prints it "
                "as <code>authoring_root</code> on the line starting "
                "<i>aion-server authoring root resolved</i>",
            ),
            (
                "the runs and their history",
                "the <code>data_dir</code> you set — printed as "
                "<code>data_root</code> just above it",
            ),
        ],
        note="Both are printed at startup, every time. When something seems "
        "missing, read those two lines first — nine times in ten the answer "
        "is that you are looking in the other folder. Remember the first one: "
        "every command from here on runs in it.",
    ),
Slide(
        eyebrow="Step 2 — the document",
        title="Make the workflow",
        lede="Open the console, go to <b>Authoring</b>. At the top of the "
        "document list there is a box marked <b>New workflow</b>.",
        points=[
            "Type <code>repo_gate</code>. Press <b>+</b>.",
            "The name must <b>start with a lowercase letter</b>, and after "
            "that may use lowercase letters, digits and underscores — "
            "<code>repo_gate</code>, not <code>Repo Gate</code>. A leading "
            "underscore is refused.",
        ],
        note="🔹 <b>Use that exact name.</b> This deck calls the workflow "
        "<code>repo_gate</code> from here to the last slide, and every command "
        "it prints names that file. Call it something else and you will be "
        "translating for the rest of the tutorial.",
    ),
Slide(
        eyebrow="Step 2 — the document",
        title="What you get",
        lede="Not a blank page — a document that already passes the checker.",
        excerpt=Excerpt("repo_gate-1-skeleton.awl"),
        caption="This is exactly what the server writes for a workflow named "
        "<code>repo_gate</code>. It is parsed and reprinted before you ever "
        "see it, so it cannot hand you something the checker would reject.",
        note="It is a placeholder in the literal sense: it declares one way to "
        "finish and one shape to finish with, and does nothing. Everything "
        "from here is you replacing it.",
    ),
Slide(
        eyebrow="Step 3 — the contract",
        title="Say what goes in and what comes back",
        lede="Before any work, settle the promise. Replace the whole skeleton "
        "with this.",
        excerpt=Excerpt("repo_gate-2-contract.awl"),
        caption="A complete document. It does nothing yet — and it checks.",
    ),
Slide(
        eyebrow="Step 3 — the contract",
        title="What each of those lines is",
        table=[
            (
                "<code>//!</code>",
                "a note about the whole document. <code>///</code> notes the "
                "thing on the next line. <code>//</code> is a plain comment.",
            ),
            (
                "<code>workflow</code>",
                "its name. This is what you start later, and it must match "
                "nothing else — the name is the workflow's identity.",
            ),
            (
                "<code>input</code>",
                "something the caller must hand over to start a run. Two here: "
                "which repository, and where to put it. Both are "
                "<code>String</code>.",
            ),
            (
                "<code>outcome</code>",
                "a way the workflow is allowed to end. It names the shape it "
                "hands back (<code>type Gate</code>) and whether that ending "
                "counts as <code>success</code> or <code>failure</code>.",
            ),
            (
                "<code>type</code>",
                "the shape of a piece of data — named fields, each typed. "
                "<code>Gate</code> is what a caller receives.",
            ),
        ],
        note="Nothing here does any work, and that is the point. A workflow "
        "decides <i>what</i> should happen and in what order; it never does "
        "the work itself. That separation is what lets Aion re-run the "
        "deciding part safely after a crash without re-running the work.",
    ),
Slide(
        eyebrow="Step 3 — the contract",
        title="Check it",
        cmd=[
            "cd ~/my-first-flow/aion-home/authoring",
            "aion awl check repo_gate.awl",
        ],
        out="ok: repo_gate.awl (0 steps)",
        points=[
            "<b>0 steps</b> is correct — you have not written one yet.",
            "Errors come out like a compiler's: file, line, column, reason.",
            "Nothing has been deployed. This is free and instant, and it is "
            "worth doing after every block you add.",
        ],
    ),
Slide(
        eyebrow="Step 4 — the first real job",
        title="🔴 Read this before you write a command",
        lede="You are about to give the server a command to run. That is a "
        "capability, and a capability that looks like a sandbox and is not "
        "one is worse than no sandbox at all.",
        points=[
            "There is <b>no sandbox</b>. <code>run \"rm -rf /\"</code> is legal.",
            "No allowlist, no denylist, no filesystem confinement, no network "
            "restriction, no privilege drop, no CPU or memory or wall-clock "
            "limit of any kind.",
            "It runs as whoever runs the server, for as long as it likes.",
            "🔴 <b>The language has a <code>timeout</code> word, and on a "
            "declared command it does nothing.</b> The value is carried to the "
            "engine and read by nobody, and cancelling a declared command is "
            "not wired either. So <code>timeout 30m</code> on a "
            "<code>cargo test</code> that hangs will not end it — nothing "
            "will. That is why this tutorial does not write one. <b>Bound the "
            "command inside the command</b>, with the program's own timeout "
            "flag, if you need a ceiling.",
        ],
        note="What protects you is <b>review, not enforcement</b>: the command "
        "is source code in a file you read before deploying, and it is sealed "
        "into the package's hash so nobody can change it afterwards without "
        "producing a different package. That is a real and strong control — "
        "it is not containment. If you need containment, run the server as a "
        "constrained user, in a container, or on a host with nothing else on "
        "it. The whole story is in "
        "<code>docs/workers/DECLARED-COMMANDS.md</code>.",
    )
]
