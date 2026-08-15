"""Declarative AWL deck slides; kept separate from build orchestration."""

from deck_model import Excerpt, Slide, opt, slot

SLIDES_B: list[Slide] = [
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
    )
]
