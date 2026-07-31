//! Worker advertisement comparison against durable package contracts.

mod schema;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use self::schema::{normalize_schema, schema_is_subset};
use crate::{ActivityDescriptor, WorkerContract};

/// One field-level incompatibility between a deployed contract and a worker.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractDiff {
    /// `.v4` package identity that requires the field.
    pub package_version: String,
    /// Activity whose contract differs.
    pub action: String,
    /// Stable dotted path within the activity contract.
    pub field: String,
    /// Value required by the deployed workflow, absent when the field is forbidden.
    pub expected: Option<Value>,
    /// Value advertised by the worker, absent when it omitted the field or action.
    pub advertised: Option<Value>,
}

impl fmt::Display for ContractDiff {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "package `{}` action `{}` field `{}` expected {} but worker advertised {}",
            self.package_version,
            self.action,
            self.field,
            rendered_value(self.expected.as_ref()),
            rendered_value(self.advertised.as_ref()),
        )
    }
}

/// Compares one queue contract with a worker advertisement.
///
/// A worker may advertise additional actions because one process can serve
/// several concurrently deployed package versions. Input compatibility is
/// contravariant: every value the workflow may send must be accepted by the
/// worker. Output compatibility is covariant: every value the worker may emit
/// must be decodable by the workflow. This admits input widening and optional
/// output-field additions while refusing narrowing and undeclared output drift.
///
/// `worker_node` is the registering connection's advertised locality — `None`
/// when it carries none. Only the actions whose dispatch can REACH that
/// connection are demanded of it; see [`dispatch_can_reach`].
#[must_use]
pub fn contract_diffs(
    package_version: &str,
    contract: &WorkerContract,
    worker_node: Option<&str>,
    advertised: &[ActivityDescriptor],
) -> Vec<ContractDiff> {
    let advertised = advertised
        .iter()
        .map(|activity| (activity.name.as_str(), activity))
        .collect::<BTreeMap<_, _>>();
    // An action whose body the DECLARATION carries is executed by the server
    // itself, so no registering worker serves it and none can advertise it.
    // Demanding one would make a queue that mixes a declared body with a
    // bodyless action unservable — the worker doing its whole job would be
    // refused for omitting an action that was never its job.
    let mut expected = contract
        .actions
        .iter()
        .filter(|action| action.body.is_none())
        .filter(|action| dispatch_can_reach(action.node.as_deref(), worker_node))
        .collect::<Vec<_>>();
    expected.sort_by(|left, right| left.name.cmp(&right.name));
    let mut diffs = Vec::new();
    for action in expected {
        let Some(actual) = advertised.get(action.name.as_str()) else {
            diffs.push(ContractDiff {
                package_version: package_version.to_owned(),
                action: action.name.clone(),
                field: "action".to_owned(),
                expected: Some(Value::String(missing_action_requirement(
                    action.node.as_deref(),
                ))),
                advertised: None,
            });
            continue;
        };

        let expected_input = normalize_schema(&action.input_schema);
        let advertised_input = normalize_schema(&actual.input_schema);
        if !schema_is_subset(&expected_input, &advertised_input) {
            diff_schema(
                package_version,
                &action.name,
                "input_schema",
                &expected_input,
                &advertised_input,
                &mut diffs,
            );
        }

        let expected_output = normalize_schema(&action.output_schema);
        let advertised_output = normalize_schema(&actual.output_schema);
        if !schema_is_subset(&advertised_output, &expected_output) {
            diff_schema(
                package_version,
                &action.name,
                "output_schema",
                &expected_output,
                &advertised_output,
                &mut diffs,
            );
        }
    }
    diffs
}

/// Whether a dispatch for an action pinned to `action_node` can REACH a worker
/// connection advertising `worker_node`.
///
/// This is the admission-side mirror of the server registry's dispatch filter
/// (`worker_matches_node`), and the two must never drift: an unpinned action
/// (`None`) reaches every worker in the pool, while an action pinned to a node
/// reaches ONLY a connection advertising that exact node — so a connection
/// carrying no locality is reachable by unpinned actions alone.
///
/// WHY ADMISSION NEEDS IT. The server routes by (namespace × `task_queue` × node)
/// and a worker process that serves several nodes therefore opens one
/// connection PER NODE, each advertising only that node's actions. Checking
/// every bodyless action of the queue against every connection demanded each
/// connection serve the OTHER connections' actions, so a correctly partitioned
/// worker was refused on every dial — the whole queue unservable — for omitting
/// actions no dispatch could ever have handed it. Demanding exactly the
/// reachable set keeps the gate's guarantee intact: any action a dispatch could
/// land on this connection is still required of it, so an unservable dispatch is
/// still refused at registration rather than discovered at run time.
///
/// This is deliberately NOT a claim that the pool is fully staffed. A node whose
/// connection never arrives leaves its pinned actions unserved; that is an
/// unstaffed-pool condition the census reports, not an admission failure of the
/// connections that did arrive.
#[must_use]
fn dispatch_can_reach(action_node: Option<&str>, worker_node: Option<&str>) -> bool {
    match action_node {
        None => true,
        Some(pin) => worker_node == Some(pin),
    }
}

/// The requirement text for an action the connection was reachable for and did
/// not advertise. It names WHY this connection owed the action, because the
/// operator's next move differs entirely between the two cases: an unpinned
/// action is owed by every worker in the pool, while a pinned one is owed by the
/// connection on that node alone.
fn missing_action_requirement(action_node: Option<&str>) -> String {
    match action_node {
        None => "advertised: the action is unpinned, so every worker in the pool must serve it"
            .to_owned(),
        Some(node) => format!("advertised: the action is pinned to node `{node}`"),
    }
}

fn diff_schema(
    package_version: &str,
    action: &str,
    field: &str,
    expected: &Value,
    advertised: &Value,
    diffs: &mut Vec<ContractDiff>,
) {
    if schemas_equal(expected, advertised, None) {
        return;
    }
    match (expected, advertised) {
        (Value::Object(expected), Value::Object(advertised)) => {
            let keys = expected
                .keys()
                .chain(advertised.keys())
                .collect::<BTreeSet<_>>();
            for key in keys {
                let nested = format!("{field}.{key}");
                match (expected.get(key), advertised.get(key)) {
                    (Some(left), Some(right)) => {
                        diff_schema(package_version, action, &nested, left, right, diffs);
                    }
                    (left, right) => diffs.push(ContractDiff {
                        package_version: package_version.to_owned(),
                        action: action.to_owned(),
                        field: nested,
                        expected: left.cloned(),
                        advertised: right.cloned(),
                    }),
                }
            }
        }
        _ => diffs.push(ContractDiff {
            package_version: package_version.to_owned(),
            action: action.to_owned(),
            field: field.to_owned(),
            expected: Some(expected.clone()),
            advertised: Some(advertised.clone()),
        }),
    }
}

fn schemas_equal(left: &Value, right: &Value, parent: Option<&str>) -> bool {
    match (left, right) {
        (Value::Object(left), Value::Object(right)) => {
            left.len() == right.len()
                && left.iter().all(|(key, value)| {
                    right
                        .get(key)
                        .is_some_and(|other| schemas_equal(value, other, Some(key)))
                })
        }
        (Value::Array(left), Value::Array(right))
            if matches!(parent, Some("required" | "enum" | "type")) =>
        {
            let mut left = left.iter().map(stable_json).collect::<Vec<_>>();
            let mut right = right.iter().map(stable_json).collect::<Vec<_>>();
            left.sort();
            right.sort();
            left == right
        }
        (Value::Array(left), Value::Array(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| schemas_equal(left, right, None))
        }
        _ => left == right,
    }
}

fn stable_json(value: &Value) -> String {
    match value {
        Value::Object(values) => {
            let fields = values
                .iter()
                .map(|(key, value)| format!("{key}:{}", stable_json(value)))
                .collect::<Vec<_>>();
            format!("{{{}}}", fields.join(","))
        }
        Value::Array(values) => {
            let values = values.iter().map(stable_json).collect::<Vec<_>>();
            format!("[{}]", values.join(","))
        }
        _ => value.to_string(),
    }
}

fn rendered_value(value: Option<&Value>) -> String {
    value.map_or_else(|| "<missing>".to_owned(), Value::to_string)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::contract_diffs;
    use crate::{ActionContract, ActivityDescriptor, WorkerContract};
    use serde_json::Value;

    fn contract(input: serde_json::Value, output: serde_json::Value) -> WorkerContract {
        WorkerContract {
            task_queue: "payments".to_owned(),
            actions: vec![ActionContract {
                name: "charge".to_owned(),
                input_schema: input,
                output_schema: output,
                node: None,
                timeout: None,
                retry: None,
                advisory: false,
                agent: false,
                body: None,
            }],
        }
    }

    fn advertised(input: serde_json::Value, output: serde_json::Value) -> Vec<ActivityDescriptor> {
        vec![ActivityDescriptor {
            name: "charge".to_owned(),
            input_schema: input,
            output_schema: output,
        }]
    }

    #[test]
    fn mismatch_reports_the_exact_schema_field() {
        let contract = contract(
            json!({"type":"object","properties":{"amount":{"type":"integer"}}}),
            json!({"type":"boolean"}),
        );
        let advertised = advertised(
            json!({"properties":{"amount":{"type":"string"}},"type":"object"}),
            json!({"type":"boolean"}),
        );

        let diffs = contract_diffs("abc", &contract, None, &advertised);
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].field, "input_schema.properties.amount.type");
        assert_eq!(diffs[0].expected, Some(json!("integer")));
        assert_eq!(diffs[0].advertised, Some(json!("string")));
    }

    #[test]
    fn input_widening_and_optional_output_addition_are_compatible() {
        let contract = contract(
            json!({
                "$schema":"https://json-schema.org/draft/2020-12/schema",
                "type":"object",
                "properties":{"amount":{"type":"integer"}},
                "required":["amount"]
            }),
            json!({
                "type":"object",
                "properties":{"approved":{"type":"boolean"}},
                "required":["approved"]
            }),
        );
        let advertised = advertised(
            json!({
                "title":"ChargeInput",
                "type":"object",
                "properties":{"amount":{"type":"number"}},
                "required":["amount"]
            }),
            json!({
                "title":"ChargeOutput",
                "type":"object",
                "properties":{
                    "approved":{"type":"boolean"},
                    "receipt":{"type":"string"}
                },
                "required":["approved"]
            }),
        );

        assert!(contract_diffs("abc", &contract, None, &advertised).is_empty());
    }

    #[test]
    fn input_narrowing_and_output_widening_are_refused() {
        let contract = contract(json!({"type":"number"}), json!({"type":"integer"}));
        let advertised = advertised(json!({"type":"integer"}), json!({"type":"number"}));

        let diffs = contract_diffs("abc", &contract, None, &advertised);
        assert_eq!(diffs.len(), 2);
        assert_eq!(diffs[0].field, "input_schema.type");
        assert_eq!(diffs[1].field, "output_schema.type");
    }

    #[test]
    fn local_defs_and_inline_schemas_compare_semantically() {
        let contract = contract(
            json!({
                "type":"object",
                "properties":{"card":{"$ref":"#/$defs/Card"}},
                "required":["card"],
                "$defs":{"Card":{"type":"object","properties":{"last4":{"type":"string"}},"required":["last4"]}}
            }),
            json!({"type":"boolean"}),
        );
        let advertised = advertised(
            json!({
                "type":"object",
                "properties":{"card":{"type":"object","properties":{"last4":{"type":"string"}},"required":["last4"]}},
                "required":["card"]
            }),
            json!({"type":"boolean"}),
        );

        assert!(contract_diffs("abc", &contract, None, &advertised).is_empty());
    }

    /// `$comment` is non-validating by specification — draft 2020-12 reserves it
    /// for schema authors and forbids any effect on validation. But the subset
    /// check requires an unrecognised key on the CONTRACT side to appear
    /// identically on the worker's side, so leaving `$comment` in place made a
    /// commented schema satisfiable only by a worker that reproduced the comment
    /// byte for byte. A remark to a human reader must never decide admission.
    #[test]
    fn a_comment_in_a_declared_schema_does_not_have_to_be_reproduced() {
        let contract = contract(
            json!({
                "type":"object",
                "$comment":"amount is in the smallest currency unit",
                "properties":{"amount":{"type":"integer","$comment":"cents"}},
                "required":["amount"]
            }),
            json!({"type":"boolean","$comment":"true when the charge settled"}),
        );
        let advertised = advertised(
            json!({
                "type":"object",
                "properties":{"amount":{"type":"integer"}},
                "required":["amount"]
            }),
            json!({"type":"boolean"}),
        );

        assert!(
            contract_diffs("abc", &contract, None, &advertised).is_empty(),
            "a non-validating comment must not decide contract admission"
        );
    }

    /// An action whose body the DECLARATION carries is executed by the server,
    /// not by a registering worker, so no worker can advertise it and none
    /// should be required to. Requiring it makes a queue that mixes a declared
    /// body with a bodyless action unservable: the worker serving the bodyless
    /// action is refused for omitting an action that was never its job.
    #[test]
    fn a_declared_body_is_not_required_of_a_worker() {
        let contract = WorkerContract {
            task_queue: "python_box".to_owned(),
            actions: vec![
                ActionContract {
                    name: "inspect".to_owned(),
                    input_schema: json!({"type":"object"}),
                    output_schema: json!({"type":"boolean"}),
                    node: None,
                    timeout: None,
                    retry: None,
                    advisory: false,
                    agent: false,
                    body: None,
                },
                ActionContract {
                    name: "snapshot".to_owned(),
                    input_schema: json!({"type":"object"}),
                    output_schema: json!({"type":"boolean"}),
                    node: None,
                    timeout: None,
                    retry: None,
                    advisory: false,
                    agent: false,
                    body: Some(crate::ActionBodyContract::Run {
                        command: "git rev-parse HEAD".to_owned(),
                    }),
                },
            ],
        };
        // The worker serves only the bodyless action — the whole of its job.
        let advertised = vec![ActivityDescriptor {
            name: "inspect".to_owned(),
            input_schema: json!({"type":"object"}),
            output_schema: json!({"type":"boolean"}),
        }];

        assert!(
            contract_diffs("abc", &contract, None, &advertised).is_empty(),
            "a server-executed declared body must not be demanded of a worker"
        );
    }

    /// A three-node queue, exactly the shape a real multi-role worker serves.
    fn node_partitioned_contract() -> WorkerContract {
        let action = |name: &str, node: Option<&str>| ActionContract {
            name: name.to_owned(),
            input_schema: json!({"type":"object"}),
            output_schema: json!({"type":"boolean"}),
            node: node.map(str::to_owned),
            timeout: None,
            retry: None,
            advisory: false,
            agent: false,
            body: None,
        };
        WorkerContract {
            task_queue: "staged_rounds".to_owned(),
            actions: vec![
                action("gate_item", Some("shell")),
                action("dev_item", Some("developer")),
                action("review_item", Some("reviewer")),
                action("audit", None),
            ],
        }
    }

    fn descriptor(name: &str) -> ActivityDescriptor {
        ActivityDescriptor {
            name: name.to_owned(),
            input_schema: json!({"type":"object"}),
            output_schema: json!({"type":"boolean"}),
        }
    }

    /// THE DEFECT. The server routes by (namespace × `task_queue` × node), so a
    /// worker serving several nodes opens one connection PER NODE advertising
    /// only that node's actions. Checking every bodyless action of the queue
    /// against every connection demanded each connection serve the OTHER
    /// connections' actions — so on 2026-07-30 a correctly partitioned worker
    /// was refused on every dial, `WORKER_CONTRACT_MISMATCH` naming five actions
    /// that were never that connection's job, and the queue was unservable.
    #[test]
    fn a_node_partitioned_connection_is_not_demanded_another_nodes_actions() {
        let contract = node_partitioned_contract();
        let shell = vec![descriptor("gate_item"), descriptor("audit")];

        assert!(
            contract_diffs("abc", &contract, Some("shell"), &shell).is_empty(),
            "the shell connection serves its own node's actions and the unpinned \
             one — it must not be refused for omitting the developer and reviewer \
             nodes' actions"
        );
    }

    /// The gate keeps its teeth on the actions that CAN reach the connection: an
    /// action pinned to this very node and left unadvertised is still refused,
    /// and the refusal names the node so the operator knows which connection
    /// owed it.
    #[test]
    fn an_action_pinned_to_this_node_is_still_demanded() {
        let contract = node_partitioned_contract();
        // On the shell node, serving only the unpinned action.
        let short = vec![descriptor("audit")];

        let diffs = contract_diffs("abc", &contract, Some("shell"), &short);
        assert_eq!(diffs.len(), 1, "{diffs:?}");
        assert_eq!(diffs[0].action, "gate_item");
        assert_eq!(
            diffs[0].expected,
            Some(Value::String(
                "advertised: the action is pinned to node `shell`".to_owned()
            ))
        );
    }

    /// An UNPINNED action's dispatch reaches every worker in the pool (the
    /// registry's `worker_matches_node` returns true for a `None` filter against
    /// any worker), so every connection owes it no matter what node it carries.
    /// Reachability is the whole rule — narrowing it to same-node actions would
    /// admit a connection a dispatch could land the unpinned action on.
    #[test]
    fn an_unpinned_action_is_demanded_of_every_node() {
        let contract = node_partitioned_contract();
        let developer_only = vec![descriptor("dev_item")];

        let diffs = contract_diffs("abc", &contract, Some("developer"), &developer_only);
        assert_eq!(diffs.len(), 1, "{diffs:?}");
        assert_eq!(diffs[0].action, "audit");
        assert_eq!(
            diffs[0].expected,
            Some(Value::String(
                "advertised: the action is unpinned, so every worker in the pool \
                 must serve it"
                    .to_owned()
            ))
        );
    }

    /// A connection carrying NO locality is reachable only by unpinned
    /// dispatches, so it owes the unpinned action and nothing else — the same
    /// asymmetry the registry applies when filtering candidates.
    #[test]
    fn a_node_less_connection_owes_only_the_unpinned_actions() {
        let contract = node_partitioned_contract();

        assert!(
            contract_diffs("abc", &contract, None, &[descriptor("audit")]).is_empty(),
            "a node-less connection is unreachable for pinned dispatches"
        );
        let diffs = contract_diffs("abc", &contract, None, &[]);
        assert_eq!(diffs.len(), 1, "{diffs:?}");
        assert_eq!(
            diffs[0].action, "audit",
            "the unpinned action is still owed by a node-less connection"
        );
    }

    /// A connection on a node the package never mentions owes only the unpinned
    /// actions. It is admitted rather than refused because no pinned dispatch can
    /// reach it — the pool simply has a connection nothing routes to, which the
    /// census reports and admission does not adjudicate.
    #[test]
    fn a_connection_on_an_unknown_node_owes_only_the_unpinned_actions() {
        let contract = node_partitioned_contract();

        assert!(
            contract_diffs("abc", &contract, Some("stranger"), &[descriptor("audit")]).is_empty(),
            "an unknown node is unreachable for every pinned action"
        );
    }
}
