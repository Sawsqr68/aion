"""Shared model and excerpt helpers for the generated AWL deck."""

from __future__ import annotations

import html
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
