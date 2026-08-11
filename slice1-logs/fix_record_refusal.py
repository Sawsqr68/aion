"""Waffles' ruling: a supervised record replaying a deleted flag gets a NAMED refusal.

Shape (c) — stated refusal. Not (a): there is nothing to migrate a stored argv FROM,
because the values a `harness` section needs were never in the record. Not (b): a
tolerant parse that accepted the old flags would be a compat shim, which house law
forbids outright.
"""

import pathlib

p = pathlib.Path("crates/aion-cli/src/main.rs")
s = p.read_text()

old = '''    Some(format!(
        "\\nThese are not unknown arguments — they are DELETED ones. An agent worker's launch \\
         is declared in the `.awl` document it serves, in the `harness` section of its \\
         `worker` block, and there is no flag for any of it:\\n\\n{moved}\\n\\nRun `aion awl \\
         check <FILE.awl>` after writing the section: it names every setting the section is \\
         missing, scoped to the kind the section declares."
    ))'''

new = '''    Some(format!(
        "\\nThese are not unknown arguments — they are DELETED ones. An agent worker's launch \\
         is declared in the `.awl` document it serves, in the `harness` section of its \\
         `worker` block, and there is no flag for any of it:\\n\\n{moved}\\n\\nIf you typed this \\
         command: write the section into the document, then re-run without those flags. \\
         `aion awl check <FILE.awl>` names every setting the section is still missing, scoped \\
         to the kind the section declares.\\n\\nIf you did NOT type this command, a managed-worker \\
         deployment replayed it. A `builtin` worker-deployment record stores the argv it was \\
         created with, verbatim and durably, and the supervisor replays exactly that on every \\
         restart — so a record minted before the launch settings moved will fail this way on \\
         every restart until its budget is spent, and editing the document will not change it. \\
         The record is the thing to fix: add the `harness` section to the document, then \\
         re-mint the deployment (`PUT /worker-deployments/<name>`, or the ops console's deploy \\
         form) so its stored argv no longer carries these flags."
    ))'''

if old not in s:
    raise SystemExit("advice context not found")
s = s.replace(old, new, 1)

# The record-shaped test: the argv a pre-change `builtin` record replays.
old_test = '''    /// MIGRATION RIDER (b): a bare invocation refuses LOUDLY with the new shape named.'''
new_test = '''    /// A managed-worker record minted BEFORE the launch settings moved replays its stored
    /// argv on every supervised restart, and nobody is at a terminal to read a generic
    /// "unexpected argument". This is the exact argv such a record carries — the tail a
    /// `WorkerArtifactRef::Builtin` stores — and what it must earn is the NAMED refusal,
    /// not clap's bare exit-2.
    ///
    /// The supervisor forwards a managed worker's stderr into the server's tracing output
    /// tagged with the worker name, so this sentence is what an operator finds against the
    /// record that could not start.
    #[test]
    fn the_argv_a_pre_change_managed_worker_record_replays_earns_a_named_refusal() {
        // Verbatim from market-mirror's OPERATIONS.md, the surface the CLAUDECODE
        // incident lived on — a record minted from that runbook carries exactly this.
        let stored_verb = [
            "worker",
            "agent",
            "workflows/mm_fleet_loop.awl",
            "--harness",
            "norn",
            "--task-queue",
            "mm_loop_build",
            "--identity",
            "mm-loop-builder",
            "--liminal-address",
            "127.0.0.1:50061",
            "--concurrency",
            "4",
            "--reconnect-initial-backoff",
            "1",
            "--reconnect-max-backoff",
            "30",
            "--reconnect-max-attempts",
            "100",
            "--norn-arg=--account",
            "--norn-arg=default",
        ];

        // It must still be REFUSED — a tolerant parse would be a compat shim.
        let mut argv = vec!["aion"];
        argv.extend(stored_verb);
        assert!(
            Cli::try_parse_from(&argv).is_err(),
            "a stored record does not buy a deleted flag its way back in"
        );

        // ...and the refusal must NAME what happened, on the record-driven path as much
        // as on a hand-typed one.
        let owned: Vec<String> = argv.into_iter().map(str::to_owned).collect();
        let Some(advice) = super::migrated_flag_advice(&owned) else {
            unreachable!("an argv carrying deleted flags must earn the advice");
        };
        for expected in [
            "--harness",
            "--concurrency",
            "--reconnect-initial-backoff",
            // The `=`-joined form is what the runbook actually used; matching only the
            // separated spelling would miss every record minted from it.
            "--norn-arg",
            "harness",
            "managed-worker",
            "re-mint",
            "worker-deployments",
        ] {
            assert!(
                advice.contains(expected),
                "the refusal must name `{expected}`, got:\\n{advice}"
            );
        }
        assert!(
            !advice.contains("--task-queue") && !advice.contains("--identity"),
            "a flag that SURVIVED must not be named as deleted, or the operator strips a \\
             setting the verb still needs:\\n{advice}"
        );
    }

    /// An invocation carrying no deleted flag earns no advice at all — otherwise every
    /// ordinary typo would be answered with a lecture about the harness section.
    #[test]
    fn an_argv_with_no_deleted_flag_earns_no_migration_advice() {
        let argv: Vec<String> = ["aion", "worker", "agent", "flows/review.awl", "--nonsense"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        assert!(
            super::migrated_flag_advice(&argv).is_none(),
            "the advice is for DELETED flags, not for every unknown argument"
        );
    }

    /// MIGRATION RIDER (b): a bare invocation refuses LOUDLY with the new shape named.'''

if old_test not in s:
    raise SystemExit("rider (b) anchor not found")
s = s.replace(old_test, new_test, 1)
p.write_text(s)
print("advice names the record path:", "re-mint the deployment" in s)
print("record test added:", "the_argv_a_pre_change_managed_worker_record_replays" in s)
