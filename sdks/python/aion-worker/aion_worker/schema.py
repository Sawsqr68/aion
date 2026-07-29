"""JSON Schema derivation from handler type hints.

`RegisterWorker.activities` is how a worker proves it serves the typed action
surface a deployed package requires. The Rust SDK derives those schemas from
the concrete serde types through schemars; Python's equivalent evidence is the
handler's type hints, so this module turns a hint into the schema it — and only
it — actually proves.

Shapes here deliberately mirror what AWL emits (``crates/aion-awl/src/emitter/
artifact.rs``), because an AWL ``worker`` block is the realistic contract a
Python worker registers against and the server compares the two structurally.
That is also why no ``$schema``, ``$defs`` or ``$ref`` appears: subschemas are
inlined, matching the Rust generator's ``inline_subschemas`` setting.

**No emitted schema ever claims more than the hint proves.** ``additionalProperties``
is never emitted: asserting a closed object would be a claim about values the
hints say nothing about, and on the input side it would make the worker refuse
contracts it can in fact serve.
"""

from __future__ import annotations

import dataclasses
import enum
import types
from collections.abc import Mapping, Sequence
from typing import Any, Final, Literal, Union, get_args, get_origin, get_type_hints

__all__ = [
    "PERMISSIVE_SCHEMA",
    "SchemaDerivationError",
    "derive_input_schema",
    "derive_output_schema",
    "schema_for_annotation",
]

PERMISSIVE_SCHEMA: Final[bool] = True
"""JSON Schema ``true`` — the schema every value satisfies.

Honest for an INPUT whose hint proves nothing: the worker really will accept
whatever arrives, because the codec hands an unconstrained hint straight to the
handler without coercion. Never honest for an OUTPUT, where it would claim the
worker may emit anything; see :func:`derive_output_schema`.
"""

_NULL_SCHEMA: Final[dict[str, Any]] = {"type": "null"}

# Scalars whose JSON representation the codec actually moves. `bool` precedes
# `int` because `bool` is an `int` subclass in Python and JSON keeps them apart.
_SCALAR_SCHEMAS: Final[tuple[tuple[type, dict[str, Any]], ...]] = (
    (bool, {"type": "boolean"}),
    (int, {"type": "integer"}),
    (float, {"type": "number"}),
    (str, {"type": "string"}),
)


class SchemaDerivationError(TypeError):
    """A type hint proves nothing that can be honestly advertised.

    Raised for annotations with no faithful JSON Schema: ``Any``, ``object``,
    an arbitrary class the codec cannot move, a self-referential type, or a
    container of any of those. Callers choose what to do with it by direction —
    see :func:`derive_input_schema` and :func:`derive_output_schema`.
    """


def derive_input_schema(annotation: object) -> object:
    """Return the schema a handler's input hint proves, or the permissive one.

    Input compatibility is CONTRAVARIANT: the server checks the deployed
    contract's input schema is a subset of the worker's, so a worker that
    accepts more than the contract sends is admissible. A hint that proves
    nothing therefore has an honest answer — ``true``, "this handler accepts
    whatever arrives" — which is exactly what the codec does with an
    unconstrained hint. Nothing is claimed that is not true, so there is no
    reason to refuse.
    """

    try:
        return schema_for_annotation(annotation)
    except SchemaDerivationError:
        return PERMISSIVE_SCHEMA


def derive_output_schema(annotation: object) -> object:
    """Return the schema a handler's return hint proves, or raise.

    Output compatibility is COVARIANT: the server checks the worker's output
    schema is a subset of the deployed contract's, so a worker may only promise
    what the contract already promises. That makes the permissive schema
    unusable here — ``true`` claims the handler may emit anything, which no real
    contract permits, and advertising it produces a registration refused by the
    server with a schema diff far from its cause.

    ``false`` ("emits nothing") would slip past the server's subset check, and
    that is precisely why it must not be used: it would buy admission with a lie
    in the opposite direction.

    So the only honest options are a real schema or a refusal, and refusing HERE
    is the useful one: a local error naming the activity and its return type
    tells the author what to annotate, which no remote ``FailedPrecondition``
    can.
    """

    return schema_for_annotation(annotation)


def schema_for_annotation(annotation: object) -> object:
    """Derive the JSON Schema a single annotation proves.

    Raises:
        SchemaDerivationError: the annotation has no faithful schema.
    """

    return _schema(annotation, ())


def _schema(annotation: object, visiting: tuple[type, ...]) -> object:
    if annotation is None or annotation is type(None):
        return dict(_NULL_SCHEMA)
    if annotation is Any or annotation is object:
        raise SchemaDerivationError(f"{_render(annotation)} proves nothing about the value's shape")

    origin = get_origin(annotation)
    if origin is Literal:
        return _literal_schema(get_args(annotation))
    if origin is Union or origin is types.UnionType:
        return _union_schema(get_args(annotation), visiting)
    if origin is not None:
        return _generic_schema(annotation, origin, get_args(annotation), visiting)

    if isinstance(annotation, type):
        return _class_schema(annotation, visiting)
    raise SchemaDerivationError(f"{_render(annotation)} is not a type this SDK can express as JSON Schema")


def _class_schema(annotation: type, visiting: tuple[type, ...]) -> object:
    for scalar, schema in _SCALAR_SCHEMAS:
        if annotation is scalar:
            return dict(schema)
    if issubclass(annotation, enum.Enum):
        return _enum_schema(annotation)
    if dataclasses.is_dataclass(annotation):
        return _dataclass_schema(annotation, visiting)
    # Bare containers: the element type is unstated, so only the container kind
    # is proven. `dict` is object-shaped, `list`/`tuple` are array-shaped.
    if annotation is dict:
        return {"type": "object"}
    if annotation in (list, tuple):
        return {"type": "array"}
    raise SchemaDerivationError(
        f"{_render(annotation)} is not a JSON-shaped type; annotate with a dataclass, "
        "a JSON scalar, or a container of those"
    )


def _enum_schema(annotation: type[enum.Enum]) -> dict[str, Any]:
    """Schema for an enum whose members are JSON strings.

    Only string-valued enums are expressible: the codec encodes a member as its
    value, and a member whose value is not a JSON scalar has nothing to encode
    to. This matches the shape AWL emits for a declared enum.
    """

    values = [member.value for member in annotation]
    if not values:
        raise SchemaDerivationError(f"enum {_render(annotation)} declares no members")
    if not all(isinstance(value, str) for value in values):
        raise SchemaDerivationError(
            f"enum {_render(annotation)} has non-string member values, which have no "
            "JSON representation this SDK can encode"
        )
    return {"type": "string", "enum": values}


def _literal_schema(values: tuple[object, ...]) -> dict[str, Any]:
    if not values:
        raise SchemaDerivationError("Literal[] declares no values")
    for value in values:
        if not isinstance(value, (str, int, float, bool)) and value is not None:
            raise SchemaDerivationError(f"Literal value {value!r} is not a JSON scalar")
    literals = [value.value if isinstance(value, enum.Enum) else value for value in values]
    if all(isinstance(value, str) for value in literals):
        return {"type": "string", "enum": literals}
    return {"enum": literals}


def _union_schema(args: tuple[object, ...], visiting: tuple[type, ...]) -> dict[str, Any]:
    """Schema for a union, in the ``anyOf`` shape AWL emits for `Option`.

    ``Optional[T]`` is the common case and produces ``anyOf: [T, null]``
    verbatim, so it compares equal to an AWL optional field rather than merely
    being semantically equivalent to it.
    """

    branches = [_schema(argument, visiting) for argument in args]
    return {"anyOf": branches}


def _generic_schema(
    annotation: object,
    origin: object,
    args: tuple[object, ...],
    visiting: tuple[type, ...],
) -> dict[str, Any]:
    if origin in (list, set, frozenset) or (
        isinstance(origin, type) and origin is not tuple and _is_sequence_origin(origin)
    ):
        if origin in (set, frozenset):
            raise SchemaDerivationError(f"{_render(annotation)} has no JSON representation; JSON has no set type")
        return {"type": "array", "items": _schema(args[0], visiting)}
    if origin is tuple:
        # Only the homogeneous `tuple[T, ...]` spelling is an array of one type.
        # A fixed-length tuple would need `prefixItems`, which the server's
        # structural comparison does not interpret, so advertising it would
        # claim something the matcher cannot check.
        if len(args) == 2 and args[1] is Ellipsis:
            return {"type": "array", "items": _schema(args[0], visiting)}
        raise SchemaDerivationError(
            f"{_render(annotation)} is a fixed-length tuple; use `tuple[T, ...]` or a "
            "dataclass to describe a positional record"
        )
    if origin is dict or (isinstance(origin, type) and issubclass(origin, Mapping)):
        key, value = args[0], args[1]
        if key is not str:
            raise SchemaDerivationError(f"{_render(annotation)} has non-string keys; JSON objects are keyed by string")
        try:
            values = _schema(value, visiting)
        except SchemaDerivationError:
            # `dict[str, Any]` proves object-ness and nothing about its values.
            return {"type": "object"}
        return {"type": "object", "additionalProperties": values}
    raise SchemaDerivationError(f"{_render(annotation)} is not a JSON-shaped generic this SDK can express")


def _is_sequence_origin(origin: type) -> bool:
    return issubclass(origin, Sequence) and not issubclass(origin, (str, bytes))


def _dataclass_schema(annotation: type, visiting: tuple[type, ...]) -> dict[str, Any]:
    if annotation in visiting:
        raise SchemaDerivationError(
            f"dataclass {_render(annotation)} is self-referential; a recursive shape "
            "cannot be inlined into a single schema"
        )
    hints = _resolved_hints(annotation)
    properties: dict[str, object] = {}
    required: list[str] = []
    nested = (*visiting, annotation)
    for field in dataclasses.fields(annotation):
        if not field.init:
            # Not constructible from the payload, so it is not part of the wire
            # shape either. Advertising it would describe a field no caller can
            # send and no decode can fill.
            continue
        if field.name not in hints:
            raise SchemaDerivationError(
                f"dataclass {_render(annotation)} field {field.name!r} has no resolvable type annotation"
            )
        properties[field.name] = _schema(hints[field.name], nested)
        if _field_is_required(field, hints[field.name]):
            required.append(field.name)
    return {"type": "object", "properties": properties, "required": required}


def _field_is_required(field: dataclasses.Field[Any], annotation: object) -> bool:
    """A field is required unless it is optional or carries its own default.

    Both directions of the server's comparison read ``required`` exactly, so
    this must be neither over- nor under-stated: over-stating refuses contracts
    the worker can serve, under-stating claims outputs it may not produce.
    """

    if field.default is not dataclasses.MISSING:
        return False
    if field.default_factory is not dataclasses.MISSING:
        return False
    return not _is_optional(annotation)


def _is_optional(annotation: object) -> bool:
    if get_origin(annotation) not in (Union, types.UnionType):
        return False
    return any(argument is type(None) for argument in get_args(annotation))


def _resolved_hints(annotation: type) -> dict[str, Any]:
    """Resolve a dataclass's annotations, which `from __future__` leaves as strings."""

    try:
        return get_type_hints(annotation)
    except Exception as error:  # noqa: BLE001 - any resolution failure is the same refusal
        raise SchemaDerivationError(
            f"dataclass {_render(annotation)} has annotations that cannot be resolved: {error}"
        ) from error


def _render(annotation: object) -> str:
    if annotation is type(None):
        return "None"
    if isinstance(annotation, type):
        return annotation.__qualname__
    return str(annotation).replace("typing.", "")
