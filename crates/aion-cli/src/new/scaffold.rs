//! Scaffolding for `aion new`: name validation, target-directory checks,
//! placeholder substitution, and file emission.

use std::path::Path;

use anyhow::{Context, Result, bail};
use clap::{Args, ValueEnum};
use serde::Serialize;
use serde_json::Value;

use crate::new::template::{ManifestFile, Template};
use crate::output::to_value;

/// Worker SDK languages `--worker` can scaffold.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum WorkerLanguage {
    /// A Rust crate under `worker/` built on the `aion-worker` SDK.
    Rust,
}

/// Arguments for `aion new`.
#[derive(Args, Clone, Debug)]
pub struct NewArgs {
    /// Project name; becomes the directory and the AWL workflow or Gleam entry
    /// module. Must be lowercase `snake_case`: a lowercase letter followed by
    /// lowercase letters, digits, or underscores.
    name: String,
    /// Scaffold template.
    #[arg(long, value_enum, default_value_t = Template::Awl)]
    template: Template,
    /// Additionally scaffold an activity worker crate under `worker/`
    /// serving the template's activities. Only templates that dispatch
    /// activities (saga, dev-pipeline) accept this; dev-pipeline requires
    /// it.
    #[arg(long, value_enum)]
    worker: Option<WorkerLanguage>,
}

impl NewArgs {
    /// Test constructor mirroring the clap surface.
    #[cfg(test)]
    pub(crate) fn for_tests(
        name: &str,
        template: Template,
        worker: Option<WorkerLanguage>,
    ) -> Self {
        Self {
            name: name.to_owned(),
            template,
            worker,
        }
    }
}

/// JSON document printed on stdout after a successful `new` run.
#[derive(Serialize)]
struct NewOutput {
    project: String,
    path: String,
    template: &'static str,
    worker: Option<&'static str>,
    files: Vec<String>,
    next_steps: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    warning: Option<String>,
}

/// The mint-time refusal warning for the legacy Gleam `agent` template: its
/// implicit activity declarations cannot carry a queue-scoped contract, so
/// the packaged workflow fails the `.v4` structural start admission. The
/// break must be loud here, at mint — not discovered at the server's door.
fn refusal_warning(args: &NewArgs) -> Option<String> {
    (args.template == Template::Agent).then(|| {
        let mut activities = args.template.activities().to_vec();
        activities.sort_unstable();
        format!(
            "the `agent` template is the legacy Gleam scaffold: its activities ({}) carry no \
             queue-scoped contract, so `aion start` refuses the deployed package with \
             NO_QUEUE_DECLARATION. Scaffold it to study the shape; the AWL agent scaffold \
             (template-AWL-migration lane) is its successor",
            activities.join(", ")
        )
    })
}

/// Runs the `new` subcommand against the invoker's current directory.
pub fn run(args: &NewArgs) -> Result<Value> {
    let parent = std::env::current_dir().context("failed to resolve the current directory")?;
    scaffold(&parent, args)
}

/// Scaffolds `<parent>/<name>` from the selected template.
fn scaffold(parent: &Path, args: &NewArgs) -> Result<Value> {
    validate_name(&args.name)?;
    if args.template == Template::Awl {
        validate_awl_name(&args.name)?;
    }
    let files = manifest(args)?;
    let target = parent.join(&args.name);
    ensure_target_is_empty(&target)?;

    let mut written = Vec::with_capacity(files.len());
    for (path_template, contents_template) in files {
        let relative = render(path_template, &args.name)?;
        let contents = render(contents_template, &args.name)?;
        let destination = target.join(&relative);
        if let Some(directory) = destination.parent() {
            std::fs::create_dir_all(directory)
                .with_context(|| format!("failed to create directory {}", directory.display()))?;
        }
        std::fs::write(&destination, contents)
            .with_context(|| format!("failed to write {}", destination.display()))?;
        written.push(relative);
    }

    let warning = refusal_warning(args);
    if let Some(warning) = &warning {
        eprintln!("warning: {warning}");
    }
    to_value(NewOutput {
        project: args.name.clone(),
        path: target.display().to_string(),
        template: args.template.id(),
        worker: args.worker.map(|WorkerLanguage::Rust| "rust"),
        files: written,
        next_steps: format!("see {}/README.md", args.name),
        warning,
    })
}

/// Resolves the full file manifest: template files plus, when requested and
/// available, the worker crate. Refuses `--worker` for templates whose
/// workflows dispatch no activities — there is nothing for a worker to
/// serve, and a dangling worker crate would be a lie. Conversely refuses to
/// OMIT `--worker` for templates whose every activity is worker-served: a
/// dev pipeline without its worker cannot run live, and emitting one would
/// be a different lie.
fn manifest(args: &NewArgs) -> Result<Vec<ManifestFile>> {
    let mut files = args.template.files();
    match args.worker {
        Some(WorkerLanguage::Rust) => {
            if args.template.activities().is_empty() {
                bail!(
                    "the {} template declares no activities, so there is no worker to scaffold; \
                     use `--template saga`, `--template agent`, `--template dev-pipeline`, or \
                     drop `--worker`",
                    args.template.id()
                );
            }
            files.extend_from_slice(args.template.worker_files());
        }
        None => {
            if let Some(reason) = args.template.worker_requirement_reason() {
                bail!(
                    "the {} template requires a worker: {reason}, so a scaffold without one \
                     cannot run; pass `--worker rust`",
                    args.template.id()
                );
            }
        }
    }
    Ok(files)
}

/// Refuses a snake-case name when AWL tokenizes it as reserved vocabulary
/// rather than as the identifier used after `workflow`.
fn validate_awl_name(name: &str) -> Result<()> {
    let tokens = aion_awl::lex(name).context("failed to validate the project name as AWL")?;
    let is_identifier = matches!(
        tokens.as_slice(),
        [token]
            if matches!(&token.kind, aion_awl::TokenKind::Identifier(identifier) if identifier == name)
    );
    if !is_identifier {
        bail!(
            "invalid project name {name:?} for the awl template: an AWL workflow name must be a \
             lowercase snake_case identifier and cannot be an AWL reserved word"
        );
    }
    Ok(())
}

/// Validates the project name against Gleam module naming: lowercase
/// `snake_case`, starting with a lowercase ASCII letter.
fn validate_name(name: &str) -> Result<()> {
    let mut characters = name.chars();
    let starts_with_letter = characters
        .next()
        .is_some_and(|first| first.is_ascii_lowercase());
    let rest_is_snake = characters.all(|character| {
        character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
    });
    if !starts_with_letter || !rest_is_snake {
        bail!(
            "invalid project name {name:?}: the name becomes the project's entry identifier \
             (the AWL workflow name or the Gleam entry module), so it must be lowercase \
             snake_case — a lowercase ASCII letter (a-z) followed by lowercase letters, \
             digits, or underscores"
        );
    }
    if name == "gleam" {
        bail!(
            "invalid project name \"gleam\": reserved across all templates — the Gleam build \
             tool cannot build a project by this name, and a project keeps its name when it \
             adopts a Gleam-family template later"
        );
    }
    Ok(())
}

/// Refuses to scaffold over existing content. An existing *empty* directory
/// is acceptable; anything else is a loud error.
fn ensure_target_is_empty(target: &Path) -> Result<()> {
    if !target.exists() {
        return Ok(());
    }
    if !target.is_dir() {
        bail!(
            "refusing to scaffold: {} already exists and is not a directory",
            target.display()
        );
    }
    let mut entries = std::fs::read_dir(target)
        .with_context(|| format!("failed to inspect {}", target.display()))?;
    if entries.next().is_some() {
        bail!(
            "refusing to scaffold into {}: the directory is not empty",
            target.display()
        );
    }
    Ok(())
}

/// Substitutes every supported placeholder and fails loudly on any leftover
/// `{{` — an unresolved placeholder is a template bug, never emitted code.
fn render(template_text: &str, name: &str) -> Result<String> {
    let rendered = template_text
        .replace("{{name}}", name)
        .replace("{{aion_worker_version}}", env!("CARGO_PKG_VERSION"));
    if let Some(index) = rendered.find("{{") {
        let snippet: String = rendered[index..].chars().take(40).collect();
        bail!("unresolved template placeholder: {snippet}");
    }
    Ok(rendered)
}

#[cfg(test)]
#[path = "scaffold_tests.rs"]
mod tests;
