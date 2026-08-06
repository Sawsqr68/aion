#!/usr/bin/env python3
"""Build the from-scratch tutorial deck from artifacts that are known to work.

Every AWL snippet is sliced out of a complete document under
`tutorial-examples/`, and the build refuses outright if any of them stops
passing `aion awl check`. The manifest is gated the same way: its wiring is
checked against the document it claims to serve, including the rule that made
this tutorial necessary — a worker must advertise its queue's FULL declared
action set or it is refused at registration, silently, on both sides.

The point is that the deck cannot teach a shape the tools reject.
"""

import html
import shutil
import subprocess
import sys
import tomllib
from dataclasses import dataclass, field
from pathlib import Path

HERE = Path(__file__).resolve().parent
EXAMPLES = HERE / "tutorial-examples"
SHELL = HERE / "deck-shell.html"
OUTPUT = HERE / "first-workflow.html"

SHELL_TITLE = "<title>AWL — the syntax, keyword by keyword</title>"
DECK_TITLE = "<title>Your first Aion workflow — start to finish</title>"

# Every `aion` verb the deck tells the reader to type. Gated: a deck that
# teaches a command the binary does not have is the failure this list exists
# to prevent.
TAUGHT_VERBS: list[list[str]] = [
    ["server", "--help"],
    ["awl", "check", "--help"],
    ["deploy", "--help"],
    ["worker", "shell", "--help"],
    ["run", "--help"],
    ["list", "--help"],
    ["describe", "--help"],
]


# --------------------------------------------------------------------------
# Excerpts
# --------------------------------------------------------------------------


@dataclass(frozen=True)
class Excerpt:
    """A contiguous run of lines from one gated example file.

    Anchored on substrings, not line numbers, so editing an example cannot
    silently shift what a slide shows. A missing anchor is a hard error.
    """

    document: str  # file name inside tutorial-examples, with suffix
    start: str = ""  # empty means "from the first line"
    end: str = ""  # empty means "to the last line"
    dedent: bool = True
    end_exact: bool = False
    strip_comments: bool = False

    @property
    def path(self) -> Path:
        return EXAMPLES / self.document

    def _matches_end(self, line: str) -> bool:
        return line.strip() == self.end if self.end_exact else self.end in line

    def text(self) -> str:
        lines = self.path.read_text(encoding="utf-8").rstrip("\n").split("\n")
        if self.strip_comments:
            lines = [line for line in lines if not line.lstrip().startswith("#")]
            while lines and not lines[0].strip():
                lines.pop(0)
        first = 0
        if self.start:
            try:
                first = next(i for i, ln in enumerate(lines) if self.start in ln)
            except StopIteration:
                raise LookupError(
                    f"{self.path.name}: no line contains the start anchor "
                    f"{self.start!r}"
                ) from None
        last = len(lines) - 1
        if self.end:
            try:
                last = next(
                    i
                    for i, ln in enumerate(lines)
                    if i >= first and self._matches_end(ln)
                )
            except StopIteration:
                raise LookupError(
                    f"{self.path.name}: no line at or after the start anchor "
                    f"matches the end anchor {self.end!r}"
                ) from None
        chunk = lines[first : last + 1]
        if self.dedent:
            indents = [len(ln) - len(ln.lstrip()) for ln in chunk if ln.strip()]
            trim = min(indents) if indents else 0
            chunk = [ln[trim:] if ln.strip() else "" for ln in chunk]
        return "\n".join(chunk)


@dataclass
class Slide:
    eyebrow: str
    title: str
    lede: str = ""
    excerpt: Excerpt | None = None
    cmd: list[str] = field(default_factory=list)
    out: str = ""
    points: list[str] = field(default_factory=list)
    bites: list[str] = field(default_factory=list)
    table: list[tuple[str, str]] = field(default_factory=list)
    note: str = ""
    caption: str = ""
    stamp: str = ""
    big: bool = False


def code(text: str) -> str:
    return f"<code>{html.escape(text)}</code>"


# --------------------------------------------------------------------------
# The deck
# --------------------------------------------------------------------------

SLIDES: list[Slide] = [
    Slide(
        eyebrow="Aion — from nothing",
        title="Your first workflow, start to finish",
        lede="Eight steps. No Rust, no Gleam, no compiler. "
        "At the end you have a workflow that ran a real command on your "
        "machine and kept the answer forever.",
        big=True,
        caption="Every command and every file in this deck was run end to end "
        "before it was written down.",
    ),
    Slide(
        eyebrow="What you end up with",
        title="A run you can point at",
        points=[
            "A workflow you wrote yourself, in a file you can read.",
            "A worker that ran a real command — no code, just a wiring file.",
            "A completed run, with its whole history on disk.",
            "All of it visible in the console in your browser.",
        ],
        note="If you can do this, you can do the big version. "
        "The big version is the same five pieces, more times.",
    ),
    Slide(
        eyebrow="Before you start",
        title="One binary. Check it is there.",
        cmd=["aion --version"],
        out="aion 0.11.0",
        points=[
            "That one binary is the server, the worker, and the tools.",
            "Nothing else to install.",
        ],
    ),
    # ---------------------------------------------------------------- step 1
    Slide(
        eyebrow="Step 1 of 8",
        title="Pick a folder and write the config",
        lede="One file tells the server which ports to use and where to keep "
        "its data.",
        excerpt=Excerpt("aion.toml", strip_comments=True),
        note="🔴 <b>Use a full path for <code>data_dir</code>.</b> A relative "
        "path is resolved against whatever folder you happened to start the "
        "server from — start it from somewhere else and it looks like every "
        "run you ever did has vanished. Nothing is lost; you are just looking "
        "at a different store.",
    ),
    Slide(
        eyebrow="Step 1 of 8",
        title="Start it",
        cmd=[
            "cd ~/my-first-flow",
            "aion server --config aion.toml",
        ],
        points=[
            "Leave it running. Open a second terminal for everything else.",
            "The console is now at <code>http://127.0.0.1:8099</code>.",
        ],
        note="If it refuses to start saying an ancestor <i>is not "
        "owner-controlled</i>, your folder is somewhere the whole machine can "
        "write to — <code>/tmp</code>, typically. Move it under your home "
        "folder. The refusal is the store protecting itself, and it names the "
        "exact folder at fault.",
    ),
    Slide(
        eyebrow="Step 1 of 8",
        title="Where things land",
        table=[
            (
                "your workflows",
                "<code>$AION_HOME/authoring/</code> — the server prints this "
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
        "is that you are looking in the other folder.",
    ),
    # ---------------------------------------------------------------- step 2
    Slide(
        eyebrow="Step 2 of 8",
        title="Make a new workflow",
        lede="Open the console, go to <b>Authoring</b>. At the top of the "
        "document list there is a box marked <b>New workflow</b>.",
        points=[
            "Type a name. Press <b>+</b>.",
            "The name must be lowercase letters, digits and underscores, "
            "starting with a letter or underscore — <code>order_flow</code>, "
            "not <code>Order Flow</code>.",
        ],
        note="The name rule is not shown until you break it. Now you know it "
        "up front.",
    ),
    Slide(
        eyebrow="Step 2 of 8",
        title="What you get",
        lede="Not a blank page — a document that already passes the checker.",
        excerpt=Excerpt("00-skeleton.awl"),
        caption="This is what the server writes for a workflow named "
        "<code>greet_flow</code>. It is parsed and reprinted before you ever "
        "see it, so it cannot hand you something the checker would reject.",
    ),
    # ---------------------------------------------------------------- step 3
    Slide(
        eyebrow="Step 3 of 8",
        title="Now write the real thing",
        lede="Replace the skeleton with this. It is the whole workflow.",
        excerpt=Excerpt("first_flow.awl"),
        caption="A complete document — this exact file was checked, deployed "
        "and run.",
    ),
    Slide(
        eyebrow="Step 3 of 8",
        title="Line by line",
        table=[
            ("<code>workflow</code>", "its name — how you start it later"),
            ("<code>input</code>", "what you have to hand it to start"),
            (
                "<code>outcome</code>",
                "how it is allowed to end, and what it hands back",
            ),
            ("<code>type</code>", "the shape of a piece of data"),
            (
                "<code>worker</code>",
                "a name for the thing that does real work — the queue",
            ),
            (
                "<code>action</code>",
                "one job the worker can do, with its types and its time limit",
            ),
            ("<code>step</code>", "a stage of the workflow"),
            ("<code>route</code>", "finish, this way, with this"),
        ],
    ),
    Slide(
        eyebrow="Step 4 of 8",
        title="Check it",
        cmd=["aion awl check first_flow.awl"],
        out="ok: first_flow.awl (1 step)",
        points=[
            "Errors come out like a compiler's: file, line, column, reason.",
            "Nothing has been deployed yet. This is free and instant.",
        ],
    ),
    # ---------------------------------------------------------------- worker
    Slide(
        eyebrow="Step 5 of 8",
        title="The worker — what it actually is",
        lede="The workflow decides <i>what</i> should happen and in what "
        "order. It never does the work itself.",
        points=[
            "The worker is a separate process that does the real jobs.",
            "That separation is the whole trick: the workflow can be replayed "
            "safely because it only ever decides.",
            "For this tutorial the worker is the <code>aion</code> binary you "
            "already have. You write no code at all.",
        ],
    ),
    Slide(
        eyebrow="Step 5 of 8",
        title="Wire the action to a command",
        lede="One small file. It says which queue to serve and what each "
        "action runs.",
        excerpt=Excerpt("worker.toml", strip_comments=True),
        caption="Wiring only — the types, the timeout and the retries all "
        "stay in the <code>.awl</code>.",
    ),
    Slide(
        eyebrow="Step 5 of 8",
        title="The rule that will bite you",
        lede="Keep one document per worker queue, and let that document "
        "declare every action on it.",
        points=[
            "A worker is only let in if it offers the <b>full</b> set of "
            "actions that <i>every</i> deployed document declares for its "
            "queue.",
            "So a second workflow reusing the same <code>worker</code> name "
            "with a different action locks the queue for any single-document "
            "worker.",
            "A worker already running keeps working — it breaks the next time "
            "you restart it, which is what makes it so confusing.",
        ],
        note="🔴 This fails <b>silently on both sides</b>: the worker prints "
        "nothing and stays alive, and the server logs no warning and no "
        "error. Measured on 0.11.0 — see "
        "<code>SHARED-QUEUE-SILENT-REFUSAL-2026-08-06.md</code>. Until it is "
        "fixed, the rule above is the way round it.",
    ),
    # ---------------------------------------------------------------- deploy
    Slide(
        eyebrow="Step 6 of 8",
        title="Deploy",
        cmd=[
            "aion --endpoint http://127.0.0.1:50099 \\",
            "     deploy first_flow.awl",
        ],
        out='{"workflow_type": "first_flow",\n'
        ' "content_hash": "629b5141…",\n'
        ' "freshly_loaded": true,\n'
        ' "route_changed": true}',
        points=[
            "The hash is the version's real name. Deploy it again unchanged "
            "and you get the same hash back.",
            "<code>route_changed: true</code> means new runs will use this "
            "one. Runs already going keep the version they started on.",
        ],
    ),
    # ---------------------------------------------------------------- worker up
    Slide(
        eyebrow="Step 7 of 8",
        title="Start the worker",
        cmd=[
            "aion --endpoint http://127.0.0.1:50099 worker shell \\",
            "     --manifest worker.toml --awl first_flow.awl",
        ],
        points=[
            "Leave it running too. A third terminal.",
            "It needs the <code>.awl</code> as well as the manifest — the "
            "types come from the document, and only from there.",
        ],
        note="🔴 The <code>http://</code> is not optional here. Leave the "
        "scheme off and this worker never connects, and never says so.",
    ),
    Slide(
        eyebrow="Step 7 of 8",
        title="Silence is not success",
        lede="A working shell worker prints nothing at all. So does a broken "
        "one. Ask the server instead.",
        cmd=[
            "curl -s -X POST http://127.0.0.1:8099/awl/workers/availability \\",
            "  -H 'Content-Type: application/json' \\",
            '  -d \'{"namespace":"default","task_queue":"desk",\'\\',
            '     \'"activity_types":["list_folder"]}\'',
        ],
        out='{"available": true, "task_queue": "desk", '
        '"connected_workers": 1}',
        note="<code>connected_workers</code> is the number that matters. If "
        "it is <code>0</code> while your worker process is plainly alive, you "
        "have hit the queue rule two slides back.",
    ),
    # ---------------------------------------------------------------- run
    Slide(
        eyebrow="Step 8 of 8",
        title="Run it",
        cmd=[
            "aion --endpoint http://127.0.0.1:50099 --pretty \\",
            "     run first_flow.awl --input '{\"folder\": \"/Users/you\"}'",
        ],
        points=[
            "<code>run</code> is the one-motion verb: compile, deploy, start, "
            "wait for the answer.",
            "Day to day you would use <code>start</code> and let it go.",
        ],
    ),
    Slide(
        eyebrow="Step 8 of 8",
        title="What comes back",
        out='{\n'
        '  "workflow_type": "first_flow",\n'
        '  "workflow_id": "e623051b-6901-4d91-87b4-3c6a2024c37c",\n'
        '  "run_id": "700da026-ac14-4955-a93e-d24bcbfc6c8e",\n'
        '  "result": {\n'
        '    "outcome": "done",\n'
        '    "payload": {\n'
        '      "found": "aion-data\\naion.toml\\nfirst_flow.awl\\n'
        'worker.toml"\n'
        "    }\n"
        "  }\n"
        "}",
        caption="The real output of this tutorial's own run. A command ran on "
        "the machine and its answer came back through the workflow.",
    ),
    Slide(
        eyebrow="Look at it",
        title="In the console",
        points=[
            "<b>Workflows</b> lists the run: type, status, when it started "
            "and ended.",
            "Open it for the history — every event, in order, exactly as it "
            "was recorded.",
            "That history is the run. It is not a log <i>about</i> the run.",
        ],
    ),
    Slide(
        eyebrow="What just happened",
        title="Why any of this is worth the trouble",
        lede="The workflow never did the work. It decided, and every decision "
        "and every answer was written down before moving on.",
        points=[
            "Kill the server mid-run and start it again: it picks up from the "
            "recorded history rather than starting over.",
            "The command that already ran is not run a second time — its "
            "recorded answer is handed back instead.",
            "That is the whole point of Aion. Everything else is detail.",
        ],
        note="Worth doing next, on your own, once this run works: start "
        "something slow, <code>kill -9</code> the server halfway, start it "
        "again, and watch. That exercise is not part of this tutorial's "
        "proven path — try it and see for yourself.",
    ),
    # ---------------------------------------------------------------- rescue
    Slide(
        eyebrow="When it goes wrong",
        title="Three silences, and what they mean",
        table=[
            (
                "worker prints nothing",
                "normal — it always does. Use the availability probe, never "
                "the absence of output.",
            ),
            (
                "<code>connected_workers: 0</code><br>but the process is alive",
                "the queue rule: two documents declare that queue with "
                "different actions. Merge them into one document.",
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
        eyebrow="Where next",
        title="You have the shape of it now",
        points=[
            "The AWL deck covers the rest of the language — waits, signals, "
            "children, parallel work, failure handling.",
            "The same five pieces scale up: document, worker, deploy, run, "
            "watch.",
            "A worker that needs real code instead of a command is "
            "<code>aion awl scaffold</code> — it writes the crate for you.",
        ],
        big=False,
    ),
]


# --------------------------------------------------------------------------
# Gate
# --------------------------------------------------------------------------


def _binary() -> str:
    found = shutil.which("aion")
    if found is None:
        raise SystemExit(
            "refusing to build: `aion` is not on PATH, so nothing in this "
            "deck can be checked. Install it with "
            "`cargo install --path crates/aion-cli`."
        )
    return found


def gate_documents(binary: str, documents: list[Path]) -> None:
    """Refuse to write unless every example document checks."""
    failed: list[str] = []
    for path in documents:
        result = subprocess.run(
            [binary, "awl", "check", str(path)],
            capture_output=True,
            text=True,
            check=False,
        )
        print(f"  {'ok' if result.returncode == 0 else 'FAILED':>6}  {path.name}")
        if result.returncode != 0:
            failed.append(f"{path.name}: {(result.stdout + result.stderr).strip()}")
    if failed:
        raise SystemExit(
            "refusing to write the deck — these documents do not check:\n  "
            + "\n  ".join(failed)
        )


def declared_actions(document: Path) -> dict[str, set[str]]:
    """Queue name -> declared action names, read off the document text.

    Deliberately literal: the deck teaches what the file says, so the gate
    reads what the file says.
    """
    queues: dict[str, set[str]] = {}
    current: str | None = None
    for raw in document.read_text(encoding="utf-8").split("\n"):
        line = raw.strip()
        if line.startswith("worker ") and not raw.startswith(" "):
            current = line.split()[1]
            queues.setdefault(current, set())
        elif line.startswith(("step ", "workflow ", "type ")) and not raw.startswith(
            " "
        ):
            current = None
        elif current is not None and line.startswith("action "):
            queues[current].add(line[len("action ") :].split("(")[0].strip())
    return queues


def gate_manifest(manifest: Path, document: Path) -> None:
    """Refuse a manifest the runtime would refuse — including, silently.

    Encodes the 2026-08-06 finding: a worker must advertise its queue's FULL
    declared action set or registration is refused with no output on either
    side. A deck that taught a partial manifest would be teaching a shape that
    looks fine locally and never connects.
    """
    wiring = tomllib.loads(manifest.read_text(encoding="utf-8"))
    queue = wiring["worker"]["task_queue"]
    wired = {entry["name"] for entry in wiring["action"]}
    queues = declared_actions(document)
    if queue not in queues:
        raise SystemExit(
            f"refusing to write the deck — {manifest.name} serves queue "
            f"`{queue}`, which {document.name} does not declare. It declares: "
            + (", ".join(sorted(queues)) or "no queues")
        )
    declared = queues[queue]
    if wired != declared:
        raise SystemExit(
            f"refusing to write the deck — {manifest.name} wires "
            f"{sorted(wired)} on queue `{queue}` but {document.name} declares "
            f"{sorted(declared)}. A worker advertising less than its queue's "
            "full action set is refused at registration, silently on both "
            "sides."
        )
    print(f"  {'ok':>6}  {manifest.name} wires all {len(declared)} of `{queue}`")


def gate_verbs(binary: str) -> None:
    """Refuse to teach a command this binary does not have."""
    missing: list[str] = []
    for verb in TAUGHT_VERBS:
        result = subprocess.run(
            [binary, *verb], capture_output=True, text=True, check=False
        )
        if result.returncode != 0:
            missing.append(" ".join(verb))
    if missing:
        raise SystemExit(
            "refusing to write the deck — this binary does not answer:\n  "
            + "\n  ".join(missing)
        )
    print(f"  {'ok':>6}  all {len(TAUGHT_VERBS)} taught verbs resolve")


# --------------------------------------------------------------------------
# Render
# --------------------------------------------------------------------------


def render(slide: Slide, path_of: dict[str, str]) -> str:
    parts = [f'    <p class="eyebrow">{slide.eyebrow}</p>']
    heading = "h1" if slide.big else "h2"
    parts.append(f"    <{heading}>{slide.title}</{heading}>")
    if slide.lede:
        parts.append(f'    <p class="lede">{slide.lede}</p>')
    if slide.stamp:
        parts.append(f'    <span class="stamp">{slide.stamp}</span>')
    if slide.cmd:
        body = html.escape("\n".join(slide.cmd))
        parts.append(f'    <pre class="cmd"><code>{body}</code></pre>')
    if slide.excerpt is not None:
        body = html.escape(slide.excerpt.text())
        parts.append(f"    <pre><code>{body}</code></pre>")
        parts.append(
            f'    <p class="src">from <b>{path_of[slide.excerpt.document]}</b>'
            " — a complete file, checked</p>"
        )
    if slide.out:
        parts.append(f'    <pre class="out"><code>{html.escape(slide.out)}</code></pre>')
    if slide.points:
        parts.append('    <ul class="points">')
        parts.extend(f"      <li>{point}</li>" for point in slide.points)
        parts.append("    </ul>")
    if slide.bites:
        parts.append('    <ul class="points bite">')
        parts.extend(f"      <li>{bite}</li>" for bite in slide.bites)
        parts.append("    </ul>")
    if slide.table:
        parts.append('    <div class="scroller"><table class="kw">')
        for term, meaning in slide.table:
            parts.append(f"      <tr><td>{term}</td><td>{meaning}</td></tr>")
        parts.append("    </table></div>")
    if slide.note:
        parts.append(f'    <p class="note">{slide.note}</p>')
    if slide.caption:
        parts.append(f'    <p class="caption">{slide.caption}</p>')
    return '  <section class="slide">\n' + "\n".join(parts) + "\n  </section>"


def main() -> int:
    binary = _binary()
    version = subprocess.run(
        [binary, "--version"], capture_output=True, text=True, check=False
    ).stdout.strip()
    print(f"gating the tutorial with {version} ({binary})")

    referenced = {s.excerpt.document for s in SLIDES if s.excerpt is not None}
    missing = [name for name in sorted(referenced) if not (EXAMPLES / name).is_file()]
    if missing:
        raise SystemExit(
            "refusing to build — these referenced files do not exist: "
            + ", ".join(missing)
        )

    documents = sorted(EXAMPLES.glob("*.awl"))
    if not documents:
        raise SystemExit(f"refusing to build — no documents found in {EXAMPLES}")
    gate_documents(binary, documents)
    gate_manifest(EXAMPLES / "worker.toml", EXAMPLES / "first_flow.awl")
    gate_verbs(binary)

    root = HERE.parent.parent
    path_of = {
        path.name: str(path.relative_to(root))
        for path in sorted(EXAMPLES.iterdir())
        if path.is_file()
    }

    shell = SHELL.read_text(encoding="utf-8")
    if "<!--SLIDES-->" not in shell:
        raise SystemExit(f"{SHELL.name} has no <!--SLIDES--> placeholder")
    if SHELL_TITLE not in shell:
        raise SystemExit(
            f"{SHELL.name} no longer contains the title this deck replaces "
            f"({SHELL_TITLE!r}) — it would be published under the AWL deck's "
            "name. Update SHELL_TITLE."
        )
    rendered = "\n\n".join(render(slide, path_of) for slide in SLIDES)
    page = shell.replace(SHELL_TITLE, DECK_TITLE).replace("<!--SLIDES-->", rendered)
    OUTPUT.write_text(page, encoding="utf-8")

    quoted = len([s for s in SLIDES if s.excerpt is not None])
    print(
        f"wrote {OUTPUT.relative_to(root)} — {len(SLIDES)} slides, "
        f"{quoted} excerpts from {len(referenced)} of {len(path_of)} example "
        f"files, {len(documents)} documents checked"
    )
    unquoted = sorted(set(path_of) - referenced)
    if unquoted:
        print("  not quoted by any slide: " + ", ".join(unquoted))
    return 0


if __name__ == "__main__":
    sys.exit(main())
