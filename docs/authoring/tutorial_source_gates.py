"""Source and document consistency gates for the AWL tutorial."""

import json
import re
import shlex
import subprocess

from tutorial_model import EXAMPLES, ROOT, WORKFLOW_NAME, Slide

HEREDOC_SOURCES = {"aion.toml"}


def gate_command_payloads(slides: list[Slide]) -> int:
    """Every `-d` payload the deck prints must survive being pasted.

    The deck's commands are wrapped across lines to fit a slide, and a wrap can
    change what the shell actually passes without making the line invalid: a
    `\\` continuation ENDS a quoted string, so the next line's leading spaces
    become an argument separator. That is how the availability probe came to
    hand `curl` only the first 42 characters of its JSON — `EOF while parsing
    a value at line 1 column 43` — while still looking perfectly correct on the
    slide, and while `bash -n` still called it valid.

    So this does not check syntax. It joins each command the way a shell would,
    splits it the way a shell would, and asserts that what actually lands in
    the `-d` slot parses as JSON. That is the property the reader depends on.
    """
    checked = 0
    for slide in slides:
        if not slide.cmd:
            continue
        joined = "\n".join(slide.cmd)
        # A backslash-newline is removed WITH the newline; nothing else is.
        joined = joined.replace("\\\n", "")
        if "<<EOF" in joined:  # a heredoc body is data, not an argument list
            continue
        try:
            tokens = shlex.split(joined)
        except ValueError as error:
            raise SystemExit(
                f"refusing to build — a command on slide {slide.title!r} "
                f"cannot be split the way a shell would: {error}"
            ) from error
        # Keyed on the SHAPE of the argument, not on the flag carrying it. An
        # allowlist of flags is a second place the truth lives: it omitted
        # `--data-binary` and `--data-ascii`, so renaming a flag would drop a
        # payload out of the check while the same "ok" still printed. Anything
        # that opens like JSON is held to being JSON, whichever flag delivers
        # it and whether or not a flag delivers it at all.
        for index, token in enumerate(tokens):
            if not token.startswith(("{", "[")):
                continue
            carrier = tokens[index - 1] if index else "(first argument)"
            try:
                json.loads(token)
            except json.JSONDecodeError as error:
                raise SystemExit(
                    f"refusing to build — the {carrier} payload on slide "
                    f"{slide.title!r} is not valid JSON once a shell has "
                    f"parsed the command as printed: {error}\n"
                    f"  what the shell would actually pass: {token!r}\n"
                    "  a line continuation after a closing quote splits the "
                    "argument — keep the payload on one unbroken line."
                ) from error
            checked += 1
    if checked == 0:
        raise SystemExit(
            "refusing to build — no slide passes a JSON payload to a command, "
            "so this gate measured nothing. Either the deck stopped printing "
            "`--input`/`-d` commands (delete this gate) or the payloads stopped "
            "being recognised (fix it) — a zero here is never a pass."
        )
    return checked


def language_vocabulary() -> set[str]:
    """Every reserved word AWL has, read off the lexer that defines them.

    Not a list kept here. `Keyword::as_word` is where a word's spelling is
    decided, so that is where this reads it from — a keyword added to the
    language is one this deck must explain the moment an example uses it,
    without anybody remembering to come back here.
    """
    source = (
        ROOT / "crates" / "aion-awl" / "src" / "lexer" / "tokens.rs"
    ).read_text(encoding="utf-8")
    start = source.find("fn as_word(")
    if start < 0:
        raise SystemExit(
            "refusing to build — aion-awl's lexer no longer has `as_word`, so "
            "the language's vocabulary cannot be derived. Find where a "
            "keyword's spelling is decided and read it from there."
        )
    body = source[start : source.find("\n    }", start)]
    words = set(re.findall(r'=>\s*"([a-z_]+)"', body))
    if len(words) < 40:
        raise SystemExit(
            f"refusing to build — only {len(words)} keyword spellings could be "
            "read out of `as_word`, which is far fewer than the language has. "
            "The shape of that function has changed; fix this reader rather "
            "than trusting a short list."
        )
    return words


def words_in_examples() -> set[str]:
    """Every bare word the example documents contain, comments and strings out.

    A `run` body is a command, not AWL — `git clone` must not make `clone` look
    like a word the language needs explained. Same for comment prose.
    """
    seen: set[str] = set()
    for path in sorted(EXAMPLES.glob("*.awl")):
        for line in path.read_text(encoding="utf-8").splitlines():
            stripped = line.strip()
            if stripped.startswith(("//", "/*")):
                continue
            stripped = re.sub(r'"[^"]*"', " ", stripped)
            seen.update(re.findall(r"\b[a-z_][a-z0-9_]*\b", stripped))
    return seen


def gate_keyword_coverage(slides: list[Slide]) -> int:
    """Refuse to build unless every keyword the examples USE is explained.

    The keyword table is the reader's only answer to "what does this
    line mean", and a table maintained by hand beside documents maintained by
    hand is one truth in two places: it drifts the moment an example gains a
    word. It had already drifted — the examples used `timeout` and `run` and
    the table explained neither.

    So the expectation is DERIVED from the examples rather than listed here:
    every leading keyword in every gated `.awl` must appear in the table.
    """
    used: set[str] = set()
    for path in sorted(EXAMPLES.glob("*.awl")):
        for line in path.read_text(encoding="utf-8").splitlines():
            stripped = line.strip()
            if not stripped or stripped.startswith(("//", "/*")):
                continue
            word = stripped.split(" ", 1)[0].split("(", 1)[0]
            if word.isidentifier() and not word[0].isupper():
                used.add(word)
    # The line above sees only the FIRST word of a line, and that is not the
    # same thing as the words a document uses. `after` sits in the middle of
    # `step verify after fetch`, so it was invisible: deleting its row from the
    # table left the build green, which is a gate that cannot see its own
    # subject. So the language's own vocabulary is read out of the lexer and
    # intersected with every word the examples contain, wherever it sits.
    #
    # Both halves are kept, because neither covers the other: `run` is parsed
    # contextually and is NOT a lexer keyword, so the vocabulary half misses it
    # — and that was the previous drift. Two detectors, unioned.
    used |= language_vocabulary() & words_in_examples()
    # Words that are a document's own names rather than language keywords: a
    # step name, an action call, a route target. They are introduced by the
    # line above them, never by the table.
    declared_names: set[str] = set()
    for path in sorted(EXAMPLES.glob("*.awl")):
        for line in path.read_text(encoding="utf-8").splitlines():
            parts = line.strip().split()
            if len(parts) >= 2 and parts[0] in {"workflow", "worker", "step", "action", "type"}:
                declared_names.add(parts[1].split("(", 1)[0])
    used -= declared_names

    explained = set()
    for slide in slides:
        if not slide.keywords or not slide.table:
            continue
        for term, _ in slide.table:
            explained.update(re.findall(r"<code>(\w+)</code>", term))
    if not explained:
        raise SystemExit(
            "refusing to build — no slide is flagged `keywords=True` with a "
            "table, so nothing explains the document the reader just wrote"
        )
    missing = sorted(used - explained)
    if missing:
        raise SystemExit(
            "refusing to build — these keywords appear in the example documents "
            f"but nothing explains them on the keyword slide: {missing}\n"
            "  a reader looking there for what a line means would find nothing."
        )
    # And the other direction, which is the one that actually drifted. A row
    # explaining a word no example uses is how the table came to promise
    # fourteen words over thirteen: `timeout` left the examples, and its row
    # stayed behind because nothing was watching this way round. Both sets are
    # already in scope, so the check costs three lines — documenting it as a
    # permanent gap, as an earlier revision did, was understating it.
    stranded = sorted(explained - used)
    if stranded:
        raise SystemExit(
            "refusing to build — the keyword slide explains words that appear in "
            f"no example document: {stranded}\n"
            "  a reader is being taught vocabulary the tutorial never shows them "
            "use; delete the row, or use the word."
        )
    return len(used)


def gate_one_workflow_name() -> str:
    """Refuse to build unless every example document is the SAME workflow.

    This is the gate the deck most needed and did not have. Its predecessor
    told the reader to make a workflow called `order_flow`, showed a skeleton
    for `greet_flow`, and then walked through `first_flow` — three names for
    one tutorial, and no mechanism that could notice. A reader following it had
    to work out on their own which of the three they were meant to be holding.

    A name is not checkable by reading carefully. It is checkable by counting
    the distinct ones, so that is what this does.
    """
    names: dict[str, str] = {}
    for path in sorted(EXAMPLES.glob("*.awl")):
        for line in path.read_text(encoding="utf-8").splitlines():
            if line.startswith("workflow "):
                names[path.name] = line.split()[1]
                break
        else:
            raise SystemExit(
                f"refusing to build — {path.name} declares no `workflow` line, "
                "so it is not a document this deck can be teaching"
            )
    distinct = sorted(set(names.values()))
    if distinct != [WORKFLOW_NAME]:
        listing = "\n  ".join(f"{doc}: {name}" for doc, name in sorted(names.items()))
        raise SystemExit(
            f"refusing to build — this deck teaches ONE workflow, "
            f"`{WORKFLOW_NAME}`, but the examples declare {distinct}:\n  "
            + listing
            + "\n  a reader cannot tell which of those they are building."
        )
    return WORKFLOW_NAME


def gate_skeleton_is_the_servers_own(binary: str) -> None:
    """The skeleton slide must show what the server actually writes.

    The deck cannot ask the server for it — creating a document to photograph
    it would leave that document in the reader's own workspace. So the template
    is read out of the server's source and put through the same canonical
    printer the server puts it through, and the result must equal the example
    file byte for byte.

    Without this the skeleton slide is a screenshot in prose: true when it was
    written, silently false the day the template changes.
    """
    source = (
        ROOT / "crates" / "aion-server" / "src" / "awl" / "documents.rs"
    ).read_text(encoding="utf-8")
    marker = "outcome done: type Placeholder"
    line = next((ln for ln in source.splitlines() if marker in ln), None)
    if line is None or line.count('"') < 2:
        raise SystemExit(
            "refusing to build — the new-document template is no longer a "
            "single quoted literal in aion-server/src/awl/documents.rs, so "
            "the skeleton slide cannot be derived from it. Find where the "
            "server writes a new document and derive it from there."
        )
    literal = line[line.index('"') + 1 : line.rindex('"')]
    derived = (
        literal.replace("{name}", WORKFLOW_NAME)
        .replace("{{", "{")
        .replace("}}", "}")
        .replace("\\n", "\n")
    )
    scratch = EXAMPLES / ".skeleton-check.awl"
    try:
        scratch.write_text(derived, encoding="utf-8")
        printed = subprocess.run(
            [binary, "awl", "fmt", str(scratch)],
            capture_output=True,
            text=True,
            check=False,
        )
        if printed.returncode != 0:
            raise SystemExit(
                "refusing to build — the server's own new-document template "
                "does not survive `aion awl fmt`:\n"
                + (printed.stdout + printed.stderr).strip()
            )
        expected = scratch.read_text(encoding="utf-8")
    finally:
        scratch.unlink(missing_ok=True)
    shown = (EXAMPLES / f"{WORKFLOW_NAME}-1-skeleton.awl").read_text(encoding="utf-8")
    if shown != expected:
        raise SystemExit(
            "refusing to build — the skeleton slide does not show what the "
            "server writes for a new document.\n  the server would write:\n"
            + "".join(f"    {ln}\n" for ln in expected.splitlines())
            + "  the deck shows:\n"
            + "".join(f"    {ln}\n" for ln in shown.splitlines())
        )
    print(f"  {'ok':>6}  the skeleton slide is the server's own template")
