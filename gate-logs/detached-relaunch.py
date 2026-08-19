#!/usr/bin/env /usr/bin/python3
"""Session-detached aion server relaunch (Waffles' 03:53Z posture ruling).

Double-fork + os.setsid so the server lives in its OWN session and process
group, belonging to no tool shell — the reaper kill vector identified in
gate-logs/262-dig/FINDINGS.md cannot reach it by group. Instrumented per
standard: dated log, pid file. Run from anywhere; the grandchild chdirs to
the aion repo root (store is CWD-relative ./aion-data — house law).

Usage: /usr/bin/python3 gate-logs/detached-relaunch.py <logpath> <pidpath>
Prints the server pid on stdout (read back from the pid file after boot).
"""
import os
import sys
import time

REPO_ROOT = "/Users/tom/Developer/ablative/stack/aion"
BINARY = "/Users/tom/.cargo/bin/aion"


def main() -> None:
    if len(sys.argv) != 3:
        sys.exit("usage: detached-relaunch.py <logpath> <pidpath>")
    logpath = os.path.abspath(sys.argv[1])
    pidpath = os.path.abspath(sys.argv[2])

    first = os.fork()
    if first > 0:
        # Launcher: wait for the pid file to appear, then report and exit.
        for _ in range(50):
            time.sleep(0.2)
            if os.path.exists(pidpath):
                with open(pidpath, encoding="ascii") as fh:
                    print(fh.read().strip())
                return
        sys.exit("pid file never appeared — relaunch failed")

    # Intermediate child: new session, then fork the real server process.
    os.setsid()
    second = os.fork()
    if second > 0:
        os._exit(0)

    # Grandchild: session member with no controlling terminal, pgid/sid
    # belonging to no shell. Wire stdio to the dated log and exec.
    log_fd = os.open(logpath, os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o644)
    null_fd = os.open(os.devnull, os.O_RDONLY)
    os.dup2(null_fd, 0)
    os.dup2(log_fd, 1)
    os.dup2(log_fd, 2)
    os.chdir(REPO_ROOT)
    with open(pidpath, "w", encoding="ascii") as fh:
        fh.write(str(os.getpid()))
    os.execv(BINARY, ["aion", "server"])


if __name__ == "__main__":
    main()
