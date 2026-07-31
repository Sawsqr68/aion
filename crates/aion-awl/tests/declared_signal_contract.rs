//! An AWL `signal` declaration must reach the package contract as a USABLE
//! schema — the thing the engine's signal boundary admits against.
//!
//! WHY THIS EXISTS. `crates/aion/src/signal/admission.rs` refuses a signal
//! whose payload does not satisfy its declared type, before anything is
//! recorded or consumed. That is what stops an operator's typo from killing a
//! durable run parked for months. It reads the declaration from
//! `PackageContract::signals`, and for an AWL document the ONLY thing that puts
//! anything there is `compile.rs`'s mapping over `document.signals`.
//!
//! That mapping was ungated. The one contract golden in this crate
//! (`retry_counting.rs`) pins a document that declares no signal, so a
//! regression of the mapping to `Vec::new()` would have left every test in the
//! workspace green — and `catalog_admission.rs` treats an empty `signals` as
//! "this package declares nothing, admit everything". The protection would have
//! gone silently dark for every AWL document, which is every document the
//! language produces. A silent hole behind a green suite is the exact shape
//! these pins exist to catch.
//!
//! These assert the PROPERTY, not a shape. A hardcoded copy of the expected
//! JSON Schema would pin whatever the deriver happens to emit today and would
//! pass just as well if the schema stopped discriminating; what matters is that
//! the schema ACCEPTS a conforming payload and REFUSES a non-conforming one,
//! because that is the only behaviour the admission boundary asks of it.

use std::path::Path;

use aion_awl::compile;
use serde_json::json;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Two declared signals with different payload shapes, so the per-signal
/// mapping is exercised rather than a single-entry special case. `end` is
/// REQUIRED on `Continuation` and absent from `Nudge`, which is what lets a
/// payload be valid for one signal and invalid for the other.
const DECLARES_SIGNALS: &str = r"//! Contract pin: a declared signal reaches the package contract.
workflow signal_contract_pin
  input label: String
  signal control: Continuation
  signal ping: Nudge
  outcome done: type Done, route success

type Continuation { message: String, end: Bool }
type Nudge { note: String }
type Done { label: String }

step wait_for_operator
  wait control -> answer

  outcome done: when answer.end,
    route done(label: label)
  outcome again: otherwise,
    route done(label: answer.message)
";

/// The declared signal names in the contract are exactly the ones the document
/// declares — compared against the PARSED document, not a hardcoded list, so
/// the pin cannot drift from the fixture it is pinning.
#[test]
fn every_declared_signal_reaches_the_contract() -> TestResult {
    let document = aion_awl::parse(DECLARES_SIGNALS)?;
    let mut declared: Vec<String> = document
        .signals
        .iter()
        .map(|signal| signal.name.clone())
        .collect();
    declared.sort();
    assert!(
        !declared.is_empty(),
        "the fixture must declare at least one signal or this pin proves nothing"
    );

    let compiled = compile(DECLARES_SIGNALS, Path::new("."))?;
    let mut in_contract: Vec<String> = compiled
        .contract
        .signals
        .iter()
        .map(|signal| signal.name.clone())
        .collect();
    in_contract.sort();

    assert_eq!(
        in_contract, declared,
        "every signal the document declares must reach `PackageContract::signals` — \
         the engine's signal admission has nothing else to admit against"
    );
    Ok(())
}

/// The schema carried for a signal DISCRIMINATES: it admits a conforming
/// payload and refuses one that omits a required field.
///
/// This is the whole contract the admission boundary depends on. A schema that
/// were present but vacuous (`{}`, or one that lost its `required`) would
/// satisfy the name pin above and still let every malformed payload through.
#[test]
fn the_declared_schema_admits_conforming_and_refuses_malformed() -> TestResult {
    let compiled = compile(DECLARES_SIGNALS, Path::new("."))?;
    let control = compiled
        .contract
        .declared_signal("control")
        .ok_or("the contract lost the `control` signal")?;

    let conforming = json!({ "message": "carry on", "end": false });
    assert!(
        aion_package::admit_value(&control.input_schema, &conforming).is_ok(),
        "a payload matching the declared type must be admitted"
    );

    // `end` is declared and not optional, so its absence is the operator typo
    // this whole boundary exists to catch.
    let malformed = json!({ "message": "carry on" });
    let refusal = aion_package::admit_value(&control.input_schema, &malformed);
    assert!(
        refusal.is_err(),
        "a payload missing the required `end` field must be REFUSED — a schema that \
         admits it is present but not load-bearing, and the signal boundary would \
         pass an operator's typo straight through to a live run"
    );
    Ok(())
}

/// Two signals declared in one document do not share a schema.
///
/// The mapping builds one `SignalContract` per declaration; if it ever
/// collapsed to the first (or to the workflow's own input schema), both pins
/// above would still pass. A payload valid for `ping` is invalid for
/// `control`, which is only observable if the two schemas are genuinely
/// distinct.
#[test]
fn each_signal_carries_its_own_declared_schema() -> TestResult {
    let compiled = compile(DECLARES_SIGNALS, Path::new("."))?;
    let control = compiled
        .contract
        .declared_signal("control")
        .ok_or("the contract lost the `control` signal")?;
    let ping = compiled
        .contract
        .declared_signal("ping")
        .ok_or("the contract lost the `ping` signal")?;

    let ping_payload = json!({ "note": "still here" });
    assert!(
        aion_package::admit_value(&ping.input_schema, &ping_payload).is_ok(),
        "`ping`'s own payload must satisfy `ping`'s schema"
    );
    assert!(
        aion_package::admit_value(&control.input_schema, &ping_payload).is_err(),
        "`ping`'s payload must NOT satisfy `control`'s schema — if it does, the two \
         declarations are not carrying their own types and signal admission is \
         measuring every payload against the wrong ruler"
    );
    Ok(())
}
