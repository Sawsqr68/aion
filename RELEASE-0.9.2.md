# aion 0.9.2 — shipped 2026-07-20 (report for Tom)

*(Meridian bridge was down for 2h+ — DM banked at ~/.norn/delegations/tom-092-ping.md
and left here as the morning artifact. Terminal notification was sent 19:20Z.)*

**aion 0.9.2 is live on crates.io** — all 21 crates registry-verified
(one new crate: `aion-darwin-acl`). Main pushed at `3f4da809`.

## Your two asks, done properly

1. **Authoring works out of the box.** `cargo install aion-cli` on any
   machine → `aion server` with zero config → the full AWL studio just
   works (documents, check, deploy, revisions). No workspace_dir, no
   gleam, nothing to configure. Live-proven repeatedly against real
   scratch servers.
2. **Central ~/.aion home, as you ruled.** `AION_HOME` env > `~/.aion`;
   config discovery: `--config` > `./aion.toml` > `~/.aion/config.toml` >
   defaults; data + authoring live under the home by default; a migration
   guard keeps existing CWD-relative dirs working with a loud startup note.

## The bar it cleared

8 dev rounds against 6 independent adversarial safety reviews, ending
zero-findings. The reviews caught real things pre-release: the studio
routes were bypassing auth entirely (pre-existing hole the new default
would have armed), symlink/rename attacks on the data root, and on macOS
even ACL-shared parent dirs are now refused with UUID-level principal
checks. The default-on surface ships hardened.

Also merged to main: the event-stream honesty overhaul (5 review passes,
zero findings) — real resume cursors, one server-enforced subscription
per socket, recovery proven rather than presumed (durable feeds replay
from cursor; live-only feeds show an explicit "possible gap" state).

## Heads-up for this box — WITHDRAWN 2026-07-30

This section used to say your `~/.aion` was mode 0755, that a stock
`aion server` would refuse, and that running `chmod 700 ~/.aion`
yourself was "the privacy gate doing its job".

That was wrong, and it was the wrong thing to ask of you. The server
already chmods its whole data tree to 0700/0600 on startup — it just
did that *after* the refusal that made it unreachable. So it was
willing to tighten the directory itself and stopped to make you do it
by hand instead.

Fixed. The server now creates `~/.aion` owner-only if it is missing,
and tightens it in place if it is loose, logging what it changed. It
still refuses — correctly — when it genuinely cannot repair the
situation: a world-writable or foreign-owned parent directory, a
macOS ACL granting someone else access, a symlinked root, or a root
owned by another user. Those refusals now say who owns what and what
would actually fix it, instead of naming a `chmod` that would have
failed for you too.

Nothing for you to run.

## Rulings whenever you want

1. `WorkflowTimedOut` is wired to nothing — authored workflow timeout was
   never implemented. Build it or drop the type.
2. The OBX branch (`dev/obx-liveness-race` @ d1766084) still awaits your
   merge word — terminal review was ACCEPT, zero blockers.
