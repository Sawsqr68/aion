//! The shipped `crates/aion-server/assistant-embed/assistant.awl` session is gated like
//! the artifact it is: it must check clean, print canonically, and — the part
//! that matters — its nested wait loop must be PROVABLY unable to fall through
//! into another agent round.
//!
//! ## The property, and why it needs a proof rather than a reading
//!
//! An AWL `loop` exits identically whether `until` came true or `max` was
//! reached. The lowering returns `Ok(var)` on both paths
//! (`src/emitter/loops.rs`), with no flag, no error, and nothing downstream can
//! tell the two apart. A nested loop is lowered to its own function whose
//! result is `use`-bound in the enclosing body, so an inner loop that gives up
//! simply CONTINUES the outer body holding whatever it last saw.
//!
//! The assistant's inner loop is the operator wait. If it gives up, it is
//! holding a blank continuation — and if the outer loop's `until` does not
//! account for that, the next iteration dispatches a real, billable agent round
//! on an EMPTY prompt, and keeps doing so until the round budget is spent. That
//! is not a hypothetical: it is what this document did before this test existed.
//!
//! ## What is proven here, and what is not
//!
//! PROVEN, exhaustively over the whole two-atom domain: for every continuation
//! value on which the inner loop can exhaust (that is, every value where its
//! `until` is FALSE, so it never exited early), the outer loop's `until` is
//! TRUE. Exhaustion therefore always ends the session, and the single agent
//! dispatch in the outer body is never reached again.
//!
//! NOT proven here: that the engine actually honours the emitted control flow.
//! That is the runtime's contract, tested where the runtime is tested. This
//! test is about the program AWL produces from this document.
//!
//! The evaluation runs over the parsed AST, so it follows the document rather
//! than a transcription of it — editing the loop conditions changes what is
//! evaluated, and a change that reintroduces the fall-through fails here.

use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use aion_awl::{
    BinaryOp, Expr, LoopStmt, PipeEnd, Statement, Step, check_in, compile, parse, print,
};

type TestResult = Result<(), Box<dyn Error>>;

/// The document, at the one place it lives: inside the server crate, because
/// the `aion` binary embeds it (#139 leg C). It is a shipped product asset, not
/// an example, and there is no second copy to check instead.
fn example() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../aion-server/assistant-embed/assistant.awl")
}

#[test]
fn the_assistant_example_checks_clean_and_is_canonically_formatted() -> TestResult {
    let path = example();
    let source = fs::read_to_string(&path)?;
    let document = parse(&source)?;
    let root = path.parent().ok_or("the example has a directory")?;
    let errors = check_in(&document, root);
    assert!(errors.is_empty(), "{errors:#?}");
    assert_eq!(
        print(&document),
        source,
        "the shipped example must already be in canonical form"
    );
    Ok(())
}

#[test]
fn the_assistant_example_compiles() -> TestResult {
    let path = example();
    let source = fs::read_to_string(&path)?;
    let root = path.parent().ok_or("the example has a directory")?;
    let compiled = compile(&source, root)?;
    assert_eq!(compiled.contract.workers.len(), 1, "one worker block");
    Ok(())
}

/// The read-only surface is the acceptance case for task #96: the AWL port
/// answers `assistant_status` with the same name and the same `{phase, round}`
/// shape as the Gleam original, so the port is no longer a capability
/// regression on the one axis an operator cares about — can I ask a running
/// session what it is doing.
///
/// The control is the SAME document with every `query`/`answer` line stripped:
/// it must compile to a module that registers nothing. Without it this test
/// would prove only that the pipeline CAN register a query, not that this
/// document is the reason it does.
#[test]
fn the_assistant_example_answers_the_status_query() -> TestResult {
    let path = example();
    let source = fs::read_to_string(&path)?;
    let root = path.parent().ok_or("the example has a directory")?;
    let document = parse(&source)?;

    let query = document
        .queries
        .iter()
        .find(|query| query.name == "assistant_status")
        .ok_or("the example must declare the `assistant_status` query")?;
    assert_eq!(
        document.queries.len(),
        1,
        "the port mirrors the Gleam original's single query"
    );
    let aion_awl::TypeRef::Named { name, .. } = &query.ty else {
        return Err("the answer type must be a declared record".into());
    };
    let fields = document
        .types
        .iter()
        .find(|declared| &declared.name == name)
        .and_then(|declared| match &declared.body {
            aion_awl::TypeBody::Record { fields } => Some(fields),
            _ => None,
        })
        .ok_or("the answer type must be a declared record")?;
    let field_names: Vec<&str> = fields.iter().map(|field| field.name.as_str()).collect();
    assert_eq!(
        field_names,
        ["phase", "round"],
        "the answer shape must match the Gleam original's `{{phase, round}}`"
    );

    let answers = source
        .lines()
        .filter(|line| line.trim_start().starts_with("answer assistant_status"))
        .count();
    assert!(
        answers >= 3,
        "the session must report live state, not just its start: found {answers} answers"
    );

    // The document compiles, and its module names the query SDK module.
    let compiled = compile(&source, root)?;
    assert!(byte_search(&compiled.beam_bytes, b"aion@query"));
    assert!(byte_search(&compiled.beam_bytes, b"assistant_status"));

    // The control: the same document with the surface removed registers none.
    let mut stripped = String::new();
    for line in source.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("query assistant_status")
            || trimmed.starts_with("answer assistant_status")
        {
            continue;
        }
        stripped.push_str(line);
        stripped.push('\n');
    }
    let control = compile(&stripped, root)?;
    assert!(
        !byte_search(&control.beam_bytes, b"aion@query"),
        "a document without the surface must not name the query SDK module"
    );
    assert_ne!(compiled.beam_bytes, control.beam_bytes);
    Ok(())
}

fn byte_search(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

// --- the control-flow proof --------------------------------------------------

/// A value in the tiny evaluation domain: AWL's own two scalar kinds, which is
/// everything these conditions are written over.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Value {
    Bool(bool),
    Text(String),
}

impl Value {
    fn as_bool(&self) -> Result<bool, String> {
        match self {
            Self::Bool(value) => Ok(*value),
            Self::Text(text) => Err(format!("expected a Bool, found the string {text:?}")),
        }
    }
}

/// The evaluation environment: record bindings (`nudge.end`) and the scalar
/// bindings a loop body also carries (`reply`, bound by the agent dispatch).
#[derive(Default)]
struct Env {
    records: BTreeMap<String, BTreeMap<String, Value>>,
    scalars: BTreeMap<String, Value>,
}

/// Evaluate an AWL expression under `env`.
///
/// Deliberately TOTAL over the constructs these loop conditions may use, and a
/// hard error on anything else: an evaluator that silently returned a default
/// for an unhandled node would turn a condition it does not understand into a
/// passing proof, which is the one failure mode a proof must not have.
fn eval(expr: &Expr, env: &Env) -> Result<Value, String> {
    match expr {
        Expr::Bool { value, .. } => Ok(Value::Bool(*value)),
        Expr::String { value, .. } => Ok(Value::Text(value.clone())),
        Expr::Not { expr, .. } => Ok(Value::Bool(!eval(expr, env)?.as_bool()?)),
        Expr::Ref { name, .. } => env
            .scalars
            .get(name)
            .cloned()
            .ok_or_else(|| format!("no scalar binding `{name}` in the environment")),
        Expr::Field { base, name, .. } => {
            let Expr::Ref { name: binding, .. } = base.as_ref() else {
                return Err(format!("unsupported field base in `{name}` access"));
            };
            let record = env
                .records
                .get(binding)
                .ok_or_else(|| format!("no record binding `{binding}` in the environment"))?;
            record
                .get(name)
                .cloned()
                .ok_or_else(|| format!("binding `{binding}` has no field `{name}`"))
        }
        Expr::Binary {
            op, left, right, ..
        } => {
            let left = eval(left, env)?;
            let right = eval(right, env)?;
            match op {
                BinaryOp::Or => Ok(Value::Bool(left.as_bool()? || right.as_bool()?)),
                BinaryOp::And => Ok(Value::Bool(left.as_bool()? && right.as_bool()?)),
                BinaryOp::Eq => Ok(Value::Bool(left == right)),
                BinaryOp::Ne => Ok(Value::Bool(left != right)),
                other => Err(format!("unsupported operator {other:?}")),
            }
        }
        other => Err(format!("unsupported expression: {other:?}")),
    }
}

/// The `session` step's outer loop, and the wait loop nested in its body.
fn session_loops(steps: &[Step]) -> Result<(&LoopStmt, &LoopStmt), String> {
    let step = steps
        .iter()
        .find(|step| step.name == "session")
        .ok_or("the document must declare a `session` step")?;
    let outer = step
        .body
        .iter()
        .find_map(|statement| match statement {
            Statement::Loop(looped) => Some(looped),
            _ => None,
        })
        .ok_or("the `session` step must contain the round loop")?;
    let inner = outer
        .body
        .iter()
        .find_map(|statement| match statement {
            Statement::Loop(looped) => Some(looped),
            _ => None,
        })
        .ok_or("the round loop must contain the nested wait loop")?;
    Ok((outer, inner))
}

/// The record expression the outer body rebinds its threaded value from —
/// `Round(...) -> round`.
fn rebind_expr(outer: &LoopStmt) -> Result<&Expr, String> {
    outer
        .body
        .iter()
        .find_map(|statement| match statement {
            Statement::Pipe(pipe) => match &pipe.end {
                PipeEnd::Bind(binding) if binding.name == outer.var => Some(&pipe.head),
                _ => None,
            },
            _ => None,
        })
        .ok_or_else(|| format!("the outer loop must rebind `{}`", outer.var))
}

/// Evaluate the rebind's record construction into a field map.
fn build_record(expr: &Expr, env: &Env) -> Result<BTreeMap<String, Value>, String> {
    let Expr::Record { args, .. } = expr else {
        return Err("the threaded rebind must be a record construction".to_owned());
    };
    let mut fields = BTreeMap::new();
    for arg in args {
        fields.insert(arg.name.clone(), eval(&arg.value, env)?);
    }
    Ok(fields)
}

/// THE PROOF.
///
/// The inner loop can only exhaust holding a value its own `until` rejected —
/// had `until` accepted it, the loop would have exited there instead. So the
/// set of continuations reachable on the exhaustion path is exactly
/// `{ v : inner.until(v) == false }`.
///
/// For every such value this asserts that the outer loop's `until` is TRUE, so
/// the outer loop exits and the agent dispatch above it is never reached again.
/// The domain is every combination of the two atoms the conditions read, so it
/// is exhaustive rather than a sample.
#[test]
fn nudge_cap_exhaustion_always_ends_the_session() -> TestResult {
    let path = example();
    let source = fs::read_to_string(&path)?;
    let document = parse(&source)?;
    let (outer, inner) = session_loops(&document.steps)?;

    let inner_until = &inner
        .until
        .as_ref()
        .ok_or("the wait loop needs `until`")?
        .expr;
    let outer_until = &outer
        .until
        .as_ref()
        .ok_or("the round loop needs `until`")?
        .expr;
    let rebind = rebind_expr(outer)?;

    // Every value of the two atoms the wait loop's condition reads. The blank
    // and non-blank strings are the only distinction `== ""` can draw.
    let domain = [true, false]
        .into_iter()
        .flat_map(|end| ["", "do the next thing"].map(|message| (end, message)));

    let mut exhaustible = 0_usize;
    for (end, message) in domain {
        let mut nudge = BTreeMap::new();
        nudge.insert("end".to_owned(), Value::Bool(end));
        nudge.insert("message".to_owned(), Value::Text(message.to_owned()));
        let mut env = Env::default();
        env.records.insert(inner.var.clone(), nudge);
        // `reply` is bound by the agent dispatch above the wait loop. It is
        // carried into the rebind but cannot influence either exit condition;
        // a document that ever made it load-bearing would show up as a type
        // error from the evaluator rather than passing silently.
        env.scalars
            .insert("reply".to_owned(), Value::Text("an agent reply".to_owned()));

        if eval(inner_until, &env)?.as_bool()? {
            // The wait loop exits here on its own terms; not an exhaustion path.
            continue;
        }
        exhaustible += 1;

        let round = build_record(rebind, &env)?;
        env.records.insert(outer.var.clone(), round);
        assert!(
            eval(outer_until, &env)?.as_bool()?,
            "the wait loop can exhaust holding {{end: {end}, message: {message:?}}}, and the \
             round loop's `until` is FALSE for it — the session would dispatch another agent \
             round on prompt {message:?} instead of ending"
        );
    }

    // Non-vacuity: an implication over an empty set proves nothing, and this
    // whole test would pass silently if the wait loop's `until` were ever
    // rewritten to be unconditionally true.
    assert!(
        exhaustible > 0,
        "no continuation value can reach the wait loop's exhaustion path, so the proof above \
         checked nothing — the wait loop's `until` must be falsifiable"
    );
    Ok(())
}

/// The counterpart guard: the outer loop body must contain exactly ONE agent
/// dispatch. The proof above is about reaching the dispatch a second time; it
/// says nothing about a second dispatch appearing beside the first, below the
/// wait loop, where the exhaustion path would run straight into it.
#[test]
fn the_round_loop_dispatches_exactly_once_per_iteration() -> TestResult {
    let path = example();
    let source = fs::read_to_string(&path)?;
    let document = parse(&source)?;
    let (outer, _) = session_loops(&document.steps)?;
    let calls: Vec<&str> = outer
        .body
        .iter()
        .filter_map(|statement| match statement {
            Statement::Call(call) => Some(call.call.name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        calls,
        vec!["assistant"],
        "the round loop body must hold exactly one agent dispatch, before the wait loop"
    );
    Ok(())
}

/// Links the AST proof to the program actually produced.
///
/// The proof above runs over the parsed document; what ships is the emitted
/// Gleam. This asserts the emitted round-loop function carries the same two
/// facts the proof relies on — its exit condition tests the stalled field, and
/// its body holds exactly one agent dispatch — so the proof cannot be true of
/// the source while the emitter produces something else.
///
/// The stalled field name is read off the AST rather than written here, so
/// renaming it in the document does not quietly bypass this test.
#[test]
fn the_emitted_round_loop_exits_on_the_stalled_field() -> TestResult {
    let path = example();
    let source = fs::read_to_string(&path)?;
    let root = path.parent().ok_or("the example has a directory")?;
    let document = parse(&source)?;
    let (outer, _) = session_loops(&document.steps)?;

    // The field the outer `until` reads besides `ended` — the one the wait
    // loop's give-up sets.
    let outer_until = &outer
        .until
        .as_ref()
        .ok_or("the round loop needs `until`")?
        .expr;
    let mut fields = Vec::new();
    collect_fields(outer_until, &outer.var, &mut fields);
    assert!(
        fields.len() >= 2,
        "the round loop's `until` must read more than one field of `{}`, or exhaustion of the \
         wait loop cannot be distinguished from an ordinary pass; it reads {fields:?}",
        outer.var
    );

    let generated = aion_awl::emit_in(&document, root)?;
    let loop_fn = generated
        .find("fn session_loop_0(")
        .ok_or("the emitted program must contain the round loop function")?;
    let body_end = generated[loop_fn..]
        .find("\nfn ")
        .map_or(generated.len(), |offset| loop_fn + offset);
    let body = &generated[loop_fn..body_end];

    for field in &fields {
        assert!(
            body.contains(&format!("{}.{field}", outer.var)),
            "the emitted round loop must test `{}.{field}`; it reads:\n{body}",
            outer.var
        );
    }
    assert_eq!(
        body.matches("assistant_activity(").count(),
        1,
        "the emitted round loop must dispatch exactly once per iteration:\n{body}"
    );
    Ok(())
}

/// Collect the field names read off `binding` anywhere in `expr`.
fn collect_fields(expr: &Expr, binding: &str, found: &mut Vec<String>) {
    match expr {
        Expr::Field { base, name, .. } => {
            if let Expr::Ref {
                name: base_name, ..
            } = base.as_ref()
                && base_name == binding
                && !found.iter().any(|existing| existing == name)
            {
                found.push(name.clone());
            }
        }
        Expr::Not { expr, .. } => collect_fields(expr, binding, found),
        Expr::Binary { left, right, .. } => {
            collect_fields(left, binding, found);
            collect_fields(right, binding, found);
        }
        _ => {}
    }
}

/// The wait loop must not dispatch anything at all: absorbing a blank nudge is
/// supposed to be free, and an action inside it would make every nudge cost a
/// real call.
#[test]
fn the_wait_loop_dispatches_nothing() -> TestResult {
    let path = example();
    let source = fs::read_to_string(&path)?;
    let document = parse(&source)?;
    let (_, inner) = session_loops(&document.steps)?;
    let calls: Vec<&str> = inner
        .body
        .iter()
        .filter_map(|statement| match statement {
            Statement::Call(call) => Some(call.call.name.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        calls.is_empty(),
        "absorbing a nudge must be free, but the wait loop dispatches {calls:?}"
    );
    Ok(())
}
