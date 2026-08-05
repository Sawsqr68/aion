# Evidence: the one-command setup could not work on a fresh machine

**Date:** 2026-08-05. **Seat:** Vesper Lynd. **Fix:** `37635b73`.
(The commit body lost two backticked phrases to shell interpretation; this
file carries the verbatim record.)

## The claim under test

`scripts/setup.sh` prepares `$AION_HOME` but writes no `config.toml`.
Config discovery is read-only (`crates/aion-server/src/config/home.rs:26,35-36`),
so a home without one boots on built-in defaults — where the deploy gRPC
service is mounted only when `deploy.enabled` (`run.rs:314`) and the liminal
listener that remote workers connect in to exists only when the outbox is
commissioned (`run.rs:93-101`). Both are dark by default
(`config/sections.rs:437`, `:481`); with no config at all the loader reports
`ConfigSource::BuiltInDefaults` (`config/load_home_tests.rs:78-79`).

Therefore the script's deploy step and worker step should both have nothing
to talk to on a machine that has never run Aion.

## Method

An isolated `AION_HOME` (created and removed by the harness; not `~/.aion`),
distinct HTTP/gRPC/liminal ports so the box's live server is untouched, and
the same binary in both arms (`aion 0.11.0`). The treatment config is
EXTRACTED FROM `setup.sh` ITSELF rather than retyped, and the harness refuses
to run unless both sections actually landed in the file — a treatment that
silently degrades into a second control proves nothing. (An earlier run of
this harness did exactly that; the guard exists because of it.)

The probe document is a body-free corpus fixture. The bodied assistant
document carries `{workspace_root}`, which the installed 0.11.0 binary
predates — deploying that here would be the version-skew hazard
`CUT-PLAN-139-LIVE.md` forbids.

Two discriminators, both read from the server's own output: is a liminal
worker listener bound, and does the deploy service answer or is it
unimplemented.

## Result — both discriminators flipped

```
=== aion binary: aion 0.11.0 ===

########## CONTROL: empty home, no config ##########
--- CONTROL ---
liminal listener: ABSENT (a worker has nothing to connect to)
deploy says: error: failed to deploy package: grpc Unimplemented:

########## TREATMENT: the config setup.sh writes ##########
--- TREATMENT ---
liminal listener: BOUND
deploy says: {"workflow_type":"analyze_snapshot","content_hash":"a497d96a…",
"deployed_entry_module":"analyze_snapshot$a497d96a…","entry_function":"run",
"freshly_loaded":true,"route_changed":true,"superseded_versions":[]}

=== cleanup ===
test server gone
```

## Why it was never caught

Every prior proof ran against a home that already carried a config (the
operator's own box; the "isolated home" of `19016210`). The script was
live-proven — but never under the one condition that matters, which is the
machine of the person being set up. An environment that carries the thing
under test cannot measure whether it is required.

## What the fix writes

Only the two `enabled` switches. Every other value in both sections already
has a default that applies once its surface is on, so the script invents
none of them. An existing config is left untouched.

**Open for ruling (permissive change, flagged not hidden):** enabling the
deploy surface is a deliberately-dark security posture being turned on by a
setup script. The alternative shape is to refuse and tell the operator to
enable it by hand. Recorded here so the choice is visible rather than
implied by a working script.
