//! Composition-root tests: argument parsing (incl. `--max-parallel`), the
//! role table (nodes, schemas, deny-lists, concurrency), its grouping into
//! one connection per NODE, the per-run session mechanic's wiring, and the
//! node-routing contract.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    COORDINATOR_CONCURRENCY, Connection, DEFAULT_ADDRESS, Profiles, Role, SHELL_NODE, Shell,
    connection_norn_args, connections, inner_norn_harness, parse_args_from, roles, shell_registry,
};

fn parse(args: &[&str]) -> anyhow::Result<super::Args> {
    parse_args_from(
        args.iter().map(|arg| (*arg).to_owned()),
        "norn-default".to_owned(),
    )
}

fn profiles() -> Profiles {
    Profiles {
        planner: "plan".to_owned(),
        developer: "dev".to_owned(),
        reviewer: "rev".to_owned(),
        remediator: "rem".to_owned(),
    }
}

/// The production role table at a chosen `--max-parallel`.
fn role_table(max_parallel: usize) -> Vec<Role> {
    roles(profiles(), max_parallel, COORDINATOR_CONCURRENCY)
}

/// The production connection table (the role table folded per node).
fn connection_table(max_parallel: usize) -> anyhow::Result<Vec<Connection>> {
    connections(role_table(max_parallel)).map_err(anyhow::Error::msg)
}

/// The connection serving `activity_type`, or a loud test failure.
fn connection_for(connections: &[Connection], activity_type: &str) -> anyhow::Result<&'static str> {
    for connection in connections {
        if connection
            .activities
            .iter()
            .any(|activity| activity.activity_type == activity_type)
        {
            return Ok(connection.node);
        }
    }
    anyhow::bail!("no connection serves {activity_type}")
}

#[test]
fn profiles_dir_is_required() -> anyhow::Result<()> {
    let Err(error) = parse(&[]) else {
        anyhow::bail!("arguments unexpectedly parsed without --profiles-dir");
    };
    assert!(
        error.to_string().contains("--profiles-dir"),
        "error: {error}"
    );
    Ok(())
}

#[test]
fn minimal_arguments_yield_the_defaults() -> anyhow::Result<()> {
    let args = parse(&["--profiles-dir", "/pkg/profiles"])?;
    assert_eq!(args.candidates, vec![DEFAULT_ADDRESS.to_owned()]);
    assert_eq!(args.identity_prefix, "staged-rounds-worker");
    assert_eq!(args.ready_file, None);
    assert_eq!(args.norn_bin, "norn-default");
    assert_eq!(args.profiles_dir, "/pkg/profiles");
    assert_eq!(args.max_parallel, 4);
    Ok(())
}

#[test]
fn max_parallel_parses_and_rejects_zero() -> anyhow::Result<()> {
    let args = parse(&["--profiles-dir", "/p", "--max-parallel", "7"])?;
    assert_eq!(args.max_parallel, 7);
    let Err(error) = parse(&["--profiles-dir", "/p", "--max-parallel", "0"]) else {
        anyhow::bail!("--max-parallel 0 unexpectedly parsed");
    };
    assert!(error.to_string().contains("at least 1"), "error: {error}");
    assert!(parse(&["--profiles-dir", "/p", "--max-parallel", "x"]).is_err());
    Ok(())
}

#[test]
fn every_value_taking_flag_bails_when_missing() {
    for flag in [
        "--address",
        "--identity",
        "--ready-file",
        "--norn-bin",
        "--profiles-dir",
        "--max-parallel",
    ] {
        assert_eq!(
            parse(&[flag]).err().map(|error| error.to_string()),
            Some(format!("{flag} requires a value")),
        );
    }
}

#[test]
fn unknown_argument_bails() {
    assert_eq!(
        parse(&["--bogus"]).err().map(|error| error.to_string()),
        Some("unknown argument `--bogus`".to_owned()),
    );
}

/// The role wiring: activity type, node, deny-list, and concurrency per
/// role. The developer and reviewer take the operator's `--max-parallel`
/// (the genuine-parallelism knob); the coordinator roles stay narrow. The
/// reviewer's concurrency now bounds `items × lenses` concurrent panel seats,
/// not `items`.
#[test]
fn roles_bind_node_deny_list_and_concurrency() {
    let roles = role_table(6);
    let summary: Vec<(&str, &str, Option<&str>, usize)> = roles
        .iter()
        .map(|role| {
            (
                role.activity_type,
                role.node,
                role.disallowed_tools,
                role.max_concurrency,
            )
        })
        .collect();
    assert_eq!(
        summary,
        vec![
            (
                "planner",
                "planner",
                Some("write,edit,apply_patch"),
                COORDINATOR_CONCURRENCY
            ),
            ("dev_item", "developer", None, 6),
            ("review_item", "reviewer", Some("write,edit,apply_patch"), 6),
            ("remediate", "remediation", None, COORDINATOR_CONCURRENCY),
            (
                "fix_integration",
                "remediation",
                None,
                COORDINATOR_CONCURRENCY
            ),
        ]
    );
    for role in &roles {
        assert!(role.output_schema.trim_start().starts_with('{'));
        assert!(!role.profile.is_empty());
    }
}

/// The routing constraint the connection grouping exists for: the server
/// routes by (namespace, `task_queue`, node) ONLY, so every activity pinned to
/// one node must be served by ONE connection. `remediate` and
/// `fix_integration` are both pinned to `remediation`, so they must land on a
/// single connection that serves both.
#[test]
fn activities_sharing_a_node_land_on_one_connection() -> anyhow::Result<()> {
    let connections = connection_table(4)?;
    let nodes: Vec<&str> = connections.iter().map(|entry| entry.node).collect();
    assert_eq!(
        nodes,
        vec!["planner", "developer", "reviewer", "remediation"],
        "one connection per node, in role-table order"
    );
    let remediation = connections
        .iter()
        .find(|entry| entry.node == "remediation")
        .ok_or_else(|| anyhow::anyhow!("no remediation connection"))?;
    let served: Vec<&str> = remediation
        .activities
        .iter()
        .map(|activity| activity.activity_type)
        .collect();
    assert_eq!(
        served,
        vec!["remediate", "fix_integration"],
        "both remediation-node activities must be served by the ONE connection \
         registered on that node — two connections on one node would let the \
         server land an activity on a handlerless connection"
    );
    Ok(())
}

/// A connection owns ONE composed Norn command, so two roles sharing a node
/// must agree on everything the connection owns. A disagreement is refused at
/// startup, by name — never silently run with the wrong doctrine.
#[test]
fn two_roles_on_one_node_must_agree_on_what_the_connection_owns() -> anyhow::Result<()> {
    for (label, mutate) in [
        (
            "--output-schema",
            (|role: &mut Role| role.output_schema = "{\"different\":true}") as fn(&mut Role),
        ),
        ("the profile doctrine", |role: &mut Role| {
            role.profile = "a different doctrine".to_owned();
        }),
        ("--disallowed-tools", |role: &mut Role| {
            role.disallowed_tools = Some("write");
        }),
        ("max_concurrency", |role: &mut Role| {
            role.max_concurrency += 1;
        }),
    ] {
        let mut roles = role_table(4);
        let fixer = roles
            .iter_mut()
            .find(|role| role.activity_type == "fix_integration")
            .ok_or_else(|| anyhow::anyhow!("no fix_integration role"))?;
        mutate(fixer);
        let Err(error) = connections(roles) else {
            anyhow::bail!("a {label} disagreement on one node was accepted");
        };
        assert!(
            error.contains("remediation"),
            "the refusal must name the node; got: {error}"
        );
        assert!(
            error.contains("fix_integration"),
            "the refusal must name the joining activity; got: {error}"
        );
    }
    Ok(())
}

/// The per-run session mechanic's wiring, asserted on the REAL argument
/// structure (`connection_norn_args`) rather than on a `Debug` rendering: NO
/// connection bakes a static `--session-id` or `--workspace-root` into its
/// composed Norn command — both are per-run values the wrapper appends at start
/// (the arg template cannot express a per-item-per-lens session id). The
/// read-only roles carry the file-mutating deny-list; the writing roles carry
/// none.
#[test]
fn no_static_session_or_workspace_args_and_deny_lists_split_by_role() -> anyhow::Result<()> {
    for connection in connection_table(4)? {
        let args = connection_norn_args(&connection);
        let node = connection.node;
        for forbidden in ["--session-id", "--resume-if-exists", "--workspace-root"] {
            assert!(
                !args.iter().any(|arg| arg == forbidden),
                "connection {node} must not bake a static {forbidden} (the wrapper \
                 appends it with the derived value); args were:\n{args:#?}"
            );
        }
        let read_only = matches!(node, "planner" | "reviewer");
        assert_eq!(
            flag_value(&args, "--disallowed-tools"),
            read_only.then_some("write,edit,apply_patch"),
            "connection {node} deny-list wiring; args were:\n{args:#?}"
        );
    }
    Ok(())
}

/// The connection's profile doctrine reaches Norn as the value IMMEDIATELY
/// after `--append-system-prompt` (which APPENDS to Norn's own system prompt —
/// never `--system-prompt`, which would OVERWRITE it), byte-identical.
/// Asserted on the argument list, then confirmed to survive into the composed
/// harness.
#[test]
fn the_profile_rides_as_append_system_prompt_byte_identical() {
    let connection = Connection {
        node: "developer",
        output_schema: "{}",
        profile: "MARKER_PROFILE_TEXT".to_owned(),
        disallowed_tools: None,
        max_concurrency: 4,
        activities: vec![],
    };
    let args = connection_norn_args(&connection);
    assert_eq!(
        flag_value(&args, "--append-system-prompt"),
        Some("MARKER_PROFILE_TEXT"),
        "the profile must be the value immediately after --append-system-prompt, \
         byte-identical; args were:\n{args:#?}"
    );
    assert!(
        !args.iter().any(|arg| arg == "--system-prompt"),
        "the doctrine must APPEND, never OVERWRITE Norn's system prompt"
    );
    // And the argument list is really what the composed harness carries: every
    // argument survives into the built NornHarness. Asserted per-argument so
    // it cannot rot on a change to the adapter's Debug shape.
    let composed = format!("{:?}", inner_norn_harness("norn", &connection));
    for arg in &args {
        assert!(
            composed.contains(arg.as_str()),
            "argument {arg:?} did not reach the composed harness:\n{composed}"
        );
    }
}

/// The value immediately following `flag`, or `None` when the flag is absent.
fn flag_value<'args>(args: &'args [String], flag: &str) -> Option<&'args str> {
    args.windows(2)
        .find(|pair| pair[0] == flag)
        .map(|pair| pair[1].as_str())
}

/// The derived session suffixes, read through each role's production
/// extractor: per-item for the dev role, per-item-per-LENS for the panel seat
/// (concurrent lenses over one item must never share a session), and the SAME
/// `planner` suffix for the planner, the remediator, AND the integration fixer
/// — the resumed-coordinator mechanic.
#[test]
fn session_suffixes_derive_per_role() -> anyhow::Result<()> {
    let roles = role_table(4);
    let inputs: BTreeMap<&str, String> = BTreeMap::from([
        (
            "planner",
            "{\"material\":{},\"repo_root\":\"/r\",\"workspace_path\":\"/r\"}".to_owned(),
        ),
        (
            "dev_item",
            "{\"work\":{\"item\":{\"id\":\"it-9\",\"title\":\"t\",\"goal\":\"g\",\"phase\":1,\
             \"feedback\":\"\"},\"workspace_path\":\"/w\",\"branch\":\"b\",\
             \"base_commit\":\"c\"},\"gates\":[]}"
                .to_owned(),
        ),
        (
            "review_item",
            "{\"task\":{\"lens\":{\"name\":\"correctness\",\"charter\":\"hunt defects\"},\
             \"work\":{\"item\":{\"id\":\"it-9\",\"title\":\"t\",\"goal\":\"g\",\"phase\":1,\
             \"feedback\":\"\"},\"workspace_path\":\"/w\",\"branch\":\"b\",\
             \"base_commit\":\"c\",\"report\":{\"item_id\":\"it-9\",\"summary\":\"s\"},\
             \"gate\":{\"pass\":true,\"runs\":[],\"diff\":\"\",\"diagnostics\":\"\"}}}}"
                .to_owned(),
        ),
        (
            "remediate",
            "{\"merge\":{},\"plan\":{},\"workspace_path\":\"/w/integration\"}".to_owned(),
        ),
        (
            "fix_integration",
            "{\"verify\":{},\"plan\":{},\"integration_branch\":\"staged/wf/integration\",\
             \"workspace_path\":\"/w/integration\"}"
                .to_owned(),
        ),
    ]);
    let mut suffixes: BTreeMap<&str, String> = BTreeMap::new();
    for role in &roles {
        let input = inputs
            .get(role.activity_type)
            .ok_or_else(|| anyhow::anyhow!("no test input for {}", role.activity_type))?;
        let context = (role.extract)(input).map_err(anyhow::Error::msg)?;
        suffixes.insert(role.activity_type, context.session_suffix);
    }
    assert_eq!(suffixes["planner"], "planner");
    assert_eq!(suffixes["dev_item"], "dev-it-9");
    assert_eq!(
        suffixes["review_item"], "review-it-9-correctness",
        "a panel seat's session must name the LENS as well as the item, or \
         concurrent lenses over one item collide onto one session"
    );
    assert_eq!(
        suffixes["remediate"], "planner",
        "the remediator must resume the planner's session"
    );
    assert_eq!(
        suffixes["fix_integration"], "planner",
        "the integration fixer must resume the planner's session too — the \
         coordinator answers for its own decomposition"
    );
    Ok(())
}

/// The routing contract this worker's five connections uphold: the server
/// routes by (namespace, `task_queue`, node) only, so the connection node
/// table must be INJECTIVE (no two connections share a node) and the activity
/// mapping EXHAUSTIVE (every served activity type maps to exactly one node).
/// Reads the served sets from the production `roles`/`shell_registry`
/// definitions so the guard cannot drift from what actually registers.
#[test]
fn node_mapping_is_exhaustive_and_injective() -> anyhow::Result<()> {
    let connections = connection_table(4)?;

    let mut nodes: BTreeSet<&str> = connections.iter().map(|entry| entry.node).collect();
    assert_eq!(
        nodes.len(),
        connections.len(),
        "two agent connections share a node — the server would land activities \
         on a handlerless connection"
    );
    assert!(
        nodes.insert(SHELL_NODE),
        "an agent connection reuses the shell connection's node"
    );

    let registry = shell_registry(Shell::inherited())?;
    let mut activity_to_node: BTreeMap<String, &str> = BTreeMap::new();
    for activity_type in registry.activity_types() {
        assert!(
            activity_to_node
                .insert(activity_type.clone(), SHELL_NODE)
                .is_none(),
            "shell activity `{activity_type}` registered twice"
        );
    }
    for connection in &connections {
        for activity in &connection.activities {
            assert!(
                activity_to_node
                    .insert(activity.activity_type.to_owned(), connection.node)
                    .is_none(),
                "activity `{}` is served on two nodes",
                activity.activity_type
            );
        }
    }
    assert_eq!(
        activity_to_node
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec![
            "dev_item",
            "fix_integration",
            "fold_phase",
            "gate_item",
            "merge_branches",
            "plan_reviews",
            "planner",
            "provision_item",
            "remediate",
            "review_item",
            "verify_integration",
        ],
        "the served activity-type set changed; keep the worker and the AWL \
         document's action table synchronized"
    );
    Ok(())
}

/// Pin the exact node-id strings to the document-side source of truth (the
/// `node` lines in `../awl/staged_rounds.awl`, re-verified independently by
/// `tests/compile_direct.rs`). The server matches these strings blindly; a
/// drift on either side strands activities on handlerless connections.
#[test]
fn node_ids_mirror_the_document_table() -> anyhow::Result<()> {
    assert_eq!(SHELL_NODE, "shell");
    let connections = connection_table(4)?;
    for (activity_type, node) in [
        ("planner", "planner"),
        ("provision_item", SHELL_NODE),
        ("dev_item", "developer"),
        ("gate_item", SHELL_NODE),
        ("plan_reviews", SHELL_NODE),
        ("review_item", "reviewer"),
        ("fold_phase", SHELL_NODE),
        ("merge_branches", SHELL_NODE),
        ("remediate", "remediation"),
        ("verify_integration", SHELL_NODE),
        ("fix_integration", "remediation"),
    ] {
        if node == SHELL_NODE {
            assert!(
                shell_registry(Shell::inherited())?
                    .activity_types()
                    .contains(activity_type),
                "shell activity `{activity_type}` is not registered"
            );
            continue;
        }
        assert_eq!(
            connection_for(&connections, activity_type)?,
            node,
            "activity `{activity_type}` must be served on node `{node}`"
        );
    }
    Ok(())
}
