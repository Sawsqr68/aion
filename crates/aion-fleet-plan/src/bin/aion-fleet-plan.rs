//! `aion-fleet-plan` — the thin binary the sealed `/bin/sh` seam calls.
//!
//! Every subcommand prints ONE JSON object on stdout and nothing else, so
//! the calling script can hand it straight to the worker's `run json` face
//! without reshaping it. Human-readable detail travels INSIDE that object,
//! never as loose stdout prose, because design §5.3 requires exit 4 to
//! carry "a stable cause code and human-readable detail" together.
//!
//! Exit codes are `check_plan`'s contract (§5.3) and only `check-plan` uses
//! them: `0` valid, `1` absent/incomplete, `4` `PLAN GRAPH INVALID`. Exit 3
//! (committed briefs disagree with dispatch-fixed leg ids) belongs to the
//! pre-existing shell checker and is deliberately NOT reachable from here —
//! widening it is exactly what §5.3 forbids.

use std::path::PathBuf;
use std::process::ExitCode;

use aion_fleet_plan::chain::{ChainLedger, next_action};
use aion_fleet_plan::graph::NormalizedGraph;
use aion_fleet_plan::validate::{CheckOutcome, PlanInputs, check_plan};
use aion_fleet_plan::verify::{ExactCommit, Verdict, VerifyRequest, verify_conclusion};
use clap::{Parser, Subcommand};

/// The staged-chain plan contract, as a command.
#[derive(Debug, Parser)]
#[command(name = "aion-fleet-plan", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Validate the committed plan's `fleet-stages-v1` block (§5.2) and
    /// emit the normalized typed graph.
    CheckPlan {
        /// Path to the committed `.fleet/plan.md` text, already extracted
        /// from HEAD by the caller. This binary resolves no ref.
        #[arg(long)]
        plan_file: PathBuf,
        /// The dispatch-fixed leg ids, comma separated.
        #[arg(long)]
        legs: String,
        /// Verbatim `git status --porcelain -- .fleet` output. Non-empty
        /// means loose `.fleet` state (§5.2 clause 17).
        #[arg(long, default_value = "")]
        loose: String,
        /// The chain's dispatch-authored `max_stages` ceiling.
        #[arg(long)]
        max_stages: Option<usize>,
    },
    /// Read and verify one stage's conclusion token from an EXACT commit
    /// (§2.6, §6 clause 4).
    VerifyConclusion {
        /// The repository to read the committed tree object from.
        #[arg(long)]
        repo: PathBuf,
        /// The stage id.
        #[arg(long)]
        stage: String,
        /// The child flight's workflow id, as recorded at dispatch.
        #[arg(long)]
        child: String,
        /// The child's EXACT recorded pushed commit — 40 lowercase hex.
        /// A ref name is refused before git is invoked.
        #[arg(long)]
        commit: String,
        /// The stage's declared closed vocabulary, comma separated.
        #[arg(long)]
        vocabulary: String,
        /// The kept run root, carried on a refusal.
        #[arg(long, default_value = "")]
        kept_root: String,
        /// The last accepted commit, carried on a refusal.
        #[arg(long, default_value = "")]
        last_accepted: String,
    },
    /// Decide the next chain action from the normalized graph and the
    /// recorded ledger (§6). Pure: no ref, no clock, no tree.
    ChainNext {
        /// Path to the serialized normalized graph.
        #[arg(long)]
        graph_file: PathBuf,
        /// Path to the serialized recorded ledger.
        #[arg(long)]
        ledger_file: PathBuf,
        /// The chain's root base commit, resolved once at chain start.
        #[arg(long)]
        root_base: String,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli.command) {
        Ok(code) => code,
        Err(message) => {
            emit(&serde_json::json!({ "ok": false, "error": message }));
            ExitCode::from(2)
        }
    }
}

fn run(command: Command) -> Result<ExitCode, String> {
    match command {
        Command::CheckPlan {
            plan_file,
            legs,
            loose,
            max_stages,
        } => {
            let plan_text = std::fs::read_to_string(&plan_file)
                .map_err(|e| format!("cannot read {}: {e}", plan_file.display()))?;
            let inputs = PlanInputs {
                plan_text,
                dispatch_leg_ids: split_csv(&legs),
                fleet_state_loose: !loose.trim().is_empty(),
                loose_detail: loose,
                max_stages,
            };
            match check_plan(&inputs) {
                Ok(CheckOutcome::NoChainBlock) => {
                    emit(&serde_json::json!({
                        "ok": true,
                        "chain_block": false,
                        "detail": "no fleet-stages-v1 block; the legacy entry point owns this plan"
                    }));
                    Ok(ExitCode::SUCCESS)
                }
                Ok(CheckOutcome::Valid(graph)) => {
                    emit(&serde_json::json!({
                        "ok": true,
                        "chain_block": true,
                        "graph": &*graph,
                    }));
                    Ok(ExitCode::SUCCESS)
                }
                Err(refusal) => {
                    let class = refusal.exit_class();
                    emit(&serde_json::json!({
                        "ok": false,
                        "chain_block": true,
                        "exit_code": class.code(),
                        "cause_code": refusal.cause_code.as_str(),
                        "stage_id": refusal.stage_id,
                        "detail": refusal.detail,
                    }));
                    Ok(ExitCode::from(u8::try_from(class.code()).unwrap_or(4)))
                }
            }
        }
        Command::VerifyConclusion {
            repo,
            stage,
            child,
            commit,
            vocabulary,
            kept_root,
            last_accepted,
        } => {
            let exact = ExactCommit::parse(&commit).map_err(|e| e.to_string())?;
            let vocab = split_csv(&vocabulary);
            let request = VerifyRequest {
                stage_id: &stage,
                child_workflow_id: &child,
                result_commit: &exact,
                vocabulary: &vocab,
                kept_root: &kept_root,
                last_accepted_commit: &last_accepted,
            };
            let verdict = verify_conclusion(&repo, &request).map_err(|e| e.to_string())?;
            let verified = matches!(verdict, Verdict::Verified(_));
            emit(&serde_json::json!({
                "ok": true,
                "verified": verified,
                "verdict": verdict,
            }));
            Ok(ExitCode::SUCCESS)
        }
        Command::ChainNext {
            graph_file,
            ledger_file,
            root_base,
        } => {
            let graph: NormalizedGraph = read_json(&graph_file)?;
            // An absent ledger is an EMPTY ledger, not an error: boundary 1
            // (parse complete) is exactly the state in which no fact has
            // been recorded yet, and the next action must still be
            // derivable there.
            let ledger: ChainLedger = if ledger_file.exists() {
                read_json(&ledger_file)?
            } else {
                ChainLedger::new()
            };
            let action = next_action(&graph, &ledger, &root_base).map_err(|e| e.to_string())?;
            emit(&serde_json::json!({ "ok": true, "next": action }));
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn read_json<T: serde::de::DeserializeOwned>(path: &PathBuf) -> Result<T, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("cannot decode {}: {e}", path.display()))
}

fn split_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

fn emit(value: &serde_json::Value) {
    println!("{value}");
}
