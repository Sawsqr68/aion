%% Workflow-side query pump runtime for aion_flow.
%%
%% The engine answers workflow queries at yield points: when a query is
%% pending, each suspending await returns the sentinel
%% `{error, <<"aion_query:", Json/binary>>}` with
%% `Json = {"query_id":"<uuid>","name":"<query name>","arguments":"<json>"}`
%% instead of resolving. `arguments` is the caller's argument document carried
%% as a JSON *string* (so its bytes are escaped and can never forge another
%% key), and is `"null"` when the caller supplied none.
%% The SDK pump loop (`aion/internal/pump`) hands the JSON payload to
%% `service/1`, which decodes it, fetches the handler fun the SDK stored in
%% the process dictionary under `{aion_query_handler, Name}` (the engine
%% contract key; GC-rooted, unlike Rust-held terms), and applies
%% `Handler(QueryId, Arguments)` inside try/catch. On a raise, or when no
%% handler is registered, the failure is replied through
%% `aion_flow_ffi:reply_query_error/2` so the caller observes a typed
%% handler failure.
%%
%% This module is plain Erlang because it is the only place in the package
%% that can catch exceptions: a query handler raise must NEVER crash the
%% workflow process, and reply failures (a late reply after the caller timed
%% out) are non-fatal by design. Diagnostics cross the engine's non-durable
%% tracing tap; query handling never writes workflow history. `service/1`
%% always returns normally.
%%
%% `aion_flow_ffi` resolves to the engine's NIF registry namespace in
%% production and to the in-process test double under `gleam test`; this
%% module is byte-identical in both environments and must only use Erlang
%% constructs the beamr VM executes (no OTP library modules).
-module(aion_flow_query_pump).

-export([register/2, service/1]).

-define(MAX_PAYLOAD_PREFIX_BYTES, 256).
-define(MAX_REASON_BYTES, 512).
-define(MAX_STACKTRACE_BYTES, 1024).
-define(MAX_DIAGNOSTIC_BYTES, 2048).

%% Store a query handler fun in the workflow process dictionary under the
%% engine-contract key `{aion_query_handler, Name}`, where the engine's
%% yield-point pump expects it. Gleam never touches raw process-dictionary
%% types; `aion/internal/ffi` binds this helper instead. Returns `nil`
%% (Gleam's `Nil`).
%%
%% The fun guards are arity-blind `is_function/1`, not `is_function/2`: the
%% beamr VM (as of 0.4.9) cannot execute the `is_function2` test instruction
%% (its loader drops the arity operand), and this module's contract is to
%% use only constructs beamr executes. Handler arity is already fixed to 2
%% (query id, arguments) by the typed Gleam binding in `aion/internal/ffi`.
register(Name, Handler) when is_binary(Name), is_function(Handler) ->
    erlang:put({aion_query_handler, Name}, Handler),
    nil.

%% Service one query sentinel payload (the JSON binary after the
%% `aion_query:` prefix). Always returns `nil` (Gleam's `Nil`) — never
%% raises, whatever the handler does.
service(Payload) when is_binary(Payload) ->
    case scan_query_id(Payload) of
        {ok, QueryId} ->
            case scan_sentinel_fields(Payload) of
                {ok, Name, Arguments} ->
                    run_handler(QueryId, Name, Arguments);
                {error, Reason} ->
                    report_malformed(Payload, Reason),
                    reply_error(
                        QueryId,
                        <<"malformed query sentinel: ", (reason_binary(Reason))/binary>>
                    )
            end;
        {error, Reason} ->
            %% Without a query id there is no reply channel to fail onto;
            %% the engine-side caller observes its configured timeout. The
            %% pump must keep the workflow alive regardless.
            report_malformed(Payload, Reason),
            nil
    end.

run_handler(QueryId, Name, Arguments) ->
    case erlang:get({aion_query_handler, Name}) of
        undefined ->
            reply_error(
                QueryId,
                <<"no handler registered in the process dictionary for query ", Name/binary>>
            );
        Handler when is_function(Handler) ->
            try Handler(QueryId, Arguments) of
                {ok, _Confirmation} -> nil;
                {error, Reason} ->
                    report_reply_failure(<<"reply_query">>, QueryId, Reason),
                    nil;
                Other ->
                    report_reply_failure(<<"reply_query_unexpected_outcome">>, QueryId, Other),
                    nil
            catch
                Class:Reason ->
                    report_reply_exception(
                        <<"reply_query_exception">>,
                        QueryId,
                        Class,
                        Reason,
                        <<"aion_flow_query_pump:run_handler/3">>
                    ),
                    reply_error(QueryId, describe_exception(Class, Reason))
            end;
        _NotAFun ->
            reply_error(
                QueryId,
                <<"registered query handler for ", Name/binary, " is not a fun">>
            )
    end.

%% Send a handler-failure reply. Reply failures are swallowed deliberately:
%% the reply path runs outside the handler try/catch, and the pump's
%% never-crash-the-workflow invariant must not depend on the FFI boundary's
%% own error contract (a late error reply after the caller timed out comes
%% back as `{error, <<"unknown_query_id:...">>}` and means nobody is
%% listening any more).
reply_error(QueryId, Message) ->
    try aion_flow_ffi:reply_query_error(QueryId, Message) of
        {ok, _Confirmation} -> nil;
        {error, Reason} ->
            report_reply_failure(<<"reply_query_error">>, QueryId, Reason),
            nil;
        Other ->
            report_reply_failure(<<"reply_query_error_unexpected_outcome">>, QueryId, Other),
            nil
    catch
        Class:Reason ->
            report_reply_exception(
                <<"reply_query_error_exception">>,
                QueryId,
                Class,
                Reason,
                <<"aion_flow_query_pump:reply_error/2">>
            ),
            nil
    end.

report_malformed(Payload, Reason) ->
    Prefix = bounded_binary(Payload, ?MAX_PAYLOAD_PREFIX_BYTES),
    report_diagnostic(
        <<"malformed_query">>,
        <<"payload_bytes=", (integer_to_binary(byte_size(Payload)))/binary,
            " prefix=", Prefix/binary, " decode_reason=", (reason_binary(Reason))/binary>>
    ).

report_reply_failure(Operation, QueryId, Reason) ->
    report_diagnostic(
        <<"query_reply_failed">>,
        <<"operation=", Operation/binary, " query_id=",
            (bounded_binary(QueryId, ?MAX_PAYLOAD_PREFIX_BYTES))/binary, " reason=",
            (bounded_binary(describe_reason(Reason), ?MAX_REASON_BYTES))/binary>>
    ).

report_reply_exception(Operation, QueryId, Class, Reason, CatchSite) ->
    report_diagnostic(
        <<"query_reply_exception">>,
        <<"operation=", Operation/binary, " query_id=",
            (bounded_binary(QueryId, ?MAX_PAYLOAD_PREFIX_BYTES))/binary, " class=",
            (reason_binary(Class))/binary, " reason=",
            (bounded_binary(describe_reason(Reason), ?MAX_REASON_BYTES))/binary, " stacktrace=",
            (describe_exception_trace(Reason, CatchSite))/binary>>
    ).

%% The engine implementation forwards this message to its existing tracing
%% subscriber. The call is itself isolated because diagnostics must never
%% weaken the query pump's liveness contract.
report_diagnostic(Category, Message) ->
    try aion_flow_ffi:report_query_diagnostic(
        Category, bounded_binary(Message, ?MAX_DIAGNOSTIC_BYTES)
    ) of
        _Outcome -> nil
    catch
        _Class:_Reason -> nil
    end.

reason_binary(Reason) when is_atom(Reason) ->
    bounded_binary(atom_to_binary(Reason, utf8), ?MAX_REASON_BYTES);
reason_binary(Reason) ->
    bounded_binary(describe_reason(Reason), ?MAX_REASON_BYTES).

%% beamr 0.16.2 executes `Class:Reason` catches correctly, but Erlang's
%% `Class:Reason:Stacktrace` form emits `build_stacktrace`, whose heap
%% allocation does not collect or retry. A handler raise with one free heap
%% word therefore kills the workflow before its error reply. Gleam panic
%% reasons already carry their source frame; other exception terms retain the
%% exact pump catch boundary instead of fabricating unavailable VM frames.
describe_exception_trace(
        #{module := Module, function := Function, file := File, line := Line}, CatchSite
    )
        when is_binary(Module), is_binary(Function), is_binary(File), is_integer(Line) ->
    SourceFrame = <<
        (bounded_binary(Module, ?MAX_REASON_BYTES))/binary,
        ":",
        (bounded_binary(Function, ?MAX_REASON_BYTES))/binary,
        "@",
        (bounded_binary(File, ?MAX_REASON_BYTES))/binary,
        ":",
        (integer_to_binary(Line))/binary
    >>,
    bounded_binary(<<SourceFrame/binary, ";", CatchSite/binary>>, ?MAX_STACKTRACE_BYTES);
describe_exception_trace(_Reason, CatchSite) ->
    bounded_binary(CatchSite, ?MAX_STACKTRACE_BYTES).

bounded_binary(Binary, Limit) when byte_size(Binary) =< Limit -> Binary;
bounded_binary(Binary, Limit) ->
    <<Prefix:Limit/binary, _Rest/binary>> = Binary,
    Prefix.

describe_exception(Class, Reason) ->
    <<"query handler raised ", (atom_to_binary(Class, utf8))/binary, ": ",
        (describe_reason(Reason))/binary>>.

%% Render an exception reason as readable text without OTP formatting
%% helpers (io_lib is not part of the beamr-executable surface). Gleam
%% `panic`/`let assert` reasons are maps carrying a binary `message`.
describe_reason(Reason) when is_binary(Reason) ->
    Reason;
describe_reason(Reason) when is_atom(Reason) ->
    atom_to_binary(Reason, utf8);
describe_reason(#{message := Message}) when is_binary(Message) ->
    Message;
describe_reason(Reason) when is_tuple(Reason), tuple_size(Reason) > 0 ->
    case element(1, Reason) of
        Tag when is_atom(Tag) -> atom_to_binary(Tag, utf8);
        _NotAnAtom -> <<"unrecognised exception term">>
    end;
describe_reason(_Reason) ->
    <<"unrecognised exception term">>.

%% --- sentinel JSON field extraction -------------------------------------
%%
%% The engine emits the sentinel JSON with serde_json: compact encoding,
%% exactly the string fields `query_id`, `name`, and `arguments`, with `"`
%% and `\` inside values always escaped. A raw `"query_id":"` / `"name":"` /
%% `"arguments":"` byte sequence therefore never occurs inside a value, so
%% scanning for the literal key pattern is exact. That is precisely why the
%% engine carries the caller's argument document as an escaped JSON *string*
%% rather than a nested value: caller-controlled bytes must not be able to
%% forge a key the scan would find. Key order is irrelevant to the scan.

scan_sentinel_fields(Payload) ->
    case scan_name(Payload) of
        {ok, Name} ->
            case scan_arguments(Payload) of
                {ok, Arguments} -> {ok, Name, Arguments};
                {error, Reason} -> {error, Reason}
            end;
        {error, Reason} ->
            {error, Reason}
    end.

scan_query_id(<<"\"query_id\":\"", Rest/binary>>) ->
    case unescape(Rest, <<>>) of
        {ok, QueryId} -> {ok, QueryId};
        error -> {error, malformed_query_id}
    end;
scan_query_id(<<_Byte, Tail/binary>>) -> scan_query_id(Tail);
scan_query_id(<<>>) -> {error, missing_query_id}.

scan_name(<<"\"name\":\"", Rest/binary>>) ->
    case unescape(Rest, <<>>) of
        {ok, Name} -> {ok, Name};
        error -> {error, malformed_name}
    end;
scan_name(<<_Byte, Tail/binary>>) -> scan_name(Tail);
scan_name(<<>>) -> {error, missing_name}.

%% The arguments field is mandatory: the engine always emits one (`"null"`
%% when the caller supplied nothing). A sentinel without it is a protocol
%% break, reported as such rather than silently handled as "no arguments" —
%% a handler must never mistake a broken carrier for an empty request.
scan_arguments(<<"\"arguments\":\"", Rest/binary>>) ->
    case unescape(Rest, <<>>) of
        {ok, Arguments} -> {ok, Arguments};
        error -> {error, malformed_arguments}
    end;
scan_arguments(<<_Byte, Tail/binary>>) -> scan_arguments(Tail);
scan_arguments(<<>>) -> {error, missing_arguments}.

%% Decode a JSON string value up to its closing quote, resolving the escape
%% sequences serde_json emits (`\"`, `\\`, `\/`, `\b`, `\f`, `\n`, `\r`,
%% `\t`, and `\uXXXX` for control characters). Unterminated or malformed
%% input yields `error`.
unescape(<<>>, _Acc) -> error;
unescape(<<$", _Rest/binary>>, Acc) -> {ok, Acc};
unescape(<<$\\, Rest/binary>>, Acc) -> unescape_escaped(Rest, Acc);
unescape(<<Byte, Rest/binary>>, Acc) -> unescape(Rest, <<Acc/binary, Byte>>).

unescape_escaped(<<$", Rest/binary>>, Acc) -> unescape(Rest, <<Acc/binary, $">>);
unescape_escaped(<<$\\, Rest/binary>>, Acc) -> unescape(Rest, <<Acc/binary, $\\>>);
unescape_escaped(<<$/, Rest/binary>>, Acc) -> unescape(Rest, <<Acc/binary, $/>>);
unescape_escaped(<<$b, Rest/binary>>, Acc) -> unescape(Rest, <<Acc/binary, $\b>>);
unescape_escaped(<<$f, Rest/binary>>, Acc) -> unescape(Rest, <<Acc/binary, $\f>>);
unescape_escaped(<<$n, Rest/binary>>, Acc) -> unescape(Rest, <<Acc/binary, $\n>>);
unescape_escaped(<<$r, Rest/binary>>, Acc) -> unescape(Rest, <<Acc/binary, $\r>>);
unescape_escaped(<<$t, Rest/binary>>, Acc) -> unescape(Rest, <<Acc/binary, $\t>>);
unescape_escaped(<<$u, H1, H2, H3, H4, Rest/binary>>, Acc) ->
    case hex_codepoint(H1, H2, H3, H4) of
        {ok, Codepoint} -> unescape(Rest, <<Acc/binary, Codepoint/utf8>>);
        error -> error
    end;
unescape_escaped(_Other, _Acc) ->
    error.

%% serde_json only `\u`-escapes control characters (< U+0020), never
%% surrogate pairs, so a lone Basic Multilingual Plane codepoint outside the
%% surrogate range is the complete valid input space.
hex_codepoint(H1, H2, H3, H4) ->
    case {hex_value(H1), hex_value(H2), hex_value(H3), hex_value(H4)} of
        {{ok, V1}, {ok, V2}, {ok, V3}, {ok, V4}} ->
            Codepoint = V1 * 4096 + V2 * 256 + V3 * 16 + V4,
            case Codepoint >= 16#D800 andalso Codepoint =< 16#DFFF of
                true -> error;
                false -> {ok, Codepoint}
            end;
        _Invalid ->
            error
    end.

hex_value(Char) when Char >= $0, Char =< $9 -> {ok, Char - $0};
hex_value(Char) when Char >= $a, Char =< $f -> {ok, Char - $a + 10};
hex_value(Char) when Char >= $A, Char =< $F -> {ok, Char - $A + 10};
hex_value(_Char) -> error.
