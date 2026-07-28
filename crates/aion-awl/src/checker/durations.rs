//! Range validation for every duration that native codegen represents as
//! signed milliseconds.

use crate::Span;
use crate::ast::{ConfigLine, DurationLiteral, RetrySpec, Statement, Step};

use super::context::Ctx;

pub(super) fn run(ctx: &mut Ctx<'_>) {
    let mut out_of_range = Vec::new();
    for worker in &ctx.doc.workers {
        for action in &worker.actions {
            if let Some(config) = &action.config {
                check_config(config, &mut out_of_range);
            }
        }
    }
    check_steps(&ctx.doc.steps, &mut out_of_range);
    for subflow in &ctx.doc.subflows {
        check_steps(&subflow.steps, &mut out_of_range);
    }
    for span in out_of_range {
        ctx.error(span, "duration exceeds the native signed-millisecond range");
    }
}

fn check_steps(steps: &[Step], out_of_range: &mut Vec<Span>) {
    for step in steps {
        check_statements(&step.body, out_of_range);
        if let Some(on_failure) = &step.on_failure {
            check_statements(&on_failure.body, out_of_range);
        }
    }
}

fn check_statements(statements: &[Statement], out_of_range: &mut Vec<Span>) {
    for statement in statements {
        match statement {
            Statement::Call(call) => {
                if let Some(config) = &call.config {
                    check_config(config, out_of_range);
                }
            }
            Statement::Wait(wait) => {
                if let Some(timeout) = &wait.timeout {
                    check_duration(timeout, out_of_range);
                }
            }
            Statement::Sleep(sleep) => check_duration(&sleep.duration, out_of_range),
            Statement::Fork(fork) => check_statements(&fork.body, out_of_range),
            Statement::Loop(looped) => check_statements(&looped.body, out_of_range),
            Statement::SubStep(step) => check_steps(std::slice::from_ref(step), out_of_range),
            Statement::Spawn(_)
            | Statement::Pipe(_)
            | Statement::Route(_)
            | Statement::Distribute(_)
            | Statement::Collect(_) => {}
        }
    }
}

fn check_config(config: &ConfigLine, out_of_range: &mut Vec<Span>) {
    // `advisory` and `node` carry no duration — deliberately nothing to
    // range-check here.
    if let Some(timeout) = &config.timeout {
        check_duration(timeout, out_of_range);
    }
    match &config.retry {
        Some(RetrySpec::Every { every, .. }) => check_duration(every, out_of_range),
        Some(RetrySpec::Backoff { min, max, .. }) => {
            check_duration(min, out_of_range);
            check_duration(max, out_of_range);
        }
        None => {}
    }
}

fn check_duration(duration: &DurationLiteral, out_of_range: &mut Vec<Span>) {
    if duration.checked_milliseconds().is_none() {
        out_of_range.push(duration.span);
    }
}
