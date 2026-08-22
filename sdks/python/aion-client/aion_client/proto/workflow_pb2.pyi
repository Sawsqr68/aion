from . import common_pb2 as _common_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class NamespaceMintOrigin(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    NAMESPACE_MINT_ORIGIN_UNSPECIFIED: _ClassVar[NamespaceMintOrigin]
    NAMESPACE_MINT_ORIGIN_WORKER: _ClassVar[NamespaceMintOrigin]
    NAMESPACE_MINT_ORIGIN_START: _ClassVar[NamespaceMintOrigin]
    NAMESPACE_MINT_ORIGIN_EXPLICIT: _ClassVar[NamespaceMintOrigin]
    NAMESPACE_MINT_ORIGIN_INFERRED_FROM_STATE: _ClassVar[NamespaceMintOrigin]
NAMESPACE_MINT_ORIGIN_UNSPECIFIED: NamespaceMintOrigin
NAMESPACE_MINT_ORIGIN_WORKER: NamespaceMintOrigin
NAMESPACE_MINT_ORIGIN_START: NamespaceMintOrigin
NAMESPACE_MINT_ORIGIN_EXPLICIT: NamespaceMintOrigin
NAMESPACE_MINT_ORIGIN_INFERRED_FROM_STATE: NamespaceMintOrigin

class MintNamespaceRequest(_message.Message):
    __slots__ = ("namespaces", "origin")
    NAMESPACES_FIELD_NUMBER: _ClassVar[int]
    ORIGIN_FIELD_NUMBER: _ClassVar[int]
    namespaces: _containers.RepeatedScalarFieldContainer[str]
    origin: NamespaceMintOrigin
    def __init__(self, namespaces: _Optional[_Iterable[str]] = ..., origin: _Optional[_Union[NamespaceMintOrigin, str]] = ...) -> None: ...

class MintNamespaceResponse(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class StartWorkflowRequest(_message.Message):
    __slots__ = ("namespace", "workflow_type", "input", "routing_key", "task_queue", "display_name")
    NAMESPACE_FIELD_NUMBER: _ClassVar[int]
    WORKFLOW_TYPE_FIELD_NUMBER: _ClassVar[int]
    INPUT_FIELD_NUMBER: _ClassVar[int]
    ROUTING_KEY_FIELD_NUMBER: _ClassVar[int]
    TASK_QUEUE_FIELD_NUMBER: _ClassVar[int]
    DISPLAY_NAME_FIELD_NUMBER: _ClassVar[int]
    namespace: str
    workflow_type: str
    input: _common_pb2.Payload
    routing_key: str
    task_queue: str
    display_name: str
    def __init__(self, namespace: _Optional[str] = ..., workflow_type: _Optional[str] = ..., input: _Optional[_Union[_common_pb2.Payload, _Mapping]] = ..., routing_key: _Optional[str] = ..., task_queue: _Optional[str] = ..., display_name: _Optional[str] = ...) -> None: ...

class StartWorkflowResponse(_message.Message):
    __slots__ = ("workflow_id", "run_id")
    WORKFLOW_ID_FIELD_NUMBER: _ClassVar[int]
    RUN_ID_FIELD_NUMBER: _ClassVar[int]
    workflow_id: _common_pb2.WorkflowId
    run_id: _common_pb2.RunId
    def __init__(self, workflow_id: _Optional[_Union[_common_pb2.WorkflowId, _Mapping]] = ..., run_id: _Optional[_Union[_common_pb2.RunId, _Mapping]] = ...) -> None: ...

class SignalRequest(_message.Message):
    __slots__ = ("namespace", "workflow_id", "run_id", "signal_name", "payload")
    NAMESPACE_FIELD_NUMBER: _ClassVar[int]
    WORKFLOW_ID_FIELD_NUMBER: _ClassVar[int]
    RUN_ID_FIELD_NUMBER: _ClassVar[int]
    SIGNAL_NAME_FIELD_NUMBER: _ClassVar[int]
    PAYLOAD_FIELD_NUMBER: _ClassVar[int]
    namespace: str
    workflow_id: _common_pb2.WorkflowId
    run_id: _common_pb2.RunId
    signal_name: str
    payload: _common_pb2.Payload
    def __init__(self, namespace: _Optional[str] = ..., workflow_id: _Optional[_Union[_common_pb2.WorkflowId, _Mapping]] = ..., run_id: _Optional[_Union[_common_pb2.RunId, _Mapping]] = ..., signal_name: _Optional[str] = ..., payload: _Optional[_Union[_common_pb2.Payload, _Mapping]] = ...) -> None: ...

class SignalResponse(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class QueryRequest(_message.Message):
    __slots__ = ("namespace", "workflow_id", "run_id", "query_name", "arguments")
    NAMESPACE_FIELD_NUMBER: _ClassVar[int]
    WORKFLOW_ID_FIELD_NUMBER: _ClassVar[int]
    RUN_ID_FIELD_NUMBER: _ClassVar[int]
    QUERY_NAME_FIELD_NUMBER: _ClassVar[int]
    ARGUMENTS_FIELD_NUMBER: _ClassVar[int]
    namespace: str
    workflow_id: _common_pb2.WorkflowId
    run_id: _common_pb2.RunId
    query_name: str
    arguments: _common_pb2.Payload
    def __init__(self, namespace: _Optional[str] = ..., workflow_id: _Optional[_Union[_common_pb2.WorkflowId, _Mapping]] = ..., run_id: _Optional[_Union[_common_pb2.RunId, _Mapping]] = ..., query_name: _Optional[str] = ..., arguments: _Optional[_Union[_common_pb2.Payload, _Mapping]] = ...) -> None: ...

class QueryResponse(_message.Message):
    __slots__ = ("result", "error")
    RESULT_FIELD_NUMBER: _ClassVar[int]
    ERROR_FIELD_NUMBER: _ClassVar[int]
    result: _common_pb2.Payload
    error: _common_pb2.WireError
    def __init__(self, result: _Optional[_Union[_common_pb2.Payload, _Mapping]] = ..., error: _Optional[_Union[_common_pb2.WireError, _Mapping]] = ...) -> None: ...

class CancelRequest(_message.Message):
    __slots__ = ("namespace", "workflow_id", "run_id", "reason")
    NAMESPACE_FIELD_NUMBER: _ClassVar[int]
    WORKFLOW_ID_FIELD_NUMBER: _ClassVar[int]
    RUN_ID_FIELD_NUMBER: _ClassVar[int]
    REASON_FIELD_NUMBER: _ClassVar[int]
    namespace: str
    workflow_id: _common_pb2.WorkflowId
    run_id: _common_pb2.RunId
    reason: str
    def __init__(self, namespace: _Optional[str] = ..., workflow_id: _Optional[_Union[_common_pb2.WorkflowId, _Mapping]] = ..., run_id: _Optional[_Union[_common_pb2.RunId, _Mapping]] = ..., reason: _Optional[str] = ...) -> None: ...

class CancelResponse(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class ReopenRequest(_message.Message):
    __slots__ = ("namespace", "workflow_id", "run_id")
    NAMESPACE_FIELD_NUMBER: _ClassVar[int]
    WORKFLOW_ID_FIELD_NUMBER: _ClassVar[int]
    RUN_ID_FIELD_NUMBER: _ClassVar[int]
    namespace: str
    workflow_id: _common_pb2.WorkflowId
    run_id: _common_pb2.RunId
    def __init__(self, namespace: _Optional[str] = ..., workflow_id: _Optional[_Union[_common_pb2.WorkflowId, _Mapping]] = ..., run_id: _Optional[_Union[_common_pb2.RunId, _Mapping]] = ...) -> None: ...

class ReopenResponse(_message.Message):
    __slots__ = ("run_id", "status")
    RUN_ID_FIELD_NUMBER: _ClassVar[int]
    STATUS_FIELD_NUMBER: _ClassVar[int]
    run_id: _common_pb2.RunId
    status: _common_pb2.WorkflowStatus
    def __init__(self, run_id: _Optional[_Union[_common_pb2.RunId, _Mapping]] = ..., status: _Optional[_Union[_common_pb2.WorkflowStatus, str]] = ...) -> None: ...

class PauseRequest(_message.Message):
    __slots__ = ("namespace", "workflow_id", "run_id", "reason")
    NAMESPACE_FIELD_NUMBER: _ClassVar[int]
    WORKFLOW_ID_FIELD_NUMBER: _ClassVar[int]
    RUN_ID_FIELD_NUMBER: _ClassVar[int]
    REASON_FIELD_NUMBER: _ClassVar[int]
    namespace: str
    workflow_id: _common_pb2.WorkflowId
    run_id: _common_pb2.RunId
    reason: str
    def __init__(self, namespace: _Optional[str] = ..., workflow_id: _Optional[_Union[_common_pb2.WorkflowId, _Mapping]] = ..., run_id: _Optional[_Union[_common_pb2.RunId, _Mapping]] = ..., reason: _Optional[str] = ...) -> None: ...

class PauseResponse(_message.Message):
    __slots__ = ("run_id", "status")
    RUN_ID_FIELD_NUMBER: _ClassVar[int]
    STATUS_FIELD_NUMBER: _ClassVar[int]
    run_id: _common_pb2.RunId
    status: _common_pb2.WorkflowStatus
    def __init__(self, run_id: _Optional[_Union[_common_pb2.RunId, _Mapping]] = ..., status: _Optional[_Union[_common_pb2.WorkflowStatus, str]] = ...) -> None: ...

class ResumeRequest(_message.Message):
    __slots__ = ("namespace", "workflow_id", "run_id")
    NAMESPACE_FIELD_NUMBER: _ClassVar[int]
    WORKFLOW_ID_FIELD_NUMBER: _ClassVar[int]
    RUN_ID_FIELD_NUMBER: _ClassVar[int]
    namespace: str
    workflow_id: _common_pb2.WorkflowId
    run_id: _common_pb2.RunId
    def __init__(self, namespace: _Optional[str] = ..., workflow_id: _Optional[_Union[_common_pb2.WorkflowId, _Mapping]] = ..., run_id: _Optional[_Union[_common_pb2.RunId, _Mapping]] = ...) -> None: ...

class ResumeResponse(_message.Message):
    __slots__ = ("run_id", "status")
    RUN_ID_FIELD_NUMBER: _ClassVar[int]
    STATUS_FIELD_NUMBER: _ClassVar[int]
    run_id: _common_pb2.RunId
    status: _common_pb2.WorkflowStatus
    def __init__(self, run_id: _Optional[_Union[_common_pb2.RunId, _Mapping]] = ..., status: _Optional[_Union[_common_pb2.WorkflowStatus, str]] = ...) -> None: ...

class RenameRequest(_message.Message):
    __slots__ = ("namespace", "workflow_id", "run_id", "display_name")
    NAMESPACE_FIELD_NUMBER: _ClassVar[int]
    WORKFLOW_ID_FIELD_NUMBER: _ClassVar[int]
    RUN_ID_FIELD_NUMBER: _ClassVar[int]
    DISPLAY_NAME_FIELD_NUMBER: _ClassVar[int]
    namespace: str
    workflow_id: _common_pb2.WorkflowId
    run_id: _common_pb2.RunId
    display_name: str
    def __init__(self, namespace: _Optional[str] = ..., workflow_id: _Optional[_Union[_common_pb2.WorkflowId, _Mapping]] = ..., run_id: _Optional[_Union[_common_pb2.RunId, _Mapping]] = ..., display_name: _Optional[str] = ...) -> None: ...

class RenameResponse(_message.Message):
    __slots__ = ("run_id", "display_name")
    RUN_ID_FIELD_NUMBER: _ClassVar[int]
    DISPLAY_NAME_FIELD_NUMBER: _ClassVar[int]
    run_id: _common_pb2.RunId
    display_name: str
    def __init__(self, run_id: _Optional[_Union[_common_pb2.RunId, _Mapping]] = ..., display_name: _Optional[str] = ...) -> None: ...

class ListWorkflowsRequest(_message.Message):
    __slots__ = ("namespace", "filter")
    NAMESPACE_FIELD_NUMBER: _ClassVar[int]
    FILTER_FIELD_NUMBER: _ClassVar[int]
    namespace: str
    filter: _common_pb2.WireEnvelope
    def __init__(self, namespace: _Optional[str] = ..., filter: _Optional[_Union[_common_pb2.WireEnvelope, _Mapping]] = ...) -> None: ...

class ListWorkflowsResponse(_message.Message):
    __slots__ = ("summaries",)
    SUMMARIES_FIELD_NUMBER: _ClassVar[int]
    summaries: _containers.RepeatedCompositeFieldContainer[_common_pb2.WireEnvelope]
    def __init__(self, summaries: _Optional[_Iterable[_Union[_common_pb2.WireEnvelope, _Mapping]]] = ...) -> None: ...

class CountWorkflowsRequest(_message.Message):
    __slots__ = ("namespace", "filter")
    NAMESPACE_FIELD_NUMBER: _ClassVar[int]
    FILTER_FIELD_NUMBER: _ClassVar[int]
    namespace: str
    filter: _common_pb2.WireEnvelope
    def __init__(self, namespace: _Optional[str] = ..., filter: _Optional[_Union[_common_pb2.WireEnvelope, _Mapping]] = ...) -> None: ...

class CountWorkflowsResponse(_message.Message):
    __slots__ = ("count",)
    COUNT_FIELD_NUMBER: _ClassVar[int]
    count: int
    def __init__(self, count: _Optional[int] = ...) -> None: ...

class DescribeWorkflowRequest(_message.Message):
    __slots__ = ("namespace", "workflow_id", "run_id", "include_history")
    NAMESPACE_FIELD_NUMBER: _ClassVar[int]
    WORKFLOW_ID_FIELD_NUMBER: _ClassVar[int]
    RUN_ID_FIELD_NUMBER: _ClassVar[int]
    INCLUDE_HISTORY_FIELD_NUMBER: _ClassVar[int]
    namespace: str
    workflow_id: _common_pb2.WorkflowId
    run_id: _common_pb2.RunId
    include_history: bool
    def __init__(self, namespace: _Optional[str] = ..., workflow_id: _Optional[_Union[_common_pb2.WorkflowId, _Mapping]] = ..., run_id: _Optional[_Union[_common_pb2.RunId, _Mapping]] = ..., include_history: _Optional[bool] = ...) -> None: ...

class DescribeWorkflowResponse(_message.Message):
    __slots__ = ("summary", "history", "run_id", "history_head_seq", "terminal_event")
    SUMMARY_FIELD_NUMBER: _ClassVar[int]
    HISTORY_FIELD_NUMBER: _ClassVar[int]
    RUN_ID_FIELD_NUMBER: _ClassVar[int]
    HISTORY_HEAD_SEQ_FIELD_NUMBER: _ClassVar[int]
    TERMINAL_EVENT_FIELD_NUMBER: _ClassVar[int]
    summary: _common_pb2.WireEnvelope
    history: _containers.RepeatedCompositeFieldContainer[_common_pb2.WireEnvelope]
    run_id: _common_pb2.RunId
    history_head_seq: int
    terminal_event: _common_pb2.WireEnvelope
    def __init__(self, summary: _Optional[_Union[_common_pb2.WireEnvelope, _Mapping]] = ..., history: _Optional[_Iterable[_Union[_common_pb2.WireEnvelope, _Mapping]]] = ..., run_id: _Optional[_Union[_common_pb2.RunId, _Mapping]] = ..., history_head_seq: _Optional[int] = ..., terminal_event: _Optional[_Union[_common_pb2.WireEnvelope, _Mapping]] = ...) -> None: ...

class ReadHistoryRequest(_message.Message):
    __slots__ = ("namespace", "workflow_id", "from_seq", "limit")
    NAMESPACE_FIELD_NUMBER: _ClassVar[int]
    WORKFLOW_ID_FIELD_NUMBER: _ClassVar[int]
    FROM_SEQ_FIELD_NUMBER: _ClassVar[int]
    LIMIT_FIELD_NUMBER: _ClassVar[int]
    namespace: str
    workflow_id: _common_pb2.WorkflowId
    from_seq: int
    limit: int
    def __init__(self, namespace: _Optional[str] = ..., workflow_id: _Optional[_Union[_common_pb2.WorkflowId, _Mapping]] = ..., from_seq: _Optional[int] = ..., limit: _Optional[int] = ...) -> None: ...

class ReadHistoryResponse(_message.Message):
    __slots__ = ("events", "next_from_seq", "head_seq")
    EVENTS_FIELD_NUMBER: _ClassVar[int]
    NEXT_FROM_SEQ_FIELD_NUMBER: _ClassVar[int]
    HEAD_SEQ_FIELD_NUMBER: _ClassVar[int]
    events: _containers.RepeatedCompositeFieldContainer[_common_pb2.WireEnvelope]
    next_from_seq: int
    head_seq: int
    def __init__(self, events: _Optional[_Iterable[_Union[_common_pb2.WireEnvelope, _Mapping]]] = ..., next_from_seq: _Optional[int] = ..., head_seq: _Optional[int] = ...) -> None: ...

class CreateScheduleRequest(_message.Message):
    __slots__ = ("namespace", "config")
    NAMESPACE_FIELD_NUMBER: _ClassVar[int]
    CONFIG_FIELD_NUMBER: _ClassVar[int]
    namespace: str
    config: _common_pb2.WireEnvelope
    def __init__(self, namespace: _Optional[str] = ..., config: _Optional[_Union[_common_pb2.WireEnvelope, _Mapping]] = ...) -> None: ...

class CreateScheduleResponse(_message.Message):
    __slots__ = ("schedule_id", "state")
    SCHEDULE_ID_FIELD_NUMBER: _ClassVar[int]
    STATE_FIELD_NUMBER: _ClassVar[int]
    schedule_id: _common_pb2.ScheduleId
    state: _common_pb2.WireEnvelope
    def __init__(self, schedule_id: _Optional[_Union[_common_pb2.ScheduleId, _Mapping]] = ..., state: _Optional[_Union[_common_pb2.WireEnvelope, _Mapping]] = ...) -> None: ...

class UpdateScheduleRequest(_message.Message):
    __slots__ = ("namespace", "schedule_id", "config")
    NAMESPACE_FIELD_NUMBER: _ClassVar[int]
    SCHEDULE_ID_FIELD_NUMBER: _ClassVar[int]
    CONFIG_FIELD_NUMBER: _ClassVar[int]
    namespace: str
    schedule_id: _common_pb2.ScheduleId
    config: _common_pb2.WireEnvelope
    def __init__(self, namespace: _Optional[str] = ..., schedule_id: _Optional[_Union[_common_pb2.ScheduleId, _Mapping]] = ..., config: _Optional[_Union[_common_pb2.WireEnvelope, _Mapping]] = ...) -> None: ...

class UpdateScheduleResponse(_message.Message):
    __slots__ = ("state",)
    STATE_FIELD_NUMBER: _ClassVar[int]
    state: _common_pb2.WireEnvelope
    def __init__(self, state: _Optional[_Union[_common_pb2.WireEnvelope, _Mapping]] = ...) -> None: ...

class ScheduleIdRequest(_message.Message):
    __slots__ = ("namespace", "schedule_id")
    NAMESPACE_FIELD_NUMBER: _ClassVar[int]
    SCHEDULE_ID_FIELD_NUMBER: _ClassVar[int]
    namespace: str
    schedule_id: _common_pb2.ScheduleId
    def __init__(self, namespace: _Optional[str] = ..., schedule_id: _Optional[_Union[_common_pb2.ScheduleId, _Mapping]] = ...) -> None: ...

class PauseScheduleResponse(_message.Message):
    __slots__ = ("state",)
    STATE_FIELD_NUMBER: _ClassVar[int]
    state: _common_pb2.WireEnvelope
    def __init__(self, state: _Optional[_Union[_common_pb2.WireEnvelope, _Mapping]] = ...) -> None: ...

class ResumeScheduleResponse(_message.Message):
    __slots__ = ("state",)
    STATE_FIELD_NUMBER: _ClassVar[int]
    state: _common_pb2.WireEnvelope
    def __init__(self, state: _Optional[_Union[_common_pb2.WireEnvelope, _Mapping]] = ...) -> None: ...

class DeleteScheduleResponse(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class ListSchedulesRequest(_message.Message):
    __slots__ = ("namespace",)
    NAMESPACE_FIELD_NUMBER: _ClassVar[int]
    namespace: str
    def __init__(self, namespace: _Optional[str] = ...) -> None: ...

class ListSchedulesResponse(_message.Message):
    __slots__ = ("schedules",)
    SCHEDULES_FIELD_NUMBER: _ClassVar[int]
    schedules: _containers.RepeatedCompositeFieldContainer[_common_pb2.WireEnvelope]
    def __init__(self, schedules: _Optional[_Iterable[_Union[_common_pb2.WireEnvelope, _Mapping]]] = ...) -> None: ...

class DescribeScheduleResponse(_message.Message):
    __slots__ = ("state",)
    STATE_FIELD_NUMBER: _ClassVar[int]
    state: _common_pb2.WireEnvelope
    def __init__(self, state: _Optional[_Union[_common_pb2.WireEnvelope, _Mapping]] = ...) -> None: ...
