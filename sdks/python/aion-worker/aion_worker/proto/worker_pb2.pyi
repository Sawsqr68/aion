from . import common_pb2 as _common_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class ActivityErrorKind(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    ACTIVITY_ERROR_KIND_UNSPECIFIED: _ClassVar[ActivityErrorKind]
    ACTIVITY_ERROR_KIND_RETRYABLE: _ClassVar[ActivityErrorKind]
    ACTIVITY_ERROR_KIND_TERMINAL: _ClassVar[ActivityErrorKind]
ACTIVITY_ERROR_KIND_UNSPECIFIED: ActivityErrorKind
ACTIVITY_ERROR_KIND_RETRYABLE: ActivityErrorKind
ACTIVITY_ERROR_KIND_TERMINAL: ActivityErrorKind

class DrainRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class WorkerToServer(_message.Message):
    __slots__ = ("register", "result", "heartbeat")
    REGISTER_FIELD_NUMBER: _ClassVar[int]
    RESULT_FIELD_NUMBER: _ClassVar[int]
    HEARTBEAT_FIELD_NUMBER: _ClassVar[int]
    register: RegisterWorker
    result: ActivityResult
    heartbeat: Heartbeat
    def __init__(self, register: _Optional[_Union[RegisterWorker, _Mapping]] = ..., result: _Optional[_Union[ActivityResult, _Mapping]] = ..., heartbeat: _Optional[_Union[Heartbeat, _Mapping]] = ...) -> None: ...

class ServerToWorker(_message.Message):
    __slots__ = ("task", "drain", "register_ack", "result_ack")
    TASK_FIELD_NUMBER: _ClassVar[int]
    DRAIN_FIELD_NUMBER: _ClassVar[int]
    REGISTER_ACK_FIELD_NUMBER: _ClassVar[int]
    RESULT_ACK_FIELD_NUMBER: _ClassVar[int]
    task: ActivityTask
    drain: DrainRequest
    register_ack: RegisterAck
    result_ack: ResultAck
    def __init__(self, task: _Optional[_Union[ActivityTask, _Mapping]] = ..., drain: _Optional[_Union[DrainRequest, _Mapping]] = ..., register_ack: _Optional[_Union[RegisterAck, _Mapping]] = ..., result_ack: _Optional[_Union[ResultAck, _Mapping]] = ...) -> None: ...

class RegisterAck(_message.Message):
    __slots__ = ("worker_id", "namespace", "heartbeat_window_ms")
    WORKER_ID_FIELD_NUMBER: _ClassVar[int]
    NAMESPACE_FIELD_NUMBER: _ClassVar[int]
    HEARTBEAT_WINDOW_MS_FIELD_NUMBER: _ClassVar[int]
    worker_id: int
    namespace: str
    heartbeat_window_ms: int
    def __init__(self, worker_id: _Optional[int] = ..., namespace: _Optional[str] = ..., heartbeat_window_ms: _Optional[int] = ...) -> None: ...

class ResultAck(_message.Message):
    __slots__ = ("workflow_id", "activity_id")
    WORKFLOW_ID_FIELD_NUMBER: _ClassVar[int]
    ACTIVITY_ID_FIELD_NUMBER: _ClassVar[int]
    workflow_id: _common_pb2.WorkflowId
    activity_id: _common_pb2.ActivityId
    def __init__(self, workflow_id: _Optional[_Union[_common_pb2.WorkflowId, _Mapping]] = ..., activity_id: _Optional[_Union[_common_pb2.ActivityId, _Mapping]] = ...) -> None: ...

class ActivityDescriptor(_message.Message):
    __slots__ = ("name", "input_schema_json", "output_schema_json")
    NAME_FIELD_NUMBER: _ClassVar[int]
    INPUT_SCHEMA_JSON_FIELD_NUMBER: _ClassVar[int]
    OUTPUT_SCHEMA_JSON_FIELD_NUMBER: _ClassVar[int]
    name: str
    input_schema_json: str
    output_schema_json: str
    def __init__(self, name: _Optional[str] = ..., input_schema_json: _Optional[str] = ..., output_schema_json: _Optional[str] = ...) -> None: ...

class RegisterWorker(_message.Message):
    __slots__ = ("namespaces", "activity_types", "task_queue", "node", "activities", "identity")
    NAMESPACES_FIELD_NUMBER: _ClassVar[int]
    ACTIVITY_TYPES_FIELD_NUMBER: _ClassVar[int]
    TASK_QUEUE_FIELD_NUMBER: _ClassVar[int]
    NODE_FIELD_NUMBER: _ClassVar[int]
    ACTIVITIES_FIELD_NUMBER: _ClassVar[int]
    IDENTITY_FIELD_NUMBER: _ClassVar[int]
    namespaces: _containers.RepeatedScalarFieldContainer[str]
    activity_types: _containers.RepeatedScalarFieldContainer[str]
    task_queue: str
    node: str
    activities: _containers.RepeatedCompositeFieldContainer[ActivityDescriptor]
    identity: str
    def __init__(self, namespaces: _Optional[_Iterable[str]] = ..., activity_types: _Optional[_Iterable[str]] = ..., task_queue: _Optional[str] = ..., node: _Optional[str] = ..., activities: _Optional[_Iterable[_Union[ActivityDescriptor, _Mapping]]] = ..., identity: _Optional[str] = ...) -> None: ...

class ActivityTask(_message.Message):
    __slots__ = ("workflow_id", "activity_id", "activity_type", "input", "attempt", "labels", "run_id", "completion_token", "idempotency_key")
    class LabelsEntry(_message.Message):
        __slots__ = ("key", "value")
        KEY_FIELD_NUMBER: _ClassVar[int]
        VALUE_FIELD_NUMBER: _ClassVar[int]
        key: str
        value: str
        def __init__(self, key: _Optional[str] = ..., value: _Optional[str] = ...) -> None: ...
    WORKFLOW_ID_FIELD_NUMBER: _ClassVar[int]
    ACTIVITY_ID_FIELD_NUMBER: _ClassVar[int]
    ACTIVITY_TYPE_FIELD_NUMBER: _ClassVar[int]
    INPUT_FIELD_NUMBER: _ClassVar[int]
    ATTEMPT_FIELD_NUMBER: _ClassVar[int]
    LABELS_FIELD_NUMBER: _ClassVar[int]
    RUN_ID_FIELD_NUMBER: _ClassVar[int]
    COMPLETION_TOKEN_FIELD_NUMBER: _ClassVar[int]
    IDEMPOTENCY_KEY_FIELD_NUMBER: _ClassVar[int]
    workflow_id: _common_pb2.WorkflowId
    activity_id: _common_pb2.ActivityId
    activity_type: str
    input: _common_pb2.Payload
    attempt: int
    labels: _containers.ScalarMap[str, str]
    run_id: _common_pb2.RunId
    completion_token: str
    idempotency_key: str
    def __init__(self, workflow_id: _Optional[_Union[_common_pb2.WorkflowId, _Mapping]] = ..., activity_id: _Optional[_Union[_common_pb2.ActivityId, _Mapping]] = ..., activity_type: _Optional[str] = ..., input: _Optional[_Union[_common_pb2.Payload, _Mapping]] = ..., attempt: _Optional[int] = ..., labels: _Optional[_Mapping[str, str]] = ..., run_id: _Optional[_Union[_common_pb2.RunId, _Mapping]] = ..., completion_token: _Optional[str] = ..., idempotency_key: _Optional[str] = ...) -> None: ...

class ActivityResult(_message.Message):
    __slots__ = ("workflow_id", "activity_id", "result", "error", "run_id", "completion_token")
    WORKFLOW_ID_FIELD_NUMBER: _ClassVar[int]
    ACTIVITY_ID_FIELD_NUMBER: _ClassVar[int]
    RESULT_FIELD_NUMBER: _ClassVar[int]
    ERROR_FIELD_NUMBER: _ClassVar[int]
    RUN_ID_FIELD_NUMBER: _ClassVar[int]
    COMPLETION_TOKEN_FIELD_NUMBER: _ClassVar[int]
    workflow_id: _common_pb2.WorkflowId
    activity_id: _common_pb2.ActivityId
    result: _common_pb2.Payload
    error: ActivityError
    run_id: _common_pb2.RunId
    completion_token: str
    def __init__(self, workflow_id: _Optional[_Union[_common_pb2.WorkflowId, _Mapping]] = ..., activity_id: _Optional[_Union[_common_pb2.ActivityId, _Mapping]] = ..., result: _Optional[_Union[_common_pb2.Payload, _Mapping]] = ..., error: _Optional[_Union[ActivityError, _Mapping]] = ..., run_id: _Optional[_Union[_common_pb2.RunId, _Mapping]] = ..., completion_token: _Optional[str] = ...) -> None: ...

class ActivityError(_message.Message):
    __slots__ = ("kind", "message", "details")
    KIND_FIELD_NUMBER: _ClassVar[int]
    MESSAGE_FIELD_NUMBER: _ClassVar[int]
    DETAILS_FIELD_NUMBER: _ClassVar[int]
    kind: ActivityErrorKind
    message: str
    details: _common_pb2.Payload
    def __init__(self, kind: _Optional[_Union[ActivityErrorKind, str]] = ..., message: _Optional[str] = ..., details: _Optional[_Union[_common_pb2.Payload, _Mapping]] = ...) -> None: ...

class Heartbeat(_message.Message):
    __slots__ = ("workflow_id", "activity_id", "progress")
    WORKFLOW_ID_FIELD_NUMBER: _ClassVar[int]
    ACTIVITY_ID_FIELD_NUMBER: _ClassVar[int]
    PROGRESS_FIELD_NUMBER: _ClassVar[int]
    workflow_id: _common_pb2.WorkflowId
    activity_id: _common_pb2.ActivityId
    progress: _common_pb2.Payload
    def __init__(self, workflow_id: _Optional[_Union[_common_pb2.WorkflowId, _Mapping]] = ..., activity_id: _Optional[_Union[_common_pb2.ActivityId, _Mapping]] = ..., progress: _Optional[_Union[_common_pb2.Payload, _Mapping]] = ...) -> None: ...
