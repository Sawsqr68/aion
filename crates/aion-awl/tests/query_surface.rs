//! The AWL query surface: `query <name>: <Type>` in the header and
//! `answer <name>(…)` as a statement.
//!
//! Every positive assertion here is paired with its CONTROL — the same
//! property measured on a document that declares no query at all. A test that
//! only ever fires positively proves the code CAN register a query; it cannot
//! prove the code registers one only when the document asks for it. The
//! controls are what make these tests discriminating, and each one is computed
//! from the same pipeline as its positive, never from a hardcoded expectation.

use std::error::Error;
use std::path::Path;

use aion_awl::mir::{MirFn, RuntimeFn, Stmt, lower};
use aion_awl::{check, compile, emit, parse, print, schema_for_queries, schema_for_workflow};

type TestResult = Result<(), Box<dyn Error>>;

/// A workflow that declares and answers one query in every form the surface
/// admits: a record built from named fields, a bare value, a `Nil` answer with
/// no parentheses, and a re-registration inside a loop.
const WITH_QUERY: &str = r#"//! A run that reports what it is doing.
workflow reporter
  input item_id: String
  input max_passes: Int
  query review_status: Status
  query passes_left: Int
  query heartbeat: Nil
  outcome done: type Done, route success

type Done   { item: String }
type Status { phase: String, note: String? }

worker gates
  action evaluate(item: String) -> Done

step gate
  answer review_status(phase: "gating", note: "opening the gate")
  answer passes_left(max_passes)
  answer heartbeat
  loop verdict = Done(item: "") counting passes
    answer review_status(phase: "reviewing")
    evaluate(item: item_id) -> verdict
    until verdict.item != ""
    max max_passes
  answer review_status(phase: "settled", note: item_id)
  outcome settled: when verdict.item != "",
    route done(item: verdict.item)
  outcome exhausted: otherwise,
    route done(item: item_id)
"#;

/// The CONTROL: the same workflow with every `query` and `answer` line
/// removed, and nothing else changed. Every "a query document does X"
/// assertion is paired with "this document does not".
const WITHOUT_QUERY: &str = r#"//! A run that reports what it is doing.
workflow reporter
  input item_id: String
  input max_passes: Int
  outcome done: type Done, route success

type Done   { item: String }
type Status { phase: String, note: String? }

worker gates
  action evaluate(item: String) -> Done

step gate
  loop verdict = Done(item: "") counting passes
    evaluate(item: item_id) -> verdict
    until verdict.item != ""
    max max_passes
  outcome settled: when verdict.item != "",
    route done(item: verdict.item)
  outcome exhausted: otherwise,
    route done(item: item_id)
"#;

fn parsed(source: &str) -> Result<aion_awl::Document, Box<dyn Error>> {
    let document = parse(source).map_err(|error| {
        format!(
            "parse failed: {} at line {}",
            error.message, error.span.line
        )
    })?;
    let errors = check(&document);
    if let Some(first) = errors.first() {
        return Err(format!(
            "check failed: {} at line {}",
            first.message, first.span.line
        )
        .into());
    }
    Ok(document)
}

/// Every declared query reaches the parsed model, and a document without the
/// construct carries none — the parse-level control.
#[test]
fn queries_parse_only_when_declared() -> TestResult {
    let with = parsed(WITH_QUERY)?;
    let without = parsed(WITHOUT_QUERY)?;
    let names: Vec<&str> = with
        .queries
        .iter()
        .map(|query| query.name.as_str())
        .collect();
    assert_eq!(names, ["review_status", "passes_left", "heartbeat"]);
    assert!(
        without.queries.is_empty(),
        "the control document declares no query but parsed {} of them",
        without.queries.len()
    );
    Ok(())
}

/// `aion awl fmt` round-trips both constructs byte-identically, and the
/// control proves the printer emits no `query` line for a document that
/// declares none.
#[test]
fn the_printer_round_trips_the_surface() -> TestResult {
    for source in [WITH_QUERY, WITHOUT_QUERY] {
        let printed = print(&parsed(source)?);
        assert_eq!(printed, source, "print(parse(src)) must equal src");
        let twice = print(&parsed(&printed)?);
        assert_eq!(twice, printed, "printing must be idempotent");
    }
    let control = print(&parsed(WITHOUT_QUERY)?);
    assert!(
        !control.contains("query ") && !control.contains("answer "),
        "the control document must print neither construct:\n{control}"
    );
    Ok(())
}

/// The derived read-only surface names every query and carries its answer
/// type; the control derives an empty surface. The start contract is untouched
/// by either — a query is not an input.
#[test]
fn the_derived_schema_carries_the_query_surface() -> TestResult {
    let with = schema_for_queries(&parsed(WITH_QUERY)?)?;
    let properties = with
        .get("properties")
        .and_then(serde_json::Value::as_object)
        .ok_or("derived query surface has no properties object")?;
    assert_eq!(properties.len(), 3);
    assert!(properties.contains_key("review_status"));
    assert_eq!(
        properties.get("passes_left"),
        Some(&serde_json::json!({ "type": "integer" })),
        "a leaf-typed query derives its answer type directly"
    );
    let status = with
        .get("$defs")
        .and_then(|defs| defs.get("Status"))
        .ok_or("the record answer type is not in $defs")?;
    assert_eq!(
        status.get("required"),
        Some(&serde_json::json!(["phase"])),
        "an optional answer field is not required"
    );

    let without = schema_for_queries(&parsed(WITHOUT_QUERY)?)?;
    let control = without
        .get("properties")
        .and_then(serde_json::Value::as_object)
        .ok_or("the control surface has no properties object")?;
    assert!(
        control.is_empty(),
        "a document declaring no query must derive an empty surface, got {control:?}"
    );

    // The start contract is the same either way: queries are not inputs.
    assert_eq!(
        schema_for_workflow(&parsed(WITH_QUERY)?)?,
        schema_for_workflow(&parsed(WITHOUT_QUERY)?)?
    );
    Ok(())
}

/// The Gleam reference backend emits one `query.handler` per `answer` and the
/// conditional `aion/query` import; the control emits neither.
#[test]
fn the_reference_backend_emits_one_handler_per_answer() -> TestResult {
    let with = emit(&parsed(WITH_QUERY)?)?;
    assert_eq!(
        with.matches("query.handler(").count(),
        5,
        "one registration per `answer` statement:\n{with}"
    );
    assert!(with.contains("import aion/query"));
    // The failure contract: every registration is threaded through `result.try`
    // so a refusal faults the step instead of passing quietly.
    assert_eq!(
        with.matches("use _ <- result.try(case query.handler(")
            .count(),
        5
    );

    let without = emit(&parsed(WITHOUT_QUERY)?)?;
    assert_eq!(
        without.matches("query.handler(").count(),
        0,
        "the control must emit no registration:\n{without}"
    );
    assert!(
        !without.contains("import aion/query"),
        "an unused import would not compile; the control must not carry one"
    );
    Ok(())
}

/// The number of `aion@query:handler/4` calls the direct-BEAM backend lowers,
/// measured over every function body of a module.
fn handler_calls(source: &str) -> Result<usize, Box<dyn Error>> {
    let module = lower(&parsed(source)?, None).map_err(|error| error.to_string())?;
    let mut found = 0;
    for function in &module.functions {
        if let MirFn::Flow(flow) = function {
            found += block_handler_calls(&flow.body);
        }
    }
    Ok(found)
}

fn block_handler_calls(block: &aion_awl::mir::Block) -> usize {
    let mut found = block.stmts.iter().map(stmt_handler_calls).sum();
    found += tail_handler_calls(&block.tail);
    found
}

fn stmt_handler_calls(stmt: &Stmt) -> usize {
    match stmt {
        Stmt::CallRt {
            callee: RuntimeFn::QueryHandler,
            ..
        } => 1,
        Stmt::ShortCircuit { rhs_stmts, .. } => rhs_stmts.iter().map(stmt_handler_calls).sum(),
        Stmt::Attempt { on_ok, on_err, .. } => {
            block_handler_calls(on_ok) + block_handler_calls(on_err)
        }
        _ => 0,
    }
}

fn tail_handler_calls(tail: &aion_awl::mir::Tail) -> usize {
    match tail {
        aion_awl::mir::Tail::If {
            then_block,
            else_block,
            ..
        } => block_handler_calls(then_block) + block_handler_calls(else_block),
        aion_awl::mir::Tail::SelectEnum { arms, .. } => arms
            .iter()
            .map(|(_, block)| block_handler_calls(block))
            .sum(),
        _ => 0,
    }
}

/// The direct-BEAM backend lowers one `aion@query:handler/4` call per `answer`,
/// and the control lowers ZERO — measured the same way, over the same walk, so
/// the positive cannot pass vacuously.
#[test]
fn the_direct_backend_lowers_one_handler_call_per_answer() -> TestResult {
    assert_eq!(handler_calls(WITH_QUERY)?, 5);
    assert_eq!(
        handler_calls(WITHOUT_QUERY)?,
        0,
        "a document with no query must lower no registration"
    );
    Ok(())
}

/// Both backends agree on how many registrations a document performs. Neither
/// count is hardcoded against the other: each is measured from its own
/// pipeline, and the assertion is that they match.
#[test]
fn both_backends_register_the_same_number_of_answers() -> TestResult {
    let reference = emit(&parsed(WITH_QUERY)?)?
        .matches("query.handler(")
        .count();
    assert_eq!(reference, handler_calls(WITH_QUERY)?);
    let control = emit(&parsed(WITHOUT_QUERY)?)?
        .matches("query.handler(")
        .count();
    assert_eq!(control, handler_calls(WITHOUT_QUERY)?);
    assert_ne!(
        reference, control,
        "the two documents must differ, or neither measurement discriminates"
    );
    Ok(())
}

/// Both backends register against the handler's ARGUMENTS-BEARING shape.
///
/// An AWL `answer` declares no arguments, but the SDK handler takes an
/// arguments codec and calls its reply with the decoded value — so a backend
/// still emitting the old three-argument form would compile to a call that
/// does not exist. Counting registrations cannot see that; the shape must be
/// asserted directly, on each backend's own artifact, with the no-query
/// control proving the property is not universally true of any module.
#[test]
fn both_backends_register_against_the_arguments_bearing_handler() -> TestResult {
    let with = emit(&parsed(WITH_QUERY)?)?;
    // Reference backend: inspect the registration LINES themselves rather than
    // counting substrings module-wide, so an unrelated `fn(_)` elsewhere in the
    // emission can neither satisfy nor break the assertion.
    let registrations: Vec<&str> = with
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("use _ <- result.try(case query.handler("))
        .collect();
    assert_eq!(
        registrations.len(),
        5,
        "expected one line per answer:\n{with}"
    );
    for line in &registrations {
        assert!(
            line.contains("awlc.nil_codec(), "),
            "a registration omitted the unit arguments codec: {line}"
        );
        assert!(
            line.contains("fn(_) {"),
            "a reply does not bind the decoded arguments: {line}"
        );
    }

    // Direct-BEAM backend: the runtime row it calls is arity 4, and the reply
    // functions it lifts are callable at arity 1 (two parameters, the last of
    // which `MakeClosure` supplies as the captured answer value).
    let handler_rows: Vec<(&str, String, u32)> = RuntimeFn::import_surface()
        .into_iter()
        .filter(|(module, function, _)| *module == "aion@query" && function == "handler")
        .collect();
    assert_eq!(
        handler_rows,
        vec![("aion@query", "handler".to_owned(), 4)],
        "the runtime import row the lowering mints must match the SDK's arity"
    );
    let module = lower(&parsed(WITH_QUERY)?, None).map_err(|error| error.to_string())?;
    let replies: Vec<usize> = module
        .functions
        .iter()
        .filter_map(|function| match function {
            MirFn::Flow(flow) if flow.name.starts_with("awl_answer_reply_") => {
                Some(flow.params.len())
            }
            _ => None,
        })
        .collect();
    assert_eq!(replies, vec![2; 5], "reply arity drifted: {replies:?}");

    // The control: the same walk over a document with no query finds nothing,
    // so neither assertion above is a property of every AWL module.
    let control = emit(&parsed(WITHOUT_QUERY)?)?;
    assert_eq!(control.matches("query.handler(\"").count(), 0);
    let control_module = lower(&parsed(WITHOUT_QUERY)?, None).map_err(|error| error.to_string())?;
    assert!(
        !control_module.functions.iter().any(|function| matches!(
            function,
            MirFn::Flow(flow) if flow.name.starts_with("awl_answer_reply_")
        )),
        "a document with no query must lift no reply function"
    );
    Ok(())
}

/// The whole compile seam accepts the surface and produces deterministic
/// bytes, and adding the query genuinely changes the emitted module — the
/// control proves the construct reaches the artifact rather than being
/// discarded somewhere between the checker and instruction selection.
#[test]
fn the_compile_seam_carries_the_surface_into_the_beam() -> TestResult {
    let root = Path::new(".");
    let with = compile(WITH_QUERY, root).map_err(|error| error.to_string())?;
    let again = compile(WITH_QUERY, root).map_err(|error| error.to_string())?;
    assert_eq!(
        with.beam_bytes, again.beam_bytes,
        "compilation is deterministic"
    );

    let without = compile(WITHOUT_QUERY, root).map_err(|error| error.to_string())?;
    assert_ne!(
        with.beam_bytes, without.beam_bytes,
        "the query surface must change the emitted module"
    );
    // The SDK module the registration calls into is named in the query
    // document's atom table and absent from the control's.
    assert!(contains(&with.beam_bytes, b"aion@query"));
    assert!(
        !contains(&without.beam_bytes, b"aion@query"),
        "the control module must not import the query SDK module"
    );
    Ok(())
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

/// One refusal per rule the surface states, each paired with the REPAIRED
/// document, which must check clean. Without the repair a test proves only
/// that the document is rejected — not that it is rejected for the stated
/// reason rather than an unrelated one.
#[test]
fn the_checker_refuses_each_broken_shape_and_accepts_its_repair() -> TestResult {
    let matrix: &[(&str, String, String, &str)] = &[
        (
            "declared but never answered",
            report_document("  query status: Report\n", ""),
            report_document(
                "  query status: Report\n",
                "  answer status(phase: \"working\")\n",
            ),
            "is declared but never answered",
        ),
        (
            "answered but never declared",
            report_document("", "  answer status(phase: \"working\")\n"),
            report_document(
                "  query status: Report\n",
                "  answer status(phase: \"working\")\n",
            ),
            "no query named `status` is declared in the workflow header",
        ),
        (
            "duplicate query names",
            report_document(
                "  query status: Report\n  query status: Report\n",
                "  answer status(phase: \"working\")\n",
            ),
            report_document(
                "  query status: Report\n",
                "  answer status(phase: \"working\")\n",
            ),
            "duplicate query declaration `status`",
        ),
        (
            "an answer whose field set is not the declared type's",
            report_document(
                "  query status: Report\n",
                "  answer status(stage: \"working\")\n",
            ),
            report_document(
                "  query status: Report\n",
                "  answer status(phase: \"working\")\n",
            ),
            "has no field `stage`",
        ),
        (
            "an answer reading a binding not yet in scope",
            report_document(
                "  query status: Report\n",
                "  answer status(phase: later)\n  probe(item: item_id) -> later\n",
            ),
            report_document(
                "  query status: Report\n",
                "  probe(item: item_id) -> later\n  answer status(phase: later)\n",
            ),
            "`later` is bound on some path but not guaranteed",
        ),
        (
            "a non-`Nil` query answered bare",
            report_document("  query status: Report\n", "  answer status\n"),
            report_document(
                "  query status: Report\n",
                "  answer status(phase: \"working\")\n",
            ),
            "so the answer must construct it",
        ),
    ];

    for (what, broken, repaired, substring) in matrix {
        let document = parse(broken).map_err(|error| {
            format!(
                "{what}: the broken document must still PARSE: {}",
                error.message
            )
        })?;
        let errors = check(&document);
        assert!(
            errors.iter().any(|error| error.message.contains(substring)),
            "{what}: expected a diagnostic containing {substring:?}, got {:#?}",
            errors
                .iter()
                .map(|error| error.message.as_str())
                .collect::<Vec<_>>()
        );

        let repaired_document = parse(repaired)
            .map_err(|error| format!("{what}: the repair must parse: {}", error.message))?;
        let repaired_errors = check(&repaired_document);
        assert!(
            repaired_errors.is_empty(),
            "{what}: the repair must check clean, got {:#?}",
            repaired_errors
                .iter()
                .map(|error| error.message.as_str())
                .collect::<Vec<_>>()
        );
    }
    Ok(())
}

/// One workflow skeleton with two holes: the header's query declarations and
/// the step body's statements. Every case in the refusal matrix differs only
/// in those two, so a diagnostic can only come from the construct under test.
fn report_document(header: &str, body: &str) -> String {
    format!(
        "//! A run under test.\nworkflow reporter\n  input item_id: String\n{header}  outcome \
         done: type Done, route success\n\ntype Done   {{ item: String }}\ntype Report {{ \
         phase: String }}\n\nworker gates\n  action probe(item: String) -> String\n\nstep \
         gate\n{body}  route done(item: item_id)\n"
    )
}

/// `query` and `answer` are POSITIONAL, never reserved: the checked-in corpus
/// uses both words as ordinary names, and reserving either would break
/// documents that compile today.
#[test]
fn both_words_stay_usable_as_ordinary_names() -> TestResult {
    const CORPUS_USES: &str = r"//! `query` is an action parameter and `answer` a binding, exactly as the
//! checked-in corpus uses them today.
workflow name_clash
  input topic: String
  signal decision: Decision
  outcome done: type Done, route success

type Decision { approved: Bool }
type Done     { item: String }

worker tools
  action search(query: String) -> String
  action answer(prompt: String) -> String

step ask
  search(query: topic) -> hits
  answer(prompt: hits) -> reply
  wait decision -> answer
  route done(item: reply)
";
    let document = parsed(CORPUS_USES)?;
    assert!(document.queries.is_empty());
    assert_eq!(print(&document), CORPUS_USES);
    Ok(())
}
