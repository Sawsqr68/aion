"""CLI contract and derived tutorial statistics."""

import shlex
import tomllib

from tutorial_model import EXAMPLES, ROOT, VALUELESS_FLAGS, WORKFLOW_NAME, Slide

def taught_verbs(slides: list[Slide]) -> dict[tuple[str, ...], set[str]]:
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
    for slide in slides:
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
    # The floor the reader's toolchain must clear. Derived from the crate's
    # manifest, resolving workspace inheritance — the crate has carried its
    # own raised floor before and may again, and a deck that said only "if
    # you have Rust" sends anyone below the real floor into a failed install
    # with no explanation.
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
