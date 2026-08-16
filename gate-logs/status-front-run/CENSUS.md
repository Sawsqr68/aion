# R1 live status census

Task: `b41d541c-f0c5-4f31-b53c-9c9874e1ede1`

## Observation and request boundary

- Production server: `http://localhost:8080` (read-only).
- Namespace: `default`.
- List observation: `2026-08-16T04:56:08Z`.
- History sweep: `2026-08-16T04:58:30Z` through `2026-08-16T04:58:32Z`.
- Exact list request: `POST /workflows/list` with `{"namespace":"default"}` (one request).
- Exact history request: `POST /workflows/history` with `namespace=default`, each listed `workflow_id`, `from_seq` cursor initially `0`, `limit=2000`, and `payload_limit_bytes=0` (143 requests).
- No mutating endpoint was called. The server was not restarted, killed, deployed, signalled, cancelled, or otherwise changed.
- `ProtoListWorkflowsRequest` has only `namespace` and optional `filter`; it has no page cursor or limit. The one response contained the complete 143-row `summaries` array.
- Every history response ended at its head in one page (`next_from_seq: null`); no workflow needed a second page.
- Required-first input check: `gate-logs/status-front/census-raw.json` was read first at `2026-08-16T04:54:30Z`, before server access, but the path did not exist (ENOENT). A repository-wide search found no copy. This run therefore uses the live list response as its row set.

## Classification rules

- **consistent** — displayed status equals the last lifecycle-event projection from history.
- **F1** — displayed `Running`, but the final history projection is terminal.
- **F2** — displayed status is terminal, but history projects a nonterminal or different terminal status.
- Lifecycle projection follows `aion_core::status_from_events`: start/reopen/resume → `Running`; pause → `Paused`; completed/failed/cancelled/timed-out/continued-as-new → the matching terminal status; unrelated events do not change status.
- Failure class **opaque-counter** matches bare `workflow N returned error`; **stacktrace** is multiline or contains stack trace/backtrace markers; **real-message** is a specific human-readable message matching neither. Rows without `failure_reason` are explicitly not applicable (`null` in JSON, `—` below).

## Totals

- Rows: **143**.
- History events examined: **3037**.
- Classification: **143 consistent**, **0 F1**, **0 F2**.
- Displayed statuses: **80 Completed**, **56 Failed**, **5 Cancelled**, **2 Running**.
- Failure reason classes: **53 opaque-counter**, **3 stacktrace**, **0 real-message**, **87 not applicable**.
- Displayed/history failure-reason mismatches: **0**.
- Required IDs are present: `e4ad76e8-3bd5-4e54-8943-5e537875bb22` and `70f3b331-54b4-4f24-8fdc-a54e51019f49`.

## Complete row census

| workflow_id | type | displayed status | history-derived status | classification | failure_reason class | events | lifecycle events (seq:type) |
|---|---|---:|---:|---:|---:|---:|---|
| `51e45459-7948-4ed5-b638-ef6238d041e8` | `agent_dev` | Failed | Failed | consistent | opaque-counter | 6 | 1:WorkflowStarted, 6:WorkflowFailed |
| `5a4803c0-8a11-4860-adba-355b72c6c3f1` | `agent_dev` | Failed | Failed | consistent | opaque-counter | 6 | 1:WorkflowStarted, 6:WorkflowFailed |
| `21d6bb69-841c-410a-b051-c9a3fabeb4c1` | `agent_dev` | Failed | Failed | consistent | opaque-counter | 9 | 1:WorkflowStarted, 9:WorkflowFailed |
| `141852b2-20b9-4e94-8361-7a1ea3d5f910` | `agent_dev` | Failed | Failed | consistent | opaque-counter | 9 | 1:WorkflowStarted, 9:WorkflowFailed |
| `a572f4b4-60d0-4ac5-954c-7f8acc0d520b` | `agent_dev` | Failed | Failed | consistent | opaque-counter | 9 | 1:WorkflowStarted, 9:WorkflowFailed |
| `7915bb19-725e-43b3-9e5a-12f19b74ac15` | `agent_dev` | Failed | Failed | consistent | opaque-counter | 9 | 1:WorkflowStarted, 9:WorkflowFailed |
| `faaa1b04-02cc-4065-b201-cc7a8b1b29bb` | `agent_dev` | Failed | Failed | consistent | opaque-counter | 18 | 1:WorkflowStarted, 18:WorkflowFailed |
| `71b450bc-c529-44bf-b6dd-1c359a6d40bc` | `agent_dev` | Failed | Failed | consistent | opaque-counter | 20 | 1:WorkflowStarted, 20:WorkflowFailed |
| `f9b062c2-d260-43f9-8374-d3210330f567` | `assistant` | Failed | Failed | consistent | opaque-counter | 21 | 1:WorkflowStarted, 21:WorkflowFailed |
| `0aef6172-aa9c-4f96-8563-6f23175b9b20` | `agent_dev` | Failed | Failed | consistent | opaque-counter | 6 | 1:WorkflowStarted, 6:WorkflowFailed |
| `253d7836-f167-4d51-80b9-c750fdf362d3` | `agent_dev` | Failed | Failed | consistent | opaque-counter | 6 | 1:WorkflowStarted, 6:WorkflowFailed |
| `c8b25530-62fb-4af9-80b5-01363f39bdfd` | `agent_dev` | Failed | Failed | consistent | opaque-counter | 6 | 1:WorkflowStarted, 6:WorkflowFailed |
| `72eb0c5c-64fa-4299-aa16-12d0e3cd4722` | `agent_dev` | Failed | Failed | consistent | opaque-counter | 6 | 1:WorkflowStarted, 6:WorkflowFailed |
| `178fb728-eec3-49dc-aa58-bc2e7238bc07` | `agent_dev` | Failed | Failed | consistent | opaque-counter | 6 | 1:WorkflowStarted, 6:WorkflowFailed |
| `b35d83ff-8ebd-4d49-ac7f-0d84d9cada10` | `assistant` | Completed | Completed | consistent | — | 12 | 1:WorkflowStarted, 12:WorkflowCompleted |
| `702b648c-e2e8-4f2b-8ea5-32991ef92883` | `agent_dev` | Completed | Completed | consistent | — | 23 | 1:WorkflowStarted, 23:WorkflowCompleted |
| `0cd47656-0317-425d-bc35-194d59e9cd4c` | `assistant` | Failed | Failed | consistent | opaque-counter | 20 | 1:WorkflowStarted, 20:WorkflowFailed |
| `87756be9-fbb5-42e1-bece-8c55dc8fdba3` | `shell_run` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `9da28cf9-5eaa-45b8-9b3d-eccc194dceed` | `staged_rounds` | Failed | Failed | consistent | opaque-counter | 38 | 1:WorkflowStarted, 37:WorkflowFailed |
| `5b86b02e-4f55-4f0e-badb-fdae64b7f623` | `assistant_spike` | Completed | Completed | consistent | — | 11 | 1:WorkflowStarted, 10:WorkflowCompleted |
| `1b3afbde-823e-4374-8910-7e3fa3a83fae` | `assistant` | Completed | Completed | consistent | — | 14 | 1:WorkflowStarted, 14:WorkflowCompleted |
| `88a5733f-6000-42f6-be99-b665d89dd4e7` | `assistant` | Cancelled | Cancelled | consistent | — | 9 | 1:WorkflowStarted, 9:WorkflowCancelled |
| `b37de221-2f99-4b58-855e-a909913764e3` | `assistant` | Cancelled | Cancelled | consistent | — | 9 | 1:WorkflowStarted, 9:WorkflowCancelled |
| `b6bd67bc-0fe4-42c2-b7ee-ba421ef5c6db` | `assistant_spike` | Completed | Completed | consistent | — | 11 | 1:WorkflowStarted, 10:WorkflowCompleted |
| `9b966811-f279-4389-a095-ddda8a7f9513` | `assistant` | Completed | Completed | consistent | — | 11 | 1:WorkflowStarted, 11:WorkflowCompleted |
| `231361fd-907a-4ef7-8f82-ef969089097d` | `git_status_sweep` | Failed | Failed | consistent | opaque-counter | 6 | 1:WorkflowStarted, 6:WorkflowFailed |
| `57595652-24c4-4bb0-af25-d2cb5bac1919` | `git_status_sweep` | Failed | Failed | consistent | opaque-counter | 6 | 1:WorkflowStarted, 6:WorkflowFailed |
| `86428c78-90d3-4f80-a80b-a3aa57ab34dd` | `git_status_sweep` | Completed | Completed | consistent | — | 12 | 1:WorkflowStarted, 12:WorkflowCompleted |
| `710b7fd1-b14d-4f33-8cf6-52b45eb2a14b` | `git_status_sweep` | Completed | Completed | consistent | — | 12 | 1:WorkflowStarted, 12:WorkflowCompleted |
| `e2f2d07c-964d-4335-8488-eaf8b5383c41` | `norn_fleet` | Completed | Completed | consistent | — | 9 | 1:WorkflowStarted, 9:WorkflowCompleted |
| `880a4a1a-2854-4a4e-9f89-c7532ac86e46` | `norn_fleet` | Completed | Completed | consistent | — | 9 | 1:WorkflowStarted, 9:WorkflowCompleted |
| `54931caf-d31b-406e-82fc-f1cff66a64c3` | `norn_fleet` | Failed | Failed | consistent | opaque-counter | 8 | 1:WorkflowStarted, 7:WorkflowFailed |
| `4a00291d-2b60-4eb6-ae49-c0333da4ef3a` | `norn_fleet` | Completed | Completed | consistent | — | 14 | 1:WorkflowStarted, 13:WorkflowCompleted |
| `09365786-1fef-42b6-b8a4-1aee9364894b` | `norn_fleet` | Completed | Completed | consistent | — | 14 | 1:WorkflowStarted, 13:WorkflowCompleted |
| `f6690d4a-fefa-417c-ae71-7ab7601a6457` | `shell_run` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `a1fcf3d6-2739-4b09-895f-a5ba483a7dcc` | `shell_run` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `1b0dfc21-d99e-405e-9194-c63df8a90ec1` | `shell_run` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `0815e5a6-f075-456f-941d-fabefdf57172` | `shell_run` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `2a1cc80a-3b76-4584-ac96-918defc42dbd` | `shell_run` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `7d94eaa0-8f29-4d42-a0e9-2b4d8c398a5a` | `shell_run` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `1cffc952-22f5-4cf2-96b2-53c14487cde5` | `branch_sweep` | Completed | Completed | consistent | — | 12 | 1:WorkflowStarted, 12:WorkflowCompleted |
| `4e62463e-f381-4d0d-88eb-f4c5cac0121e` | `repo_advisor` | Failed | Failed | consistent | opaque-counter | 6 | 1:WorkflowStarted, 6:WorkflowFailed |
| `fb1fb616-7ebf-4e5e-bbfe-e39f4532b634` | `repo_advisor` | Failed | Failed | consistent | opaque-counter | 6 | 1:WorkflowStarted, 6:WorkflowFailed |
| `f2922250-52ed-4c20-8528-a2630f4c3a76` | `repo_advisor` | Failed | Failed | consistent | opaque-counter | 6 | 1:WorkflowStarted, 6:WorkflowFailed |
| `1c2ff54b-301e-448d-bae0-a59f81869c4b` | `shell_run` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `7837046a-ef09-4d1b-b796-0359f12445ee` | `shell_run` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `c082511c-07dc-434a-b801-822ed92082cd` | `shell_run` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `746117d9-8303-4b67-8275-4af6b72fde31` | `debug_claude` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `529f8d27-249a-4507-a713-a3d42674e0f0` | `debug_claude2` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `36051471-0f41-4923-885d-b45119d8b757` | `repo_advisor` | Failed | Failed | consistent | opaque-counter | 6 | 1:WorkflowStarted, 6:WorkflowFailed |
| `a8159400-6096-470e-a710-e735dcdcb1f4` | `debug_claude` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `c416c4b9-e139-4ad4-be6d-2e60c04d3043` | `debug_claude2` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `1bf5c05b-47f8-4ea5-aa2d-9378c1c5f65b` | `repo_advisor` | Failed | Failed | consistent | opaque-counter | 6 | 1:WorkflowStarted, 6:WorkflowFailed |
| `4024e031-344f-47e0-95a1-04d04a565d3d` | `debug_claude` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `6c9f2da6-a1fa-401e-8825-67f18d1019ca` | `debug_claude` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `f19dca9c-1d5b-4d23-90be-72a2002a7b9c` | `repo_advisor` | Completed | Completed | consistent | — | 9 | 1:WorkflowStarted, 9:WorkflowCompleted |
| `ffb3f32a-9ae3-4e73-b5d1-31dfd6a83e79` | `branch_sweep` | Completed | Completed | consistent | — | 12 | 1:WorkflowStarted, 12:WorkflowCompleted |
| `edc64834-4704-40c8-a74d-600af2756725` | `due_diligencer` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `0576a76c-c64a-4260-b322-74c5a80e7eb5` | `shell_run` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `bc811fd0-36a6-463a-b2c5-22292c4d7e02` | `shell_run` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `6ba962e4-c2e6-4cb0-b4dc-984f098f7d29` | `due_diligencer` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `25265510-ee4b-4d23-aa32-d7a54c0e33b5` | `shell_run` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `9e31f607-b074-4cf5-a36a-c6e3d5fe1ce3` | `gates` | Failed | Failed | consistent | opaque-counter | 14 | 1:WorkflowStarted, 13:WorkflowFailed |
| `00919767-73c4-44fc-9561-d5873d177d25` | `gates` | Failed | Failed | consistent | opaque-counter | 14 | 1:WorkflowStarted, 13:WorkflowFailed |
| `2d833db3-e9dc-4126-b5f9-cb307994efb6` | `gates` | Failed | Failed | consistent | opaque-counter | 14 | 1:WorkflowStarted, 13:WorkflowFailed |
| `11b014b6-95f8-4d5a-8f42-852cf44c18dc` | `gates` | Failed | Failed | consistent | opaque-counter | 26 | 1:WorkflowStarted, 25:WorkflowFailed |
| `d0baf923-3d1a-4208-bae1-8f128203b8ad` | `gates` | Failed | Failed | consistent | opaque-counter | 26 | 1:WorkflowStarted, 25:WorkflowFailed |
| `cb4316df-194d-49b7-863b-8366a740f0bd` | `gates` | Completed | Completed | consistent | — | 26 | 1:WorkflowStarted, 25:WorkflowCompleted |
| `4b51642c-1958-4713-bd2a-a695a89ea6c3` | `mm_gates_probe` | Cancelled | Cancelled | consistent | — | 5 | 1:WorkflowStarted, 5:WorkflowCancelled |
| `3f538342-ae81-45d3-8fd6-8ccb1c3dcd6d` | `mm_gates_probe` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `30f0437d-c6f0-4baf-866c-fa65a4c514ab` | `mm_gates_queue_probe` | Cancelled | Cancelled | consistent | — | 5 | 1:WorkflowStarted, 5:WorkflowCancelled |
| `22e4714c-c30f-4b22-ac32-180bbd09479d` | `shell_run` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `1268ede0-63d1-41c3-b25a-da2a7dc0a679` | `gates` | Failed | Failed | consistent | opaque-counter | 29 | 1:WorkflowStarted, 28:WorkflowFailed |
| `6f96b2a5-6cd4-421b-b289-d335960ea5e7` | `gates` | Failed | Failed | consistent | opaque-counter | 29 | 1:WorkflowStarted, 28:WorkflowFailed |
| `760943dc-526a-4a84-b18e-1cab42f1d499` | `gates` | Completed | Completed | consistent | — | 29 | 1:WorkflowStarted, 28:WorkflowCompleted |
| `e277dc3d-7769-4e8f-8c74-f86c82d3cbd2` | `update_check` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `a54a0989-18c3-4836-930e-101c0d878ea6` | `mm_fleet_loop` | Completed | Completed | consistent | — | 31 | 1:WorkflowStarted, 31:WorkflowCompleted |
| `ba49f638-5a87-4aec-a865-a64f48517ec9` | `update_check` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `ff54f77d-d968-40b8-90df-7dd5af9523a0` | `mm_fleet_swarm` | Completed | Completed | consistent | — | 72 | 1:WorkflowStarted, 72:WorkflowCompleted |
| `86917426-0c93-47d9-8cf7-eca6cbfc3d47` | `aion_internal_awl_child_mm_fleet_swarm_fan_0` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `4265e0ce-28cb-4035-b6aa-f2c30d5096ce` | `aion_internal_awl_child_mm_fleet_swarm_fan_0` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `18d295ef-b14b-420d-a52d-c9540b3f350a` | `aion_internal_awl_child_mm_fleet_swarm_fan_0` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `6a32c1f5-b4a1-4b43-89e4-43bd6dc578af` | `aion_internal_awl_child_mm_fleet_swarm_fan_0` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `60ae7eeb-b614-4090-af39-6229e38e78d4` | `aion_internal_awl_child_mm_fleet_swarm_fan_0` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `62da87c6-e16d-4eb3-b4d2-0dbf4fa6886f` | `aion_internal_awl_child_mm_fleet_swarm_fan_0` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `3a28536a-859e-4483-9f2b-5d2784d91705` | `aion_internal_awl_child_mm_fleet_swarm_fan_0` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `fa56ff23-5407-447c-8252-9b2a8482795b` | `aion_internal_awl_child_mm_fleet_swarm_fan_0` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `fd5b6d7f-2df5-4dd4-818e-cc3439620780` | `aion_internal_awl_child_mm_fleet_swarm_fan_0` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `1f81ae95-edfb-44f0-9eff-a59eb5ca7be6` | `assistant` | Running | Running | consistent | — | 8 | 1:WorkflowStarted |
| `a168cbb4-903e-40a5-bdb7-67b29c4c1a89` | `ladder_hello_0140` | Completed | Completed | consistent | — | 3 | 1:WorkflowStarted, 3:WorkflowCompleted |
| `b08d439b-894a-40a4-9b25-6a96ed79b018` | `repo_advisor` | Completed | Completed | consistent | — | 9 | 1:WorkflowStarted, 9:WorkflowCompleted |
| `c00c018c-2191-47ba-8aca-c4ae1ba8e483` | `mm_capture_run` | Failed | Failed | consistent | opaque-counter | 6 | 1:WorkflowStarted, 6:WorkflowFailed |
| `e338cc8e-3141-4a5d-909e-e3a29e110cad` | `update_check` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `553e0813-b611-4264-b44c-9a7f5c5d539a` | `update_check` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `4819490a-f01c-498d-bcf0-dde1c1d22d19` | `update_check` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `dbdcb3a4-5c96-4080-8614-50e20ec7c30f` | `update_check` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `5731fd2b-2460-4089-8492-d6cf87e93cc5` | `update_check` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `e4ad76e8-3bd5-4e54-8943-5e537875bb22` | `mm_capture_run` | Failed | Failed | consistent | stacktrace | 604 | 1:WorkflowStarted, 604:WorkflowFailed |
| `fc6d071d-7728-4214-8b81-8034efa977f6` | `update_watch` | Completed | Completed | consistent | — | 292 | 1:WorkflowStarted, 292:WorkflowCompleted |
| `a5bb51b5-389e-42ed-96fc-a5b2c1f69564` | `assistant` | Completed | Completed | consistent | — | 10 | 1:WorkflowStarted, 10:WorkflowCompleted |
| `e0b4753e-1b1e-49d8-93bf-86bb9161bb61` | `shell_run` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `430b6cd1-a592-4432-b5aa-dd3ae86b9306` | `update_check` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `0a3713eb-4448-4844-911a-164f800bbbf8` | `update_check` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `271d5441-d4b8-4f0e-b6b5-17a79a806cc4` | `update_check` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `16ec3250-76b8-4acb-af83-e8ac187df60b` | `update_check` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `655839e9-6134-4571-9e4c-98fecb8616c7` | `update_check` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `2b3af639-3f1e-4608-bc66-cda3e19a4034` | `mm_capture_run` | Failed | Failed | consistent | stacktrace | 5 | 1:WorkflowStarted, 5:WorkflowFailed |
| `70f3b331-54b4-4f24-8fdc-a54e51019f49` | `mm_capture_run` | Failed | Failed | consistent | stacktrace | 605 | 1:WorkflowStarted, 605:WorkflowFailed |
| `e53755d9-2f24-4173-bae7-49969a72e43c` | `update_check` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `426e73f8-6c62-453d-a71c-42ff1a311d45` | `legc_guard_probe` | Completed | Completed | consistent | — | 9 | 1:WorkflowStarted, 9:WorkflowCompleted |
| `5f6958a1-decf-4b70-9a0a-6ff0ad707c8b` | `legc_guard_probe` | Failed | Failed | consistent | opaque-counter | 9 | 1:WorkflowStarted, 9:WorkflowFailed |
| `8dc5294d-c9f5-4959-a6e6-20e7b70331a0` | `legc_guard_probe` | Failed | Failed | consistent | opaque-counter | 9 | 1:WorkflowStarted, 9:WorkflowFailed |
| `402fdf3f-378e-49ef-8bde-e268b2223a66` | `legc_guard_probe` | Failed | Failed | consistent | opaque-counter | 9 | 1:WorkflowStarted, 9:WorkflowFailed |
| `de71f3e1-6bbc-4888-924d-e59f59c7a49c` | `legc_guard_probe` | Completed | Completed | consistent | — | 9 | 1:WorkflowStarted, 9:WorkflowCompleted |
| `870ca646-4deb-418f-8d95-43f6be85cb6f` | `legc_guard_probe` | Failed | Failed | consistent | opaque-counter | 9 | 1:WorkflowStarted, 9:WorkflowFailed |
| `2b5114e4-4cd6-46e8-b757-7c0630f051ec` | `legc_guard_probe` | Failed | Failed | consistent | opaque-counter | 9 | 1:WorkflowStarted, 9:WorkflowFailed |
| `5cbd90db-aaab-42fa-904b-87d729e9a7a4` | `legc_guard_probe` | Failed | Failed | consistent | opaque-counter | 9 | 1:WorkflowStarted, 9:WorkflowFailed |
| `b0a2c091-9428-4ecd-8c3e-ffec1016dd1c` | `legc_guard_probe` | Failed | Failed | consistent | opaque-counter | 9 | 1:WorkflowStarted, 9:WorkflowFailed |
| `e5507f9a-8265-4fb5-b0e4-104b7723410e` | `legc_guard_probe` | Failed | Failed | consistent | opaque-counter | 9 | 1:WorkflowStarted, 9:WorkflowFailed |
| `59038d72-58d4-4e65-be43-c9150c95dc68` | `repo_gates` | Failed | Failed | consistent | opaque-counter | 6 | 1:WorkflowStarted, 6:WorkflowFailed |
| `f6b60bb4-1be7-4dd5-953f-b60c2b8acc91` | `repo_gates` | Cancelled | Cancelled | consistent | — | 5 | 1:WorkflowStarted, 5:WorkflowCancelled |
| `4a6361de-b7f2-4d12-b3ce-f85e1d9a18a0` | `shell_run` | Completed | Completed | consistent | — | 6 | 1:WorkflowStarted, 6:WorkflowCompleted |
| `766e0bfe-d8b4-48ba-b1b5-afd2f8991cca` | `norn_dev` | Completed | Completed | consistent | — | 14 | 1:WorkflowStarted, 13:WorkflowCompleted |
| `02428d75-e8a7-4fd9-b1fe-1c920f9eeb53` | `fleet_dev` | Failed | Failed | consistent | opaque-counter | 8 | 1:WorkflowStarted, 7:WorkflowFailed |
| `efc84610-86f5-4466-a4d0-e1e6259957d3` | `fleet_dev` | Failed | Failed | consistent | opaque-counter | 41 | 1:WorkflowStarted, 26:WorkflowFailed, 28:WorkflowReopened, 41:WorkflowFailed |
| `95f5e6c1-2e6e-40c1-a84c-5190d2961859` | `aion_internal_awl_child_fleet_dev_open_0` | Completed | Completed | consistent | — | 8 | 1:WorkflowStarted, 7:WorkflowCompleted |
| `6ac6d497-c985-4c8a-a6f0-15692e4b0489` | `aion_internal_awl_child_fleet_dev_open_0` | Completed | Completed | consistent | — | 8 | 1:WorkflowStarted, 7:WorkflowCompleted |
| `b7c5fb1f-8734-452f-975c-2c3b7725049f` | `fleet_dev` | Failed | Failed | consistent | opaque-counter | 33 | 1:WorkflowStarted, 32:WorkflowFailed |
| `f7104295-e699-46dc-81d5-98a0ccdfda25` | `aion_internal_awl_child_fleet_dev_open_0` | Completed | Completed | consistent | — | 10 | 1:WorkflowStarted, 9:WorkflowCompleted |
| `ba6e8c67-efe0-4027-91db-e24ed60a2e4d` | `aion_internal_awl_child_fleet_dev_open_0` | Completed | Completed | consistent | — | 10 | 1:WorkflowStarted, 9:WorkflowCompleted |
| `7649531a-817f-4168-bc2f-85b25e5ee8d6` | `norn_dev` | Failed | Failed | consistent | opaque-counter | 15 | 1:WorkflowStarted, 14:WorkflowFailed |
| `ca4cc8f5-3a5e-463c-aeaa-13373077168a` | `norn_dev` | Completed | Completed | consistent | — | 14 | 1:WorkflowStarted, 13:WorkflowCompleted |
| `08d578c6-ad13-4a61-b4fe-8e242da869ba` | `norn_dev` | Failed | Failed | consistent | opaque-counter | 14 | 1:WorkflowStarted, 13:WorkflowFailed |
| `f3e22809-c1d5-4669-a3e4-ab8c1e20d40e` | `norn_dev` | Failed | Failed | consistent | opaque-counter | 14 | 1:WorkflowStarted, 13:WorkflowFailed |
| `a070baae-8d2f-4e18-adbe-aeb7bdc02e2a` | `norn_dev` | Failed | Failed | consistent | opaque-counter | 22 | 1:WorkflowStarted, 17:WorkflowCancelled, 18:WorkflowReopened, 22:WorkflowFailed |
| `f808f698-7b46-41a7-b72a-e6f628ae7084` | `fleet_dev` | Failed | Failed | consistent | opaque-counter | 41 | 1:WorkflowStarted, 33:WorkflowPaused, 34:WorkflowResumed, 36:WorkflowCancelled, 37:WorkflowReopened, 41:WorkflowFailed |
| `2f0867d0-60d7-4ecc-b08c-3fd6c1f2d2b5` | `aion_internal_awl_child_fleet_dev_open_0` | Completed | Completed | consistent | — | 10 | 1:WorkflowStarted, 9:WorkflowCompleted |
| `1e6f8383-88d7-4c71-9a8f-de4a6d9c30f2` | `aion_internal_awl_child_fleet_dev_open_0` | Completed | Completed | consistent | — | 10 | 1:WorkflowStarted, 9:WorkflowCompleted |
| `e0625271-f3a4-44ad-b5a7-512aa929c9fa` | `norn_dev` | Failed | Failed | consistent | opaque-counter | 20 | 1:WorkflowStarted, 14:WorkflowFailed, 16:WorkflowReopened, 20:WorkflowFailed |
| `4b0139d2-05d7-4401-9a66-e82070b75218` | `norn_dev` | Failed | Failed | consistent | opaque-counter | 14 | 1:WorkflowStarted, 13:WorkflowFailed |
| `221b52c3-9e1f-41c0-9871-d892b2bf2b13` | `norn_dev` | Failed | Failed | consistent | opaque-counter | 14 | 1:WorkflowStarted, 13:WorkflowFailed |
| `4c4ef686-ccc7-49d7-a352-63df61f6f3da` | `norn_dev` | Failed | Failed | consistent | opaque-counter | 16 | 1:WorkflowStarted, 15:WorkflowFailed |
| `38633615-dbd3-4aab-9fd5-58999e9c977d` | `norn_dev` | Running | Running | consistent | — | 8 | 1:WorkflowStarted |

## Machine-readable detail

`census.json` contains the same 143 rows plus exact failure messages, event type counts, lifecycle sequence markers, history head/first/last sequence checks, page counts, and request metadata.
