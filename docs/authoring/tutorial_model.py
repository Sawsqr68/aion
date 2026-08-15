"""Shared paths and value objects for the generated AWL tutorial."""

import html
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
