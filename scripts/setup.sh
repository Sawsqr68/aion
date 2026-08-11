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
if [ ! -f "$REPO_ROOT/crates/aion-server/assistant-embed/assistant.awl" ]; then
    REPO_ROOT="${AION_CHECKOUT_DIR:-$PWD/aion}"
    if [ ! -f "$REPO_ROOT/crates/aion-server/assistant-embed/assistant.awl" ]; then
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
ASSISTANT_AWL="$REPO_ROOT/crates/aion-server/assistant-embed/assistant.awl"

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
    # Installed FROM THE CLONE, not from crates.io. This script and the server
    # it drives share a contract — the startup banner it reads fields out of —
    # and a published crate is a second copy of that contract with nothing
    # forcing it to agree with this file. It has already disagreed once: the
    # banner's `workspace_root`, which this script requires, was added after
    # 0.11.0 was published, so `cargo install aion-cli` produced a server this
    # script could not finish against. Building from the tree this script
    # ships in makes the two agree by construction.
    note "aion not found — installing from this checkout ($REPO_ROOT)."
    note "A cold install compiles the whole server and can take ~25 minutes."
    cargo install --path "$REPO_ROOT/crates/aion-cli"
    command -v aion >/dev/null || fail "cargo install finished but aion is not on PATH — is ~/.cargo/bin on your PATH?"
    note "installed: $(aion --version)"
fi

say "Preparing the Aion home directory ($AION_HOME)"
# The server provisions and tightens its own home at boot, but the launch
# below cds into it and writes the server log there BEFORE the server exists,
# so the directory itself is created here.
mkdir -p "$AION_HOME"
chmod 700 "$AION_HOME"
note "ready (owner-only permissions)"

# The config itself is the SERVER'S job now (#180): a boot that finds no
# config anywhere writes $AION_HOME/config.toml (owner-only) from its
# embedded template, with the two surfaces this script needs already
# commissioned — the deploy gRPC service (mounted only when `deploy.enabled`)
# and the outbox, which hosts the liminal listener remote workers connect IN
# to. An existing config is never touched, so operator edits survive re-runs.
if [ -f "$AION_HOME/config.toml" ]; then
    note "existing config kept as-is: $AION_HOME/config.toml"
else
    note "no config yet — the server scaffolds $AION_HOME/config.toml on first start."
    note "It enables the deploy surface: deploying is running code on this server,"
    note "which is why that ships off by default. It listens on localhost only."
    note "On a shared machine, read that file before going further."
fi

# The server's own last ERROR line, which says WHY it would not start — an
# unsafe store path, an address already bound, a config it would not accept.
# A timeout only establishes that nothing answered; this establishes the cause,
# and the cause is what the operator needs.
server_failure_reason() {
    local reason
    # `[^"]*` rather than `.*`: a greedy match runs past the end of the error
    # string and drags the following JSON fields into the message the operator
    # reads. Stopping at the closing quote costs a truncated reason only if the
    # error text itself contains an escaped quote, which is the better trade.
    reason="$(grep '"level":"ERROR"' "$SERVER_LOG" 2>/dev/null | tail -1 \
        | sed -n 's/.*"error":"\([^"]*\)".*/\1/p')"
    [ -n "$reason" ] || return 1
    printf '%s\n' "$reason"
}

say "Starting the Aion server"
SERVER_PID=""
if server_is_up; then
    note "this Aion home's server is already up — leaving it alone"
else
    ( cd "$AION_HOME" && nohup aion server >"$SERVER_LOG" 2>&1 & echo $! >"$AION_HOME/server.pid" )
    SERVER_PID="$(cat "$AION_HOME/server.pid" 2>/dev/null || true)"
    note "launched; log: $SERVER_LOG"
fi

say "Waiting for the server to answer"
SERVER_READY=""
for _ in $(seq 1 120); do
    if server_is_up; then
        SERVER_READY=yes
        break
    fi
    # A server that has already exited will never answer, so waiting out the
    # full deadline buys nothing and buries the reason under two minutes of
    # silence. The worker startup loop below has always done this; the server
    # loop needs it for exactly the same reason.
    if [ -n "$SERVER_PID" ] && ! kill -0 "$SERVER_PID" 2>/dev/null; then
        if REASON="$(server_failure_reason)"; then
            fail "the server exited during startup: $REASON
   (full log: $SERVER_LOG)"
        fi
        fail "the server exited during startup and its log records no error — see $SERVER_LOG"
    fi
    sleep 1
done
if [ -z "$SERVER_READY" ]; then
    if REASON="$(server_failure_reason)"; then
        fail "the server did not become ready within 120s: $REASON
   (full log: $SERVER_LOG)"
    fi
    fail "server did not become ready within 120s — see $SERVER_LOG"
fi
HTTP_ADDRESS="$(banner_field http_address)"
GRPC_ADDRESS="$(banner_field grpc_address)" || fail "startup banner carries no grpc_address — see $SERVER_LOG"
SERVER_BUILD="$(curl -sf "http://$HTTP_ADDRESS/build")"
note "server up at http://$HTTP_ADDRESS — $SERVER_BUILD"

say "Reading the server's workspace root"
# An `aion` that was already on PATH never went through the install step above,
# so it can be any version — including one older than the banner field this
# script needs. Say which mismatch it is and how to end it, rather than
# reporting an absent field and leaving the operator to work out why.
WORKSPACE_ROOT="$(banner_field workspace_root)" || fail "this server's startup banner carries no \`workspace_root\`.
   That field is what tells the assistant worker where the server will look for
   repositories, and a server that does not publish it predates this script.
   The \`aion\` in use is $(command -v aion) ($(aion --version 2>/dev/null || echo 'version unavailable')).
   Install the CLI from this checkout so the two agree:
       cargo install --path $REPO_ROOT/crates/aion-cli
   then stop the running server and re-run this script.
   (server log: $SERVER_LOG)"
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
    # ...but say so if it is serving a queue the deployed document no longer
    # dispatches on. The assistant moved off `default` onto its own private
    # queue (#200); a worker started before that move connects, registers, and
    # then receives nothing at all — a silent park that looks exactly like a
    # wedged agent. The document states the queue; the worker's own startup
    # line states what it serves.
    DECLARED_QUEUE="$(awk '/^worker [a-z][a-z0-9_]*$/ { print $2; exit }' "$ASSISTANT_AWL")"
    if [ -z "$DECLARED_QUEUE" ]; then
        note "WARNING: could not read the task queue from $ASSISTANT_AWL, so the running worker's"
        note "         queue could not be checked. If the assistant never answers, restart it."
    elif ! grep -qE "task_queue=$DECLARED_QUEUE([^A-Za-z0-9_]|$)" "$WORKER_LOG" 2>/dev/null; then
        note "WARNING: the running worker does not report serving task queue '$DECLARED_QUEUE',"
        note "         which is the queue $ASSISTANT_AWL declares. A worker on another queue"
        note "         registers happily and then receives no dispatch at all. Stop it and re-run"
        note "         this script:  pkill -f \"agent-dev-worker --address $LIMINAL_ADDRESS\""
        note "         (evidence: $WORKER_LOG)"
    else
        note "the running worker serves task queue '$DECLARED_QUEUE', as the document declares"
    fi
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
