"""Generate the harness-section fixture corpus."""

import pathlib

root = pathlib.Path("crates/aion-awl/tests/fixtures/rev2/harness-section")
(root / "valid").mkdir(parents=True, exist_ok=True)
(root / "invalid").mkdir(parents=True, exist_ok=True)

HEAD = """workflow ask_the_agent
  input question: String
  outcome answered: type Answer, route success

type Answer {{ reply: String }}

/// The canonical agent-outcome record: the checker demands the STRUCTURE, not
/// a blessed name.
type Reply {{ text: String, stop_reason: String }}

worker assistants
  harness
{settings}  action ask(prompt: String) -> Reply
    agent

step ask
  ask(prompt: question) -> reply
  route answered(reply: reply.text)
"""


def doc(narration, settings):
    body = "".join(f"    {line}\n" for line in settings)
    return narration + HEAD.format(settings=body)


ACP = [
    "kind acp",
    "concurrency 4",
    "reconnect_initial_backoff 5s",
    "reconnect_max_backoff 2m",
    "reconnect_max_attempts 100",
    'env_pass "PATH", "HOME"',
    'command "/usr/local/bin/claude-code-acp"',
    'cwd "/srv/agents/workspace"',
    'permission "deny"',
    "exit_grace 10s",
]

NORN = [
    "kind norn",
    "concurrency 2",
    "reconnect_initial_backoff 5s",
    "reconnect_max_backoff 1m",
    "reconnect_max_attempts 50",
    'env_pass "PATH", "HOME"',
    'args "--account", "default"',
    'binary "/usr/local/bin/norn"',
]


def without(settings, prefix):
    return [line for line in settings if line.split()[0] != prefix]


def replace(settings, prefix, new):
    return [new if line.split()[0] == prefix else line for line in settings]


valid = {
    "acp_harness.awl": (
        """//! A complete `acp` harness declaration: an Agent Client Protocol agent
//! launched as a subprocess, with every setting the operator's own. It carries
//! no Norn setting and is COMPLETE — the required set is scoped to the declared
//! kind, so an `acp` document is not asked for a `norn` binary it has no use
//! for.
//!
//! The `env_pass` list is the child's WHOLE environment: `PATH` and `HOME` and
//! nothing else cross into the agent, whatever the launching shell exports.
""",
        ACP,
    ),
    "norn_harness.awl": (
        """//! A complete `norn` harness declaration. It carries no `command`, no `cwd`,
//! no `permission` and no `exit_grace` — those are ACP's, and Norn's argv is
//! the adapter's own — and it is COMPLETE, for the same kind-scoping reason the
//! ACP document is.
""",
        NORN,
    ),
    "norn_harness_without_optional_settings.awl": (
        """//! The same `norn` launch with both optional settings omitted: no `args`
//! (the agent needs none) and no `binary` (the adapter's own documented
//! `norn`-on-`PATH` default stands, which is the library's value and not one
//! invented at this boundary). A section is complete when it carries its
//! kind's REQUIRED set; an optional setting left out is a declaration, not a
//! gap.
""",
        without(without(NORN, "args"), "binary"),
    ),
}

for name, (narration, settings) in valid.items():
    (root / "valid" / name).write_text(doc(narration, settings))

missing_cases = [
    ("missing_kind", "kind", ACP, "declares no `kind`"),
    ("missing_concurrency", "concurrency", ACP, "declares no `concurrency`"),
    (
        "missing_reconnect_initial_backoff",
        "reconnect_initial_backoff",
        ACP,
        "declares no `reconnect_initial_backoff`",
    ),
    (
        "missing_reconnect_max_backoff",
        "reconnect_max_backoff",
        ACP,
        "declares no `reconnect_max_backoff`",
    ),
    (
        "missing_reconnect_max_attempts",
        "reconnect_max_attempts",
        ACP,
        "declares no `reconnect_max_attempts`",
    ),
    ("missing_env_pass", "env_pass", ACP, "declares no `env_pass`"),
    ("missing_acp_command", "command", ACP, "declares no `command`"),
    ("missing_acp_cwd", "cwd", ACP, "declares no `cwd`"),
    ("missing_acp_permission", "permission", ACP, "declares no `permission`"),
    ("missing_acp_exit_grace", "exit_grace", ACP, "declares no `exit_grace`"),
    ("missing_kind_on_a_norn_shape", "kind", NORN, "declares no `kind`"),
    ("missing_env_pass_on_norn", "env_pass", NORN, "declares no `env_pass`"),
    (
        "missing_concurrency_on_norn",
        "concurrency",
        NORN,
        "declares no `concurrency`",
    ),
]

for name, field, base, substring in missing_cases:
    settings = without(base, field)
    narration = (
        f"//! The `{field}` setting removed. Nothing supplies it, so the refusal\n"
        f"//! names the field and the queue whose section omitted it.\n"
    )
    text = doc(narration, settings)
    (root / "invalid" / f"{name}.awl").write_text(text)
    line = text.split("\n").index("  harness") + 1
    (root / "invalid" / f"{name}.expected").write_text(
        f"CHECK\n{substring}\n{line}\n"
    )

# Kind-scoped refusals: a setting written for a kind the section does not declare.
foreign_cases = [
    (
        "norn_section_carrying_an_acp_command",
        replace(NORN, "binary", 'command "/usr/local/bin/claude-code-acp"'),
        "`command` configures the `acp` harness",
        'command "/usr/local/bin/claude-code-acp"',
        "A `command` written on a `norn` section configures nothing: Norn's argv is\n"
        "//! the adapter's own. A setting that silently does nothing is how an operator\n"
        "//! comes to believe a policy is in force when it is not, so it is refused.",
    ),
    (
        "acp_section_carrying_a_norn_binary",
        ACP + ['binary "/usr/local/bin/norn"'],
        "`binary` configures the `norn` harness",
        'binary "/usr/local/bin/norn"',
        "A `binary` written on an `acp` section configures nothing: the ACP agent's\n"
        "//! executable is `command`.",
    ),
]

for name, settings, substring, needle, why in foreign_cases:
    narration = f"//! {why}\n"
    text = doc(narration, settings)
    (root / "invalid" / f"{name}.awl").write_text(text)
    line = next(
        index + 1
        for index, source_line in enumerate(text.split("\n"))
        if needle in source_line
    )
    (root / "invalid" / f"{name}.expected").write_text(
        f"CHECK\n{substring}\n{line}\n"
    )

value_cases = [
    (
        "unknown_kind",
        replace(ACP, "kind", "kind claude"),
        "names no agent harness",
        "kind claude",
        "A kind that names no harness. Nothing is inferred from an agent's name.",
    ),
    (
        "unknown_permission",
        replace(ACP, "permission", 'permission "ask"'),
        "names no permission policy",
        'permission "ask"',
        "An unattended activity has no operator to ask, so there is no third\n"
        "//! reading of the permission decision.",
    ),
    (
        "relative_acp_command",
        replace(ACP, "command", 'command "claude-code-acp"'),
        "must be an ABSOLUTE path",
        'command "claude-code-acp"',
        "A worker is started from whatever directory a supervisor or a shell\n"
        "//! happens to be in, so a relative command names a different file on every\n"
        "//! launch.",
    ),
    (
        "env_pass_carrying_a_value",
        replace(ACP, "env_pass", 'env_pass "PATH", "ANTHROPIC_API_KEY=sk-live-1"'),
        "takes variable NAMES",
        "ANTHROPIC_API_KEY=sk-live-1",
        "The list says WHICH variables cross into the harness child; each value\n"
        "//! comes from the launching context. Writing a value into a document puts a\n"
        "//! secret in a file that gets committed, diffed and deployed.",
    ),
    (
        "zero_concurrency",
        replace(ACP, "concurrency", "concurrency 0"),
        "must be greater than zero",
        "concurrency 0",
        "A worker that serves no activities at once is a stopped worker rather\n"
        "//! than a configured one.",
    ),
    (
        "reconnect_cap_below_the_first_wait",
        replace(ACP, "reconnect_max_backoff", "reconnect_max_backoff 1s"),
        "is shorter than",
        "reconnect_max_backoff 1s",
        "The backoff climbs from the initial delay towards the cap, so a cap\n"
        "//! already exceeded by the first wait is not a policy anything can follow.",
    ),
]

for name, settings, substring, needle, why in value_cases:
    narration = f"//! {why}\n"
    text = doc(narration, settings)
    (root / "invalid" / f"{name}.awl").write_text(text)
    line = next(
        index + 1
        for index, source_line in enumerate(text.split("\n"))
        if needle in source_line
    )
    (root / "invalid" / f"{name}.expected").write_text(
        f"CHECK\n{substring}\n{line}\n"
    )

# A harness on a queue whose every action carries a declared body: nothing to serve.
NOTHING_TO_SERVE = """//! Every action on this queue carries a declared body, which whatever
//! dispatched it runs itself. A harness section here would launch a subprocess
//! no work is ever sent to.
worker greeter
  harness
    kind norn
    concurrency 1
    reconnect_initial_backoff 5s
    reconnect_max_backoff 1m
    reconnect_max_attempts 10
    env_pass "PATH"
  action greet(name: String) -> String
    run text "printf %s $name"
"""
(root / "invalid" / "harness_with_nothing_to_serve.awl").write_text(NOTHING_TO_SERVE)
(root / "invalid" / "harness_with_nothing_to_serve.expected").write_text(
    "CHECK\nnothing on this queue for an agent harness to serve\n5\n"
)

print("valid:", sorted(p.name for p in (root / "valid").iterdir()))
print("invalid awl:", len(list((root / "invalid").glob("*.awl"))))
