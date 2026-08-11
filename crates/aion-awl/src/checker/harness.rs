//! The `harness` section's rules: every setting the declared kind needs, and no
//! setting for a kind it is not.
//!
//! # Why the required set is scoped to the declared kind
//!
//! An ACP agent's launch command is genuinely not standardised — nothing may be inferred
//! from an agent's name — so an `acp` section without a `command` cannot launch anything
//! and is refused. A `norn` section has no use for a command at all: the adapter's own
//! argv is fixed. Demanding one anyway would make an operator write a value that does
//! nothing, which is how a file comes to be full of settings nobody believes.
//!
//! So the required set is the DECLARED KIND's, and a complete `acp` section is complete
//! even though it carries no Norn setting.
//!
//! # Why a foreign setting is refused rather than ignored
//!
//! A setting that silently does nothing is how an operator comes to believe a policy is
//! in force when it is not: `permission "deny"` accepted and ignored on a `norn` section
//! reads exactly like a denial being enforced. Every setting is attributed to a kind,
//! and one belonging to a kind this section does not declare is named.
//!
//! # Why nothing here supplies a value
//!
//! Every setting is operator policy — how many activities run at once, how long an agent
//! gets to flush its logs, which variables cross into a subprocess. A default invented
//! here would be a decision made by whoever wrote this file for every deployment that
//! ever runs, and it would be invisible in the document an operator reads. A missing
//! setting is named; it is never filled in.

use crate::Span;
use crate::ast::{
    HARNESS_ARGS_FIELD, HARNESS_BINARY_FIELD, HARNESS_COMMAND_FIELD, HARNESS_CONCURRENCY_FIELD,
    HARNESS_CWD_FIELD, HARNESS_ENV_PASS_FIELD, HARNESS_EXIT_GRACE_FIELD, HARNESS_KIND_ACP,
    HARNESS_KIND_FIELD, HARNESS_KIND_NORN, HARNESS_PERMISSION_ALLOW_ONCE, HARNESS_PERMISSION_DENY,
    HARNESS_PERMISSION_FIELD, HARNESS_RECONNECT_INITIAL_BACKOFF_FIELD,
    HARNESS_RECONNECT_MAX_ATTEMPTS_FIELD, HARNESS_RECONNECT_MAX_BACKOFF_FIELD, HARNESS_WORD,
    HarnessDecl, HarnessSetting, HarnessText,
};

use super::context::Ctx;

/// Run the harness pass over every worker block in the document.
///
/// Both families get it: a `worker` block declaring an agent queue is the same
/// construct in a worker document and in the workflow document that requires the queue,
/// and a launch declared in one and refused in the other would be a rule stated twice.
pub(super) fn run(ctx: &mut Ctx<'_>) {
    let mut refusals = Vec::new();
    for worker in &ctx.doc.workers {
        let Some(harness) = &worker.harness else {
            continue;
        };
        // A harness serves the actions the document leaves for a worker to implement.
        // Every action carrying a declared body is run by whatever dispatched it, so a
        // queue where all of them do has nothing for an agent to serve — and the launch
        // would start a subprocess that is never sent any work. Refused here rather
        // than by the worker at registration, where the reason lives on the far side
        // of a socket.
        if worker.actions.iter().all(|action| action.body.is_some()) {
            let bodied = worker
                .actions
                .iter()
                .map(|action| action.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            refusals.push((
                harness.span,
                format!(
                    "worker `{queue}` declares a `{HARNESS_WORD}` section, but every one of its \
                     actions carries a declared body ({bodied}), which whatever dispatched them \
                     runs itself. There is nothing on this queue for an agent harness to serve, \
                     so the launch would start a subprocess no work is ever sent to. Drop the \
                     section, or declare the action an agent serves without a body.",
                    queue = worker.name
                ),
            ));
        }
        refusals.extend(check_section(&worker.name, harness));
    }
    for (span, message) in refusals {
        ctx.error(span, message);
    }
}

/// Every refusal one section earns.
fn check_section(queue: &str, harness: &HarnessDecl) -> Vec<(Span, String)> {
    let mut refusals = Vec::new();
    check_kind_independent(queue, harness, &mut refusals);
    match harness.kind_word() {
        Some(HARNESS_KIND_ACP) => check_acp(queue, harness, &mut refusals),
        Some(HARNESS_KIND_NORN) => check_norn(queue, harness, &mut refusals),
        // The kind itself is already refused below (missing, or a word that names no
        // harness). The per-kind required set is unknowable without it, and guessing a
        // kind to check against would report settings the operator never asked for.
        _ => {}
    }
    refusals
}

/// The settings every harness needs, whatever kind it is.
fn check_kind_independent(queue: &str, harness: &HarnessDecl, refusals: &mut Vec<(Span, String)>) {
    check_kind(queue, harness, refusals);
    check_concurrency(queue, harness, refusals);
    check_reconnect(queue, harness, refusals);
    check_environment(queue, harness, refusals);
}

/// Which harness the section declares — the setting every other one is scoped by.
fn check_kind(queue: &str, harness: &HarnessDecl, refusals: &mut Vec<(Span, String)>) {
    match &harness.kind {
        None => refusals.push((
            harness.span,
            missing(
                queue,
                HARNESS_KIND_FIELD,
                &format!(
                    "which harness this queue's agent worker launches decides every other \
                     setting the section needs; write `{HARNESS_KIND_FIELD} \
                     {HARNESS_KIND_ACP}` or `{HARNESS_KIND_FIELD} {HARNESS_KIND_NORN}`"
                ),
            ),
        )),
        Some(kind) => {
            let word = kind.value.word.as_str();
            if word != HARNESS_KIND_ACP && word != HARNESS_KIND_NORN {
                refusals.push((
                    kind.value.span,
                    format!(
                        "`{HARNESS_KIND_FIELD} {word}` names no agent harness; the harnesses \
                         are `{HARNESS_KIND_ACP}` (an Agent Client Protocol agent launched as \
                         a subprocess) and `{HARNESS_KIND_NORN}`."
                    ),
                ));
            }
        }
    }
}

/// How many activities the worker serves at once.
fn check_concurrency(queue: &str, harness: &HarnessDecl, refusals: &mut Vec<(Span, String)>) {
    match &harness.concurrency {
        None => refusals.push((
            harness.span,
            missing(
                queue,
                HARNESS_CONCURRENCY_FIELD,
                "how many activities this worker serves at once is operator policy, and a \
                 guess here would size somebody else's machine",
            ),
        )),
        Some(concurrency) if concurrency.value.value == 0 => refusals.push((
            concurrency.value.span,
            format!(
                "`{HARNESS_CONCURRENCY_FIELD}` must be greater than zero; a worker that serves \
                 no activities at once serves none at all, which is a stopped worker rather \
                 than a configured one"
            ),
        )),
        Some(_) => {}
    }
}

/// The three reconnect settings, and the one relation between them.
fn check_reconnect(queue: &str, harness: &HarnessDecl, refusals: &mut Vec<(Span, String)>) {
    positive_duration(
        queue,
        harness,
        HARNESS_RECONNECT_INITIAL_BACKOFF_FIELD,
        harness.reconnect_initial_backoff.as_ref(),
        "how long to wait before the first reconnect attempt is operator policy",
        refusals,
    );
    positive_duration(
        queue,
        harness,
        HARNESS_RECONNECT_MAX_BACKOFF_FIELD,
        harness.reconnect_max_backoff.as_ref(),
        "the cap the reconnect backoff climbs to is operator policy",
        refusals,
    );
    if let (Some(initial), Some(max)) = (
        &harness.reconnect_initial_backoff,
        &harness.reconnect_max_backoff,
    ) && let (Some(initial_duration), Some(max_duration)) = (
        initial.value.checked_duration(),
        max.value.checked_duration(),
    ) && initial_duration > max_duration
    {
        refusals.push((
            max.value.span,
            format!(
                "`{HARNESS_RECONNECT_MAX_BACKOFF_FIELD}` is shorter than \
                 `{HARNESS_RECONNECT_INITIAL_BACKOFF_FIELD}`, so the cap is already exceeded by \
                 the first wait; the backoff climbs from the initial delay towards the cap"
            ),
        ));
    }

    match &harness.reconnect_max_attempts {
        None => refusals.push((
            harness.span,
            missing(
                queue,
                HARNESS_RECONNECT_MAX_ATTEMPTS_FIELD,
                "how many session drops a run may ride out before it fails is operator policy",
            ),
        )),
        Some(attempts) if attempts.value.value == 0 => refusals.push((
            attempts.value.span,
            format!(
                "`{HARNESS_RECONNECT_MAX_ATTEMPTS_FIELD}` must be greater than zero; a budget \
                 of none fails the run on the first drop, which is not a reconnect policy"
            ),
        )),
        Some(_) => {}
    }
}

/// What crosses the process boundary: the pass-through list and the agent's arguments.
fn check_environment(queue: &str, harness: &HarnessDecl, refusals: &mut Vec<(Span, String)>) {
    match &harness.env_pass {
        None => refusals.push((
            harness.span,
            missing(
                queue,
                HARNESS_ENV_PASS_FIELD,
                "the harness child's environment is CONSTRUCTED from this list and never \
                 inherited, so a section without it declares an environment nobody chose; \
                 name every variable the agent needs, for example \
                 `env_pass \"PATH\", \"HOME\"`",
            ),
        )),
        Some(env_pass) => refusals.extend(variable_names(env_pass)),
    }

    if let Some(args) = &harness.args {
        for entry in &args.value {
            if entry.value.trim().is_empty() {
                refusals.push((
                    entry.span,
                    format!(
                        "`{HARNESS_ARGS_FIELD}` carries an empty argument; an empty string is \
                         passed to the agent as a real, empty argument, so it is written \
                         deliberately or not at all"
                    ),
                ));
            }
        }
    }
}

/// The `acp` kind's own settings.
fn check_acp(queue: &str, harness: &HarnessDecl, refusals: &mut Vec<(Span, String)>) {
    absolute_path(
        queue,
        harness,
        HARNESS_COMMAND_FIELD,
        harness.command.as_ref(),
        "an ACP agent's launch command is not standardised and nothing may be inferred from \
         an agent's name — a wrong guess spawns the agent in its interactive mode, which \
         speaks no JSON-RPC at all",
        refusals,
    );
    absolute_path(
        queue,
        harness,
        HARNESS_CWD_FIELD,
        harness.cwd.as_ref(),
        "ACP's session/new requires an absolute working directory, and defaulting it to the \
         worker's own would root the agent somewhere nobody chose",
        refusals,
    );

    match &harness.permission {
        None => refusals.push((
            harness.span,
            missing(
                queue,
                HARNESS_PERMISSION_FIELD,
                &format!(
                    "an unattended activity has no operator to ask, so how the agent's \
                     permission requests are answered is decided up front: write \
                     `{HARNESS_PERMISSION_FIELD} \"{HARNESS_PERMISSION_DENY}\"` or \
                     `{HARNESS_PERMISSION_FIELD} \"{HARNESS_PERMISSION_ALLOW_ONCE}\"`"
                ),
            ),
        )),
        Some(permission) => {
            let value = permission.value.value.as_str();
            if value != HARNESS_PERMISSION_DENY && value != HARNESS_PERMISSION_ALLOW_ONCE {
                refusals.push((
                    permission.value.span,
                    format!(
                        "`{HARNESS_PERMISSION_FIELD} \"{value}\"` names no permission policy; \
                         it is `\"{HARNESS_PERMISSION_DENY}\"` (refuse every gated action) or \
                         `\"{HARNESS_PERMISSION_ALLOW_ONCE}\"` (a one-shot approval, never a \
                         remember-this-choice grant). There is no third reading of a security \
                         decision."
                    ),
                ));
            }
        }
    }

    match &harness.exit_grace {
        None => refusals.push((
            harness.span,
            missing(
                queue,
                HARNESS_EXIT_GRACE_FIELD,
                "how long an agent needs to flush its logs and release what it holds once its \
                 stdin closes is a property of THAT agent; `0s` is a legitimate setting and \
                 means close stdin, then kill at once",
            ),
        )),
        Some(grace) if grace.value.checked_duration().is_none() => refusals.push((
            grace.value.span,
            format!("`{HARNESS_EXIT_GRACE_FIELD}` is too large to represent"),
        )),
        Some(_) => {}
    }

    foreign(
        harness.binary.as_ref().map(|setting| setting.span),
        HARNESS_BINARY_FIELD,
        HARNESS_KIND_ACP,
        HARNESS_KIND_NORN,
        refusals,
    );
}

/// The `norn` kind's own settings.
///
/// Norn's argv is the adapter's, so there is no `command` to declare and no `cwd` and no
/// permission policy: those are ACP's, and one written here would configure nothing.
/// `binary` is genuinely optional — absent, the adapter uses its own documented
/// `norn`-on-`PATH` default, which is the library's value and not one invented here.
fn check_norn(_queue: &str, harness: &HarnessDecl, refusals: &mut Vec<(Span, String)>) {
    for (span, field) in [
        (
            harness.command.as_ref().map(|setting| setting.span),
            HARNESS_COMMAND_FIELD,
        ),
        (
            harness.cwd.as_ref().map(|setting| setting.span),
            HARNESS_CWD_FIELD,
        ),
        (
            harness.permission.as_ref().map(|setting| setting.span),
            HARNESS_PERMISSION_FIELD,
        ),
        (
            harness.exit_grace.as_ref().map(|setting| setting.span),
            HARNESS_EXIT_GRACE_FIELD,
        ),
    ] {
        foreign(span, field, HARNESS_KIND_NORN, HARNESS_KIND_ACP, refusals);
    }

    if let Some(binary) = &harness.binary
        && binary.value.value.trim().is_empty()
    {
        refusals.push((
            binary.value.span,
            format!(
                "`{HARNESS_BINARY_FIELD}` is empty; omit it to take the adapter's own \
                 `{HARNESS_KIND_NORN}`-on-`PATH` default, or name the binary"
            ),
        ));
    }
}

/// Refuses a setting that configures a kind this section does not declare.
fn foreign(
    span: Option<Span>,
    field: &str,
    declared: &str,
    belongs_to: &str,
    refusals: &mut Vec<(Span, String)>,
) {
    let Some(span) = span else {
        return;
    };
    refusals.push((
        span,
        format!(
            "`{field}` configures the `{belongs_to}` harness, and this section declares \
             `{HARNESS_KIND_FIELD} {declared}`. A setting that silently does nothing is how an \
             operator comes to believe a policy is in force when it is not, so it is refused \
             rather than ignored."
        ),
    ));
}

/// A required duration setting that must be positive and representable.
fn positive_duration(
    queue: &str,
    harness: &HarnessDecl,
    field: &str,
    setting: Option<&HarnessSetting<crate::ast::DurationLiteral>>,
    why: &str,
    refusals: &mut Vec<(Span, String)>,
) {
    let Some(setting) = setting else {
        refusals.push((harness.span, missing(queue, field, why)));
        return;
    };
    if setting.value.magnitude == 0 {
        refusals.push((
            setting.value.span,
            format!("`{field}` must be a positive duration"),
        ));
        return;
    }
    if setting.value.checked_duration().is_none() {
        refusals.push((
            setting.value.span,
            format!("`{field}` is too large to represent"),
        ));
    }
}

/// A required path setting that must be absolute.
///
/// The path is never made absolute for the operator: resolving it against whatever
/// directory the worker was started in would root an agent somewhere nobody chose, which
/// is the same silent scoping the missing default exists to prevent.
fn absolute_path(
    queue: &str,
    harness: &HarnessDecl,
    field: &str,
    setting: Option<&HarnessSetting<HarnessText>>,
    why: &str,
    refusals: &mut Vec<(Span, String)>,
) {
    let Some(setting) = setting else {
        refusals.push((harness.span, missing(queue, field, why)));
        return;
    };
    let value = setting.value.value.as_str();
    if value.trim().is_empty() {
        refusals.push((
            setting.value.span,
            format!("`{field}` is empty; it names an absolute path"),
        ));
        return;
    }
    if !std::path::Path::new(value).is_absolute() {
        refusals.push((
            setting.value.span,
            format!(
                "`{field} \"{value}\"` must be an ABSOLUTE path. A worker is started from \
                 whatever directory a supervisor or a shell happens to be in, so a relative \
                 path names a different place on every launch."
            ),
        ));
    }
}

/// Every refusal an `env_pass` list earns.
fn variable_names(setting: &HarnessSetting<Vec<HarnessText>>) -> Vec<(Span, String)> {
    let mut refusals = Vec::new();
    for entry in &setting.value {
        let name = entry.value.as_str();
        if name.trim().is_empty() {
            refusals.push((
                entry.span,
                format!(
                    "`{HARNESS_ENV_PASS_FIELD}` carries an empty variable name; no process \
                     environment can hold one"
                ),
            ));
            continue;
        }
        if name.contains('=') {
            refusals.push((
                entry.span,
                format!(
                    "`{HARNESS_ENV_PASS_FIELD}` takes variable NAMES, and `{name}` contains \
                     `=`. The list says which variables cross into the harness child; each \
                     value comes from the launching context, so a `KEY=VALUE` pair here is a \
                     mistake to correct rather than reinterpret — and writing a value into a \
                     document puts it in a file that gets committed, diffed and deployed."
                ),
            ));
        }
    }
    refusals
}

/// The refusal for a missing setting: the field, the queue it belongs to, and why the
/// value is the operator's rather than something to assume.
fn missing(queue: &str, field: &str, why: &str) -> String {
    format!(
        "the `{HARNESS_WORD}` section of worker `{queue}` declares no `{field}`, and nothing \
         supplies one: {why}."
    )
}

#[cfg(test)]
mod tests;
