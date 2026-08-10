use std::io;
use std::path::{Component, Path};

use aion_awl::{Span, TypeBody, parse, print, semantic, utf16_range_for_span};
use serde::{Deserialize, Serialize};

use crate::filesystem::ConfinedDir;

#[derive(Debug, Deserialize)]
pub struct CheckRequest {
    pub source: String,
    pub path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CheckResponse {
    pub ok: bool,
    pub deploys_green: bool,
    pub steps: Option<usize>,
    pub diagnostics: Vec<Diagnostic>,
    pub semantic: Option<SemanticIndex>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub class: DiagnosticClass,
    pub message: String,
    pub line: usize,
    pub column: usize,
    /// The diagnostic's full extent as a UTF-16 range (ruled: the wire carries
    /// the editors' index space, converted server-side — never byte offsets).
    pub range: DiagnosticRange,
}

/// A half-open UTF-16 range on the wire: `start` inclusive, `end` exclusive,
/// both zero-based, `character` in UTF-16 code units — the index space
/// `CodeMirror` and the LSP protocol share, so no client re-derives it.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct DiagnosticRange {
    pub start: DiagnosticPosition,
    pub end: DiagnosticPosition,
}

/// One zero-based UTF-16 position inside a [`DiagnosticRange`].
#[derive(Debug, Clone, Copy, Serialize)]
pub struct DiagnosticPosition {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticClass {
    Error,
}

#[derive(Debug, Deserialize)]
pub struct FormatRequest {
    pub source: String,
}

#[derive(Debug, Serialize)]
pub struct FormatResponse {
    pub formatted: String,
}

#[derive(Debug, Serialize)]
pub struct SemanticIndex {
    pub entries: Vec<SemanticEntry>,
    pub graph: super::projection::GraphProjection,
    pub studio: super::studio_projection::StudioProjection,
}

#[derive(Debug, Serialize)]
pub struct SemanticEntry {
    pub span: SourceSpan,
    #[serde(rename = "type")]
    pub type_text: Option<String>,
    pub declaration: Option<SemanticDeclaration>,
}

#[derive(Debug, Serialize)]
pub struct SemanticDeclaration {
    pub name: String,
    pub kind: String,
    pub documentation: Option<String>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct SourceSpan {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

pub fn check_source(request: &CheckRequest) -> CheckResponse {
    check_source_at(request, None)
}

/// Checks `source` with schema imports resolved against `root`, carrying no
/// workspace document path.
///
/// The deployed-AWL read surface uses this: its schemas come out of the
/// deployed archive and are staged into a directory of their own, so there is
/// a root but deliberately no workspace path — a deployed document is not a
/// workspace document and must never be resolved as one.
pub(crate) fn check_source_at_root(source: &str, root: Option<&Path>) -> CheckResponse {
    check_source_at(
        &CheckRequest {
            source: source.to_owned(),
            path: None,
        },
        root,
    )
}

fn check_source_at(request: &CheckRequest, root: Option<&Path>) -> CheckResponse {
    let document = match parse(&request.source) {
        Ok(document) => document,
        Err(error) => {
            return CheckResponse {
                ok: false,
                deploys_green: false,
                steps: None,
                diagnostics: vec![diagnostic(
                    &request.source,
                    DiagnosticClass::Error,
                    error.message,
                    error.span,
                )],
                semantic: None,
            };
        }
    };
    let analysis = root.map_or_else(
        || semantic::analyze(&document),
        |root| semantic::analyze_in(&document, root),
    );
    let diagnostics: Vec<_> = analysis
        .diagnostics()
        .iter()
        .map(|error| {
            diagnostic(
                &request.source,
                DiagnosticClass::Error,
                error.message.clone(),
                error.span,
            )
        })
        .collect();
    if !diagnostics.is_empty() {
        return CheckResponse {
            ok: false,
            deploys_green: false,
            steps: None,
            diagnostics,
            semantic: None,
        };
    }
    let graph = super::projection::build(&document, analysis.step_kinds());
    let studio = super::studio_projection::build(&document);
    let semantic = SemanticIndex {
        entries: analysis
            .iter()
            .map(|info| SemanticEntry {
                span: info.span.into(),
                type_text: info.ty.clone(),
                declaration: info
                    .declaration
                    .as_ref()
                    .map(|declaration| SemanticDeclaration {
                        name: declaration.name.clone(),
                        kind: declaration.kind.as_str().to_owned(),
                        documentation: declaration.documentation.clone(),
                        span: declaration.span.into(),
                    }),
            })
            .collect(),
        graph,
        studio,
    };
    CheckResponse {
        ok: true,
        deploys_green: diagnostics.is_empty(),
        steps: Some(document.steps.len()),
        diagnostics,
        semantic: Some(semantic),
    }
}

pub async fn check_source_in_workspace(
    workspace_root: &Path,
    request: &CheckRequest,
) -> Result<CheckResponse, super::documents::DocumentError> {
    let Some(requested_path) = request.path.as_deref() else {
        return Ok(check_source(request));
    };
    let workspace_root = workspace_root.to_owned();
    let source = request.source.clone();
    let requested_path = requested_path.to_owned();
    tokio::task::spawn_blocking(move || {
        let Ok(_) = parse(&source) else {
            return Ok(check_source(&CheckRequest {
                source,
                path: Some(requested_path),
            }));
        };
        let (_staging, staged_root) =
            stage_schema_imports(&workspace_root, &requested_path, &source)?;
        Ok(check_source_at(
            &CheckRequest {
                source,
                path: Some(requested_path),
            },
            Some(&staged_root),
        ))
    })
    .await
    .map_err(|error| {
        super::documents::DocumentError::Io(io::Error::other(format!(
            "AWL check task failed: {error}"
        )))
    })?
}

pub(crate) fn stage_schema_imports(
    workspace_root: &Path,
    requested_path: &str,
    source: &str,
) -> Result<(tempfile::TempDir, std::path::PathBuf), super::documents::DocumentError> {
    let document_path = super::documents::document_path(requested_path)?;
    let document = parse(source)
        .map_err(|error| super::documents::DocumentError::InvalidPath(error.message))?;
    // A workspace directory that does not exist yet is a NORMAL state on a
    // fresh server — the default workspace is materialized on first write.
    // No directory means no schema files can exist to import, so the check
    // proceeds against an empty staged root instead of failing with a raw
    // errno (mirrors `documents::list` on the same absence).
    let workspace = match ConfinedDir::open(workspace_root) {
        Ok(workspace) => Some(workspace),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(super::documents::DocumentError::Io(error)),
    };
    let staging = tempfile::Builder::new().prefix("aion-schema-").tempdir()?;
    let document_parent = document_path.parent().unwrap_or_else(|| Path::new(""));
    let analysis_root = staging.path().join(document_parent);
    std::fs::create_dir_all(&analysis_root)?;
    let Some(workspace) = workspace else {
        return Ok((staging, analysis_root));
    };
    for declaration in &document.types {
        let TypeBody::SchemaImport { path, .. } = &declaration.body else {
            continue;
        };
        let import = Path::new(path);
        if path.is_empty()
            || import
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            continue;
        }
        let bytes = match workspace.read(&document_parent.join(import)) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::InvalidInput | io::ErrorKind::NotADirectory
                ) || error.raw_os_error() == Some(rustix::io::Errno::LOOP.raw_os_error()) =>
            {
                return Err(super::documents::DocumentError::InvalidPath(format!(
                    "schema import `{path}` contains a link: {error}"
                )));
            }
            Err(error) => return Err(super::documents::DocumentError::Io(error)),
        };
        let staged = analysis_root.join(import);
        if let Some(parent) = staged.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(staged, bytes)?;
    }
    Ok((staging, analysis_root))
}

pub fn format_source(request: &FormatRequest) -> Result<FormatResponse, Diagnostic> {
    parse(&request.source)
        .map(|document| FormatResponse {
            formatted: print(&document),
        })
        .map_err(|error| {
            diagnostic(
                &request.source,
                DiagnosticClass::Error,
                error.message,
                error.span,
            )
        })
}

fn diagnostic(source: &str, class: DiagnosticClass, message: String, span: Span) -> Diagnostic {
    let range = utf16_range_for_span(source, span);
    Diagnostic {
        class,
        message,
        line: span.line,
        column: span.column,
        range: DiagnosticRange {
            start: DiagnosticPosition {
                line: range.start.line,
                character: range.start.character,
            },
            end: DiagnosticPosition {
                line: range.end.line,
                character: range.end.character,
            },
        },
    }
}

impl From<Span> for SourceSpan {
    fn from(span: Span) -> Self {
        Self {
            start: span.start,
            end: span.end,
            line: span.line,
            column: span.column,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HAPPY: &str =
        include_str!("../../../aion-awl/tests/fixtures/rev2/dag-fork/valid/after_single.awl");
    const INVALID: &str = include_str!(
        "../../../aion-awl/tests/fixtures/rev2/dag-fork/invalid/unknown_after_target.awl"
    );
    const EMIT_REFUSED: &str = "//! Emitter refusal probe.\nworkflow emit_refused\n  outcome done: type Result, route success\n\ntype Result { value: String }\n\nworker work\n  action make() -> Result\n\nstep finish\n  make() -> result\n  route done(value: result.value)\n  on failure\n    route done(value: \"failed\")\n";

    #[test]
    fn check_happy_path_returns_steps_and_semantics() {
        let response = check_source(&CheckRequest {
            source: HAPPY.to_owned(),
            path: None,
        });
        assert!(response.ok);
        assert!(response.deploys_green);
        assert_eq!(response.steps, Some(2));
        assert!(response.diagnostics.is_empty());
        assert!(
            response
                .semantic
                .is_some_and(|semantic| !semantic.entries.is_empty())
        );
    }

    #[test]
    fn checker_message_surfaces_verbatim() -> Result<(), Box<dyn std::error::Error>> {
        let document = parse(INVALID)?;
        let expected = aion_awl::check(&document)
            .first()
            .ok_or("fixture unexpectedly checks cleanly")?
            .message
            .clone();
        let response = check_source(&CheckRequest {
            source: INVALID.to_owned(),
            path: None,
        });
        assert!(!response.ok);
        assert_eq!(response.diagnostics[0].message, expected);
        assert!(matches!(
            response.diagnostics[0].class,
            DiagnosticClass::Error
        ));
        Ok(())
    }

    /// The wire diagnostic carries the checker span's FULL extent as a UTF-16
    /// range — the fidelity this surface lost when `diagnostic()` dropped
    /// `span.start`/`span.end` and clients underlined one code point.
    #[test]
    fn checker_diagnostics_carry_their_span_as_a_utf16_range()
    -> Result<(), Box<dyn std::error::Error>> {
        let document = parse(INVALID)?;
        let span = aion_awl::check(&document)
            .first()
            .ok_or("fixture unexpectedly checks cleanly")?
            .span;
        let expected = utf16_range_for_span(INVALID, span);
        assert!(
            span.end > span.start,
            "the fixture's checker span must have extent for this pin to bite"
        );
        let response = check_source(&CheckRequest {
            source: INVALID.to_owned(),
            path: None,
        });
        let wire = serde_json::to_value(&response.diagnostics)?;
        assert_eq!(
            wire[0]["range"]["start"]["line"],
            u64::from(expected.start.line)
        );
        assert_eq!(
            wire[0]["range"]["start"]["character"],
            u64::from(expected.start.character)
        );
        assert_eq!(
            wire[0]["range"]["end"]["line"],
            u64::from(expected.end.line)
        );
        assert_eq!(
            wire[0]["range"]["end"]["character"],
            u64::from(expected.end.character)
        );
        assert!(
            wire[0].get("span").is_none() && wire[0].get("start").is_none(),
            "ruled: the wire gains a UTF-16 range ONLY — no byte-span field"
        );
        Ok(())
    }

    /// A diagnostic AFTER a multibyte character on its own line is positioned
    /// in UTF-16 code units, not bytes — the drift class the editor seam
    /// documents (2026-07-12). The prefix guard makes the pin non-vacuous: if
    /// the flagged token ever moves before the emoji, byte and UTF-16 columns
    /// coincide and this test refuses to certify anything.
    #[test]
    fn a_diagnostic_after_a_multibyte_character_ranges_in_utf16_units()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = "//! Range probe.\nworkflow probe\n  outcome done: type Result, route success\n\ntype Result { value: String }\n\nworker work\n  action make(name: String, tag: String) -> Result\n\nstep finish\n  make(name: \"🧭 café\", tag: missing_binding) -> result\n  route done(value: result.value)\n";
        let span = match parse(source) {
            Err(error) => error.span,
            Ok(document) => {
                aion_awl::check(&document)
                    .first()
                    .ok_or("the multibyte probe must produce a diagnostic")?
                    .span
            }
        };
        let line_start = source[..span.start].rfind('\n').map_or(0, |at| at + 1);
        let prefix = &source[line_start..span.start];
        assert_ne!(
            prefix.encode_utf16().count(),
            prefix.len(),
            "the flagged token must sit AFTER the multibyte text on its line, \
             or byte and UTF-16 columns coincide and the pin is vacuous"
        );
        let expected_line =
            u64::try_from(source[..span.start].matches('\n').count()).unwrap_or(u64::MAX);
        let expected_character = u64::try_from(prefix.encode_utf16().count()).unwrap_or(u64::MAX);
        // The END position, by the same independent arithmetic: the extent
        // after multibyte text must land in UTF-16 units too, or a client
        // underlines the right start with the wrong width.
        assert!(span.end > span.start, "the probe span must have extent");
        let end_line_start = source[..span.end].rfind('\n').map_or(0, |at| at + 1);
        let end_prefix = &source[end_line_start..span.end];
        let expected_end_line =
            u64::try_from(source[..span.end].matches('\n').count()).unwrap_or(u64::MAX);
        let expected_end_character =
            u64::try_from(end_prefix.encode_utf16().count()).unwrap_or(u64::MAX);
        let response = check_source(&CheckRequest {
            source: source.to_owned(),
            path: None,
        });
        assert!(!response.ok);
        let wire = serde_json::to_value(&response.diagnostics)?;
        assert_eq!(wire[0]["range"]["start"]["line"], expected_line);
        assert_eq!(wire[0]["range"]["start"]["character"], expected_character);
        assert_eq!(wire[0]["range"]["end"]["line"], expected_end_line);
        assert_eq!(wire[0]["range"]["end"]["character"], expected_end_character);
        Ok(())
    }

    #[test]
    fn checker_only_check_does_not_consult_the_legacy_emitter() {
        let response = check_source(&CheckRequest {
            source: EMIT_REFUSED.to_owned(),
            path: None,
        });
        assert!(response.ok, "diagnostics: {:?}", response.diagnostics);
        assert!(response.deploys_green);
        assert!(response.diagnostics.is_empty());
    }

    /// A fresh server's default workspace directory does not exist until the
    /// first write materializes it. A check that names a `path` must still
    /// succeed against that absence — no directory means no schema files to
    /// import, not a raw errno.
    #[tokio::test]
    async fn a_pathed_check_succeeds_when_the_workspace_does_not_exist_yet()
    -> Result<(), Box<dyn std::error::Error>> {
        let parent = crate::test_support::private_tempdir()?;
        let workspace = parent.path().join("never-created");
        assert!(!workspace.exists());
        let response = check_source_in_workspace(
            &workspace,
            &CheckRequest {
                source: HAPPY.to_owned(),
                path: Some("drafts/after_single.awl".to_owned()),
            },
        )
        .await?;
        assert!(response.ok, "diagnostics: {:?}", response.diagnostics);
        assert!(response.deploys_green);
        assert!(
            !workspace.exists(),
            "a check must not materialize the workspace"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn http_check_confines_document_paths_and_schema_imports()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;

        let workspace = crate::test_support::private_tempdir()?;
        let fixture_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../aion-awl/tests/fixtures/rev2/schema-doors/valid");
        let source = std::fs::read_to_string(fixture_dir.join("mixed_doors.awl"))?;
        let schema = std::fs::read(fixture_dir.join("intake.schema.json"))?;
        std::fs::create_dir(workspace.path().join("nested"))?;
        std::fs::write(workspace.path().join("nested/intake.schema.json"), &schema)?;
        std::fs::write(workspace.path().join("intake.schema.json"), &schema)?;

        let valid = check_source_in_workspace(
            workspace.path(),
            &CheckRequest {
                source: source.clone(),
                path: Some("nested/mixed_doors.awl".to_owned()),
            },
        )
        .await?;
        assert!(valid.ok, "confined import failed: {:?}", valid.diagnostics);

        let missing_name = "missing-intake.schema.json";
        std::fs::write(workspace.path().join(missing_name), &schema)?;
        let missing = check_source_in_workspace(
            workspace.path(),
            &CheckRequest {
                source: source.replace("intake.schema.json", missing_name),
                path: Some("nested/mixed_doors.awl".to_owned()),
            },
        )
        .await?;
        let missing_reason = missing
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!missing.ok);
        assert!(missing_reason.contains(missing_name));
        assert!(!missing_reason.contains("AWL workspace I/O failed"));

        let outside = workspace.path().parent().ok_or("workspace had no parent")?;
        let absolute_source = source.replace(
            "intake.schema.json",
            &outside.join("outside.schema.json").to_string_lossy(),
        );
        let absolute = check_source_in_workspace(
            workspace.path(),
            &CheckRequest {
                source: absolute_source,
                path: Some("nested/mixed_doors.awl".to_owned()),
            },
        )
        .await?;
        assert!(!absolute.ok);
        assert!(absolute.diagnostics[0].message.contains("relative path"));

        let traversal = check_source_in_workspace(
            workspace.path(),
            &CheckRequest {
                source: source.replace("intake.schema.json", "../outside.schema.json"),
                path: Some("nested/mixed_doors.awl".to_owned()),
            },
        )
        .await?;
        assert!(!traversal.ok);
        assert!(traversal.diagnostics[0].message.contains("no `..`"));

        let absolute_document = check_source_in_workspace(
            workspace.path(),
            &CheckRequest {
                source: source.clone(),
                path: Some("/tmp/mixed_doors.awl".to_owned()),
            },
        )
        .await;
        assert!(matches!(
            absolute_document,
            Err(super::super::documents::DocumentError::InvalidPath(_))
        ));

        let external_schema = outside.join("external.schema.json");
        std::fs::write(&external_schema, b"{\"type\":\"object\"}")?;
        symlink(
            &external_schema,
            workspace.path().join("nested/linked.schema.json"),
        )?;
        let linked = check_source_in_workspace(
            workspace.path(),
            &CheckRequest {
                source: source.replace("intake.schema.json", "linked.schema.json"),
                path: Some("nested/mixed_doors.awl".to_owned()),
            },
        )
        .await;
        let Err(error) = linked else {
            return Err("schema symlink was followed".into());
        };
        assert!(matches!(
            error,
            super::super::documents::DocumentError::InvalidPath(reason)
                if reason.contains("contains a link")
        ));
        Ok(())
    }

    #[test]
    fn format_is_canonical_and_idempotent() -> Result<(), Diagnostic> {
        let once = format_source(&FormatRequest {
            source: HAPPY.replace("type Summary  {", "type Summary {"),
        })?;
        let twice = format_source(&FormatRequest {
            source: once.formatted.clone(),
        })?;
        assert_eq!(once.formatted, twice.formatted);
        assert_eq!(once.formatted, HAPPY);
        Ok(())
    }

    #[test]
    fn every_valid_fixture_projects_without_mutating_source()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../aion-awl/tests/fixtures");
        let mut paths = Vec::new();
        collect_valid_fixtures(&fixtures, &mut paths)?;
        assert!(!paths.is_empty());
        for path in paths {
            let source = std::fs::read_to_string(&path)?;
            let original = source.clone();
            let document = parse(&source)
                .map_err(|error| format!("{} did not parse: {}", path.display(), error.message))?;
            let canonical = print(&document);
            assert_eq!(print(&parse(&canonical)?), canonical, "{}", path.display());
            let response = check_source_at(
                &CheckRequest {
                    source: source.clone(),
                    path: Some(path.to_string_lossy().into_owned()),
                },
                path.parent(),
            );
            assert_eq!(source, original, "projection mutated {}", path.display());
            assert!(
                response.ok,
                "{}: {:?}",
                path.display(),
                response.diagnostics
            );
            let graph = response
                .semantic
                .ok_or("valid fixture had no semantics")?
                .graph;
            assert_eq!(
                graph.steps.len(),
                document.steps.len(),
                "{}",
                path.display()
            );
        }
        Ok(())
    }

    fn collect_valid_fixtures(
        directory: &Path,
        found: &mut Vec<std::path::PathBuf>,
    ) -> std::io::Result<()> {
        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                collect_valid_fixtures(&path, found)?;
            } else if path.extension().is_some_and(|extension| extension == "awl")
                && path
                    .components()
                    .any(|component| component.as_os_str() == "valid")
            {
                found.push(path);
            }
        }
        Ok(())
    }
}
