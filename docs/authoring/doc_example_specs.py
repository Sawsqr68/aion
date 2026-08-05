"""How each documentation fragment becomes a checkable document.

One entry per ```awl block. `doc_example_gate.py` refuses to report at all if a
block has no entry here, so a block added to the documentation cannot slip
through as silently unmeasured.

Keep the scaffolds minimal. Every declaration listed in `needs` is something
the gate lends the fragment, and the report prints that list beside the pass —
a pass that borrows eight declarations is a weaker result than one that borrows
none, and the reader is entitled to tell them apart.
"""

from __future__ import annotations

from doc_example_gate import Spec

# Shorthands for the surrounding documents these fragments come from.
REPORT_OUTCOME = ["outcome done: type Report, route success"]
END = 'step gate_end\n  route done(summary: "ok")'

# The file the schema-door block imports. Deliberately ORDINARY — an object
# with required and optional properties and nothing exotic — because the block
# demonstrates the import, not the schema. Anything clever here would risk the
# row failing for a reason the documentation never claimed.
SCHEMA_DOOR_FIXTURE = """{
  "type": "object",
  "required": ["id"],
  "properties": {
    "id": { "type": "string", "minLength": 1 },
    "note": { "type": "string" }
  }
}
"""

SPECS: dict[str, Spec] = {
    # ---------------------------------------------------------------- whole
    "docs_GETTING-STARTED__01": Spec(place="whole"),
    "docs_authoring_AWL__01": Spec(place="whole"),
    "docs_workers_SHELL-WORKER-MANIFEST__01": Spec(place="whole"),
    "examples_assistant_resources_AWL-AUTHORING__01": Spec(place="whole"),
    "examples_assistant_resources_AWL-REFERENCE__01": Spec(
        place="whole",
        illustrative=(
            "the canonical-order skeleton; its own prose says "
            "'the sample illustrates declaration placement; adapt the flow rather "
            "than copying unused declarations'"
        ),
    ),
    "examples_assistant_resources_AWL-REFERENCE__26": Spec(place="whole"),
    "examples_assistant_resources_AWL-REFERENCE__27": Spec(place="whole"),
    "examples_assistant_resources_AWL-AUTHORING__08": Spec(place="whole"),
    "examples_assistant_resources_AWL-AUTHORING__09": Spec(place="whole"),
    # ------------------------------------------------------ declarations
    "docs_authoring_AWL__02": Spec(place="decls", needs=["Report", "Finding"], tail=END),
    "examples_assistant_resources_AWL-REFERENCE__02": Spec(place="decls", tail=END),
    "examples_assistant_resources_AWL-REFERENCE__05": Spec(
        place="decls", needs=["Report", "Finding"], tail=END
    ),
    "examples_assistant_resources_AWL-REFERENCE__06": Spec(
        place="decls", needs=["Build"], tail=END
    ),
    "examples_assistant_resources_AWL-REFERENCE__09": Spec(place="decls", tail=END),
    "examples_assistant_resources_AWL-REFERENCE__10": Spec(place="decls", tail=END),
    "examples_assistant_resources_AWL-REFERENCE__11": Spec(
        place="decls",
        needs=["Report", "Finding", "Request"],
        inputs=["request: Request"],
        tail=END,
    ),
    "examples_assistant_resources_AWL-REFERENCE__12": Spec(
        place="decls", needs=["Report", "Finding", "inspect"], tail=END
    ),
    "examples_assistant_resources_AWL-AUTHORING__02": Spec(place="decls", tail=END),
    "examples_assistant_resources_AWL-AUTHORING__03": Spec(
        place="decls", needs=["Report", "Finding"], tail=END
    ),
    "examples_assistant_resources_AWL-AUTHORING__06": Spec(
        place="decls",
        needs=["Report", "Finding", "Request"],
        inputs=["request: Request"],
        tail=END,
    ),
    "examples_assistant_resources_WORKERS__01": Spec(place="decls", tail=END),
    "examples_assistant_resources_WORKERS__02": Spec(
        place="decls", needs=["CommandResult"], tail=END
    ),
    "examples_assistant_resources_WORKERS__03": Spec(place="decls", tail=END),
    # ------------------------------------------------------------ statements
    "examples_assistant_resources_AWL-REFERENCE__07": Spec(
        place="stmts",
        needs=["Request", "Build", "build"],
        inputs=["request: Request"],
        tail=END,
    ),
    "examples_assistant_resources_AWL-REFERENCE__08": Spec(
        place="stmts",
        needs=["Request", "Build", "build"],
        inputs=["request: Request"],
        tail=END,
    ),
    "examples_assistant_resources_AWL-REFERENCE__14": Spec(
        place="stmts",
        needs=["Request", "Report", "Finding", "normalize", "inspect"],
        inputs=["request: Request"],
        outcomes=REPORT_OUTCOME,
    ),
    "examples_assistant_resources_AWL-REFERENCE__16": Spec(
        place="stmts", tail=END.replace("gate_end", "next_step")
    ),
    "examples_assistant_resources_AWL-REFERENCE__17": Spec(
        place="stmts",
        needs=["Item", "Report", "Finding", "inspect"],
        inputs=["items: [Item]"],
        tail=END,
    ),
    "examples_assistant_resources_AWL-REFERENCE__18": Spec(
        place="stmts",
        needs=["Item", "Receipt", "publish"],
        inputs=["items: [Item]"],
        tail=END,
    ),
    "examples_assistant_resources_AWL-REFERENCE__19": Spec(
        place="stmts",
        needs=["Request", "Profile", "History", "fetch_profile", "fetch_history"],
        inputs=["request: Request"],
        tail=END,
    ),
    "examples_assistant_resources_AWL-REFERENCE__21": Spec(
        place="stmts",
        needs=["Report", "Finding", "improve"],
        inputs=["initial: Report", "max_rounds: Int"],
        tail=END,
        illustrative=(
            "the loop form on its own; the prose immediately below it states the "
            "missing part — 'a step containing a loop needs explicit outcomes that "
            "distinguish completion from exhaustion'"
        ),
    ),
    "examples_assistant_resources_AWL-REFERENCE__25": Spec(
        place="stmts",
        needs=["Receipt", "remove_assets", "publish"],
        inputs=["assets: [String]"],
        stmt_head='publish(repo: "x") -> published\nroute done(summary: "ok")',
    ),
    "examples_assistant_resources_AWL-AUTHORING__04": Spec(
        place="stmts",
        needs=["Request", "Report", "Finding", "inspect"],
        inputs=["request: Request"],
        tail=END,
    ),
    # ----------------------------------------------------------------- steps
    "examples_assistant_resources_AWL-REFERENCE__13": Spec(
        place="steps",
        head="step prepare\n  route policy\n\nstep policy\n  route build",
        tail=END,
    ),
    "examples_assistant_resources_AWL-REFERENCE__15": Spec(
        place="steps",
        needs=["Finding"],
        inputs=["findings: [Finding]"],
        tail="step remediation\n" + END.split("\n", 1)[1],
    ),
    "examples_assistant_resources_AWL-REFERENCE__20": Spec(
        place="steps",
        needs=["Item", "Report", "Finding", "inspect"],
        inputs=["items: [Item]"],
        tail=END,
    ),
    "examples_assistant_resources_AWL-REFERENCE__22": Spec(
        place="steps",
        needs=["Request", "Report", "Finding", "inspect"],
        inputs=["request: Request"],
        extra_outcomes=["outcome finished: type Report, route success"],
        tail="step revise\n  route review",
    ),
    "examples_assistant_resources_AWL-REFERENCE__23": Spec(
        place="steps", needs=["Approval"], signals=["approval: Approval"], tail=END
    ),
    "examples_assistant_resources_AWL-REFERENCE__24": Spec(
        place="steps",
        needs=["Approval"],
        signals=["approval: Approval"],
        tail="step continue_work\n" + END.split("\n", 1)[1],
    ),
    "examples_assistant_resources_AWL-REFERENCE__28": Spec(
        place="steps",
        needs=["Status", "Decision", "GaveUp", "evaluate"],
        inputs=["item_id: String", "max_passes: Int"],
        queries=["review_status: Status"],
        outcomes=[
            "outcome done: type Status, route success",
            "outcome gave_up: type GaveUp, route failure",
        ],
        head='step announce\n  answer review_status(phase: "announcing")\n  route gate',
    ),
    "examples_assistant_resources_AWL-AUTHORING__05": Spec(
        place="steps",
        needs=["Request", "Report", "Finding", "inspect"],
        inputs=["request: Request"],
        head="step look\n  inspect(repo: request.repo) -> report\n  route decide",
        tail="step revision\n" + END.split("\n", 1)[1],
    ),
    "examples_assistant_resources_AWL-AUTHORING__07": Spec(
        place="steps",
        needs=["Item", "Report", "Finding", "inspect"],
        inputs=["items: [Item]"],
        tail=END,
    ),
    # ---------------------------------------------------------- unverifiable
    "examples_assistant_resources_AWL-REFERENCE__03": Spec(
        unverifiable=(
            "two expression forms shown out of statement position — checking them "
            "requires adding a binding, which would change the fragment"
        )
    ),
    # The schema-door block was filed UNVERIFIABLE for a reason that said
    # nothing about the block: the gate had no way to put a file where the
    # import looks. It does now, so the construct is checked for real. The
    # reader still brings their own schema — the report row names the file the
    # gate supplied, so this PASS cannot be read as "copy it and it runs".
    "examples_assistant_resources_AWL-REFERENCE__04": Spec(
        fixtures={"schemas/existing.schema.json": SCHEMA_DOOR_FIXTURE},
    ),
}
