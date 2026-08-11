"""m9: let the deck be gated by the binary under change, not only the installed one."""

import pathlib

p = pathlib.Path("docs/authoring/build-tutorial.py")
s = p.read_text()

old = '''def _binary() -> str:
    found = shutil.which("aion")
    if found is None:
        raise SystemExit(
            "refusing to build: `aion` is not on PATH, so nothing in this "
            f"deck can be checked. Install it with `cargo install "
            f"{CLI_CRATE}`, or from this clone with `cargo install --path "
            "crates/aion-cli`."
        )
    return found'''

new = '''# Overrides which `aion` gates the deck. Without it the gate uses whatever is
# installed on PATH — which is the right default for an author checking that a
# RELEASED binary can follow the deck, and exactly wrong when the change being
# made is to the CLI itself. A deck gated by a binary that predates the change
# reports green about a command line that no longer exists; that is how the
# `--concurrency` slide survived the flags being deleted.
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
    return found'''

if old not in s:
    raise SystemExit("_binary context not found")
s = s.replace(old, new, 1)

if "\nimport os\n" not in s:
    s = s.replace("import shutil\n", "import os\nimport shutil\n", 1)
p.write_text(s)
print("override added:", "AION_TUTORIAL_BIN" in s)
print("os imported:", "\nimport os\n" in s)
