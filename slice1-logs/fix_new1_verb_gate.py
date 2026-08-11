"""NEW-1 (blocker): the migration advice is scoped to `worker agent`, not to the binary."""

import pathlib

p = pathlib.Path("crates/aion-cli/src/main.rs")
s = p.read_text()

old = '''/// The extra sentence a deleted launch flag earns, when `argv` carries one.
///
/// Read off the RAW argv rather than off clap's error, because clap reports the first
/// unexpected argument and an operator pasting an old runbook line has typically
/// supplied several — naming one of six would send them round the loop six times.
///
/// `=`-joined forms (`--norn-arg=--account`) are matched on their flag half, because
/// that is the shape market-mirror's own runbook used.
fn migrated_flag_advice(argv: &[String]) -> Option<String> {
    let supplied: Vec<&(&str, &str)> = MIGRATED_LAUNCH_FLAGS'''

new = '''/// The subcommand path whose launch flags were deleted.
///
/// The advice is scoped to THIS verb and no other, which is not a nicety: four of the
/// deleted spellings — `--concurrency` and the three `--reconnect-*` — are LIVE, REQUIRED
/// flags on `aion worker awl` (see [`crate::worker_awl::args`], and the start command
/// `docs/workers/AWL-WORKER-DOCUMENTS.md` publishes). An operator who mistypes that
/// documented line must not be told those flags are deleted and sent to write a `harness`
/// section — a worker-family document is FORBIDDEN to carry one, because every action in
/// it has a declared body and there is nothing for an agent to serve. That advice would
/// send them to a refusal, from a refusal, for a flag that was never wrong.
const MIGRATED_FLAG_VERB: [&str; 2] = ["worker", "agent"];

/// Whether `argv` invokes the verb whose launch flags were deleted.
///
/// Matched as the adjacent token pair, which is what a subcommand path looks like on a
/// raw argv: `aion [global flags] worker agent …`. Deliberately not read off clap's
/// rendered usage line — the advice stays a pure function of the argv, so it can be
/// tested without a parser in the loop and cannot change meaning when clap rewords
/// itself.
fn invokes_migrated_flag_verb(argv: &[String]) -> bool {
    argv.windows(MIGRATED_FLAG_VERB.len())
        .any(|pair| pair == MIGRATED_FLAG_VERB)
}

/// The extra sentence a deleted launch flag earns, when `argv` invokes `worker agent`
/// AND carries one.
///
/// Read off the RAW argv rather than off clap's error, because clap reports the first
/// unexpected argument and an operator pasting an old runbook line has typically
/// supplied several — naming one of six would send them round the loop six times.
///
/// `=`-joined forms (`--norn-arg=--account`) are matched on their flag half, because
/// that is the shape market-mirror's own runbook used.
fn migrated_flag_advice(argv: &[String]) -> Option<String> {
    if !invokes_migrated_flag_verb(argv) {
        return None;
    }
    let supplied: Vec<&(&str, &str)> = MIGRATED_LAUNCH_FLAGS'''

if old not in s:
    raise SystemExit("advice context not found")
s = s.replace(old, new, 1)

# The discriminating tests.
anchor = """    /// An invocation carrying no deleted flag earns no migration advice at all — otherwise
    /// every ordinary typo would be answered with a lecture about the harness section."""
if anchor not in s:
    anchor = """    /// An invocation carrying no deleted flag earns no advice at all — otherwise every
    /// ordinary typo would be answered with a lecture about the harness section."""
if anchor not in s:
    raise SystemExit("no-advice test anchor not found")

discriminating = '''    /// THE VERB GATE. `--concurrency` and the three `--reconnect-*` are DELETED on
    /// `worker agent` and LIVE, REQUIRED flags on `worker awl` — same spellings, two
    /// verbs, opposite truths.
    ///
    /// This argv is the start command `docs/workers/AWL-WORKER-DOCUMENTS.md` publishes,
    /// with one typo'd flag so the parse fails and the advice path runs. It must earn
    /// NOTHING: telling this operator those flags are deleted would send them to write a
    /// `harness` section into a worker-family document, which the checker forbids
    /// outright — a refusal reached from a refusal, over a flag that was never wrong.
    #[test]
    fn a_worker_awl_invocation_earns_no_migration_advice() {
        let published: Vec<String> = [
            "aion",
            "worker",
            "awl",
            "greeter.awl",
            "--concurrency",
            "4",
            "--reconnect-initial-backoff",
            "0.1",
            "--reconnect-max-backoff",
            "5",
            "--reconnect-max-attempts",
            "1000",
            // The typo that makes clap fail and puts the advice path in play.
            "--identty",
            "greeter-worker",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();

        assert!(
            Cli::try_parse_from(&published).is_err(),
            "the control needs a FAILING parse, or the advice path never runs and this \\
             test proves nothing"
        );
        assert_eq!(
            super::migrated_flag_advice(&published),
            None,
            "`--concurrency` and the reconnect trio are live REQUIRED flags on `worker \\
             awl`; advice claiming they are deleted would send an operator to write a \\
             `harness` section into a worker-family document, which cannot carry one"
        );
    }

    /// The presence twin of the gate: the SAME four flags, on the verb where they really
    /// were deleted, still earn the advice. Without this arm the gate above could pass on
    /// an implementation that never gives advice at all.
    #[test]
    fn the_same_flags_on_worker_agent_still_earn_the_advice() {
        let agent: Vec<String> = [
            "aion",
            "worker",
            "agent",
            "flows/review.awl",
            "--concurrency",
            "4",
            "--reconnect-initial-backoff",
            "1",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        let Some(advice) = super::migrated_flag_advice(&agent) else {
            unreachable!("the deleted flags on their own verb must earn the advice");
        };
        assert!(advice.contains("--concurrency") && advice.contains("harness"));
    }

    /// A deleted flag on a THIRD verb earns nothing either — the gate is an allow-list of
    /// one path, not a deny-list of one other.
    #[test]
    fn a_deleted_flag_on_an_unrelated_verb_earns_no_advice() {
        let unrelated: Vec<String> = ["aion", "worker", "shell", "--concurrency", "4"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        assert_eq!(super::migrated_flag_advice(&unrelated), None);
    }

'''

s = s.replace(anchor, discriminating + anchor, 1)
p.write_text(s)
print("verb gate added:", "invokes_migrated_flag_verb" in s)
print("discriminating tests added:", "a_worker_awl_invocation_earns_no_migration_advice" in s)
