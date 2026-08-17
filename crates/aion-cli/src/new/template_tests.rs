use std::collections::HashSet;

use super::Template;

/// Files every Gleam scaffold must continue to contain, placeholder form.
const GLEAM_REQUIRED_PATHS: &[&str] = &[
    "gleam.toml",
    ".gitignore",
    "aion.toml",
    "workflow.toml",
    "schemas/input.json",
    "schemas/output.json",
    "src/{{name}}.gleam",
    "README.md",
];
const AWL_REQUIRED_PATHS: &[&str] = &[".gitignore", "aion.toml", "{{name}}.awl", "README.md"];

#[test]
fn every_template_manifest_is_complete() {
    for template in Template::all() {
        let files = template.files();
        let paths: Vec<&str> = files.iter().map(|(path, _)| *path).collect();
        let required_paths = if *template == Template::Awl {
            AWL_REQUIRED_PATHS
        } else {
            GLEAM_REQUIRED_PATHS
        };
        for required in required_paths {
            assert!(
                paths.contains(required),
                "template {} is missing {required}",
                template.id()
            );
        }
        if *template == Template::Awl {
            assert_eq!(
                paths, AWL_REQUIRED_PATHS,
                "the AWL scaffold must stay minimal and descriptor-free"
            );
        }
        let unique: HashSet<&str> = paths.iter().copied().collect();
        assert_eq!(
            unique.len(),
            paths.len(),
            "template {} declares duplicate paths",
            template.id()
        );
        for (path, contents) in &files {
            assert!(
                !contents.trim().is_empty(),
                "template {} embeds empty contents for {path}",
                template.id()
            );
        }
    }
}

#[test]
fn workflow_descriptors_declare_the_template_activities() {
    for template in Template::all() {
        if *template == Template::Awl {
            continue;
        }
        let files = template.files();
        let workflow_toml = files
            .iter()
            .find(|(path, _)| *path == "workflow.toml")
            .map(|(_, contents)| *contents)
            .unwrap_or_default();
        for activity in template.activities() {
            assert!(
                workflow_toml.contains(&format!("\"{activity}\"")),
                "template {} workflow.toml does not declare activity {activity}",
                template.id()
            );
        }
        assert!(
            workflow_toml.contains("entry_module = \"{{name}}\""),
            "template {} workflow.toml must name the project module as entry",
            template.id()
        );
    }
}

#[test]
fn aion_toml_carries_every_required_server_key() {
    for template in Template::all() {
        let files = template.files();
        let aion_toml = files
            .iter()
            .find(|(path, _)| *path == "aion.toml")
            .map(|(_, contents)| *contents)
            .unwrap_or_default();
        for key in [
            "listen_address",
            "grpc_address",
            // The scaffold ships the durable ablative default backend.
            "backend = \"haematite\"",
            "query_timeout_ms",
            "event_broadcast_capacity",
            // Required since WS3: the cluster topology broadcast capacity, so
            // the scaffolded config boots without further edits.
            "cluster_broadcast_capacity",
            // Required and defaultless: the transcript drain's flush policy.
            // Without both keys the scaffolded server refuses to start.
            "max_batch_events",
            "max_batch_hold_ms",
            "enabled = true",
            // The outbox trio: without all three the liminal worker listener
            // is never bound and a remote worker's dial is refused forever.
            "transport = \"liminal\"",
            "liminal_listen_address = \"127.0.0.1:50061\"",
            "max_archive_bytes",
            "max_inflated_bytes",
        ] {
            assert!(
                aion_toml.contains(key),
                "template {} aion.toml is missing {key}",
                template.id()
            );
        }
        // The template must not pin a data_dir: the server's home default
        // (`<AION_HOME>/data`) applies, and the historical literal
        // `aion-data` is the legacy working-directory path that re-activates
        // the migration guard (resolution.rs) on every boot beside it.
        assert!(
            !aion_toml.contains("\ndata_dir"),
            "template {} aion.toml must not set data_dir",
            template.id()
        );
        // The loader fills these keys when omitted (fill_operational_defaults);
        // claiming they are required was a stale lie this scaffold used to teach.
        assert!(
            !aion_toml.contains("REQUIRED, no default"),
            "template {} aion.toml carries a stale REQUIRED claim",
            template.id()
        );
    }
}

#[test]
fn dev_pipeline_declares_three_workflow_entries_and_its_gates() {
    let files = Template::DevPipeline.files();
    let workflow_toml = files
        .iter()
        .find(|(path, _)| *path == "workflow.toml")
        .map(|(_, contents)| *contents)
        .unwrap_or_default();
    for entry in [
        "entry_module = \"{{name}}\"",
        "entry_module = \"{{name}}_dev\"",
        "entry_module = \"{{name}}_gate\"",
    ] {
        assert!(
            workflow_toml.contains(entry),
            "dev-pipeline workflow.toml must declare {entry}"
        );
    }
    for timeout in [
        "timeout_seconds = 604800",
        "timeout_seconds = 86400",
        "timeout_seconds = 21600",
    ] {
        assert!(
            workflow_toml.contains(timeout),
            "dev-pipeline workflow.toml must keep the documented {timeout}"
        );
    }
    assert!(Template::DevPipeline.worker_requirement_reason().is_some());
}

#[test]
fn embedded_awl_is_clean_and_canonical_after_name_substitution()
-> Result<(), Box<dyn std::error::Error>> {
    let source = Template::Awl
        .files()
        .iter()
        .find(|(path, _)| *path == "{{name}}.awl")
        .map(|(_, contents)| contents.replace("{{name}}", "demo_flow"))
        .ok_or("AWL template must carry its project document")?;
    let document = aion_awl::parse(&source)?;
    let diagnostics = aion_awl::check(&document);
    assert!(
        diagnostics.is_empty(),
        "embedded AWL must check with zero diagnostics: {diagnostics:?}"
    );
    assert_eq!(
        aion_awl::print(&document),
        source,
        "embedded AWL must already be canonical"
    );
    Ok(())
}

#[test]
fn worker_free_templates_do_not_require_a_worker() {
    for template in [
        Template::Awl,
        Template::HelloWorld,
        Template::ApprovalFlow,
        Template::Saga,
    ] {
        assert!(
            template.worker_requirement_reason().is_none(),
            "template {} must not require a worker",
            template.id()
        );
    }
}

#[test]
fn agent_template_requires_a_worker() {
    // The agent loop's three steps are worker-served and the driver is
    // worker-side, so the template demands `--worker rust`.
    assert!(Template::Agent.worker_requirement_reason().is_some());
}

#[test]
fn agent_template_declares_its_three_steps_and_a_review_deadline_input() {
    let files = Template::Agent.files();
    let workflow_toml = files
        .iter()
        .find(|(path, _)| *path == "workflow.toml")
        .map(|(_, contents)| *contents)
        .unwrap_or_default();
    for activity in ["scout", "act", "verify"] {
        assert!(
            workflow_toml.contains(&format!("\"{activity}\"")),
            "agent workflow.toml must declare the {activity} step"
        );
    }

    // The human-review pause is a durable `workflow.receive` raced against
    // a caller-chosen deadline — never a poll, never a default.
    let project = files
        .iter()
        .find(|(path, _)| *path == "src/{{name}}.gleam")
        .map(|(_, contents)| *contents)
        .unwrap_or_default();
    let condensed: String = project.split_whitespace().collect();
    assert!(
        condensed.contains("workflow.with_timeout(fn(){workflow.receive("),
        "agent review must be workflow.receive raced by with_timeout, not a poll"
    );
    assert!(
        condensed.contains("duration.milliseconds(input.review_timeout_ms)"),
        "agent review deadline must come from the start input, never a default"
    );

    // The review deadline is a required start-input field, not defaulted.
    let input_schema = files
        .iter()
        .find(|(path, _)| *path == "schemas/input.json")
        .map(|(_, contents)| *contents)
        .unwrap_or_default();
    assert!(
        input_schema.contains("\"review_timeout_ms\""),
        "agent input schema must require the review deadline"
    );
}

#[test]
fn types_first_templates_ship_marked_generated_artifacts() {
    // hello_world, dev_pipeline, and agent are on the types-first path:
    // they ship an AUTHORED types module plus pre-generated artifacts
    // carrying the generated markers, so `aion generate` recognises and
    // reproduces them (a stray unmarked schema would be a loud error).
    for template in [Template::HelloWorld, Template::DevPipeline, Template::Agent] {
        for (path, contents) in template.files() {
            if path.starts_with("schemas/") {
                assert!(
                    contents.contains("Generated by aion generate from src/"),
                    "template {} schema {path} must carry the generated marker",
                    template.id()
                );
            }
            if path == "src/{{name}}_codecs.gleam" {
                assert!(
                    contents.starts_with("//// Generated by aion generate"),
                    "template {} codecs module must carry the generated header",
                    template.id()
                );
            }
            if path == "src/{{name}}_io.gleam" {
                assert!(
                    !contents.contains("do not edit"),
                    "template {} types module must be authored, not generated",
                    template.id()
                );
            }
        }
    }
}

#[test]
fn worker_manifests_exist_exactly_for_templates_with_activities() {
    for template in Template::all() {
        let has_worker = !template.worker_files().is_empty();
        let has_activities = !template.activities().is_empty();
        assert_eq!(
            has_worker,
            has_activities,
            "template {} worker manifest must match its activity surface",
            template.id()
        );
        let worker_files = template.worker_files();
        if !worker_files.is_empty() {
            let paths: Vec<&str> = worker_files.iter().map(|(path, _)| *path).collect();
            assert!(paths.contains(&"worker/Cargo.toml"));
            assert!(paths.contains(&"worker/src/main.rs"));
            let main = worker_files
                .iter()
                .find(|(path, _)| *path == "worker/src/main.rs")
                .map(|(_, contents)| *contents)
                .unwrap_or_default();
            // Whitespace-insensitive: registrations may be wrapped
            // across lines by rustfmt.
            let condensed: String = main.split_whitespace().collect();
            for activity in template.activities() {
                assert!(
                    condensed.contains(&format!("register_activity(\"{activity}\"")),
                    "template {} worker must register {activity}",
                    template.id()
                );
            }
        }
    }
}
