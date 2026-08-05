#!/usr/bin/env python3
"""Generate awl-deck.html from the checked example documents.

The deck teaches AWL keyword by keyword. Every snippet on a slide is SLICED
out of a complete document under deck-examples/ rather than retyped here, so
the deck cannot claim a shape the language does not actually accept: if an
example stops checking, this script refuses to write the deck at all.

    python3 docs/authoring/build-deck.py

Exit status is 0 only when every referenced document passed `aion awl check`
and the deck was written.
"""

from __future__ import annotations

import html
import shutil
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

HERE = Path(__file__).resolve().parent
EXAMPLES = HERE / "deck-examples"
SHELL = HERE / "deck-shell.html"
OUTPUT = HERE / "awl-deck.html"


# --------------------------------------------------------------------------
# Excerpts
# --------------------------------------------------------------------------


@dataclass(frozen=True)
class Excerpt:
    """A contiguous run of lines from one checked document.

    Anchored on substrings rather than line numbers so that editing an example
    does not silently shift what a slide shows. A missing anchor is a hard
    error, not a truncated slide.
    """

    document: str
    start: str
    end: str
    dedent: bool = True
    # A closing brace or a closing `"""` is a substring of half the file, so an
    # anchor may instead demand the whole (stripped) line.
    end_exact: bool = False

    @property
    def path(self) -> Path:
        return EXAMPLES / f"{self.document}.awl"

    def _matches_end(self, line: str) -> bool:
        return line.strip() == self.end if self.end_exact else self.end in line

    def text(self) -> str:
        lines = self.path.read_text(encoding="utf-8").split("\n")
        try:
            first = next(i for i, line in enumerate(lines) if self.start in line)
        except StopIteration:
            raise LookupError(
                f"{self.path.name}: no line contains the start anchor {self.start!r}"
            ) from None
        try:
            last = next(
                i
                for i, line in enumerate(lines)
                if i >= first and self._matches_end(line)
            )
        except StopIteration:
            raise LookupError(
                f"{self.path.name}: no line at or after the start anchor matches "
                f"the end anchor {self.end!r}"
            ) from None
        chunk = lines[first : last + 1]
        if self.dedent:
            indents = [
                len(line) - len(line.lstrip()) for line in chunk if line.strip()
            ]
            trim = min(indents) if indents else 0
            chunk = [line[trim:] if line.strip() else "" for line in chunk]
        return "\n".join(chunk)


@dataclass
class Slide:
    eyebrow: str
    title: str
    lede: str = ""
    form: str = ""  # raw HTML: use <span class="slot"> / <span class="opt">
    excerpt: Excerpt | None = None
    points: list[str] = field(default_factory=list)
    bites: list[str] = field(default_factory=list)
    note: str = ""
    caption: str = ""
    stamp: str = ""
    table: list[tuple[str, str]] = field(default_factory=list)
    big: bool = False


def slot(text: str) -> str:
    return f'<span class="slot">{html.escape(text)}</span>'


def opt(text: str) -> str:
    return f'<span class="opt">{html.escape(text)}</span>'


# --------------------------------------------------------------------------
# The deck
# --------------------------------------------------------------------------

SLIDES: list[Slide] = [
    Slide(
        big=True,
        eyebrow="Aion · the AWL language",
        title="AWL",
        lede=(
            "A walkthrough of the syntax, one keyword at a time: what you type, "
            "what it means, and the rule that stops you when you get it wrong."
        ),
        stamp="every snippet is sliced from a document that passes aion awl check",
        caption=(
            "The twelve documents these slides quote live in "
            "<b>docs/authoring/deck-examples/</b>. This file is generated from them by "
            "<b>build-deck.py</b>, which refuses to write the deck if any of them stops "
            "checking — so a claim on a slide cannot outlive the code that made it true."
        ),
    ),
    Slide(
        eyebrow="The file",
        title="One file declares one workflow",
        form=(
            "//! narration          " + opt("← required, and first") + "\n"
            "workflow " + slot("name") + "\n"
            "  " + slot("header declarations") + "\n"
            "\n"
            "const / type / worker  " + opt("← what the flow is made of") + "\n"
            "step " + slot("name") + "               " + opt("← the flow itself")
        ),
        excerpt=Excerpt("01-basics", "//! Types", "outcome failed"),
        points=[
            "<code>//!</code> narrates the workflow. It must be the first thing in the file.",
            "<code>///</code> documents the declaration below it — a type, a field, an action, a step.",
            "<code>//</code> is an ordinary comment, and the only kind allowed inside a step body.",
            "Identifiers are <code>snake_case</code>. Type names and enum variants are <code>TitleCase</code>.",
            "Two-space indentation. It is significant.",
        ],
    ),
    Slide(
        eyebrow="workflow",
        title="The header is the contract",
        form=(
            "workflow " + slot("name") + "\n"
            "  timeout " + slot("2h") + "            " + opt("optional") + "\n"
            "  input   " + slot("name: Type") + "\n"
            "  signal  " + slot("name: Type") + "     " + opt("what it can be sent") + "\n"
            "  send    " + slot("name: Type") + "     " + opt("what it can send") + "\n"
            "  query   " + slot("name: Type") + "     " + opt("what it can be asked") + "\n"
            "  outcome " + slot("name") + ": type " + slot("T") + ", route success|failure"
        ),
        excerpt=Excerpt("11-signals-queries", "workflow gated_item", "outcome gave_up"),
        points=[
            "<strong>Inputs</strong> are what a caller supplies when it starts the run.",
            "<strong>Outcomes are the only way out.</strong> Each one names the type of its payload and whether it lands as a success or a failure.",
        ],
        bites=[
            "A workflow needs at least one terminal outcome, and must not fall off the end of its last step.",
        ],
    ),
    Slide(
        eyebrow="type",
        title="Records, enums, and absence",
        form=(
            "type " + slot("Name") + " { " + slot("field") + ": " + slot("Type")
            + ", " + slot("other") + ": " + slot("Type") + "? }\n"
            "type " + slot("Name") + " = " + slot("Variant") + " | " + slot("Variant")
        ),
        excerpt=Excerpt("06-steps-outcomes", "type Summary", "type Category"),
        points=[
            "Built-ins: <code>Bool</code>, <code>Int</code>, <code>Float</code>, <code>String</code>, <code>Nil</code>, <code>Dir</code>, lists <code>[T]</code>, optionals <code>T?</code>.",
            "<strong>Optional means absent</strong> — the field simply is not there. A JSON <code>null</code> is invalid, not empty.",
            "Enum variants are written bare in expressions: <code>report.category == Urgent</code>.",
        ],
        bites=[
            "<code>[T?]</code> cannot be declared. It exists in exactly one place in the language — a tolerant <code>collect</code>. <code>[T]?</code> is different: the whole list may be absent.",
        ],
    ),
    Slide(
        eyebrow="schema",
        title="A JSON Schema becomes a checked type",
        form=(
            "type " + slot("Name") + " = schema(\"" + slot("path.schema.json") + "\")\n"
            "type " + slot("Name") + " = schema { " + slot("inline JSON Schema") + " }"
        ),
        excerpt=Excerpt("12-schema-doors", "/// An inline schema", "}", end_exact=True),
        points=[
            "Import paths resolve relative to the <code>.awl</code> document.",
            "This is how AWL meets a shape somebody else already defined, without retyping it.",
        ],
        bites=[
            "<code>oneOf</code>, <code>anyOf</code>, <code>patternProperties</code> and an external <code>$ref</code> are each refused <em>by name</em>, with their own diagnostic. A <code>$ref</code> into the schema's own <code>$defs</code> is supported.",
            "A null-admitting type is a check error — absence is <code>T?</code>, not <code>null</code>.",
        ],
    ),
    Slide(
        eyebrow="const",
        title="Compile-time values, and only those",
        form="const " + slot("name") + " = " + slot("value"),
        excerpt=Excerpt("12-schema-doors", "/// `schema of Type`", '"""', end_exact=True),
        points=[
            "Literals, lists, <code>json { … }</code> blocks, <code>schema of Type</code>, and concatenation of constants.",
            "Triple-quoted strings carry multi-line text — prompts, briefs, instructions.",
        ],
        bites=[
            "A constant cannot depend on a runtime binding.",
            "<code>schema of Type</code> is a <strong>constant-position form only</strong>. Written inside a step body it checks clean and then does not lower — bind it to a <code>const</code> and use the constant.",
        ],
    ),
    Slide(
        eyebrow="worker · action",
        title="Where the work actually happens",
        form=(
            "worker " + slot("task_queue") + "\n"
            "  action " + slot("name") + "(" + slot("param") + ": " + slot("Type")
            + ") -> " + slot("ResultType")
        ),
        excerpt=Excerpt("02-workers", "worker dev", "action normalize"),
        points=[
            "The <code>worker</code> name is the <strong>task queue</strong> the call is dispatched to.",
            "An <code>action</code> declares the contract. Both sides check it — the workflow when it calls, the worker when it registers.",
            "Arguments are always named at the call site. There is no positional call.",
        ],
    ),
    Slide(
        eyebrow="the config line",
        title="Operational reality, in the language",
        form=(
            "  action " + slot("name") + "(…) -> " + slot("T") + "\n"
            "    node " + slot("N") + ", timeout " + slot("D")
            + ", retry " + slot("N") + " every " + slot("D") + "\n"
            "    " + opt("… or retry N backoff MIN..MAX, or advisory, or agent")
        ),
        excerpt=Excerpt("02-workers", "/// The config line", "advisory"),
        points=[
            "At most <strong>one</strong> config line per action, comma-separated.",
            "A call site may override <code>node</code> and <code>timeout</code> — nothing else.",
            "<code>advisory</code> means a failure never faults the step. Because it might not have run, an advisory call <strong>cannot bind a result</strong>.",
        ],
    ),
    Slide(
        eyebrow="agent",
        title="An agent is ordinary work",
        form="    agent" + opt("   — this parameter is a prompt, this result is the reply"),
        excerpt=Excerpt("03-agent-seam", "worker assistants", "agent, timeout"),
        points=[
            "One <code>String</code> parameter goes out as the prompt; one <code>String</code> comes back as the reply.",
            "Everything else about it is a normal action — same retries, same timeout, same recording in history.",
        ],
        bites=[
            "The checker refuses any other shape. <code>agent</code> is not a label you can put on an arbitrary action.",
        ],
    ),
    Slide(
        eyebrow="run",
        title="The document can carry the command",
        form='    run "' + slot("command $param") + '"',
        excerpt=Excerpt("04-declared-command", "worker greeter", "run \"printf"),
        points=[
            "A declared body needs <strong>no worker process</strong> — the server runs it.",
            "The result type is a subset of <code>{ exit_code: Int, stdout: String, stderr: String }</code>: ask for the fields you need.",
            "Interpolation reaches the action's own parameters only, as <code>$name</code> or <code>${name}</code>.",
        ],
        bites=[
            "<strong>There is no sandbox.</strong> A declared body runs with the server's authority, on the server's machine.",
        ],
    ),
    Slide(
        eyebrow="child · spawn",
        title="A child is its own durable workflow",
        form=(
            "child " + slot("name") + "(" + slot("p") + ": " + slot("T") + ") -> "
            + slot("R") + "\n\n"
            "  " + slot("name") + "(" + slot("p") + ": " + slot("v") + ") -> "
            + slot("binding") + "   " + opt("waits for it") + "\n"
            "  spawn " + slot("name") + "(" + slot("p") + ": " + slot("v") + ")     "
            + opt("starts it and moves on")
        ),
        excerpt=Excerpt("05-children-subflows", "/// A child waits", "route third"),
        points=[
            "A child has its own history, its own id, and its own lifetime. It survives the parent finishing.",
            "<code>spawn</code> is fire-and-continue: no result, no wait.",
        ],
        bites=[
            "Cancelling a parent does not cascade to its children. That is the stated contract, not an oversight.",
        ],
    ),
    Slide(
        eyebrow="subflow",
        title="A subflow is an inline graph",
        form=(
            "subflow " + slot("name") + "(" + slot("p") + ": " + slot("T") + ")\n"
            "  outcome " + slot("out") + ": type " + slot("R") + "\n"
            "  step " + slot("…")
        ),
        excerpt=Excerpt("05-children-subflows", "subflow review_once", "route done(title"),
        points=[
            "Same history, same run — a subflow is a way to name a piece of the graph, not a separate execution.",
        ],
        bites=[
            "A subflow invocation must be the <strong>only statement of its step</strong>. Bind it with <code>-&gt;</code>, then route from the next step.",
        ],
    ),
    Slide(
        eyebrow="step · after",
        title="Steps, and what waits for what",
        form=(
            "step " + slot("name") + "\n"
            "step " + slot("name") + " after " + slot("other") + ", " + slot("another")
        ),
        excerpt=Excerpt("06-steps-outcomes", "step prepare", "route sort_it"),
        points=[
            "Statements inside a step run in written order.",
            "A step with no <code>after</code> and no incoming route falls through from the step written before it.",
            "<code>after</code> makes the dependency explicit — the step starts once all the named steps are done.",
        ],
        bites=[
            "The checker rejects unknown targets, cycles through <code>after</code>, unreachable steps, and any binding that is not available on <em>every</em> path reaching its use.",
        ],
    ),
    Slide(
        eyebrow="-&gt; and |&gt;",
        title="Bind an answer, or pipe a value",
        form=(
            slot("call") + "(…) -> " + slot("binding") + "\n"
            + slot("value") + " |> " + slot("stage") + " |> ." + slot("field")
            + " |> " + slot("action") + " -> " + slot("binding")
        ),
        excerpt=Excerpt("02-workers", "step piped", "route done(artifact"),
        points=[
            "A pipe stage receives the piped value as its <strong>single argument</strong>, so it must be declared with exactly one parameter.",
            "<code>.field</code> is a stage too — pull a field out mid-pipe.",
            "A plain expression binds with the same arrow: <code>\"run: \" + result -&gt; summary</code>.",
        ],
        bites=[
            "A multi-parameter action, a child, and an advisory action can none of them be a pipe stage.",
        ],
    ),
    Slide(
        eyebrow="expressions · guards",
        title="What you can say without a worker",
        form=(
            slot("path") + " is present   " + slot("path") + " is absent   "
            + slot("list") + " is empty\n"
            + slot("list") + " |> filter · map · sort · count · any · all"
        ),
        excerpt=Excerpt("06-steps-outcomes", "step file_it", 'route done(text: "no note")'),
        points=[
            "Field access, literal indexing (<code>items[0]</code>), string <code>+</code>, comparisons, <code>not</code>/<code>and</code>/<code>or</code>, and the collection operations.",
            "<strong>No user functions and no general arithmetic.</strong> Computation belongs in a worker action, where it can be recorded.",
            "A guard that proves a path present <strong>narrows it from <code>T?</code> to <code>T</code></strong> for the whole arm — including the route payload.",
        ],
        bites=[
            "Proofs never escape their arm and never flow backwards. An optional read without a proof is a check error naming the path.",
        ],
    ),
    Slide(
        eyebrow="outcome · when · otherwise · route",
        title="Branching is a set of outcomes",
        form=(
            "  outcome " + slot("name") + ": when " + slot("condition") + ", route "
            + slot("target") + "\n"
            "  outcome " + slot("name") + ": otherwise, route " + slot("target")
        ),
        excerpt=Excerpt("06-steps-outcomes", "step triage", "outcome drop"),
        points=[
            "There is no <code>select</code> and no <code>if</code>. The first true <code>when</code> wins.",
            "A <code>route</code> names another step, or a workflow or subflow outcome.",
            "A step containing only outcomes is a pure decision.",
        ],
        bites=[
            "<code>otherwise</code> is the complement and must be last. Conditional outcomes must be <strong>exhaustive</strong>.",
            "One proved exception: if every arm compares the <em>same subject</em> against a bare enum variant and every variant is covered, the set is total on its own. An uncovered variant is an error that names it.",
        ],
    ),
    Slide(
        eyebrow="fork · join",
        title="One call per item, in parallel",
        form=(
            "fork " + slot("item") + " in " + slot("items") + " " + opt("[sequential]") + "\n"
            "  " + slot("one unbound call") + "\n"
            "join -> " + slot("results") + "\n\n"
            "fork                      " + opt("bare: named branches") + "\n"
            "  " + slot("a(…)") + " -> " + slot("x") + "\n"
            "  " + slot("b(…)") + " -> " + slot("y") + "\n"
            "join"
        ),
        excerpt=Excerpt("07-fork-join", "step fan", "join -> receipts"),
        points=[
            "The collection form runs one branch per item and joins the results <strong>in input order</strong>, however they finish.",
            "<code>sequential</code> runs them one at a time, still in input order.",
            "A bare <code>fork</code> opens named branches: different calls, run concurrently, each binding its own result.",
        ],
        bites=[
            "The collection body compiles only as <strong>exactly one unbound call</strong> — an action or a child. Named branches compile only as action calls.",
        ],
    ),
    Slide(
        eyebrow="distribute · sequence · collect",
        title="When each item needs a graph, not a call",
        form=(
            "step " + slot("open") + "\n"
            "  distribute " + slot("item") + " in " + slot("items") + "   "
            + opt("or: sequence") + "\n\n"
            "step " + slot("work") + "\n"
            "  " + slot("…several steps per item…") + "\n\n"
            "step " + slot("close") + "\n"
            "  collect " + slot("binding") + " -> " + slot("results")
        ),
        excerpt=Excerpt("08-regions", "step open", "count_all(reports"),
        points=[
            "<code>distribute</code> runs the item tracks in parallel; <code>sequence</code> runs them one at a time in collection order.",
            "Results preserve input order. An empty input collects to <code>[]</code>.",
            "Regions may nest, but an inner region must close before its outer one, and a route may not enter or escape an open region.",
        ],
        bites=[
            "The opener's region operation must be its <strong>only</strong> statement, and <code>collect</code> must be the <strong>first</strong> statement of the closing step.",
        ],
    ),
    Slide(
        eyebrow="collect ?",
        title="Tolerating the item that failed",
        form=(
            "  collect " + slot("binding") + "  -> " + slot("results") + "    "
            + opt("strict: one failure fails the run") + "\n"
            "  collect " + slot("binding") + "? -> " + slot("results") + "    "
            + opt("tolerant: [T?], one slot per item")
        ),
        excerpt=Excerpt("08-regions", "step close_again", "route done(count"),
        points=[
            "Tolerant <code>collect</code> is the one place in the language where <code>[T?]</code> exists. One optional slot per input item, in item order.",
            "The failure detail is not discarded — it stays in history.",
        ],
        bites=[
            "Every slot is an ordinary optional. It must be proved present before its fields are read, exactly like any other <code>T?</code>.",
        ],
    ),
    Slide(
        eyebrow="loop · until · max",
        title="Threading one value through a bounded loop",
        form=(
            "  loop " + slot("binding") + " = " + slot("initial") + " "
            + opt("[counting n]") + "\n"
            "    " + slot("…body, which must rebind `binding`…") + "\n"
            "    until " + slot("condition") + "\n"
            "    max " + slot("N")
        ),
        excerpt=Excerpt("09-loops", "step polish", 'route failed(reason: "exhausted'),
        points=[
            "The body runs at least once, and must rebind the one threaded value.",
            "<code>counting</code> binds the number of completed iterations.",
        ],
        bites=[
            "<code>max</code> is <strong>mandatory</strong>.",
            "<code>route</code> is not allowed inside the body.",
            "A step containing a loop needs outcomes that distinguish <em>finishing</em> from <em>running out</em>.",
        ],
    ),
    Slide(
        eyebrow="max N visits",
        title="A backward route is a loop, and must say so",
        form=(
            "  outcome " + slot("again") + ": when " + slot("condition")
            + ", route " + slot("an earlier step") + "\n"
            "  max " + slot("N") + " visits"
        ),
        excerpt=Excerpt("09-loops", "step review", "max 3 visits"),
        points=[
            "The bound may be any expression over inputs and constants — <code>max max_polish_rounds visits</code> is legal.",
            "The <code>visits</code> count is available inside guards, so a flow loop can behave differently on its last pass.",
        ],
        note=(
            "<strong>There is no unbounded loop in AWL.</strong> Both loop shapes carry a "
            "real bound, and the checker will not accept a cycle without one. A durable "
            "workflow that can spin forever is a workflow nobody can reason about."
        ),
    ),
    Slide(
        eyebrow="wait · sleep",
        title="Expiry is absence",
        form=(
            "  wait " + slot("signal") + " -> " + slot("binding") + "\n"
            "  wait " + slot("signal") + " timeout " + slot("2h") + " -> "
            + slot("binding") + "   " + opt("binds T?") + "\n"
            "  sleep " + slot("30s")
        ),
        excerpt=Excerpt("10-waits-failure", "step hold", 'route failed(reason: "approval'),
        points=[
            "A plain <code>wait</code> waits as long as it takes — days, weeks — and binds the signal's type.",
            "A <strong>timed</strong> wait binds an optional, because the way a timeout shows up is that nothing is there.",
            "<code>sleep</code> is a durable timer. The run can be evicted from memory and resumed across it.",
        ],
        bites=[
            "The binding is mandatory: <code>wait approval</code> without <code>-&gt; answer</code> is a parse error.",
            "Three timeouts, three places: an <strong>action</strong> or call timeout, the <strong>workflow</strong> header timeout, and a <strong>wait</strong> timeout. They are distinct and do not substitute for each other.",
        ],
    ),
    Slide(
        eyebrow="send … to",
        title="Signalling another running workflow",
        form=(
            "  send " + slot("name") + ": " + slot("Type") + "        "
            + opt("in the header") + "\n\n"
            "  send " + slot("name") + " to " + slot("workflow_id") + "\n"
            "  send " + slot("name") + "(" + slot("field") + ": " + slot("v")
            + ") to " + slot("workflow_id")
        ),
        excerpt=Excerpt("11-signals-queries", "step announce", "send progress("),
        points=[
            "<code>wait</code> receives a signal; <code>send</code> is the other direction.",
            "The target is another run's <strong>durable workflow id</strong> — usually passed in as an input.",
            "A <code>Nil</code> signal carries no payload: <code>send heartbeat to …</code>.",
        ],
    ),
    Slide(
        eyebrow="query · answer",
        title="What an operator can ask a running workflow",
        form=(
            "  query " + slot("name") + ": " + slot("Type") + "       "
            + opt("in the header") + "\n\n"
            "  answer " + slot("name") + "(" + slot("payload") + ")"
        ),
        excerpt=Excerpt("11-signals-queries", "step gate after", "max max_passes"),
        points=[
            "<code>answer</code> registers a value computed from the bindings in scope <em>at that point</em>.",
            "Re-answering inside a loop is what makes a long-running job report live progress.",
            "Read it with <code>aion query &lt;workflow-id&gt; &lt;name&gt;</code>. A finished run no longer answers — inspect it with <code>aion describe</code>.",
        ],
        bites=[
            "<strong>Every declared query must be answered somewhere</strong> or the check fails. The search is syntactic and descends into subflows, fork bodies, loop bodies, substeps and <code>on failure</code> blocks.",
        ],
    ),
    Slide(
        eyebrow="on failure",
        title="Compensation has to end somewhere",
        form=(
            "  on failure\n"
            "    " + slot("…undo what was done…") + "\n"
            "    route " + slot("an outcome")
        ),
        excerpt=Excerpt("10-waits-failure", "step ship", 'route failed(reason: "publish'),
        points=[
            "An action failure fails its step <em>after</em> its declared retries are spent.",
            "<code>on failure</code> is where you put the undo: cancel the booking, delete the uploaded assets, release the lock.",
        ],
        bites=[
            "It <strong>must end in a route</strong>. Swallowing a failure silently is not something the language will let you write.",
        ],
    ),
    Slide(
        eyebrow="the short list",
        title="The rules that actually bite",
        table=[
            ("//! first", "narration before <code>workflow</code>, or it is not a document"),
            ("outcome", "at least one terminal outcome; never fall off the last step"),
            ("otherwise", "last, and conditional outcomes must be exhaustive"),
            ("max", "mandatory on <code>loop</code>; <code>max N visits</code> on any backward route"),
            ("wait", "the <code>-&gt; binding</code> is not optional; a timed wait binds <code>T?</code>"),
            ("on failure", "must end in a route"),
            ("query", "every declared query must be answered somewhere"),
            ("subflow", "its invocation is the only statement of its step"),
            ("collect", "the first statement of the closing step; the opener has only its region line"),
            ("fork", "collection body is exactly one unbound call"),
            ("advisory", "cannot bind a result — it may not have run"),
            ("T?", "must be proved present before it is read; proofs stay inside their arm"),
        ],
        caption=(
            "<b>aion awl check</b> is parser and typechecker. A further band of refusals — the "
            "compile-time shape rules for fork bodies and region lowering among them — arrives "
            "at <b>aion deploy</b>. A document that checks is not yet a document that compiles."
        ),
    ),
    Slide(
        eyebrow="the commands",
        title="What to type",
        form=(
            "aion awl check  " + slot("FILE.awl") + "        parse and typecheck\n"
            "aion awl fmt    " + slot("FILE.awl") + "        canonical form\n"
            "aion awl schema " + slot("FILE.awl") + "        the input, output and query schemas\n"
            "aion awl emit   " + slot("FILE.awl") + "        what it compiles to\n"
            "\n"
            "aion deploy     " + slot("FILE.awl") + "        compile and deploy, one motion\n"
            "aion run        " + slot("FILE.awl") + " --input " + slot("JSON")
        ),
        lede=(
            "Check while you write. Deploy when it checks. <code>aion run</code> compiles, "
            "deploys, starts and waits — the whole loop in one line."
        ),
        caption=(
            "To re-verify this deck: <b>python3 docs/authoring/build-deck.py</b>. It checks every "
            "document under <b>docs/authoring/deck-examples/</b> and refuses to write "
            "<b>awl-deck.html</b> unless all of them pass."
        ),
    ),
]


# --------------------------------------------------------------------------
# Gate, render, write
# --------------------------------------------------------------------------


def gate(documents: list[Path]) -> None:
    """Refuse to build unless `aion awl check` passes on every document.

    A deck that quotes a document nobody checked is exactly the failure this
    script exists to prevent, so a missing `aion` binary is a refusal too, not
    a skip.
    """
    binary = shutil.which("aion")
    if binary is None:
        raise SystemExit(
            "refusing to build: `aion` is not on PATH, so the examples cannot be "
            "checked. Install it with `cargo install --path crates/aion-cli`."
        )
    version = subprocess.run(
        [binary, "--version"], capture_output=True, text=True, check=False
    ).stdout.strip()
    print(f"checking {len(documents)} documents with {version} ({binary})")

    failed: list[str] = []
    for path in documents:
        result = subprocess.run(
            [binary, "awl", "check", str(path)],
            capture_output=True,
            text=True,
            check=False,
        )
        status = "ok" if result.returncode == 0 else "FAILED"
        print(f"  {status:>6}  {path.name}")
        if result.returncode != 0:
            failed.append(
                f"{path.name}: {(result.stdout + result.stderr).strip()}"
            )
    if failed:
        raise SystemExit(
            "refusing to write the deck — these documents do not check:\n  "
            + "\n  ".join(failed)
        )


def render(slide: Slide, path_of: dict[str, str]) -> str:
    parts = [f'    <p class="eyebrow">{slide.eyebrow}</p>']
    heading = "h1" if slide.big else "h2"
    parts.append(f"    <{heading}>{slide.title}</{heading}>")
    if slide.lede:
        parts.append(f'    <p class="lede">{slide.lede}</p>')
    if slide.stamp:
        parts.append(f'    <span class="stamp">{slide.stamp}</span>')
    if slide.form:
        parts.append(f'    <p class="form">{slide.form}</p>')
    if slide.excerpt is not None:
        body = html.escape(slide.excerpt.text())
        parts.append(f"    <pre><code>{body}</code></pre>")
        parts.append(
            f'    <p class="src">from <b>{path_of[slide.excerpt.document]}</b>'
            f" — a complete document, checked</p>"
        )
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
            parts.append(
                f"      <tr><td>{html.escape(term)}</td><td>{meaning}</td></tr>"
            )
        parts.append("    </table></div>")
    if slide.note:
        parts.append(f'    <p class="note">{slide.note}</p>')
    if slide.caption:
        parts.append(f'    <p class="caption">{slide.caption}</p>')
    return '  <section class="slide">\n' + "\n".join(parts) + "\n  </section>"


def main() -> int:
    referenced = sorted(
        {slide.excerpt.document for slide in SLIDES if slide.excerpt is not None}
    )
    missing = [name for name in referenced if not (EXAMPLES / f"{name}.awl").is_file()]
    if missing:
        raise SystemExit(
            "refusing to build — these referenced documents do not exist: "
            + ", ".join(missing)
        )

    # Gate the WHOLE example directory, not only the documents a slide happens
    # to quote: an example that has stopped checking is a broken claim whether
    # or not this revision of the deck points at it.
    documents = sorted(EXAMPLES.glob("*.awl"))
    if not documents:
        raise SystemExit(f"refusing to build — no documents found in {EXAMPLES}")
    gate(documents)

    root = HERE.parent.parent
    path_of = {
        path.stem: str(path.relative_to(root)) for path in documents
    }

    rendered = "\n\n".join(render(slide, path_of) for slide in SLIDES)
    shell = SHELL.read_text(encoding="utf-8")
    if "<!--SLIDES-->" not in shell:
        raise SystemExit(f"{SHELL.name} has no <!--SLIDES--> placeholder")
    OUTPUT.write_text(shell.replace("<!--SLIDES-->", rendered), encoding="utf-8")

    quoted = len([s for s in SLIDES if s.excerpt is not None])
    print(
        f"wrote {OUTPUT.relative_to(root)} — {len(SLIDES)} slides, "
        f"{quoted} excerpts from {len(referenced)} of {len(documents)} documents"
    )
    unquoted = sorted(set(path_of) - set(referenced))
    if unquoted:
        print("  not quoted by any slide: " + ", ".join(unquoted))
    return 0


if __name__ == "__main__":
    sys.exit(main())
