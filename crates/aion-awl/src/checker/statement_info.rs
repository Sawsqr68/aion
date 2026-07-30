//! Retained statement shape for projection, classified with checker resolution.

use crate::ast::{
    CombinatorKind, DeliveryVerb, DurationLiteral, ForkHeader, PipeEnd, PipeStage, Statement,
};
use crate::semantic::{ForkInfo, StatementInfo, StatementKind};
use crate::{DurationUnit, expr_text};

use super::context::Ctx;

pub(super) fn statement_infos(ctx: &Ctx<'_>, statements: &[Statement]) -> Vec<StatementInfo> {
    statements
        .iter()
        .map(|statement| statement_info(ctx, statement))
        .collect()
}

fn statement_info(ctx: &Ctx<'_>, statement: &Statement) -> StatementInfo {
    match statement {
        Statement::Call(call) => call_info(ctx, call),
        Statement::Spawn(spawn) => spawn_info(ctx, spawn),
        Statement::Pipe(pipe) => pipe_info(ctx, pipe),
        Statement::Wait(wait) => wait_info(wait),
        Statement::Send(send) => send_info(send),
        Statement::Sleep(sleep) => info(
            sleep.span,
            StatementKind::Sleep,
            format!("sleep {}", duration_text(&sleep.duration)),
            None,
            None,
            Vec::new(),
        ),
        Statement::Fork(fork) => fork_info(ctx, fork),
        Statement::Loop(looped) => loop_info(ctx, looped),
        Statement::Route(route) => info(
            route.span,
            StatementKind::Route,
            format!("route {}", route.target.name),
            None,
            None,
            Vec::new(),
        ),
        Statement::SubStep(substep) => info(
            substep.span,
            StatementKind::SubStep,
            substep.name.clone(),
            Some("step".to_owned()),
            None,
            Vec::new(),
        ),
        Statement::Distribute(distribute) => distribute_info(distribute),
        Statement::Collect(collect) => collect_info(collect),
    }
}

fn call_info(ctx: &Ctx<'_>, call: &crate::ast::CallStmt) -> StatementInfo {
    info(
        call.span,
        call_kind(ctx, &call.call.name),
        call.call.name.clone(),
        call.bind.as_ref().map(|bind| format!("→ {}", bind.name)),
        None,
        Vec::new(),
    )
}

fn spawn_info(ctx: &Ctx<'_>, spawn: &crate::ast::SpawnStmt) -> StatementInfo {
    let kind = if ctx.children.contains_key(&spawn.call.name) {
        StatementKind::SpawnChild
    } else {
        StatementKind::InvalidSpawn
    };
    info(
        spawn.span,
        kind,
        spawn.call.name.clone(),
        Some("spawn".to_owned()),
        None,
        Vec::new(),
    )
}

fn pipe_info(ctx: &Ctx<'_>, pipe: &crate::ast::PipeStmt) -> StatementInfo {
    let mut children = pipe
        .stages
        .iter()
        .map(|stage| pipe_stage_info(ctx, stage))
        .collect::<Vec<_>>();
    let detail = match &pipe.end {
        PipeEnd::Bind(bind) => Some(format!("→ {}", bind.name)),
        PipeEnd::Route(target) => {
            children.push(info(
                target.span,
                StatementKind::Route,
                target.name.clone(),
                None,
                None,
                Vec::new(),
            ));
            Some(format!("route {}", target.name))
        }
    };
    info(
        pipe.span,
        StatementKind::Pipe,
        "pipe".to_owned(),
        detail,
        None,
        children,
    )
}

fn wait_info(wait: &crate::ast::WaitStmt) -> StatementInfo {
    let timeout = wait.timeout.as_ref().map_or_else(String::new, |duration| {
        format!("timeout {} · ", duration_text(duration))
    });
    info(
        wait.span,
        StatementKind::Wait,
        format!("wait {}", wait.signal),
        Some(format!("{timeout}→ {}", wait.bind.name)),
        None,
        Vec::new(),
    )
}

fn send_info(send: &crate::ast::SendStmt) -> StatementInfo {
    info(
        send.span,
        StatementKind::Send,
        format!("send {}", send.signal),
        Some(format!("to {}", expr_text(&send.target))),
        None,
        Vec::new(),
    )
}

fn loop_info(ctx: &Ctx<'_>, looped: &crate::ast::LoopStmt) -> StatementInfo {
    let detail = match (&looped.until, &looped.max) {
        (Some(until), Some(max)) => Some(format!(
            "until {} · max {}",
            expr_text(&until.expr),
            expr_text(&max.expr)
        )),
        (Some(until), None) => Some(format!("until {}", expr_text(&until.expr))),
        (None, Some(max)) => Some(format!("max {}", expr_text(&max.expr))),
        (None, None) => None,
    };
    info(
        looped.span,
        StatementKind::Loop,
        format!("loop {}", looped.var),
        detail,
        None,
        statement_infos(ctx, &looped.body),
    )
}

fn distribute_info(distribute: &crate::ast::DistributeStmt) -> StatementInfo {
    let kind = match distribute.verb {
        DeliveryVerb::Distribute => StatementKind::Distribute,
        DeliveryVerb::Sequence => StatementKind::Sequence,
    };
    info(
        distribute.span,
        kind,
        format!(
            "{} in {}",
            distribute.var,
            expr_text(&distribute.collection)
        ),
        Some(distribute.verb.as_word().to_owned()),
        None,
        Vec::new(),
    )
}

fn collect_info(collect: &crate::ast::CollectStmt) -> StatementInfo {
    info(
        collect.span,
        StatementKind::Collect,
        format!(
            "collect {}{}",
            collect.binding,
            if collect.tolerant { "?" } else { "" }
        ),
        Some(format!("→ {}", collect.bind.name)),
        None,
        Vec::new(),
    )
}

fn fork_info(ctx: &Ctx<'_>, fork: &crate::ast::ForkStmt) -> StatementInfo {
    let (kind, label, binding, collection, sequential) = match &fork.header {
        ForkHeader::Collection {
            var,
            collection,
            sequential,
            ..
        } => (
            if *sequential {
                StatementKind::SequentialFork
            } else {
                StatementKind::ParallelFork
            },
            format!("{var} in {}", expr_text(collection)),
            Some(var.clone()),
            Some(expr_text(collection)),
            *sequential,
        ),
        ForkHeader::Named => (
            StatementKind::NamedFork,
            "named branches".to_owned(),
            None,
            None,
            false,
        ),
    };
    let result = fork.join.bind.as_ref().map(|bind| bind.name.clone());
    info(
        fork.span,
        kind,
        label,
        result.as_ref().map(|name| format!("join → {name}")),
        Some(ForkInfo {
            binding,
            collection,
            sequential,
            result,
        }),
        statement_infos(ctx, &fork.body),
    )
}

fn pipe_stage_info(ctx: &Ctx<'_>, stage: &PipeStage) -> StatementInfo {
    match stage {
        PipeStage::Action { span, name } => {
            let kind = if ctx.actions.contains_key(name) {
                StatementKind::ActionStage
            } else if ctx.children.contains_key(name) {
                StatementKind::ChildStage
            } else {
                StatementKind::UnresolvedStage
            };
            info(*span, kind, name.clone(), None, None, Vec::new())
        }
        PipeStage::Field { span, name } => info(
            *span,
            StatementKind::FieldStage,
            format!(".{name}"),
            None,
            None,
            Vec::new(),
        ),
        PipeStage::Combinator(combinator) => info(
            combinator.span,
            StatementKind::CombinatorStage,
            combinator_name(combinator.kind).to_owned(),
            None,
            None,
            Vec::new(),
        ),
    }
}

fn call_kind(ctx: &Ctx<'_>, name: &str) -> StatementKind {
    if ctx.actions.contains_key(name) {
        StatementKind::ActionCall
    } else if ctx.children.contains_key(name) {
        StatementKind::ChildCall
    } else if ctx.subflows.contains_key(name) {
        StatementKind::SubflowCall
    } else {
        StatementKind::UnresolvedCall
    }
}

fn info(
    span: crate::Span,
    kind: StatementKind,
    label: String,
    detail: Option<String>,
    fork: Option<ForkInfo>,
    children: Vec<StatementInfo>,
) -> StatementInfo {
    StatementInfo {
        span,
        kind,
        label,
        detail,
        fork,
        children,
    }
}

const fn combinator_name(kind: CombinatorKind) -> &'static str {
    match kind {
        CombinatorKind::Filter => "filter",
        CombinatorKind::Map => "map",
        CombinatorKind::Sort => "sort",
        CombinatorKind::Count => "count",
        CombinatorKind::Any => "any",
        CombinatorKind::All => "all",
    }
}

fn duration_text(duration: &DurationLiteral) -> String {
    let suffix = match duration.unit {
        DurationUnit::Seconds => "s",
        DurationUnit::Minutes => "m",
        DurationUnit::Hours => "h",
        DurationUnit::Days => "d",
    };
    format!("{}{suffix}", duration.magnitude)
}
