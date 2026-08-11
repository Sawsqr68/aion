//! Proofs about the document this binary carries.
//!
//! Two properties, and they are different: that the EMBEDDED bytes are the
//! tracked artifact and compile into a usable package, and that the
//! session-contract verification can actually fail. The second is not
//! decoration — a check that has never refused anything is a check nobody has
//! measured, and it would pass just as green if the verification loop were
//! deleted.

use std::path::Path;

use super::*;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// The tracked file `include_str!` names.
fn tracked_document() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assistant-embed/assistant.awl")
}

/// The embedded bytes ARE the tracked artifact — not a copy of it, and not a
/// stale snapshot. Read from disk and compared, so editing the document without
/// rebuilding cannot leave the two disagreeing silently.
#[test]
fn the_embedded_document_is_the_tracked_artifact() -> TestResult {
    let on_disk = std::fs::read_to_string(tracked_document())?;
    assert_eq!(
        EMBEDDED_ASSISTANT_DOCUMENT, on_disk,
        "the embedded document must be the bytes of assistant-embed/assistant.awl"
    );
    assert!(
        !EMBEDDED_ASSISTANT_DOCUMENT.is_empty(),
        "an empty embedded document would satisfy every other assertion here"
    );
    Ok(())
}

/// The embedded document compiles, packages, and validates — so a stock binary
/// really carries a deployable assistant rather than a string.
#[test]
fn the_embedded_document_compiles_into_a_validated_package() -> TestResult {
    let embedded = EmbeddedAssistant::load()?;
    assert_eq!(embedded.workflow_type(), "assistant");
    assert_eq!(embedded.source(), EMBEDDED_ASSISTANT_DOCUMENT);
    assert!(
        !embedded.content_hash().to_string().is_empty(),
        "a package with no content hash has no version identity"
    );
    assert_eq!(
        embedded.package().manifest().entry_module,
        embedded.workflow_type(),
        "the workflow type is the manifest's entry module, not a name held beside it"
    );
    Ok(())
}

/// The same source compiles to the same content hash. Identity is the deploy
/// contract: two boots of one binary must not disagree about which version they
/// carry.
#[test]
fn the_embedded_identity_is_deterministic() -> TestResult {
    let first = EmbeddedAssistant::load()?;
    let second = EmbeddedAssistant::load()?;
    assert_eq!(first.content_hash(), second.content_hash());
    Ok(())
}

/// Every surface the operator verbs bind to is declared by the document, with
/// the payload fields the continuation signal really takes.
#[test]
fn the_session_contract_surfaces_are_declared_by_the_document() -> TestResult {
    let embedded = EmbeddedAssistant::load()?;
    // Read the derived shape LITERALLY rather than through the module's own
    // resolver: a test that reuses the implementation to check the
    // implementation would keep passing if both drifted together.
    let schema = embedded.continuation_schema()?;
    let definition = schema["$ref"]
        .as_str()
        .and_then(|reference| reference.strip_prefix("#/$defs/"))
        .map(|name| &schema["$defs"][name])
        .ok_or("the continuation schema must reference its own definition")?;
    for field in [CONTINUE_MESSAGE_FIELD, CONTINUE_END_FIELD] {
        assert!(
            !definition["properties"][field].is_null(),
            "the `{CONTINUE_SIGNAL}` payload schema must declare `{field}`; got {schema}"
        );
    }
    assert!(
        embedded.queries().iter().any(|name| name == STATUS_QUERY),
        "the document must declare the `{STATUS_QUERY}` query; got {:?}",
        embedded.queries()
    );
    let inputs = embedded
        .input_schema()
        .get("properties")
        .and_then(serde_json::Value::as_object)
        .ok_or("the derived input schema must be an object schema")?;
    for input in [OBJECTIVE_INPUT, REPO_PATH_INPUT] {
        assert!(
            inputs.contains_key(input),
            "the start contract must carry `{input}`; got {inputs:?}"
        );
    }
    Ok(())
}

/// THE VACUITY CONTROL. A document that compiles cleanly but renames the
/// objective input is refused BY NAME.
///
/// Without this, every assertion above would pass unchanged if the
/// session-contract verification were deleted outright: the embedded document
/// satisfies it, so it never fires. Renaming the input (and its one use, so the
/// document still typechecks) puts the check on the path and demands it refuse.
#[test]
fn a_document_that_renames_the_objective_input_is_refused_by_name() -> TestResult {
    let renamed: &'static str = Box::leak(
        EMBEDDED_ASSISTANT_DOCUMENT
            .replace("input objective: String", "input goal: String")
            .replace(
                "contract_tail + objective -> opening_prompt",
                "contract_tail + goal -> opening_prompt",
            )
            .into_boxed_str(),
    );
    assert_ne!(
        renamed, EMBEDDED_ASSISTANT_DOCUMENT,
        "the mutation must change the document, or this test measures nothing"
    );
    match EmbeddedAssistant::from_source(renamed) {
        Err(EmbeddedAssistantError::MissingInput { name }) => {
            assert_eq!(name, OBJECTIVE_INPUT);
            Ok(())
        }
        Err(other) => Err(format!(
            "the renamed document must be refused for the MISSING INPUT, not for {other}; a \
             compile failure here would mean the mutation never reached the contract check"
        )
        .into()),
        Ok(_) => Err(format!(
            "a document declaring no `{OBJECTIVE_INPUT}` input was accepted — the session \
             contract verification is not on the load path"
        )
        .into()),
    }
}

/// THE #200 SOURCE OF TRUTH. The embedded document's queue is its own workflow
/// type, and it is not `default`.
///
/// Both halves matter. The first is the derivation: the queue is a function of
/// the assistant's identity, so nothing can name a queue the document does not
/// declare. The second is the defect this fixes — `default` is the queue an
/// out-of-box worker comes up on, and an assistant sitting there refuses it.
#[test]
fn the_embedded_queue_is_derived_from_the_workflow_type_and_is_never_default() -> TestResult {
    let embedded = EmbeddedAssistant::load()?;
    assert_eq!(
        embedded.task_queue(),
        private_task_queue(embedded.workflow_type()),
        "the declared queue must BE the derivation, not a value beside it"
    );
    assert_ne!(
        embedded.task_queue(),
        aion_core::DEFAULT_TASK_QUEUE,
        "the built-in assistant must never claim the out-of-box workers' queue (#200)"
    );
    // Read the compiled contract literally rather than through the accessor:
    // the accessor is what is under test.
    let contract = embedded.package().contract()?;
    let declared: Vec<&str> = contract
        .workers
        .iter()
        .map(|worker| worker.task_queue.as_str())
        .collect();
    assert_eq!(
        declared,
        vec![embedded.task_queue()],
        "the document must declare exactly the one private queue"
    );
    Ok(())
}

/// VACUITY CONTROL for the derivation. A document that declares `worker
/// default` — the exact shape #200 removed — is refused BY NAME, naming both
/// the queue it declared and the queue the derivation requires.
///
/// Without this the check above passes just as green with the verification
/// deleted, because the shipped document satisfies it and the check never
/// fires.
#[test]
fn a_document_that_claims_the_default_queue_is_refused_by_name() -> TestResult {
    let on_default: &'static str = Box::leak(
        EMBEDDED_ASSISTANT_DOCUMENT
            .replace("\nworker assistant\n", "\nworker default\n")
            .into_boxed_str(),
    );
    assert_ne!(
        on_default, EMBEDDED_ASSISTANT_DOCUMENT,
        "the mutation must change the document, or this test measures nothing"
    );
    match EmbeddedAssistant::from_source(on_default) {
        Err(EmbeddedAssistantError::QueueNotDerived { declared, expected }) => {
            assert_eq!(declared, aion_core::DEFAULT_TASK_QUEUE);
            assert_eq!(expected, "assistant");
            Ok(())
        }
        Err(other) => Err(format!(
            "a document on `default` must be refused for its QUEUE, not for {other}; a compile \
             failure here would mean the mutation never reached the queue check"
        )
        .into()),
        Ok(_) => Err(
            "a document declaring `worker default` was accepted — the queue derivation is not on \
             the load path, and the assistant can ship on the out-of-box workers' queue again"
                .into(),
        ),
    }
}

/// Every other way the derivation refuses, exercised against the verifier
/// itself.
///
/// The document mutation above proves the check is ON THE LOAD PATH. These
/// prove the check is COMPLETE: no queue, several queues, and a workflow type
/// that would derive `default` are each refused, by name, with both the value
/// found and the value required. Driving them through the AWL compiler would
/// mean hand-writing three documents whose compile failures could masquerade as
/// refusals; the contract is the verifier's actual input, so it is the input
/// under test.
#[test]
fn the_derivation_refuses_every_other_disagreement_by_name() -> TestResult {
    let queue = |name: &str| aion_package::WorkerContract {
        task_queue: String::from(name),
        actions: Vec::new(),
    };

    let none = aion_package::PackageContract::default();
    match verified_task_queue("assistant", &none) {
        Err(EmbeddedAssistantError::MissingWorkerBlock { expected }) => {
            assert_eq!(expected, "assistant");
        }
        other => {
            return Err(format!("a contract with no queue must be refused; got {other:?}").into());
        }
    }

    let several = aion_package::PackageContract {
        workers: vec![queue("assistant"), queue("assistant_extra")],
        ..aion_package::PackageContract::default()
    };
    match verified_task_queue("assistant", &several) {
        Err(EmbeddedAssistantError::AmbiguousQueue { declared, expected }) => {
            assert_eq!(declared, "assistant, assistant_extra");
            assert_eq!(expected, "assistant");
        }
        other => {
            return Err(format!("two queues must be refused as ambiguous; got {other:?}").into());
        }
    }

    let elsewhere = aion_package::PackageContract {
        workers: vec![queue("something_else")],
        ..aion_package::PackageContract::default()
    };
    match verified_task_queue("assistant", &elsewhere) {
        Err(EmbeddedAssistantError::QueueNotDerived { declared, expected }) => {
            assert_eq!(declared, "something_else");
            assert_eq!(expected, "assistant");
        }
        other => {
            return Err(format!("an underived queue must be refused; got {other:?}").into());
        }
    }

    // The rule holds even against the derivation itself: a workflow type that
    // WOULD derive the out-of-box workers' queue is refused rather than
    // quietly producing the very collision #200 removed.
    let colliding = aion_package::PackageContract {
        workers: vec![queue(aion_core::DEFAULT_TASK_QUEUE)],
        ..aion_package::PackageContract::default()
    };
    match verified_task_queue(aion_core::DEFAULT_TASK_QUEUE, &colliding) {
        Err(EmbeddedAssistantError::QueueWouldBeDefault {
            workflow_type,
            default_queue,
        }) => {
            assert_eq!(workflow_type, aion_core::DEFAULT_TASK_QUEUE);
            assert_eq!(default_queue, aion_core::DEFAULT_TASK_QUEUE);
        }
        other => {
            return Err(format!(
                "a workflow type deriving `default` must be refused; got {other:?}"
            )
            .into());
        }
    }

    // AND the agreeing case is accepted, so the refusals above are not a
    // verifier that refuses everything.
    let derived = aion_package::PackageContract {
        workers: vec![queue("assistant")],
        ..aion_package::PackageContract::default()
    };
    assert_eq!(verified_task_queue("assistant", &derived)?, "assistant");
    Ok(())
}

/// A document carrying a `schema(…)` import is refused before compilation,
/// naming the import: the binary embeds one file and no directory to resolve
/// against, and resolving one against the server's cwd would be a silent
/// substitution.
#[test]
fn a_document_with_a_schema_import_is_refused_naming_the_import() -> TestResult {
    let with_import: &'static str = Box::leak(
        EMBEDDED_ASSISTANT_DOCUMENT
            .replace(
                "type Provisioned { exit_code: Int, stdout: String }",
                "type Provisioned = schema(\"provisioned.json\")",
            )
            .into_boxed_str(),
    );
    match EmbeddedAssistant::from_source(with_import) {
        Err(EmbeddedAssistantError::SchemaImport { path }) => {
            assert_eq!(path, "provisioned.json");
            Ok(())
        }
        Err(other) => Err(format!("the import must be refused as an import, got: {other}").into()),
        Ok(_) => Err("a document with an unresolvable schema import was accepted".into()),
    }
}

/// The process-wide handle returns the same prepared assistant every call, and
/// it is the one `load` produces.
#[test]
fn the_process_wide_handle_matches_a_direct_load() -> TestResult {
    let shared = embedded_assistant().map_err(ToString::to_string)?;
    let direct = EmbeddedAssistant::load()?;
    assert_eq!(shared.workflow_type(), direct.workflow_type());
    assert_eq!(shared.content_hash(), direct.content_hash());
    assert!(std::ptr::eq(
        shared,
        embedded_assistant().map_err(ToString::to_string)?
    ));
    Ok(())
}

/// The property predicate reads `properties` — directly, or one hop through the
/// local `$defs` reference the deriver emits — and never a bare object key.
#[test]
fn property_detection_reads_the_schema_properties_map() {
    let direct = serde_json::json!({"properties": {"message": {"type": "string"}}});
    assert!(schema_declares_property(&direct, "message"));
    assert!(!schema_declares_property(&direct, "end"));
    assert!(!schema_declares_property(
        &serde_json::json!({"message": {}}),
        "message"
    ));

    let referenced = serde_json::json!({
        "$ref": "#/$defs/Continuation",
        "$defs": {"Continuation": {"properties": {"end": {"type": "boolean"}}}},
    });
    assert!(schema_declares_property(&referenced, "end"));
    assert!(!schema_declares_property(&referenced, "message"));

    // An unresolvable reference is not silently treated as the schema itself.
    let dangling = serde_json::json!({
        "$ref": "#/$defs/Missing",
        "$defs": {},
        "properties": {"end": {"type": "boolean"}},
    });
    assert!(!schema_declares_property(&dangling, "end"));
}
