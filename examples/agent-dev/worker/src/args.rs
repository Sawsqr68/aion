//! Command-line contract for the agent-dev worker.

use agent_dev_worker::norn_invocation::DEGRADED_SESSION_OPT_IN;

/// Parsed command-line arguments.
#[derive(Debug)]
pub(crate) struct Args {
    /// One or more candidate liminal listen addresses, in dial-preference order.
    pub(crate) candidates: Vec<String>,
    /// The worker identity announced in-band.
    pub(crate) identity: String,
    /// Optional readiness file written once after the first registration.
    pub(crate) ready_file: Option<String>,
    /// Path to the `norn` binary (default: `NORN_BIN` env, else `norn` on PATH).
    pub(crate) norn_bin: String,
    /// The norn OAuth account alias passed as `--account`.
    ///
    /// Required, with no default. The harness runs norn with
    /// `--resume-if-exists`, and norn refuses to resume, fork, or open an
    /// existing OAuth session without an account — so this is an input the
    /// worker cannot supply for the operator. Guessing `default` here would
    /// turn a clear startup error into three retried activity failures and a
    /// failed workflow, which is exactly how this was found.
    pub(crate) norn_account: String,
    /// Optional model passed to norn as `--model`.
    ///
    /// No default of this worker's: omitted means norn chooses its own model,
    /// exactly as if the operator had run norn by hand without the flag. Which
    /// model answers the assistant's sessions is an operator decision, not a
    /// value this binary should bake in.
    pub(crate) norn_model: Option<String>,
    /// Whether to pass norn `--allow-degraded-session`.
    ///
    /// OPT-IN, with no default. A durable session can park far longer than the
    /// agent provider's epoch — one real run parked 28 days, and the engine
    /// resumed it perfectly — and when the epoch has moved on norn refuses to
    /// reopen the session without approval, so every round of that session
    /// fails. Allowing it resumes from a fresh provider epoch because the exact
    /// provider transcript cannot be replayed: a real fidelity tradeoff, and
    /// therefore the operator's call rather than this worker's.
    pub(crate) allow_degraded_session: bool,
}

/// Parse `--address` (repeatable), `--identity`, `--ready-file`, `--norn-bin` from the process
/// arguments, defaulting `--norn-bin` from the `NORN_BIN` environment variable.
pub(crate) fn parse_args() -> anyhow::Result<Args> {
    let default_norn_bin = match std::env::var("NORN_BIN") {
        Ok(value) => value,
        Err(_) => "norn".to_owned(),
    };
    parse_args_from(std::env::args().skip(1), default_norn_bin)
}

/// The argument-parsing core, fed an explicit argument iterator and `--norn-bin` default so
/// tests exercise the exact production logic without touching process globals.
///
/// Every value-taking flag bails with a clear message when its value is missing — a silent
/// fallback to a default would mask an operator typo.
fn parse_args_from(
    args: impl IntoIterator<Item = String>,
    default_norn_bin: String,
) -> anyhow::Result<Args> {
    let mut candidates: Vec<String> = Vec::new();
    let mut identity = "agent-dev-worker".to_owned();
    let mut ready_file: Option<String> = None;
    let mut norn_bin = default_norn_bin;
    let mut norn_account: Option<String> = None;
    let mut norn_model: Option<String> = None;
    let mut allow_degraded_session = false;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--address" => match args.next() {
                Some(value) => candidates.push(value),
                None => anyhow::bail!("--address requires a value"),
            },
            "--identity" => match args.next() {
                Some(value) => identity = value,
                None => anyhow::bail!("--identity requires a value"),
            },
            "--ready-file" => match args.next() {
                Some(value) => ready_file = Some(value),
                None => anyhow::bail!("--ready-file requires a value"),
            },
            "--norn-bin" => match args.next() {
                Some(value) => norn_bin = value,
                None => anyhow::bail!("--norn-bin requires a value"),
            },
            "--norn-account" => match args.next() {
                Some(value) => norn_account = Some(value),
                None => anyhow::bail!("--norn-account requires a value"),
            },
            "--norn-model" => match args.next() {
                Some(value) => norn_model = Some(value),
                None => anyhow::bail!("--norn-model requires a value"),
            },
            // A bare switch: it takes no value, because there is nothing to
            // configure — either the operator accepts the fidelity tradeoff
            // for this worker's sessions or they do not. Matched by the same
            // constant the degraded-session advice names, so the flag the
            // failure teaches is always one this parser accepts.
            DEGRADED_SESSION_OPT_IN => allow_degraded_session = true,
            other => anyhow::bail!("unknown argument `{other}`"),
        }
    }
    if candidates.is_empty() {
        candidates.push("127.0.0.1:50061".to_owned());
    }
    let Some(norn_account) = norn_account else {
        anyhow::bail!(
            "--norn-account <alias|default> is required: the agent harness runs norn with \
             --resume-if-exists, and norn refuses to resume or open an existing OAuth session \
             without an account. Omitting it does not fail here — it fails inside every agent \
             activity, three retries deep, as `norn: auth error: authentication failed`"
        );
    };
    Ok(Args {
        candidates,
        identity,
        ready_file,
        norn_bin,
        norn_account,
        norn_model,
        allow_degraded_session,
    })
}

#[cfg(test)]
mod tests {
    //! Unit tests of the argument-parsing core: defaults, explicit values, and the strict
    //! missing-value bail for every value-taking flag.

    use super::parse_args_from;

    /// Runs the parsing core on a literal argv with a fixed `--norn-bin` default.
    fn parse(args: &[&str]) -> anyhow::Result<super::Args> {
        parse_args_from(
            args.iter().map(|arg| (*arg).to_owned()),
            "norn-default".to_owned(),
        )
    }

    /// The account has NO default on purpose. Before this was required the
    /// worker started cleanly, logged "connected and registered; serving
    /// dispatches", and then failed every agent activity three times with
    /// `norn: auth error` — a healthy-looking worker that could not do its
    /// job. Failing at parse time is the whole fix.
    #[test]
    fn no_arguments_bails_because_the_norn_account_has_no_default() {
        let outcome = parse(&[]).err().map(|error| error.to_string());
        assert!(
            outcome.is_some(),
            "a missing --norn-account must bail, not default"
        );
        let error = outcome.unwrap_or_default();
        assert!(
            error.contains("--norn-account <alias|default> is required"),
            "the refusal must name the flag and its shape, got: {error}"
        );
        assert!(
            error.contains("norn: auth error"),
            "the refusal must name the failure it prevents, so an operator \
             who has already seen that error connects the two, got: {error}"
        );
    }

    #[test]
    fn the_other_flags_still_default_once_the_account_is_supplied() -> anyhow::Result<()> {
        let args = parse(&["--norn-account", "default"])?;
        assert_eq!(args.candidates, vec!["127.0.0.1:50061".to_owned()]);
        assert_eq!(args.identity, "agent-dev-worker");
        assert_eq!(args.ready_file, None);
        assert_eq!(args.norn_bin, "norn-default");
        assert_eq!(args.norn_account, "default");
        assert_eq!(
            args.norn_model, None,
            "the model must have no worker-side default — omitted means norn's own choice"
        );
        Ok(())
    }

    #[test]
    fn the_norn_model_passes_through_when_supplied() -> anyhow::Result<()> {
        let args = parse(&["--norn-account", "default", "--norn-model", "gpt-5.6-sol"])?;
        assert_eq!(args.norn_model.as_deref(), Some("gpt-5.6-sol"));
        Ok(())
    }

    /// The negative control for the degraded-resume opt-in. Defaulting it on
    /// would silently accept a provider-fidelity tradeoff on the operator's
    /// behalf, which is exactly the kind of assumption this codebase forbids.
    #[test]
    fn degraded_session_resume_is_off_unless_asked_for() -> anyhow::Result<()> {
        let args = parse(&["--norn-account", "default"])?;
        assert!(
            !args.allow_degraded_session,
            "the degraded-resume opt-in must never default on"
        );
        Ok(())
    }

    #[test]
    fn the_degraded_session_switch_takes_no_value() -> anyhow::Result<()> {
        // A bare switch: the argument after it must still parse as its own
        // flag, not be swallowed as this one's value.
        let args = parse(&[
            "--norn-allow-degraded-session",
            "--norn-account",
            "default",
            "--identity",
            "worker-a",
        ])?;
        assert!(args.allow_degraded_session);
        assert_eq!(args.norn_account, "default");
        assert_eq!(args.identity, "worker-a");
        Ok(())
    }

    #[test]
    fn all_flags_with_values_parse_and_addresses_repeat() -> anyhow::Result<()> {
        let args = parse(&[
            "--address",
            "127.0.0.1:1111",
            "--address",
            "127.0.0.1:2222",
            "--identity",
            "worker-a",
            "--ready-file",
            "/tmp/ready",
            "--norn-bin",
            "/opt/norn",
            "--norn-account",
            "work-alias",
        ])?;
        assert_eq!(
            args.candidates,
            vec!["127.0.0.1:1111".to_owned(), "127.0.0.1:2222".to_owned()]
        );
        assert_eq!(args.identity, "worker-a");
        assert_eq!(args.ready_file.as_deref(), Some("/tmp/ready"));
        assert_eq!(args.norn_bin, "/opt/norn");
        assert_eq!(args.norn_account, "work-alias");
        Ok(())
    }

    /// NOTE: this list is an enumeration, so it only covers the flags someone
    /// remembered to add. A new value-taking flag joins the parser without
    /// joining this list and is silently untested — which is exactly how
    /// `--norn-account` came to be missing from the harness invocation in the
    /// first place. Add the flag here whenever you add one to `parse_args_from`.
    #[test]
    fn every_value_taking_flag_bails_when_its_value_is_missing() {
        for flag in [
            "--address",
            "--identity",
            "--ready-file",
            "--norn-bin",
            "--norn-account",
            "--norn-model",
        ] {
            assert_eq!(
                parse(&[flag]).err().map(|error| error.to_string()),
                Some(format!("{flag} requires a value")),
                "a trailing value-less {flag} must bail"
            );
        }
    }

    #[test]
    fn unknown_argument_bails() {
        assert_eq!(
            parse(&["--bogus"]).err().map(|error| error.to_string()),
            Some("unknown argument `--bogus`".to_owned())
        );
    }
}
