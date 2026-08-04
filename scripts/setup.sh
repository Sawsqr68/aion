#!/usr/bin/env bash
# One-command Aion + assistant setup.
#
#     ./scripts/setup.sh          (from a clone of the aion repository)
#
# or, standalone, with no clone yet — the script clones the repository
# itself (into ./aion, or AION_CHECKOUT_DIR):
#
#     bash setup.sh
#
# Either way it ends with a running `aion server` (console included), the
# assistant workflow deployed, and the assistant worker connected and
# serving it. Safe to re-run: every step detects work already done and
# skips it.
#
# Environment overrides (all optional):
#   AION_HOME                      Aion home directory (default ~/.aion)
#   AION_CHECKOUT_DIR              where to clone the repository when the
#                                  script is run standalone (default ./aion)
#   AION_REPO_URL                  repository to clone in standalone mode
#                                  (default the ablative-io/aion GitHub repo)
#   NORN_ACCOUNT                   norn account alias the worker resumes
#                                  sessions under (default: default)
#   AION_NORN_MODEL                model passed through to norn; unset means
#                                  norn makes its own choice
#   AION_ALLOW_DEGRADED_SESSION=1  opt in to resuming agent sessions whose
#                                  provider transcript can no longer be
#                                  replayed (a real fidelity tradeoff; off
#                                  unless you say so)
set -euo pipefail

say()  { printf '\n==> %s\n' "$*"; }
note() { printf '    %s\n' "$*"; }
fail() { printf 'setup: %s\n' "$*" >&2; exit 1; }

AION_HOME="${AION_HOME:-$HOME/.aion}"
# The server and worker resolve their state through AION_HOME; export it so
# an override on this script reaches the processes it launches.
export AION_HOME
AION_REPO_URL="${AION_REPO_URL:-https://github.com/ablative-io/aion.git}"
NORN_ACCOUNT="${NORN_ACCOUNT:-default}"
AION_NORN_MODEL="${AION_NORN_MODEL:-}"
AION_ALLOW_DEGRADED_SESSION="${AION_ALLOW_DEGRADED_SESSION:-0}"

SERVER_LOG="$AION_HOME/server.log"
WORKER_LOG="$AION_HOME/assistant-worker.log"

say "Checking prerequisites"
command -v git   >/dev/null || fail "git is required and was not found on PATH"
command -v cargo >/dev/null || fail "cargo (Rust) is required — install from https://rustup.rs and re-run"
command -v norn  >/dev/null || fail "norn is required to run the assistant worker and was not found on PATH"
note "git, cargo, norn: all present"

# Locate the repository: normally this script lives in <repo>/scripts/, but
# run standalone (curl'd or copied) it clones the repository itself.
REPO_ROOT="$(cd "$(dirname "$0")/.." 2>/dev/null && pwd)"
if [ ! -f "$REPO_ROOT/examples/assistant/awl/assistant.awl" ]; then
    REPO_ROOT="${AION_CHECKOUT_DIR:-$PWD/aion}"
    if [ ! -f "$REPO_ROOT/examples/assistant/awl/assistant.awl" ]; then
        say "Cloning the aion repository into $REPO_ROOT"
        # Depth-1 on purpose: the repository's HISTORY carries ~2 GiB of
        # build artifacts committed by an old gate pipeline (removed from
        # tracking, still in every full clone's pack). A shallow clone skips
        # all of it — the build needs only the working tree.
        git clone --depth 1 "$AION_REPO_URL" "$REPO_ROOT"
    fi
fi

WORKER_DIR="$REPO_ROOT/examples/agent-dev/worker"
WORKER_BIN="$WORKER_DIR/target/release/agent-dev-worker"
ASSISTANT_AWL="$REPO_ROOT/examples/assistant/awl/assistant.awl"

# Extract one field from the server's JSON startup banner. The banner is the
# authority on what the server actually bound; nothing here assumes a port.
banner_field() {
    local value
    value="$(grep '"message":"aion-server startup banner"' "$SERVER_LOG" 2>/dev/null \
        | tail -1 | sed -n 's/.*"'"$1"'":"\([^"]*\)".*/\1/p')"
    [ -n "$value" ] || return 1
    printf '%s\n' "$value"
}

liminal_listen_address() {
    local value
    value="$(grep '"message":"liminal server listener bound"' "$SERVER_LOG" 2>/dev/null \
        | tail -1 | sed -n 's/.*"listen_address":"\([^"]*\)".*/\1/p')"
    [ -n "$value" ] || return 1
    printf '%s\n' "$value"
}

# Whether THIS Aion home's server is up: its own log names the address it
# bound, and the /build BODY proves a live aion-server is the thing
# answering (the console's SPA fallback answers any path with 200
# text/html, so a status code alone proves nothing).
server_is_up() {
    local address
    address="$(banner_field http_address 2>/dev/null)" || return 1
    curl -sf --max-time 2 "http://$address/build" 2>/dev/null | grep -q '"version"'
}

say "Checking the aion CLI"
if command -v aion >/dev/null; then
    note "aion already installed: $(aion --version 2>/dev/null || echo '(version unavailable)')"
else
    note "aion not found — installing the published aion-cli crate."
    note "A cold install compiles the whole server and can take ~25 minutes."
    cargo install aion-cli
    command -v aion >/dev/null || fail "cargo install finished but aion is not on PATH — is ~/.cargo/bin on your PATH?"
    note "installed: $(aion --version)"
fi

say "Preparing the Aion home directory ($AION_HOME)"
mkdir -p "$AION_HOME"
# The server's path-safety gate refuses a store with a group- or
# world-writable ancestor; owner-only up front means it never fires.
chmod 700 "$AION_HOME"
note "ready (owner-only permissions)"

say "Starting the Aion server"
if server_is_up; then
    note "this Aion home's server is already up — leaving it alone"
else
    ( cd "$AION_HOME" && nohup aion server >"$SERVER_LOG" 2>&1 & )
    note "launched; log: $SERVER_LOG"
fi

say "Waiting for the server to answer"
SERVER_READY=""
for _ in $(seq 1 120); do
    if server_is_up; then
        SERVER_READY=yes
        break
    fi
    sleep 1
done
[ -n "$SERVER_READY" ] || fail "server did not become ready within 120s — see $SERVER_LOG"
HTTP_ADDRESS="$(banner_field http_address)"
GRPC_ADDRESS="$(banner_field grpc_address)" || fail "startup banner carries no grpc_address — see $SERVER_LOG"
SERVER_BUILD="$(curl -sf "http://$HTTP_ADDRESS/build")"
note "server up at http://$HTTP_ADDRESS — $SERVER_BUILD"

say "Reading the server's workspace root"
WORKSPACE_ROOT="$(banner_field workspace_root)" || fail "startup banner carries no workspace_root — see $SERVER_LOG"
case "$WORKSPACE_ROOT" in
    "unresolvable:"*)
        fail "the server's workspace root is unresolvable, so declared provisioning cannot dispatch — $WORKSPACE_ROOT (see $SERVER_LOG)"
        ;;
esac
# The worker's own resolution (handlers.rs::resolve_workspace_root, override
# wins) is BY CONSTRUCTION the server's value — one resolved value threaded
# into both sides, from the server that will use it.
export AION_WORKSPACE_ROOT="$WORKSPACE_ROOT"
note "workspace root: $WORKSPACE_ROOT (threaded to the worker as AION_WORKSPACE_ROOT)"

say "Deploying the assistant workflow"
[ -f "$ASSISTANT_AWL" ] || fail "assistant document missing: $ASSISTANT_AWL"
DEPLOY_RESULT="$(aion --endpoint "http://$GRPC_ADDRESS" deploy "$ASSISTANT_AWL")"
note "$DEPLOY_RESULT"

say "Building the assistant worker (release)"
if [ -x "$WORKER_BIN" ] && [ -z "$(find "$WORKER_DIR/src" -newer "$WORKER_BIN" -name '*.rs' 2>/dev/null)" ]; then
    note "worker binary is current — skipping the build"
else
    ( cd "$WORKER_DIR" && cargo build --release )
    note "built: $WORKER_BIN"
fi

say "Starting the assistant worker"
LIMINAL_ADDRESS="$(liminal_listen_address)" || fail "the server log shows no liminal worker listener — the assistant worker has nothing to connect to.
   Check the [outbox] configuration in $AION_HOME/config.toml and $SERVER_LOG."
# A worker belongs to the server it dials, so match on the address, not just
# the binary name — another Aion home's worker on this box is not ours.
if pgrep -f "agent-dev-worker --address $LIMINAL_ADDRESS" >/dev/null 2>&1; then
    note "a worker for $LIMINAL_ADDRESS is already running (pid $(pgrep -f "agent-dev-worker --address $LIMINAL_ADDRESS" | head -1)) — leaving it alone"
else
    REGISTRATIONS_BEFORE="$(grep -c '"identity":"agent-dev-worker"' "$SERVER_LOG" || true)"
    WORKER_ARGS=(--address "$LIMINAL_ADDRESS" --norn-account "$NORN_ACCOUNT")
    if [ -n "$AION_NORN_MODEL" ]; then
        WORKER_ARGS+=(--norn-model "$AION_NORN_MODEL")
    fi
    if [ "$AION_ALLOW_DEGRADED_SESSION" = "1" ]; then
        WORKER_ARGS+=(--norn-allow-degraded-session)
    fi
    ( cd "$REPO_ROOT" && nohup "$WORKER_BIN" "${WORKER_ARGS[@]}" >"$WORKER_LOG" 2>&1 & )
    note "launched; log: $WORKER_LOG"
    REGISTERED=""
    for _ in $(seq 1 30); do
        REGISTRATIONS_NOW="$(grep -c '"identity":"agent-dev-worker"' "$SERVER_LOG" || true)"
        if [ "$REGISTRATIONS_NOW" -gt "${REGISTRATIONS_BEFORE:-0}" ]; then
            REGISTERED=yes
            break
        fi
        pgrep -f "agent-dev-worker --address $LIMINAL_ADDRESS" >/dev/null 2>&1 || fail "the worker exited during startup — see $WORKER_LOG"
        sleep 1
    done
    [ -n "$REGISTERED" ] || fail "the worker did not register with the server within 30s — see $WORKER_LOG and $SERVER_LOG"
    note "worker connected and registered with the server"
fi

say "Done"
note "Console:         http://$HTTP_ADDRESS"
note "Server build:    $SERVER_BUILD"
note "Server log:      $SERVER_LOG"
note "Worker log:      $WORKER_LOG"
note ""
note "Talk to the assistant: open the console, pick the 'assistant' workflow,"
note "and start a run. Its input is: objective (what you want) and repo_path"
note "(a git repository it works against; empty for a scratch session)."
