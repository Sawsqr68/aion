# Proof 8b — guard and selector perturbations

Class: E. Both negative perturbations execute against scratch copies.

```console
$ workflows/repo-battery/proofs/run-offline-proof.sh 8b-perturbations
da2ace47242c78b6a771b2d92c7abeb48dd64fcdb0ec9d097fdd5dd28233ddda  scripts/battery-costly-announcements.sh
2a9dbbc8b93174b5899f9dcadc4f8b0ce75e773a0c29bf02fa1cf454ed564bba  scripts/battery.sh
3b0b5794c032bb59b3d15b1de52775ea06ab711a8569ff1decf41759b0193943  scripts/gates/lib.sh
ea0eeea800a83bf727b5d9f3d5794fe0d28afb62e53e7f53d4f8f2d67b642597  scripts/gates/verify-declaration.sh
fixture_commit 6e60b6155e6430632bbaeddf444fe34d9b4ce7e0
$ bash workflows/repo-battery/bin/teardown.sh /tmp/repo-battery-proof-8b-perturbations/work ../escape
{"ok":false,"removed":"","refusal":"teardown refused path outside workspace root: /tmp/repo-battery-proof-8b-perturbations/escape"}
source_sha256 04deca9152a8035894acdf8d4ceb5ab132714fef96645aa4df9e7f4d9e67a138
$ bash census-perturbed.sh /tmp/repo-battery-proof-8b-perturbations/source
{"ok":false,"kind":"invalid_declaration","refusal":"gates.json:.missing is not an array","selector_path":"/tmp/repo-battery-proof-8b-perturbations/source/gates.json:.missing","gates_json_sha256":"","battery_sha256":"","legs_declared":[],"environment_specs":[]}
```

The guard refuses an outside path. The perturbed single selector site returns typed `invalid_declaration` naming `gates.json:.missing`; it does not return an empty census.
