# Proof 8a — receipt reads the provisioned tree

Class: E. A divergent venue `gates.json` is planted and the census still reports the tree declaration.

```console
$ workflows/repo-battery/proofs/run-offline-proof.sh 8a-tree-source
da2ace47242c78b6a771b2d92c7abeb48dd64fcdb0ec9d097fdd5dd28233ddda  scripts/battery-costly-announcements.sh
2a9dbbc8b93174b5899f9dcadc4f8b0ce75e773a0c29bf02fa1cf454ed564bba  scripts/battery.sh
3b0b5794c032bb59b3d15b1de52775ea06ab711a8569ff1decf41759b0193943  scripts/gates/lib.sh
ea0eeea800a83bf727b5d9f3d5794fe0d28afb62e53e7f53d4f8f2d67b642597  scripts/gates/verify-declaration.sh
fixture_commit dfacbf4ed6c3f036019390d4a320bd7c6ee51fd8
$ bash workflows/repo-battery/bin/provision.sh $subject_repo $ref_requested $workspace_root $workflow_id
{"ok":true,"owns_tree":true,"refusal":"","tree_path":"/tmp/repo-battery-proof-8a-tree-source/work/run/repo","head_commit":"dfacbf4ed6c3f036019390d4a320bd7c6ee51fd8"}
$ bash workflows/repo-battery/bin/precheck.sh $workspace_root $workflow_id
{"ok":true,"kind":"ready","refusal":"","selector_path":"/tmp/repo-battery-proof-8a-tree-source/work/run/repo/gates.json:.legs","venue":"aion-workflows","toolchain":"not-declared","targets":[],"environment":[],"script_witnesses":[{"path":"workflow/bin/census-declaration.sh","sha256":"44145fdc10829918b4fa3b889566db979ddacb020c3684ae1b4c064df3d0404d"},{"path":"workflow/bin/lib.sh","sha256":"95fc52395f2c96ca04309eba0708a1eb15a2af3030cd617d506b4bfd7f6d8a20"},{"path":"workflow/bin/precheck.sh","sha256":"03476660b02dbf0f9e93f8813adee1b02baad142c9a3b29ece976d6b94d55744"},{"path":"workflow/bin/provision.sh","sha256":"38067539c27b1a0df5d8d256045e33d09ae047cfbd1255ef376d096b901751fb"},{"path":"workflow/bin/receipt.sh","sha256":"7f909fedc3aca394ae4d5ecd663f0bc886c4117988babf0a706a88a36459b35d"},{"path":"workflow/bin/run-battery.sh","sha256":"5a603677c51727ccce1a50f59f61b1edf36fd49bc315446810774d4541ccd853"},{"path":"workflow/bin/teardown.sh","sha256":"53fa5e5e399c096833a0e2fb3770ba317a34f5739dd95e76b6bb91b498d16032"},{"path":"workflow/bin/verify.sh","sha256":"5eeb0952efad50c9c7c17431f58a39e8bca2a9e313d60b4d27979dfffc5b8a51"},{"path":"scripts/battery-costly-announcements.sh","sha256":"da2ace47242c78b6a771b2d92c7abeb48dd64fcdb0ec9d097fdd5dd28233ddda"},{"path":"scripts/battery.sh","sha256":"2a9dbbc8b93174b5899f9dcadc4f8b0ce75e773a0c29bf02fa1cf454ed564bba"},{"path":"scripts/gates/lib.sh","sha256":"3b0b5794c032bb59b3d15b1de52775ea06ab711a8569ff1decf41759b0193943"},{"path":"scripts/gates/verify-declaration.sh","sha256":"ea0eeea800a83bf727b5d9f3d5794fe0d28afb62e53e7f53d4f8f2d67b642597"}]}
tree legs: ["fixture:reads-tree","fixture:true"]
venue legs: ["venue:wrong"]
```
