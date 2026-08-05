#!/usr/bin/env python3
"""Check every ```awl block in the user-facing documentation.

Most documentation examples are FRAGMENTS — a `step`, a `worker`, three lines
of pipe — and a fragment is not a document, so `aion awl check` rejects it for
being a fragment rather than for anything the documentation claims. Reporting
that as "40 failures" measures the extraction, not the docs.

So each fragment is wrapped in the smallest document that lets its construct be
checked, and the wrapper's contents are RECORDED per block. A pass with an
empty scaffold is a strong result; a pass that needed six invented declarations
is a weaker one, and the report says which it was.

The wrapper is itself capable of lying: a scaffold that ignores where the
fragment went would report a pass for any text at all. So every wrapped block
is also checked with its fragment CORRUPTED, and a block whose corrupted form
still passes is reported as UNMEASURED, not as a pass.

    python3 docs/authoring/doc_example_gate.py            # check, print a table
    python3 docs/authoring/doc_example_gate.py --write    # also write the report

Exit status is 0 when every block is either verified or carries a stated
reason for not being verifiable.
"""

from __future__ import annotations

import argparse
import re
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
REPORT = ROOT / "docs/design/aion-authoring/DOC-EXAMPLE-GATE-FINDINGS.md"

# The user-facing documents whose AWL a reader is invited to copy.
SOURCES = [
    "docs/GETTING-STARTED.md",
    "docs/authoring/AWL.md",
    "docs/workers/SHELL-WORKER-MANIFEST.md",
    "examples/assistant/resources/AWL-REFERENCE.md",
    "examples/assistant/resources/AWL-AUTHORING.md",
    "examples/assistant/resources/WORKERS.md",
]

FENCE = re.compile(r"^```awl\s*$")
FENCE_END = re.compile(r"^```\s*$")


# --------------------------------------------------------------------------
# Extraction
# --------------------------------------------------------------------------


@dataclass(frozen=True)
class Block:
    """One ```awl fenced block, with the line it starts on."""

    source: str
    index: int
    line: int
    body: str

    @property
    def name(self) -> str:
        stem = self.source.replace("/", "_").removesuffix(".md")
        return f"{stem}__{self.index:02d}"

    @property
    def label(self) -> str:
        """How a reader would find it: the document and the line."""
        return f"{self.source}:{self.line}"


def extract(root: Path) -> list[Block]:
    blocks: list[Block] = []
    for source in SOURCES:
        path = root / source
        if not path.is_file():
            raise SystemExit(f"refusing to run — {source} does not exist")
        lines = path.read_text(encoding="utf-8").split("\n")
        index = 0
        i = 0
        while i < len(lines):
            if FENCE.match(lines[i]):
                start = i + 1
                j = start
                while j < len(lines) and not FENCE_END.match(lines[j]):
                    j += 1
                if j >= len(lines):
                    raise SystemExit(
                        f"{source}: unterminated ```awl block opened at line {i + 1}"
                    )
                index += 1
                blocks.append(
                    Block(
                        source=source,
                        index=index,
                        line=start + 1,
                        body="\n".join(lines[start:j]),
                    )
                )
                i = j
            i += 1
    return blocks


# --------------------------------------------------------------------------
# Scaffolding
# --------------------------------------------------------------------------

# Declarations the documentation's running examples refer to without declaring
# at the point of the fragment. Each is named so the report can say exactly
# what a block needed borrowed.
PIECES: dict[str, str] = {
    "Finding": "type Finding { title: String, blocking: Bool, detail: String? }",
    # A superset of the `Report` that several documents each declare differently:
    # AWL-REFERENCE's carries findings, AWL-AUTHORING's carries evidence. Lending
    # one type with every field lets a fragment from either document read its own.
    "Report": "type Report { findings: [Finding], title: String, complete: Bool, blocking: Bool, summary: String, evidence: [String], note: String? }",
    "Request": "type Request { repo: String, dry_run: Bool, id: String }",
    "Result": "type Result { summary: String, message: String }",
    "Failure": "type Failure { reason: String }",
    "Build": "type Build { artifact: String }",
    "Item": "type Item { repo: String }",
    "Receipt": "type Receipt { id: String }",
    "Profile": "type Profile { name: String }",
    "History": "type History { entries: [String] }",
    "Approval": "type Approval { reviewer: String }",
    "Progress": "type Progress { stage: String, item: String, state: String }",
    "Status": "type Status { phase: String, note: String? }",
    "Decision": "type Decision { approved: Bool }",
    "inspect": "action inspect(repo: String) -> Report",
    "build": "action build(repo: String, clean: Bool) -> Build",
    "normalize": "action normalize(request: Request) -> Request",
    "publish": "action publish(repo: String) -> Receipt",
    "fetch_profile": "action fetch_profile(id: String) -> Profile",
    "fetch_history": "action fetch_history(id: String) -> History",
    "improve": "action improve(prior: Report) -> Report",
    "remove_assets": "action remove_assets(ids: [String]) -> Receipt",
    "evaluate": "action evaluate(item: String) -> Decision",
    "perform": "action perform(request: Request) -> Result",
    "CommandResult": "type CommandResult { exit_code: Int, stdout: String }",
    "GaveUp": "type GaveUp { item: String }",
}

TYPE_PIECES = {k for k, v in PIECES.items() if v.startswith("type ")}


@dataclass
class Spec:
    """How one fragment becomes a document.

    `place` says where the fragment goes; `needs` names the declarations it
    borrows; `inputs`, `outcomes` and `tail` supply the rest of the surrounding
    document. `unverifiable` states, in the document's own terms, why a block
    cannot be checked at all.

    `fixtures` supplies files the fragment IMPORTS, written beside the composed
    document at the relative paths the fragment names. A block that imports a
    schema is otherwise unverifiable for a reason that says nothing about the
    block — the gate simply had no way to put the file where the import looks.
    Supplying it verifies the construct; every fixture is named in the report
    row so a PASS never reads as "copy this and it runs" when the reader must
    bring the file themselves.
    """

    place: str = "decls"  # whole | decls | stmts | steps
    needs: list[str] = field(default_factory=list)
    inputs: list[str] = field(default_factory=list)
    signals: list[str] = field(default_factory=list)
    sends: list[str] = field(default_factory=list)
    queries: list[str] = field(default_factory=list)
    outcomes: list[str] = field(default_factory=list)  # REPLACES the gate outcomes
    extra_outcomes: list[str] = field(default_factory=list)  # added alongside them
    stmt_head: str = ""  # statements written before the fragment, inside its step
    head: str = ""  # steps written BEFORE the fragment
    tail: str = ""  # steps written AFTER the fragment
    fixtures: dict[str, str] = field(default_factory=dict)  # relpath -> contents
    unverifiable: str = ""
    illustrative: str = ""  # a complete document that deliberately does not check


# The gate owns these. They are prefixed so a fragment that declares its own
# `Result` or `Failure` does not collide with the scaffold around it.
GATE_TYPES = [
    "type GateResult  { summary: String }",
    "type GateFailure { reason: String }",
]
GATE_OUTCOMES = [
    "outcome done: type GateResult, route success",
    "outcome failed: type GateFailure, route failure",
]


def compose(block: Block, spec: Spec) -> str:
    """Build the document that will be checked."""
    if spec.place == "whole":
        # Deliberately NOT lending narration here. A `//!` line the gate supplies
        # is a line the reader does not get, and a whole-document block that only
        # passes because the gate patched it is a block the reader cannot copy.
        return block.body

    types = [PIECES[n] for n in spec.needs if n in TYPE_PIECES]
    actions = [PIECES[n] for n in spec.needs if n not in TYPE_PIECES]
    if spec.outcomes:
        outcomes = spec.outcomes
    else:
        outcomes = GATE_OUTCOMES + spec.extra_outcomes
        types = GATE_TYPES + types

    lines = ["//! Gate scaffold around a documentation fragment.", "workflow gate_scaffold"]
    lines += [f"  input {i}" for i in spec.inputs]
    lines += [f"  signal {s}" for s in spec.signals]
    lines += [f"  send {s}" for s in spec.sends]
    lines += [f"  query {q}" for q in spec.queries]
    lines += [f"  {o}" for o in outcomes]
    lines.append("")
    lines += types
    if actions:
        lines.append("")
        lines.append("worker gate_queue")
        lines += [f"  {a}" for a in actions]
    lines.append("")

    if spec.place == "decls":
        lines.append(block.body)
        lines.append("")
        lines.append(spec.head.rstrip("\n") if spec.head else "")
        # The default tail routes exactly the fields GATE_TYPES declares on
        # `GateResult` — no more. It used to route a `message` too, which
        # `GateResult` does not have, so any block relying on the pure default
        # scaffold failed on the SCAFFOLD rather than on its own content.
        lines.append(spec.tail.rstrip("\n") if spec.tail else "step gate_end\n  route done(summary: \"ok\")")
    elif spec.place == "steps":
        if spec.head:
            lines.append(spec.head.rstrip("\n"))
        lines.append(block.body)
        if spec.tail:
            lines.append(spec.tail.rstrip("\n"))
    elif spec.place == "stmts":
        if spec.head:
            lines.append(spec.head.rstrip("\n"))
        lines.append("step gate_body")
        if spec.stmt_head:
            lines += ["  " + l for l in spec.stmt_head.split("\n")]
        lines += ["  " + line if line.strip() else "" for line in block.body.split("\n")]
        if spec.tail:
            lines.append(spec.tail.rstrip("\n"))
    else:
        raise ValueError(f"{block.name}: unknown placement {spec.place!r}")

    return "\n".join(line for line in lines if line is not None) + "\n"


def corrupt(body: str) -> str | None:
    """Break the fragment in a way the checker must notice.

    Renaming every identifier to something undeclared makes any wrapper that
    genuinely places the fragment fail. A wrapper that still passes is not
    reading the fragment at all — which is the thing worth knowing.
    """
    changed = re.sub(r"(?m)(?<![\w.$])([a-z][a-z0-9_]*)", r"zz_\1", body)
    return changed if changed != body else None


# --------------------------------------------------------------------------
# Checking
# --------------------------------------------------------------------------


def check(
    binary: str,
    text: str,
    workdir: Path,
    name: str,
    fixtures: dict[str, str] | None = None,
) -> tuple[bool, str]:
    path = workdir / f"{name}.awl"
    path.write_text(text, encoding="utf-8")
    for relative, contents in (fixtures or {}).items():
        # Imports resolve relative to the .awl document, so the fixture is
        # written relative to the document — not to the process's cwd, which
        # would make the check pass for a reason the documentation never states.
        target = path.parent / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(contents, encoding="utf-8")
    result = subprocess.run(
        [binary, "awl", "check", str(path)],
        capture_output=True,
        text=True,
        check=False,
    )
    # The composed document lives in a temporary directory whose name changes
    # every run. Leaving it in the message would make the report churn on every
    # regeneration and point at a path nobody can open.
    message = (result.stdout + result.stderr).strip().replace(f"{path}:", "")
    return result.returncode == 0, message


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--write", action="store_true", help="write the findings report")
    parser.add_argument("--keep", metavar="DIR", help="keep the composed documents here")
    args = parser.parse_args()

    binary = shutil.which("aion")
    if binary is None:
        raise SystemExit("refusing to run — `aion` is not on PATH")
    version = subprocess.run(
        [binary, "--version"], capture_output=True, text=True, check=False
    ).stdout.strip()

    blocks = extract(ROOT)
    print(f"{len(blocks)} blocks from {len(SOURCES)} documents, checked with {version}\n")

    from doc_example_specs import SPECS  # noqa: PLC0415  (data, kept separate)

    unspecced = [b.name for b in blocks if b.name not in SPECS]
    if unspecced:
        raise SystemExit(
            "refusing to report — these blocks have no spec, so they were not "
            "measured:\n  " + "\n  ".join(unspecced)
        )

    workdir = Path(args.keep) if args.keep else Path(tempfile.mkdtemp(prefix="awlgate-"))
    workdir.mkdir(parents=True, exist_ok=True)

    rows: list[tuple[Block, str, str]] = []
    for block in blocks:
        spec = SPECS[block.name]
        if spec.unverifiable:
            rows.append((block, "UNVERIFIABLE", spec.unverifiable))
            continue
        text = compose(block, spec)
        ok, message = check(binary, text, workdir, block.name, spec.fixtures)
        if spec.illustrative:
            verdict = "ILLUSTRATIVE" if not ok else "ILLUSTRATIVE-BUT-CHECKS"
            rows.append((block, verdict, spec.illustrative))
            continue
        if not ok:
            rows.append((block, "FAIL", message.split("\n")[0]))
            continue
        broken = corrupt(block.body)
        if broken is None:
            rows.append((block, "PASS", "scaffold: " + describe(spec)))
            continue
        control_ok, _ = check(
            binary,
            compose(Block(block.source, block.index, block.line, broken), spec),
            workdir,
            block.name + "__control",
        )
        if control_ok:
            rows.append(
                (block, "UNMEASURED", "the scaffold passes with the fragment corrupted")
            )
        else:
            rows.append((block, "PASS", "scaffold: " + describe(spec)))

    width = max(len(b.label) for b, _, _ in rows)
    for block, verdict, detail in rows:
        print(f"  {verdict:<14} {block.label:<{width}}  {detail}")

    counts: dict[str, int] = {}
    for _, verdict, _ in rows:
        counts[verdict] = counts.get(verdict, 0) + 1
    print("\n" + "  ".join(f"{v}: {n}" for v, n in sorted(counts.items())))
    if not args.keep:
        print(f"\ncomposed documents: {workdir}")

    if args.write:
        write_report(rows, version, counts)
        print(f"wrote {REPORT.relative_to(ROOT)}")

    return 1 if counts.get("FAIL") or counts.get("UNMEASURED") else 0


def describe(spec: Spec) -> str:
    if spec.place == "whole":
        return "none — a complete document"
    parts = [spec.place]
    if spec.needs:
        parts.append("borrows " + ", ".join(spec.needs))
    if spec.fixtures:
        # Named, not counted. A row that says "1 fixture" hides WHICH file the
        # reader has to bring, which is the only part of this they need.
        parts.append("gate supplies " + ", ".join(sorted(spec.fixtures)))
    return "; ".join(parts)


def write_report(
    rows: list[tuple[Block, str, str]], version: str, counts: dict[str, int]
) -> None:
    lines = [
        "# Gating the documentation's AWL examples",
        "",
        f"Generated by `docs/authoring/doc_example_gate.py` against **{version}**.",
        "Re-run it to re-verify; do not hand-edit this file.",
        "",
        "## What the verdicts mean",
        "",
        "| verdict | meaning |",
        "|---|---|",
        "| `PASS` | the block checks, in the scaffold named beside it |",
        "| `FAIL` | the block does not check — a claim the documentation makes that the language refuses |",
        "| `UNMEASURED` | the scaffold still passed with the fragment corrupted, so this row proves nothing |",
        "| `ILLUSTRATIVE` | a deliberately unrunnable block whose own prose says so |",
        "| `UNVERIFIABLE` | cannot be checked, with the reason stated |",
        "",
        "  ".join(f"**{v}: {n}**" for v, n in sorted(counts.items())),
        "",
        "## How a `PASS` was earned",
        "",
        "A fragment is wrapped in the smallest document its construct needs, and then",
        "the SAME wrapper is checked a second time with the fragment's identifiers",
        "mangled. If the mangled form still passed, the wrapper was not reading the",
        "fragment and the row would say `UNMEASURED` instead. Every `PASS` below",
        "survived that control.",
        "",
        "## The failures",
        "",
        *(
            [f"- `{b.label}` — {d}" for b, v, d in rows if v == "FAIL"]
            or ["Nothing failed."]
        ),
        "",
        "## Every block",
        "",
        "| block | verdict | detail |",
        "|---|---|---|",
    ]
    for block, verdict, detail in rows:
        # A checker message can contain `|` (as in `route success|failure`), which
        # would silently split the row into extra columns.
        lines.append(f"| `{block.label}` | {verdict} | {detail.replace('|', '\\|')} |")
    lines.append("")
    REPORT.write_text("\n".join(lines), encoding="utf-8")


if __name__ == "__main__":
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    sys.exit(main())
