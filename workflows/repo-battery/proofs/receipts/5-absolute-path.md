# Proof 5 — literal absolute path refused

Class: E. Admission/census refusal executed here.

```console
$ workflows/repo-battery/proofs/run-offline-proof.sh 5-absolute-path
da2ace47242c78b6a771b2d92c7abeb48dd64fcdb0ec9d097fdd5dd28233ddda  scripts/battery-costly-announcements.sh
2a9dbbc8b93174b5899f9dcadc4f8b0ce75e773a0c29bf02fa1cf454ed564bba  scripts/battery.sh
3b0b5794c032bb59b3d15b1de52775ea06ab711a8569ff1decf41759b0193943  scripts/gates/lib.sh
ea0eeea800a83bf727b5d9f3d5794fe0d28afb62e53e7f53d4f8f2d67b642597  scripts/gates/verify-declaration.sh
fixture_commit 862dfd9ecd507a02b90eaf236805eb13fde437e0
$ bash workflows/repo-battery/bin/provision.sh $subject_repo $ref_requested $workspace_root $workflow_id
{"ok":true,"owns_tree":true,"refusal":"","tree_path":"/tmp/repo-battery-proof-5-absolute-path/work/run/repo","head_commit":"862dfd9ecd507a02b90eaf236805eb13fde437e0"}
$ bash workflows/repo-battery/bin/precheck.sh $workspace_root $workflow_id
{"ok":false,"kind":"absolute_path","refusal":"declared leg fixture:absolute contains a literal absolute path in its command","selector_path":"/tmp/repo-battery-proof-5-absolute-path/work/run/repo/gates.json:.legs"}
```
