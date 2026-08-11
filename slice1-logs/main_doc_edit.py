"""Rewrite the `aion worker agent` verb documentation for the document-declared launch."""

import pathlib

p = pathlib.Path("crates/aion-cli/src/main.rs")
s = p.read_text()

old = """    /// WHICH harness is `--harness`, resolved over the adapters this binary was
    /// built with: a build carrying exactly one derives it, a build carrying
    /// several refuses until one is selected and names every choice, and a name
    /// this build does not carry is refused naming what it does. The default
    /// build carries `norn` and `acp`, so it requires the flag.
    ///
    /// Each harness is configured by its own settings, demanded only when it is
    /// the resolved one. The `acp` harness needs `--acp-command`, `--acp-cwd`
    /// (absolute), `--acp-permission` (`deny` or `allow-once`, no default —
    /// it decides whether an unattended agent may write files and run commands)
    /// and `--acp-exit-grace`, plus repeatable `--acp-arg` / `--acp-env`; ACP has
    /// no standard launch flag, so nothing about the command is inferred from an
    /// agent's name. The `norn` harness's settings are all optional: `--norn-binary`
    /// if `norn` is not on the worker's PATH, repeatable `--norn-arg` pass-through
    /// arguments (with `{workflow_id}` / `{activity_type}` expanded per run), and
    /// repeatable `--norn-unset-env` to strip a variable from the spawned norn's
    /// environment. A flag configuring a harness this invocation is not serving is
    /// REFUSED, never ignored: a flag that silently does nothing is how an operator
    /// comes to believe a policy is in force when it is not.
    ///"""

new = """    /// The LAUNCH is declared in the document, in the `worker` block's `harness`
    /// section, and is not a flag: `kind` (`acp` or `norn`), `concurrency`, the
    /// three reconnect settings, `env_pass`, and the per-kind settings — `command`
    /// (absolute), `cwd` (absolute), `permission` (`deny` or `allow-once`, no
    /// default, because it decides whether an unattended agent may write files and
    /// run commands) and `exit_grace` for `acp`; the optional `binary` for `norn`;
    /// optional `args` for either. Nothing is inferred: the checker refuses a
    /// section missing any field of ITS DECLARED KIND's required set, by name, at
    /// `aion awl check`. A `kind` this binary was not built with is refused here,
    /// naming what the build does carry.
    ///
    /// `env_pass` is the agent subprocess's WHOLE environment: the document names
    /// the variables, this machine supplies their values, and a variable that is
    /// not named is absent in the agent whatever this worker's own environment
    /// carries. Nothing is inherited.
    ///"""

assert old in s
s = s.replace(old, new, 1)
p.write_text(s)
print("ok")
