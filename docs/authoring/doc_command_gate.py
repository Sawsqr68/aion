#!/usr/bin/env python3
"""Check every `aion …` command the documentation claims against the binary.

`doc_example_gate.py` gates the ```awl blocks. It reads no prose, so a `--flag`
renamed in `aion-cli` leaves every sentence that names it silently false. This
gate closes that: it extracts every command string the documents print, walks
the real `--help` tree, and reports one verdict per distinct string.

What it establishes is narrow and worth stating plainly: that the subcommand
path resolves and that every flag written is one the binary accepts. It does
NOT establish that the flag means what the sentence around it says. A gate that
claimed otherwise would be a mood, not a measurement.

Usage:
    python3 docs/authoring/doc_command_gate.py [--write]

Exit 0 only when nothing fails and nothing is unmeasured.
"""

from __future__ import annotations

import argparse
import re
import shlex
import subprocess
import sys
from collections import Counter, defaultdict
from dataclasses import dataclass, field
from functools import lru_cache
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
REPORT = REPO / "docs/design/aion-authoring/DOC-COMMAND-GATE-FINDINGS.md"

SOURCES = [
    "docs/GETTING-STARTED.md",
    "docs/authoring/AWL.md",
    "docs/workers/SHELL-WORKER-MANIFEST.md",
    "examples/assistant/resources/AWL-REFERENCE.md",
    "examples/assistant/resources/AWL-AUTHORING.md",
    "examples/assistant/resources/WORKERS.md",
    "examples/assistant/resources/COMMANDS.md",
]

# Commands the documentation names in order to say they do NOT exist.
#
# Without this the gate cannot tell "the docs claim this works" from "the docs
# claim this is absent", and would report a correct sentence as a failure. The
# declaration is checked in BOTH directions: a name listed here that the binary
# turns out to offer fails just as loudly as an unlisted name it does not. So
# this cannot drift into a quiet excuse — the day someone builds one of these,
# the sentence denying it fails the gate.
ASSERTED_ABSENT = {
    "aion worker python": (
        "`examples/assistant/resources/WORKERS.md` names it under "
        "'Layer 2: not available' to state that no script host ships"
    ),
}

# A token carrying any of these is a stand-in the reader is meant to replace,
# not a literal the binary could ever resolve.
PLACEHOLDER = re.compile(r"[<>{}\[\]]|\.\.\.|…|^[A-Z][A-Z0-9_]*$")

# Where a command string stops being one command.
SHELL_BREAK = re.compile(r"\s(?:\||\|\||&&|;|>|2>|>>)\s|\s\|\s*$")

FENCED = re.compile(r"```(?:sh|bash|console|shell)\n(.*?)```", re.S)
INLINE = re.compile(r"`(aion(?:\s[^`]*)?)`")

# The coverage baseline, deliberately NOT the extractor above. A baseline
# computed with the same expression it is auditing cannot notice that
# expression going wrong — it would simply expect less and still read green.
# This one finds every backtick span, and asks separately whether the span is
# command-shaped.
BACKTICK_SPAN = re.compile(r"`([^`\n]+)`")


VERDICT_MEANING = {
    "FAIL": "the binary contradicts what the document writes",
    "UNMEASURED": "the broken form passed too, so the check never read the claim",
    "UNVERIFIABLE": "not a command string the binary could ever be asked to resolve",
    "FAMILY": "names a command family rather than a runnable command; the family exists",
    "BINARY-ONLY": "names the binary alone",
    "ABSENT": "the document says it does not exist, and the binary agrees",
    "PASS": "the subcommand path resolves and every flag written is accepted",
}


@dataclass
class Claim:
    """One `aion …` string as the documentation prints it."""

    text: str
    sites: list[str] = field(default_factory=list)


@dataclass
class Verdict:
    """What the gate could establish about one claim."""

    label: str
    detail: str


def run_help(path: tuple[str, ...]) -> subprocess.CompletedProcess[str]:
    """Ask the binary to describe one node of its command tree."""
    return subprocess.run(
        ["aion", *path, "--help"],
        capture_output=True,
        text=True,
        check=False,
        timeout=30,
    )


@lru_cache(maxsize=None)
def help_text(path: tuple[str, ...]) -> str | None:
    """Help output for a subcommand path, or None when it does not resolve."""
    proc = run_help(path)
    if proc.returncode != 0:
        return None
    return proc.stdout


@lru_cache(maxsize=None)
def subcommands(path: tuple[str, ...]) -> frozenset[str]:
    """The names listed under this node's `Commands:` block.

    Derived from the binary every run. Enumerating them here would be a second
    copy of a list the binary already owns, and the copy would drift.
    """
    text = help_text(path)
    if text is None:
        return frozenset()
    names: set[str] = set()
    in_block = False
    for line in text.splitlines():
        if line.rstrip() == "Commands:":
            in_block = True
            continue
        if in_block:
            if not line.strip():
                break
            match = re.match(r"^  (\S+)", line)
            if match:
                names.add(match.group(1))
    return frozenset(names)


@lru_cache(maxsize=None)
def demands_subcommand(path: tuple[str, ...]) -> bool:
    """Whether this node's own usage line marks a subcommand as required.

    Read off `Usage: … <COMMAND>` rather than listed here, so a node that gains
    or loses subcommands does not need this file edited to stay honest.
    """
    text = help_text(path)
    if text is None:
        return False
    for line in text.splitlines():
        if line.startswith("Usage:"):
            return "<COMMAND>" in line
    return False


def accepts_flag(path: tuple[str, ...], flag: str) -> bool:
    """Whether this subcommand's help names the flag as its own word."""
    text = help_text(path)
    if text is None:
        return False
    return re.search(rf"(?<![\w-]){re.escape(flag)}(?![\w-])", text) is not None


@lru_cache(maxsize=None)
def flag_takes_value(path: tuple[str, ...], flag: str) -> bool:
    """Whether the help writes a value placeholder immediately after the flag."""
    text = help_text(path)
    if text is None:
        return False
    pattern = rf"(?<![\w-]){re.escape(flag)}(?:[ =]<[^>\n]+>)"
    return re.search(pattern, text) is not None


@dataclass
class Walk:
    """What the binary's own command tree makes of a written command string."""

    path: tuple[str, ...]
    flags: list[str]
    positionals: list[str]
    stray: str | None  # a subcommand-position token the binary has never heard of


def walk_command(tokens: list[str]) -> Walk:
    """Resolve a written command against the real tree.

    Flags may precede the subcommand — `aion --endpoint X cancel ID` is how the
    documents write every operator command — so leading flags are consumed (with
    their values) rather than ending the walk.
    """
    path: list[str] = []
    flags: list[str] = []
    positionals: list[str] = []
    stray: str | None = None
    index = 0
    while index < len(tokens):
        token = tokens[index]
        index += 1
        if token in {"-", "--"}:
            continue
        if token.startswith("-"):
            name, _, inline_value = token.partition("=")
            if PLACEHOLDER.search(name):
                continue
            flags.append(name)
            if not inline_value and flag_takes_value(tuple(path), name):
                index += 1  # the next token is this flag's value, not a subcommand
            continue
        if token in subcommands(tuple(path)):
            path.append(token)
            continue
        if stray is None and demands_subcommand(tuple(path)):
            stray = token
        positionals.append(token)
    return Walk(tuple(path), flags, positionals, stray)


def tokenize(text: str) -> list[str] | None:
    """Words of one command as the binary would receive them, or None.

    Optional-argument brackets are stripped: the documents write `[--output <P>]`
    for a flag that is genuinely optional, and a token left as `[--output` would
    be filed as a positional and its flag never checked at all.
    """
    body = SHELL_BREAK.split(text, maxsplit=1)[0].strip()
    body = body.rstrip("\\").strip().rstrip(".,;:")
    try:
        words = shlex.split(body, comments=True)
    except ValueError:
        return None
    return [w.lstrip("[").rstrip("]") for w in words if w.strip("[]")]


def judge(text: str) -> Verdict:
    """Decide what the binary says about one documented command string."""
    tokens = tokenize(text)
    if tokens is None:
        return Verdict("UNVERIFIABLE", "not tokenizable as a shell command")
    if not tokens or tokens[0] != "aion":
        return Verdict("UNVERIFIABLE", "does not begin with the `aion` binary")

    tail = tokens[1:]
    if tail and PLACEHOLDER.search(tail[0]) and not tail[0].startswith("-"):
        return Verdict(
            "UNVERIFIABLE",
            f"names a stand-in subcommand (`{tail[0]}`), not one the binary could resolve",
        )

    walk = walk_command(tail)
    node = " ".join(("aion", *walk.path))
    # Keyed on the command path the writer named, not the whole line: a denial
    # must match `aion worker python` whether or not the sentence around it
    # carries arguments, and must match the resolving form too so that building
    # the thing turns the denial into a failure.
    named = " ".join(("aion", *walk.path, *([walk.stray] if walk.stray else [])))
    denial = ASSERTED_ABSENT.get(named)

    if walk.stray is not None:
        if denial:
            return Verdict("ABSENT", f"the binary does not offer it, as claimed — {denial}")
        return Verdict("FAIL", f"`{walk.stray}` is not a subcommand of `{node}`")
    if denial:
        return Verdict(
            "FAIL",
            f"the documentation says this does not exist, but `{node}` resolves — "
            "the denial is now false and the sentence must be retired",
        )

    rejected = [f for f in walk.flags if not accepts_flag(walk.path, f)]
    if rejected:
        return Verdict(
            "FAIL",
            f"`{node}` does not accept {', '.join(f'`{f}`' for f in rejected)}",
        )

    if not walk.path and not walk.flags:
        return Verdict(
            "BINARY-ONLY",
            "names the binary alone — nothing beyond its existence to establish",
        )
    if not walk.path:
        return Verdict("PASS", f"`aion` accepts {', '.join(f'`{f}`' for f in walk.flags)}")
    if demands_subcommand(walk.path) and not walk.flags:
        return Verdict(
            "FAMILY",
            f"`{node}` names a command family, not a runnable command; the family exists",
        )

    established = f"`{node}` resolves"
    if walk.flags:
        established += f"; accepts {', '.join(f'`{f}`' for f in walk.flags)}"
    else:
        established += " (subcommand name only)"
    if walk.positionals:
        established += f"; {len(walk.positionals)} positional arg(s) not checked"
    return Verdict("PASS", established)


def corrupt(text: str) -> list[str]:
    """Break the claim in each way the checker must notice.

    A checker that passes a command whose subcommand and flags were both
    replaced with names the binary has never heard of is not consulting the
    binary at all — which is the thing worth knowing.
    """
    tokens = tokenize(text)
    if not tokens:
        return []
    walk = walk_command(tokens[1:])
    broken: list[str] = []
    if walk.path:
        deepest = walk.path[-1]
        broken.append(
            " ".join(
                "zz" + w if w == deepest else w
                for w in tokens
            )
        )
    if walk.flags:
        broken.append(
            " ".join(
                "--zz" + w.lstrip("-") if w in walk.flags else w
                for w in tokens
            )
        )
    return broken


def collect(paths: list[str]) -> dict[str, Claim]:
    """Every distinct `aion …` string in the documents, with where it appears."""
    claims: dict[str, Claim] = {}

    def note(raw: str, source: str, line: int) -> None:
        text = raw.strip()
        if not text:
            return
        claim = claims.setdefault(text, Claim(text=text))
        site = f"{source}:{line}"
        if site not in claim.sites:
            claim.sites.append(site)

    for source in paths:
        path = REPO / source
        if not path.exists():
            raise SystemExit(f"gate refuses to run: {source} does not exist")
        content = path.read_text()
        lines = content.splitlines()
        for index, line in enumerate(lines, start=1):
            for match in INLINE.finditer(line):
                note(match.group(1), source, index)
        for match in FENCED.finditer(content):
            start = content[: match.start(1)].count("\n") + 1
            block = match.group(1).splitlines()
            offset = 0
            while offset < len(block):
                stripped = block[offset].strip().removeprefix("$").strip()
                first = offset
                # A trailing backslash means the command continues on the next
                # line; judging only the first line would check half a command.
                while stripped.endswith("\\") and offset + 1 < len(block):
                    offset += 1
                    stripped = stripped[:-1].rstrip() + " " + block[offset].strip()
                if stripped.startswith("aion ") or stripped == "aion":
                    note(stripped, source, start + first)
                offset += 1
    return claims


def uncovered(claims: dict[str, Claim]) -> list[str]:
    """Command-shaped mentions the extractor did not turn into a claim.

    A claim never extracted is never judged, so a green run over a shrinking
    set would read exactly like a green run over all of it. The rule applied
    here is mechanical rather than a list of exceptions: a mention inside
    backticks or a shell fence is a command claim; the bare word in prose
    (`cd aion`, "a local aion checkout") is the project's name and is not.
    """
    sites = Counter(site for claim in claims.values() for site in claim.sites)
    missing: list[str] = []
    for source in SOURCES:
        text = (REPO / source).read_text()
        fenced_lines: set[int] = set()
        for match in FENCED.finditer(text):
            start = text[: match.start(1)].count("\n") + 1
            for offset, line in enumerate(match.group(1).splitlines()):
                if line.strip().removeprefix("$").strip().startswith("aion"):
                    fenced_lines.add(start + offset)
        for index, line in enumerate(text.splitlines(), start=1):
            spans = [
                s
                for s in BACKTICK_SPAN.findall(line)
                if s.strip() == "aion" or s.strip().startswith("aion ")
            ]
            wanted = len(spans) + (1 if index in fenced_lines else 0)
            found = sites.get(f"{source}:{index}", 0)
            if found < wanted:
                missing.append(f"{source}:{index} ({wanted} written, {found} extracted)")
    return missing


def render(results: dict[str, tuple[Claim, Verdict]], counts: dict[str, int]) -> str:
    """The findings document, derived from the run that just happened."""
    proc = subprocess.run(
        ["aion", "--version"], capture_output=True, text=True, check=False
    )
    version = proc.stdout.strip() or "unknown"
    order = ["FAIL", "UNMEASURED", "UNVERIFIABLE", "FAMILY", "BINARY-ONLY", "ABSENT", "PASS"]
    lines = [
        "# Documentation command gate — findings",
        "",
        "**Generated** by `docs/authoring/doc_command_gate.py --write`. Do not hand-edit.",
        "",
        f"Checked against `{version}`.",
        "",
        "## What a PASS means, and what it does not",
        "",
        "A `PASS` means the binary resolves the subcommand path and accepts every",
        "flag the sentence writes. It does **not** mean the flag does what the",
        "surrounding prose says it does — no automated check can establish that, and",
        "claiming it would make this gate a mood rather than a measurement.",
        "",
        "Each `PASS` was re-checked with its deepest subcommand and each of its flags",
        "replaced by names the binary has never heard of. A claim whose broken form",
        "still passed would be reported `UNMEASURED`, never `PASS`.",
        "",
        "Positional arguments are not checked: the documents write them as stand-ins",
        "(`WORKFLOW_ID`, `FILE.awl`) that no run could resolve. The count is reported",
        "so the size of what went unchecked is visible.",
        "",
        "Each of the gate's own refusals is re-provable on demand with",
        "`--self-check`, which breaks the extractor, the absence declaration, and",
        "the subcommand walk in turn and requires each break to be caught.",
        "",
        "## Verdicts",
        "",
        "| Verdict | Count | Meaning |",
        "|---|---|---|",
        *[
            f"| {label} | {counts.get(label, 0)} | {VERDICT_MEANING[label]} |"
            for label in order
        ],
        "",
        "| Verdict | Command | Established | Appears |",
        "|---|---|---|---|",
    ]
    rank = {label: i for i, label in enumerate(order)}
    for text, (claim, verdict) in sorted(
        results.items(), key=lambda kv: (rank.get(kv[1][1].label, 99), kv[0])
    ):
        detail = verdict.detail.replace("|", r"\|")
        sites = "<br>".join(claim.sites)
        lines.append(
            f"| {verdict.label} | `{text}` | {detail} | {sites} |"
        )
    lines.append("")
    return "\n".join(lines)


def self_check() -> int:
    """Prove each refusal can actually fire, on demand rather than once.

    Every one of these was written because the first version of the check it
    exercises passed while broken. They are kept runnable so a later reader can
    re-establish that, instead of taking a commit message's word for it.
    """
    global INLINE  # noqa: PLW0603 — deliberately swapped to break the extractor
    failures: list[str] = []

    original = INLINE
    INLINE = re.compile(r"`(aion never-matches(?:\s[^`]*)?)`")
    try:
        blinded = uncovered(collect(SOURCES))
    finally:
        INLINE = original
    if not blinded:
        failures.append(
            "a crippled extractor was not noticed — the coverage baseline is "
            "being computed from the extractor it audits"
        )

    live = next(iter(subcommands(("awl",))), None)
    if live is None:
        failures.append("could not reach the binary's command tree to run the controls")
    else:
        ASSERTED_ABSENT[f"aion awl {live}"] = "control only"
        try:
            if judge(f"aion awl {live}").label != "FAIL":
                failures.append(
                    f"denying `aion awl {live}`, which the binary offers, did not fail"
                )
        finally:
            del ASSERTED_ABSENT[f"aion awl {live}"]

    if judge("aion worker zz-nonexistent").label != "FAIL":
        failures.append("an undeclared command the binary lacks did not fail")

    for name in ASSERTED_ABSENT:
        if judge(name).label != "ABSENT":
            failures.append(f"the standing denial of `{name}` no longer reads as absent")

    for line in failures:
        print(f"CONTROL FAILED  {line}")
    if not failures:
        print("controls: 4 checked, all fire as intended")
    return 1 if failures else 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--write", action="store_true", help="regenerate the findings document"
    )
    parser.add_argument(
        "--self-check",
        action="store_true",
        help="prove each of the gate's own refusals can still fire, then exit",
    )
    args = parser.parse_args()

    if subprocess.run(
        ["which", "aion"], capture_output=True, check=False
    ).returncode != 0:
        print("gate refuses to run: `aion` is not on PATH", file=sys.stderr)
        return 2

    if args.self_check:
        return self_check()

    claims = collect(SOURCES)
    dropped = uncovered(claims)
    if dropped:
        print("gate refuses to report — command-shaped mentions went unextracted:")
        for site in dropped:
            print(f"  {site}")
        return 2

    results: dict[str, tuple[Claim, Verdict]] = {}
    counts: dict[str, int] = defaultdict(int)

    for text, claim in claims.items():
        verdict = judge(text)
        if verdict.label in {"PASS", "FAMILY"}:
            intact = verdict.label
            survived = [b for b in corrupt(text) if judge(b).label == intact]
            if survived:
                verdict = Verdict(
                    "UNMEASURED",
                    f"the broken form `{survived[0]}` passed too — the check is not "
                    "reading this claim",
                )
        results[text] = (claim, verdict)
        counts[verdict.label] += 1

    for text, (claim, verdict) in sorted(results.items()):
        if verdict.label in {"PASS", "BINARY-ONLY", "FAMILY", "ABSENT"}:
            continue
        print(f"{verdict.label:13} {claim.sites[0]:55} {text}")
        print(f"{'':13} {verdict.detail}")

    print()
    print("  ".join(f"{k}: {v}" for k, v in sorted(counts.items())))

    if args.write:
        REPORT.write_text(render(results, counts))
        print(f"wrote {REPORT.relative_to(REPO)}")

    return 1 if counts.get("FAIL") or counts.get("UNMEASURED") else 0


if __name__ == "__main__":
    sys.exit(main())
