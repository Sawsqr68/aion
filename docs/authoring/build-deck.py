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
from pathlib import Path

from deck_model import EXAMPLES, HERE, OUTPUT, SHELL, Slide
from deck_slides_a import SLIDES_A
from deck_slides_b import SLIDES_B

SLIDES = [*SLIDES_A, *SLIDES_B]


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
