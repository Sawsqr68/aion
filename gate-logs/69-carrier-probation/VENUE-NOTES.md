# Venue — carrier 3 diagnosed and cured (opening probation)

- Worktree: `.worktrees/100-budget`, branch `fix/69-pin-diagnosis-completes`
- **FROZEN HEAD**: ee27aa6d9 (records) over c8ec95bbb (the fix)
- **Attested pair**: `(c8ec95bbb, 0e8872423)` — parent IS origin/main, the #111 merge
- Tom's live server (pid 71475) up, holding 8080/50051/50061 — same venue as the
  #100, #110 and #111 batteries, deliberately.

## What this battery is actually testing

The subject is one of its OWN carriers. `production_boot_..._over_liminal` failed
2/24 in the register and 13/25 in this session's venue; the claim is that it now
fails 0/N because the wait is derived from the opening probation rather than fixed
at 5s.

So a green here is weak evidence on its own — one sample of a formerly-intermittent
test. The strong evidence is the 20-run confirmation recorded in the commit, and
this battery's job is the ordinary one: prove the change breaks nothing else.

⚠️ Both tests in that module now take ~15-20s rather than ~4s, by design: they wait
out a probation the server genuinely imposes. `rust:test` and
`rust:test-all-features` will each be slightly longer.

## The other three carriers are still live

`quiet_tenant_not_starved_by_bursty_tenant_spread_across_routes`,
`round_robin_gives_quiet_tenant_a_slot_every_sweep`, and
`http_put_survives_a_second_server_state_over_the_same_store` are untouched by this
change. If one fires, cite the register — it is not evidence about this branch, and
it is NOT evidence that the probation diagnosis generalises to it.
