-module(aion_continue_fixture).
-export([park_only/0, await_then_continue/0]).

%% Fixture for the continue-as-new epoch-gate live-process test in
%% src/runtime/nif_continue_as_new_tests.rs.
%%
%% It exists to put a REAL beamr workflow process behind the
%% `aion_flow_ffi:continue_as_new/1` NIF, because the decision under test —
%% whether the NIF's gate ENDS the calling process — happens inside the NIF and
%% is invisible to any test that drives the durable half directly.
%%
%% Two properties are load-bearing, and an earlier draft had neither.
%%
%% 1. IT CANNOT DIE ON ITS OWN. The NIF's outcome is swallowed and every path
%%    parks in `receive` forever afterwards, so the process never returns. The
%%    test asserts a process is GONE, and any way of dying that is not
%%    `cancel_pid` would make that assertion pass for the wrong reason.
%%
%% 2. IT WAITS FOR THE TEST, AND EXHAUSTING THE WAIT MUST NOT CALL. A test can
%%    only arrange this run's world once it holds the pid, which it only holds
%%    after spawning, so a call made immediately would land before the run is
%%    registered and fail for an unrelated reason. `wait_registered/1` polls a
%%    PURE NIF that answers only once the registry knows this pid, and the test
%%    arranges the epoch BEFORE registering — so the one durable call sees the
%%    world the test built.
%%
%%    🔴 `wait_registered(0)` PARKS RATHER THAN CALLING, and that line is the
%%    difference between a synchronised wait and a disguised sleep. An earlier
%%    draft fell through to the call when the budget ran out; the poll was
%%    silently never succeeding, so every run was really "sleep, then call", and
%%    it passed. Parking on exhaustion turns that into a loud failure: a run
%%    that never synchronised leaves the process alive and fails the assertion
%%    instead of measuring nothing.
park_only() ->
    park().

await_then_continue() ->
    wait_registered(600),
    call_once(),
    park().

wait_registered(0) ->
    park();
wait_registered(Remaining) ->
    Resolved =
        try aion_flow_ffi:workflow_id() of
            {ok, _} -> true;
            _ -> false
        catch
            _:_ -> false
        end,
    case Resolved of
        true ->
            ok;
        false ->
            receive
            after 5 -> ok
            end,
            wait_registered(Remaining - 1)
    end.

call_once() ->
    try aion_flow_ffi:continue_as_new(<<"{}">>) of
        _ -> ok
    catch
        _:_ -> ok
    end.

park() ->
    receive
        stop -> ok
    end.
