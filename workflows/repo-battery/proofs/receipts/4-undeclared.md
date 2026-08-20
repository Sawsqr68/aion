# Proof 4 — missing declaration

Class: E+O. Census executed here; engine success-route receipt is OWED at landing. Typed refusal: `venue_lacks_engine`.

Landing command: `workflows/repo-battery/proofs/run-live-proof.sh 4 ENGINE_ENDPOINT`.

```console
$ workflows/repo-battery/proofs/run-offline-proof.sh 4-undeclared
da2ace47242c78b6a771b2d92c7abeb48dd64fcdb0ec9d097fdd5dd28233ddda  scripts/battery-costly-announcements.sh
2a9dbbc8b93174b5899f9dcadc4f8b0ce75e773a0c29bf02fa1cf454ed564bba  scripts/battery.sh
3b0b5794c032bb59b3d15b1de52775ea06ab711a8569ff1decf41759b0193943  scripts/gates/lib.sh
ea0eeea800a83bf727b5d9f3d5794fe0d28afb62e53e7f53d4f8f2d67b642597  scripts/gates/verify-declaration.sh
fixture_commit 50db5c7e72111a10cb661e01e16a902b0715609e
$ bash workflows/repo-battery/bin/provision.sh $subject_repo $ref_requested $workspace_root $workflow_id
{"ok":true,"owns_tree":true,"refusal":"","tree_path":"/tmp/repo-battery-proof-4-undeclared/work/run/repo","head_commit":"50db5c7e72111a10cb661e01e16a902b0715609e"}
$ bash workflows/repo-battery/bin/precheck.sh $workspace_root $workflow_id
{"ok":false,"kind":"undeclared","refusal":"the provisioned tree has no gates.json; no contract, no green","selector_path":"/tmp/repo-battery-proof-4-undeclared/work/run/repo/gates.json:.legs"}
```

Structural assertion only: `undeclared` lowers to success.
```console
350:      Ok(UndeclaredOutcome(Undeclared(subject_repo: subject_repo, ref_requested: subject_ref, head_commit: p.head_commit, tree_path: p.tree_path, verdict: "undeclared", refusal: c.refusal, teardown_ok: True)))
```
