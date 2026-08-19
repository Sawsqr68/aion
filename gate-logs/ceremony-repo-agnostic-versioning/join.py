"""Join a multi-line shell body into ONE line for an AWL string literal.

The naive joiner (single space) is WRONG and fails silently at check time: it
produced `m="" for f in ...`, which `sh` rejects with a syntax error the AWL
checker cannot see. `;` is required between two commands, and is a syntax error
directly after `do`, `then`, `else`, `{` or `(`.
"""
import sys, pathlib

OPEN = ("do", "then", "else", "{", "(", ";", "&&", "||", "|")

def join(src: str) -> str:
    out = []
    for raw in src.splitlines():
        s = raw.strip()
        if not s or s.startswith("###"):
            continue
        if not any(s.endswith(t) for t in OPEN):
            s += ";"
        out.append(s)
    return " ".join(out)

if __name__ == "__main__":
    sys.stdout.write(join(pathlib.Path(sys.argv[1]).read_text()))
