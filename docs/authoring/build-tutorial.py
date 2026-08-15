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
import os
import shutil
import subprocess
import sys
from pathlib import Path

from tutorial_cli import CLI_CRATE, CLI_VERSION, taught_verbs
from tutorial_model import DECK_TITLE, EXAMPLES, HERE, OUTPUT, SHELL, SHELL_TITLE, Slide
from tutorial_slides import SLIDES
from tutorial_source_gates import (
    HEREDOC_SOURCES,
    gate_command_payloads,
    gate_keyword_coverage,
    gate_one_workflow_name,
    gate_skeleton_is_the_servers_own,
)

BIN_ENV = "AION_TUTORIAL_BIN"


def _binary() -> str:
    override = os.environ.get(BIN_ENV, "").strip()
    if override:
        path = Path(override)
        if not path.is_file():
            raise SystemExit(
                f"refusing to build: ${BIN_ENV} is set to `{override}`, which is "
                "not a file. Point it at a built `aion` (for this clone: "
                "`target/debug/aion`), or unset it to gate against PATH."
            )
        return str(path.resolve())
    found = shutil.which("aion")
    if found is None:
        raise SystemExit(
            "refusing to build: `aion` is not on PATH, so nothing in this "
            f"deck can be checked. Install it with `cargo install "
            f"{CLI_CRATE}`, or from this clone with `cargo install --path "
            f"crates/aion-cli`. To gate against a binary you have just built "
            f"instead, set ${BIN_ENV} to its path."
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
    taught = taught_verbs(SLIDES)
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
    payloads = gate_command_payloads(SLIDES)
    print(f"      ok  {payloads} pasted command payload(s) parse as JSON")
    keywords = gate_keyword_coverage(SLIDES)
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
