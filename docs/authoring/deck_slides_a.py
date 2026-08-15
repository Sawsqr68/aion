"""Declarative AWL deck slides; kept separate from build orchestration."""

from deck_model import Excerpt, Slide, opt, slot

SLIDES_A: list[Slide] = [
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
        form="    agent"
        + opt("   — this parameter is a prompt, this result is the outcome record"),
        excerpt=Excerpt("03-agent-seam", "type Reply", "agent, timeout"),
        points=[
            "One <code>String</code> parameter goes out as the prompt; the agent-outcome "
            "record comes back — exactly <code>text: String</code> (all accumulated output), "
            "<code>final_message: String</code> (the last answer), "
            "<code>stop_reason: String</code>, and <code>session_id: String</code>; "
            "the flow reads the field it needs.",
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
    )
]
