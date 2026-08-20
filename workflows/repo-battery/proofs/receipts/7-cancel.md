# Proof 7 — mid-battery cancellation

Class: E+O. Shell process-group containment executed here; engine cancellation routing receipt is OWED at the landing desk. Typed refusal: `venue_lacks_engine`.

Landing command: `workflows/repo-battery/proofs/run-live-proof.sh 7 ENGINE_ENDPOINT`.

```console
$ workflows/repo-battery/proofs/run-offline-proof.sh 7-cancel
da2ace47242c78b6a771b2d92c7abeb48dd64fcdb0ec9d097fdd5dd28233ddda  scripts/battery-costly-announcements.sh
2a9dbbc8b93174b5899f9dcadc4f8b0ce75e773a0c29bf02fa1cf454ed564bba  scripts/battery.sh
3b0b5794c032bb59b3d15b1de52775ea06ab711a8569ff1decf41759b0193943  scripts/gates/lib.sh
ea0eeea800a83bf727b5d9f3d5794fe0d28afb62e53e7f53d4f8f2d67b642597  scripts/gates/verify-declaration.sh
fixture_commit 4a4aa1640774d2f88d157ab93517b4f8a0387cc3
$ bash workflows/repo-battery/bin/provision.sh $subject_repo $ref_requested $workspace_root $workflow_id
{"ok":true,"owns_tree":true,"refusal":"","tree_path":"/tmp/repo-battery-proof-7-cancel/work/run/repo","head_commit":"49bd3a986907ceb6107b4ced86adaceeba07de7c"}
$ bash workflows/repo-battery/bin/precheck.sh $workspace_root $workflow_id
{"ok":true,"kind":"ready","refusal":"","selector_path":"/tmp/repo-battery-proof-7-cancel/work/run/repo/gates.json:.legs","venue":"aion-workflows","toolchain":"not-declared","targets":[],"environment":[],"script_witnesses":[{"path":"workflow/bin/census-declaration.sh","sha256":"04deca9152a8035894acdf8d4ceb5ab132714fef96645aa4df9e7f4d9e67a138"},{"path":"workflow/bin/lib.sh","sha256":"b8c54776d510c188748842f51d596403c6b0287777a3198566a57c0362dd99e4"},{"path":"workflow/bin/precheck.sh","sha256":"ac52fcdb46bc459df7fd986d1ec9fc775daa338c4e08ea132574edd14d3ff93f"},{"path":"workflow/bin/provision.sh","sha256":"2f79faff672b4974e724599c773531a04bc9cf92430a94faacd44ad8bb589673"},{"path":"workflow/bin/receipt.sh","sha256":"7f909fedc3aca394ae4d5ecd663f0bc886c4117988babf0a706a88a36459b35d"},{"path":"workflow/bin/run-battery.sh","sha256":"53e5448215aed2ed41d8c04c90b821819f8c9b4de6809a671c32badc5dd93f50"},{"path":"workflow/bin/teardown.sh","sha256":"53fa5e5e399c096833a0e2fb3770ba317a34f5739dd95e76b6bb91b498d16032"},{"path":"workflow/bin/verify.sh","sha256":"80488a5474e183b8c076d93883f3897bd419a29568de6c3fbae4e2783cf5f6e9"},{"path":"scripts/battery-costly-announcements.sh","sha256":"da2ace47242c78b6a771b2d92c7abeb48dd64fcdb0ec9d097fdd5dd28233ddda"},{"path":"scripts/battery.sh","sha256":"2a9dbbc8b93174b5899f9dcadc4f8b0ce75e773a0c29bf02fa1cf454ed564bba"},{"path":"scripts/gates/lib.sh","sha256":"3b0b5794c032bb59b3d15b1de52775ea06ab711a8569ff1decf41759b0193943"},{"path":"scripts/gates/verify-declaration.sh","sha256":"ea0eeea800a83bf727b5d9f3d5794fe0d28afb62e53e7f53d4f8f2d67b642597"}]}
$ bash workflows/repo-battery/bin/run-battery.sh $workspace_root $workflow_id & pid=$!; kill -TERM "$pid"; wait "$pid"
{"measured":false,"cancelled":true,"rc":130,"contended":false,"log_path":"/tmp/repo-battery-proof-7-cancel/work/run/battery.log","refusal":"CANCELLED MEASUREMENT: the contained battery process group was terminated before a result existed"}
post-cancel result: {"measured":false,"cancelled":true,"rc":130,"contended":false,"log_path":"/tmp/repo-battery-proof-7-cancel/work/run/battery.log","refusal":"CANCELLED MEASUREMENT: the contained battery process group was terminated before a result existed"}
no completed EXECUTED_LEGS manifest after cancellation
$ bash workflows/repo-battery/bin/teardown.sh $workspace_root $workflow_id
{"ok":true,"removed":"/tmp/repo-battery-proof-7-cancel/work/run","refusal":""}
```

The shell arm asserts that no completed `EXECUTED_LEGS` manifest exists and records the explicit guarded teardown result. The engine cancellation classification remains OWED.
