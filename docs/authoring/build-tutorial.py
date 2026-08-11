#!/usr/bin/env python3
"""Build the from-scratch tutorial deck from artifacts that are known to work.

Every AWL snippet is sliced out of a complete document under
`tutorial-examples/`, and the build refuses outright if any of them stops
passing `aion awl check`. The deck teaches ONE workflow from the first slide to
the last, and that is gated too: every example document must declare the same
`workflow` name, because the complaint this rewrite answers was a tutorial that
named three different ones and had no mechanism that could notice.

The point is that the deck cannot teach a shape the tools reject, or a story
that does not hold together.
"""

import html
import json
import re
import shlex
import shutil
import subprocess
import sys
import tomllib
from dataclasses import dataclass, field
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
EXAMPLES = HERE / "tutorial-examples"
SHELL = HERE / "deck-shell.html"
OUTPUT = HERE / "first-workflow.html"

SHELL_TITLE = "<title>AWL — the syntax, keyword by keyword</title>"
DECK_TITLE = "<title>Your first Aion workflow — start to finish</title>"

# The one name this deck teaches. A reader following it types this word into
# the console, into every command, and reads it back off every result. It is
# declared once here and gated against the example documents below, because
# the complaint that produced this deck was exactly a name that changed three
# times between slides.
WORKFLOW_NAME = "repo_gate"

# `aion` flags that take no value. Everything else beginning with `-` is
# assumed to consume the token after it, so that `taught_verbs()` can find the
# verb in a command that leads with global flags. An unknown boolean flag makes
# the gate try to `--help` a file name, which fails loudly — the wrong way
# round is the safe way round.
VALUELESS_FLAGS = {"--pretty", "--help", "-h", "--version", "-V"}


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
    # Marks the slide whose table explains the language's words. Flagged rather
    # than matched on its title, so renaming the slide cannot quietly detach it
    # from the gate that keeps it complete.
    keywords: bool = False


def code(text: str) -> str:
    return f"<code>{html.escape(text)}</code>"


# Example files the deck publishes as a paste-able heredoc rather than as an
# Excerpt. The unused-file census must count both doors, or it reports a
# published file as dead.
HEREDOC_SOURCES = {"aion.toml"}


def gate_command_payloads() -> int:
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
    for slide in SLIDES:
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


def gate_keyword_coverage() -> int:
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
    for slide in SLIDES:
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


def taught_verbs() -> dict[tuple[str, ...], set[str]]:
    """Every `aion` invocation the deck tells the reader to type.

    Returns the verb path mapped to the flags the deck spells beside it, so
    the gate can check the WHOLE command rather than its first word. Checking
    only that `aion deploy` resolves does not gate `aion deploy --endpoint …`:
    a gate that runs a COMPONENT of the operator's command has not gated the
    command, and the reader is the one who finds out.

    Derived from the slides rather than listed beside them: a hand-kept list is
    one truth in two places, and the copy that drifts is always the one nothing
    executes.
    """
    found: dict[tuple[str, ...], set[str]] = {}
    for slide in SLIDES:
        if not slide.cmd:
            continue
        joined = "\n".join(slide.cmd).replace("\\\n", "")
        if "<<EOF" in joined:
            continue
        for line in joined.splitlines():
            try:
                tokens = shlex.split(line)
            except ValueError:
                continue
            # NOT `tokens[0]`. A slide may set an environment variable first —
            # `AION_HOME=… aion server …` — and anchoring on the first token
            # skipped that line silently, leaving `server` untaught and
            # unchecked while the gate reported success.
            at = next((i for i, token in enumerate(tokens) if token == "aion"), None)
            if at is None:
                continue
            rest = iter(tokens[at + 1 :])
            verb: list[str] = []
            flags: set[str] = set()
            ended = False
            for token in rest:
                if token.startswith("-"):
                    flags.add(token.split("=", 1)[0])
                    if token not in VALUELESS_FLAGS and "=" not in token:
                        next(rest, None)
                    continue
                if ended or not token.isalpha():
                    # A positional argument ends the verb path, but flags may
                    # still follow it — `aion run file.awl --input …` — so keep
                    # reading the line instead of breaking out of it.
                    ended = True
                    continue
                verb.append(token)
            if verb:
                found.setdefault(tuple(verb), set()).update(flags)
    if not found:
        raise SystemExit(
            "refusing to build — no slide prints an `aion` command, so this "
            "deck teaches nothing that can be checked against the binary"
        )
    return found


def config_heredoc() -> list[str]:
    """Render `tutorial-examples/aion.toml` as the paste-able heredoc block.

    The config exists in ONE place — that file — and this turns it into the
    command the reader pastes. Hand-typing the same TOML into the slide would
    put the same truth in two places, where only diligence keeps them equal.

    Its `$HOME` is deliberate: the shell expands it as the heredoc is written,
    so a full path lands on disk with no username typed by hand. A `~` would
    NOT work — the config parser is not a shell and takes it literally,
    creating a directory actually named `~` (measured 2026-08-07).
    """
    source = (EXAMPLES / "aion.toml").read_text(encoding="utf-8")
    body = [ln for ln in source.splitlines() if not ln.lstrip().startswith("#")]
    while body and not body[0].strip():
        body.pop(0)
    while body and not body[-1].strip():
        body.pop()
    if not any("$HOME" in ln for ln in body):
        raise SystemExit(
            "tutorial-examples/aion.toml no longer contains $HOME — the config "
            "slide would teach a path the reader has to edit by hand"
        )
    return [
        "mkdir -p ~/my-first-flow && cd ~/my-first-flow",
        "",
        "cat > aion.toml <<EOF",
        *body,
        "EOF",
    ]


def cli_crate_and_version() -> tuple[str, str, str]:
    """The crate that publishes the `aion` binary, the version it is at, and
    the Rust floor a reader needs to install it.

    All three are READ from the manifests rather than typed into a slide, so a
    rename, a version bump or a raised toolchain floor cannot leave the deck
    telling the reader to install something that does not exist.

    This matters more than the usual staleness argument. `aion` on crates.io
    is somebody ELSE's crate — a datetime utility, 0.2.0, with no binaries —
    so the obvious guess does not merely install the wrong thing, it fails
    with `there is nothing to install` (measured 2026-08-07). The name that
    works is this package's, and this is the one place it is written down.
    """
    manifest = tomllib.loads(
        (ROOT / "crates" / "aion-cli" / "Cargo.toml").read_text(encoding="utf-8")
    )
    package = manifest["package"]["name"]
    binaries = [entry["name"] for entry in manifest.get("bin", [])]
    if "aion" not in binaries:
        raise SystemExit(
            f"refusing to build — `{package}` no longer produces a binary "
            f"named `aion` (it produces {binaries or 'none'}), so every "
            "command in this deck names a binary the reader will not have"
        )
    if package == "aion":
        raise SystemExit(
            "refusing to build — the CLI package is now named `aion`, which "
            "on crates.io is a DIFFERENT crate (a datetime utility with no "
            "binaries). The install slide would send the reader to it."
        )
    workspace = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    version = workspace["workspace"]["package"]["version"]
    if manifest["package"].get("version", {}) != {"workspace": True}:
        raise SystemExit(
            f"refusing to build — `{package}` no longer takes its version "
            "from the workspace, so the version this slide prints is not "
            "necessarily the version the reader would install"
        )
    # The floor the reader's toolchain must clear. Derived, because the CLI
    # crate sets its own `rust-version` ABOVE the workspace floor — a deck that
    # said only "if you have Rust" sends anyone between the two floors into a
    # failed install with no explanation.
    rust_min = manifest["package"].get("rust-version")
    if isinstance(rust_min, dict):
        rust_min = workspace["workspace"]["package"].get("rust-version")
    if not isinstance(rust_min, str) or not rust_min:
        raise SystemExit(
            f"refusing to build — `{package}` declares no resolvable "
            "`rust-version`, so the install slide cannot say which toolchain "
            "the reader needs"
        )
    return package, version, rust_min


CLI_CRATE, CLI_VERSION, CLI_RUST_MIN = cli_crate_and_version()


def finished_line_count() -> int:
    """How long the finished document actually is.

    Counted, not remembered. The deck opens the final slide by naming this
    number, and a hand-written one goes stale the first time a line moves —
    which it already had, twice.
    """
    text = (EXAMPLES / f"{WORKFLOW_NAME}.awl").read_text(encoding="utf-8")
    return len(text.rstrip("\n").splitlines())


FINISHED_LINE_COUNT = finished_line_count()


def command_count() -> int:
    """How many shell commands the finished document actually runs.

    One `run` body is one command the server executes. The slide that promises
    the reader this number was typed by hand — the same shape as the keyword
    lede that had already drifted, and as `FINISHED_LINE_COUNT` before it. It is
    true today; the point is that nothing was making it stay true.
    """
    text = (EXAMPLES / f"{WORKFLOW_NAME}.awl").read_text(encoding="utf-8")
    return sum(1 for line in text.splitlines() if line.strip().startswith("run "))


def staged_document_count() -> int:
    """How many complete `.awl` documents the reader builds up through.

    Documents, NOT executions — and the distinction is the whole point. The
    first version of this docstring said "how many times the reader runs the
    document", and the prose beside it duly promised five runs. The reader runs
    it three times: `repo_gate-1-skeleton.awl` is only ever shown, and
    `repo_gate-2-contract.awl` only ever reaches `aion awl check` (the deck says
    so itself two slides later — "0 steps is correct … Nothing has been
    deployed"). Deriving a number does not make it the RIGHT number: this one
    was derived correctly from a glob and then described as something the glob
    cannot see. A derivation is only as honest as the sentence that spends it,
    so the sentence must claim documents, which is what this counts.
    """
    return len(sorted(EXAMPLES.glob("*.awl")))


# The closing slide promises "the same N motions scale up" and then lists them.
# The number and the list were two literals in one sentence: nothing but care
# held them equal, and care is not a mechanism. Counting the list is the
# subtraction — there is now one place to edit, and the count follows it.
MOTIONS = (
    "write the document",
    "check it",
    "deploy it",
    "run it",
    "watch it",
)


COMMAND_COUNT = command_count()
STAGE_COUNT = staged_document_count()

# Small numbers read as words in prose, and that is a fact about English rather
# than about this deck: it cannot drift when a document changes, which is the
# whole reason the numbers beside it now are derived.
NUMBER_WORDS = ("zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine")


def spelled(count: int) -> str:
    """`count` as an English word where English prefers one, else as digits."""
    return NUMBER_WORDS[count] if count < len(NUMBER_WORDS) else str(count)


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
            "agent-outcome record out — a declared record of "
            "<code>text</code> and <code>stop_reason</code>",
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


# --------------------------------------------------------------------------
# The deck
# --------------------------------------------------------------------------

SLIDES: list[Slide] = [
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
    # ---------------------------------------------------------------- step 1
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
    # ---------------------------------------------------------------- step 2
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
    # ---------------------------------------------------------------- step 3
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
    # ---------------------------------------------------------------- step 4
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
    ),
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
    # ---------------------------------------------------------------- step 5
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
    # ---------------------------------------------------------------- step 6
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
    # ---------------------------------------------------------------- whole
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
    # ---------------------------------------------------------------- deploy
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
    ),
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
    # ---------------------------------------------------------------- console
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
    # ---------------------------------------------------------------- rescue
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
    # ---------------------------------------------------------- judged worker
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
    ),
    Slide(
        eyebrow="Where next — deploy",
        title="Deploy the judged contract",
        cmd=[
            "aion --endpoint http://127.0.0.1:50099 --pretty \\",
            "     deploy repo_gate_judged.awl",
        ],
        points=[
            "Deploy records all five action shapes under queue "
            "<code>gate</code>.",
            "The four bodies are immediately servable by the server. "
            "<code>judge</code> is not: no process has offered it yet.",
        ],
    ),
    Slide(
        eyebrow="Where next — pre-flight",
        title="Ask before you start a run",
        cmd=[
            "curl -sS http://127.0.0.1:8099/awl/workers/availability \\",
            "  -H 'content-type: application/json' \\",
            "  -d '{\"namespace\":\"default\",\"task_queue\":\"gate\"}'",
        ],
        out='{"available":false,"task_queue":"gate","connected_workers":0,'
        '"scaffold_hint":"No connected worker serves task queue `gate`. '
        'Scaffold and run this worker from Workers & Actions, then retry '
        'start."}',
        caption="Real output, one line, exactly as <code>curl</code> prints it.",
        points=[
            "<code>available: false</code> is an honest refusal before work can "
            "park: the one worker-owed action you just declared is "
            "<code>judge</code>.",
            "0.11.0 reports that absence at queue level: no connected worker "
            "serves <code>gate</code>, and it tells you to start one before "
            "retrying. Newer servers may add a per-action census; this chapter "
            "does not depend on it.",
        ],
    ),
    Slide(
        eyebrow="Where next — connect",
        title="Start the worker in a second terminal",
        cmd=[
            "aion worker agent repo_gate_judged.awl \\",
            "  --liminal-address 127.0.0.1:50169 \\",
            "  --identity repo-gate-judge",
        ],
        bites=[
            "Declare the launch first, in <code>repo_gate_judged.awl</code>, "
            "indented under <code>worker gate</code> and above its actions: "
            "<code>harness</code>, then <code>kind norn</code>, "
            "<code>concurrency 4</code>, "
            "<code>reconnect_initial_backoff 1s</code>, "
            "<code>reconnect_max_backoff 30s</code>, "
            "<code>reconnect_max_attempts 100</code>, and "
            "<code>env_pass \"PATH\", \"HOME\"</code> — one setting per line.",
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
        ],
        note="If registration is refused, <b>you are told twice</b>: the server "
        "logs the refusal at WARN with the queue and reason, and this worker "
        "fails with the same diagnosis. Read either side; do not leave a "
        "rejected worker running and hope the queue becomes servable.",
    ),
    Slide(
        eyebrow="Where next — pre-flight again",
        title="Now the same question says yes",
        cmd=[
            "curl -sS http://127.0.0.1:8099/awl/workers/availability \\",
            "  -H 'content-type: application/json' \\",
            "  -d '{\"namespace\":\"default\",\"task_queue\":\"gate\"}'",
        ],
        out='{"available":true,"task_queue":"gate","connected_workers":1,'
        '"scaffold_hint":null}',
        caption="Real output, one line, exactly as <code>curl</code> prints it.",
        points=[
            "Nothing in the document changed. The channel did: a worker now "
            "offers the one action the server cannot execute itself.",
            "The four command bodies remain server-run. The worker advertised "
            "only <code>judge</code>; its admission log names that action, while "
            "this 0.11.0 endpoint answers the queue-level question.",
        ],
    ),
    Slide(
        eyebrow="Where next — run it",
        title="The evidence reaches the judge",
        cmd=[
            "aion --endpoint http://127.0.0.1:50099 --pretty \\",
            "     run repo_gate_judged.awl \\",
            '     --input "{\\"repo\\": \\"https://github.com/dtolnay/itoa\\", '
            '\\"workdir\\": \\"$HOME/gate-work-judged\\"}"',
        ],
        out='"result": {\n'
        '  "outcome": "passed",\n'
        '  "payload": {\n'
        '    "cloned": "Cloning into …",\n'
        '    "commit": "1577ed9",\n'
        '    "checks": "    Updating crates.io index … Finished `dev` profile …",\n'
        '    "tests": "… test result: ok. 11 passed; 0 failed; …",\n'
        '    "judgement": "GATE_APPROVED"\n'
        "  }\n"
        "}",
        caption="Real output, with the long middles elided at the <b>…</b> "
        "marks. Nothing was reworded.",
        points=[
            "Clone, head, checks and tests still run from the sealed command "
            "bodies. Their outputs are concatenated into the prompt in-flow.",
            "The worker returns one string. The workflow, not the worker, owns "
            "the exact-token routing and records the reply in its outcome.",
        ],
        note="A different reply ends at <code>failed</code> and carries "
        "<code>judge_reply</code> verbatim. A verdict is evidence about what one "
        "agent said after seeing the gate output — not proof that the repository "
        "is correct.",
    ),
    Slide(
        eyebrow="Where next",
        title="You have the shape of it now",
        points=[
            "The AWL deck covers the rest of the language — waits, signals, "
            "children, parallel work, guards, retries, failure handling.",
            f"The same {spelled(len(MOTIONS))} motions scale up: "
            f"{', '.join(MOTIONS)}.",
            "A gate is only the first useful thing. Anything that is a "
            "sequence of jobs with a verdict at the end is the same shape.",
        ],
        note="The worker story has its own documents: "
        "<code>docs/workers/</code> for worker processes and admission (a "
        "worker is let in only if it offers every reachable worker-owed "
        "action on its queue), <code>docs/workers/DECLARED-COMMANDS.md</code> "
        "for the <code>run</code> bodies you used here.",
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
            f"deck can be checked. Install it with `cargo install "
            f"{CLI_CRATE}`, or from this clone with `cargo install --path "
            "crates/aion-cli`."
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


def gate_verbs(binary: str) -> None:
    """Refuse to teach a command this binary does not have — flags included.

    The exit code of `<verb> --help` only says the VERB exists. Every flag the
    deck spells is checked against that verb's own help text, falling back to
    the root help because a global option (`--endpoint`) is documented there
    rather than on each subcommand. Rename a flag and the deck stops building
    instead of quietly becoming unfollowable.
    """
    taught = taught_verbs()
    root = subprocess.run(
        [binary, "--help"], capture_output=True, text=True, check=False
    )
    root_help = root.stdout + root.stderr
    missing_verbs: list[str] = []
    missing_flags: list[str] = []
    checked_flags = 0
    for verb, flags in taught.items():
        result = subprocess.run(
            [binary, *verb, "--help"], capture_output=True, text=True, check=False
        )
        if result.returncode != 0:
            missing_verbs.append(" ".join(verb))
            continue
        text = result.stdout + result.stderr + root_help
        for flag in sorted(flags):
            checked_flags += 1
            if flag not in text:
                missing_flags.append(f"aion {' '.join(verb)} {flag}")
    if missing_verbs:
        raise SystemExit(
            "refusing to write the deck — this binary does not answer:\n  "
            + "\n  ".join(missing_verbs)
        )
    if missing_flags:
        raise SystemExit(
            "refusing to write the deck — this binary's help does not "
            "mention flags the deck tells the reader to type:\n  "
            + "\n  ".join(missing_flags)
        )
    printed = ", ".join(" ".join(verb) for verb in taught)
    print(
        f"  {'ok':>6}  all {len(taught)} taught verbs resolve and all "
        f"{checked_flags} flag use(s) are documented: {printed}"
    )


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
    # Printing it is not checking it. A stale `aion` earlier on PATH would
    # validate every gate below against a binary the reader will not have,
    # while the install slide advertises CLI_VERSION — the gate would be
    # measuring one thing and the deck claiming another.
    if CLI_VERSION not in version:
        raise SystemExit(
            f"refusing to build — this deck says the reader installs "
            f"{CLI_CRATE} {CLI_VERSION}, but the binary gating it reports "
            f"{version!r} ({binary}). Build and use the tree's own binary, or "
            "the gates below prove nothing about what the reader would run."
        )

    referenced = {s.excerpt.document for s in SLIDES if s.excerpt is not None}
    # `aion.toml` reaches the deck through `config_heredoc()`, not an Excerpt.
    # Counting only excerpts would report a file that IS published as unused —
    # a census that cannot see one of its own two doors tells you nothing.
    referenced |= HEREDOC_SOURCES
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
    name = gate_one_workflow_name()
    print(f"  {'ok':>6}  all {len(documents)} documents are one workflow: `{name}`")
    gate_skeleton_is_the_servers_own(binary)
    gate_verbs(binary)
    payloads = gate_command_payloads()
    print(f"      ok  {payloads} pasted command payload(s) parse as JSON")
    keywords = gate_keyword_coverage()
    print(f"      ok  all {keywords} keyword(s) the examples use are explained")

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
