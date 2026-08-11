"""Replace the flag-era `worker agent` parse tests with the migration riders."""

import pathlib

p = pathlib.Path("crates/aion-cli/src/main.rs")
s = p.read_text()

start = s.index(
    "    #[test]\n    fn worker_agent_parses_the_document_and_repeatable_liminal_addresses()"
)
end = s.index("    #[test]\n    fn normalize_endpoint_preserves_explicit_scheme()")

replacement = '''    #[test]
    fn worker_agent_parses_the_document_and_repeatable_liminal_addresses() -> anyhow::Result<()> {
        let cli = Cli::try_parse_from([
            "aion",
            "worker",
            "agent",
            "flows/review.awl",
            "--liminal-address",
            "127.0.0.1:50061",
            "--liminal-address",
            "127.0.0.1:50062",
            "--identity",
            "reviewer-worker",
            "--task-queue",
            "reviewers",
        ])?;
        let Some(Command::Worker {
            command: super::WorkerCommand::Agent(args),
        }) = cli.command
        else {
            anyhow::bail!("expected worker agent command");
        };
        assert_eq!(args.awl.as_deref(), Some(Path::new("flows/review.awl")));
        // Repeating the flag builds an ordered candidate list, which is what lets a
        // worker migrate to a survivor after its server dies.
        assert_eq!(
            args.liminal_addresses,
            vec![
                String::from("127.0.0.1:50061"),
                String::from("127.0.0.1:50062")
            ]
        );
        assert_eq!(args.identity.as_deref(), Some("reviewer-worker"));
        assert_eq!(args.task_queue.as_deref(), Some("reviewers"));
        assert!(!args.check);

        // Every connection setting is optional at PARSE time — the environment may
        // supply it — and `--check` needs none of them.
        let cli = Cli::try_parse_from(["aion", "worker", "agent", "flows/review.awl", "--check"])?;
        let Some(Command::Worker {
            command: super::WorkerCommand::Agent(args),
        }) = cli.command
        else {
            anyhow::bail!("expected worker agent command");
        };
        assert!(args.check);
        assert!(args.liminal_addresses.is_empty());
        assert!(args.identity.is_none());
        Ok(())
    }

    /// MIGRATION RIDER (a): the VERB SURVIVES and takes a document reference.
    ///
    /// Deleting the launch flags must not delete the door a human starts a worker
    /// through. The assertion is on the parse, not on the help text: a verb that
    /// documents itself and no longer parses is the same outage.
    #[test]
    fn the_agent_verb_survives_the_flag_deletion_and_takes_a_document() -> anyhow::Result<()> {
        let cli = Cli::try_parse_from(["aion", "worker", "agent", "flows/review.awl"])?;
        let Some(Command::Worker {
            command: super::WorkerCommand::Agent(args),
        }) = cli.command
        else {
            anyhow::bail!("`aion worker agent <FILE.awl>` must still be the door in");
        };
        assert_eq!(args.awl.as_deref(), Some(Path::new("flows/review.awl")));
        Ok(())
    }

    /// MIGRATION RIDER: the per-setting launch flags are DELETED, not deprecated and
    /// not kept alongside.
    ///
    /// Asserted as a PARSE REFUSAL of each one by name, because that is the only
    /// difference an operator can observe between a deleted flag and a silently ignored
    /// one — and a silently ignored `--acp-permission` reads exactly like a permission
    /// policy being enforced.
    #[test]
    fn every_migrated_launch_flag_is_refused_by_the_parser() {
        for (flag, value) in [
            ("--harness", Some("acp")),
            ("--concurrency", Some("4")),
            ("--reconnect-initial-backoff", Some("5")),
            ("--reconnect-max-backoff", Some("60")),
            ("--reconnect-max-attempts", Some("100")),
            ("--acp-command", Some("/usr/local/bin/claude-code-acp")),
            ("--acp-arg", Some("--acp")),
            ("--acp-cwd", Some("/srv/workspace")),
            ("--acp-permission", Some("deny")),
            ("--acp-env", Some("KEY=value")),
            ("--acp-exit-grace", Some("2")),
            ("--norn-binary", Some("/usr/local/bin/norn")),
            ("--norn-arg", Some("--account")),
            ("--norn-unset-env", Some("OPENAI_API_KEY")),
        ] {
            let mut argv = vec!["aion", "worker", "agent", "flows/review.awl", flag];
            argv.extend(value);
            assert!(
                Cli::try_parse_from(&argv).is_err(),
                "`{flag}` moved into the document's `harness` section and must no longer \\
                 parse; accepting it would leave an operator believing a setting is in force \\
                 when the document decides it"
            );
        }
    }

    /// MIGRATION RIDER (b): a bare invocation refuses LOUDLY with the new shape named.
    ///
    /// The document is OPTIONAL to clap on purpose, so the refusal is this verb's own
    /// sentence rather than "the following required arguments were not provided" — which
    /// tells an operator arriving with the old flags nothing about where those settings
    /// went.
    #[test]
    fn a_bare_invocation_refuses_loudly_naming_the_new_shape() -> anyhow::Result<()> {
        let cli = Cli::try_parse_from(["aion", "worker", "agent"])?;
        let Some(Command::Worker {
            command: super::WorkerCommand::Agent(args),
        }) = cli.command
        else {
            anyhow::bail!("a bare invocation must PARSE, so the verb can refuse in its own words");
        };
        let Err(error) = args.document() else {
            anyhow::bail!("an invocation naming no document must be refused");
        };
        let message = error.to_string();
        for expected in [
            "aion worker agent <FILE.awl>",
            "harness",
            "aion awl check",
            "no longer a flag",
        ] {
            assert!(
                message.contains(expected),
                "the refusal must name `{expected}`, got:\\n{message}"
            );
        }
        Ok(())
    }

    #[test]
    fn worker_agent_help_documents_the_model_it_serves() -> anyhow::Result<()> {
        let mut command = Cli::command();
        let Some(worker) = command.find_subcommand_mut("worker") else {
            anyhow::bail!("worker subcommand should be registered");
        };
        let Some(agent) = worker.find_subcommand_mut("agent") else {
            anyhow::bail!("worker agent subcommand should be registered");
        };
        let help = agent.render_long_help().to_string();

        assert!(help.contains("--liminal-address"));
        assert!(help.contains("--task-queue"));
        assert!(help.contains("--check"));
        // The model, not just the flags: where the queue and the schemas come from,
        // which actions are the server's, and that a queue is served whole.
        assert!(help.contains("worker` block's name"));
        assert!(help.contains("all-or-nothing"));
        assert!(help.contains("liminal worker transport"));
        assert!(help.contains("nothing is invented"));
        // Where the LAUNCH is declared, and every setting it carries: an operator must
        // be able to read off the help alone that these are document fields and not
        // flags, and which ones the kind they are declaring needs.
        for expected in [
            "`harness` section",
            "concurrency",
            "env_pass",
            "permission",
            "exit_grace",
            "no default",
        ] {
            assert!(help.contains(expected), "help must document `{expected}`");
        }
        // The two rulings an operator cannot afford to learn by surprise: the security
        // decision has no default, and the agent's environment is not inherited.
        assert!(help.contains("Nothing is inherited"));
        Ok(())
    }

'''

s = s[:start] + replacement + s[end:]
p.write_text(s)
print("ok")
