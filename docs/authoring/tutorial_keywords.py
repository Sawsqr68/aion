"""The tutorial's mechanically counted keyword reference table."""

import re

# The keyword table the reader is handed, hoisted out of its slide so the
# sentence above it can be DERIVED from it rather than typed beside it.
KEYWORD_TABLE: list[tuple[str, str]] = [
        (
            "<b>WHAT IT IS</b>",
            "<i>the document's identity and its data shapes</i>",
        ),
        ("<code>workflow</code>", "its name — how you start it later"),
        ("<code>type</code>", "the shape of a piece of data"),
        (
            "<b>WHAT IT PROMISES</b>",
            "<i>the contract a caller sees — what goes in, what comes "
            "back</i>",
        ),
        ("<code>input</code>", "what you have to hand it to start"),
        (
            "<code>outcome</code>",
            "how it is allowed to end, and what it hands back",
        ),
        (
            "<code>success</code> <code>failure</code>",
            "which kind of ending that is. Both are real endings the "
            "workflow chose — neither is a crash.",
        ),
        (
            "<b>WHO DOES THE WORK</b>",
            "<i>the jobs themselves — never the workflow's own doing</i>",
        ),
        (
            "<code>worker</code>",
            "a name for the thing that does real work — the queue",
        ),
        (
            "<code>action</code>",
            "one job, with its parameters and the shape it returns",
        ),
        (
            "<code>run</code>",
            "the action's implementation, right here in the document — "
            "the server executes this command itself",
        ),
        (
            "<code>agent</code>",
            "an action served by an agent worker: one String prompt in, the "
            "agent-outcome record out — exactly <code>text</code>, "
            "<code>final_message</code>, <code>stop_reason</code>, and "
            "<code>session_id</code>",
        ),
        (
            "<code>timeout</code>",
            "an authored action deadline carried in the contract; not enforced "
            "by the 0.11.0 engine",
        ),
        (
            "<code>retry</code> <code>backoff</code>",
            "how many transient failures to absorb, and how long to wait "
            "between attempts",
        ),
        (
            "<b>WHAT HAPPENS, IN WHAT ORDER</b>",
            "<i>the plan the workflow actually executes</i>",
        ),
        ("<code>step</code>", "a stage of the workflow"),
        ("<code>after</code>", "what must finish before this stage starts"),
        ("<code>route</code>", "finish, this way, with this"),
        (
            "<code>when</code> <code>otherwise</code>",
            "choose between exact evidence and every other reply",
        ),
        (
            "<code>on</code> <code>failure</code>",
            "what to do when the stage faults — and it must route too",
        ),
]


def keyword_table_shape(table: list[tuple[str, str]]) -> tuple[int, int]:
    """How many distinct words the keyword table explains, in how many groups.

    Both numbers are READ OFF the table rather than typed into the sentence
    above it, because that sentence had already drifted: it promised the
    reader fourteen words over a table that explains thirteen. `timeout`
    left the examples, its row went with it, and the lede did not move.
    The build's own keyword gate had been printing the corrected 13 the
    whole time — one truth in two places, and only one of them maintained.

    Distinct, not row-counted: `failure` is explained twice, once as an
    outcome kind and once in `on failure`, and a reader counting words in
    the table counts it once.

    Note the direction this closes and the one it does not. The lede now
    cannot disagree with the table. `gate_keyword_coverage` separately
    forbids a word the examples USE from being missing here. Neither
    forbids the reverse — a row explaining a word no example contains —
    which would leave both numbers self-consistent and the deck teaching a
    word the reader never sees.
    """
    words: set[str] = set()
    groups = 0
    for term, _ in table:
        if term.startswith("<b>"):
            groups += 1
            continue
        words.update(re.findall(r"<code>(\w+)</code>", term))
    return len(words), groups


KEYWORD_COUNT, KEYWORD_GROUP_COUNT = keyword_table_shape(KEYWORD_TABLE)
