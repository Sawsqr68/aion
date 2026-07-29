"""Payload <-> annotated-type conversion for activity handlers.

A handler's type hints are the same evidence twice over: :mod:`aion_worker.schema`
turns them into the schemas a worker advertises, and this module makes the
runtime match that advertisement. A worker that advertises
``ImageFacts`` and then hands its handler a raw ``dict`` — or fails to serialise
what the handler returned — has advertised something it does not do.

Both directions refuse rather than coerce. A payload that does not fit the
annotated shape is a TERMINAL failure naming the exact field: the value came off
the wire and no retry will change it. A result the codec cannot encode is
likewise terminal, because a handler returning an unserialisable value is a code
defect that a retry cannot fix.
"""

from __future__ import annotations

import dataclasses
import enum
import types
from collections.abc import Mapping, Sequence
from typing import Any, Literal, Union, get_args, get_origin, get_type_hints

__all__ = [
    "PayloadDecodeError",
    "ResultEncodeError",
    "decode_activity_input",
    "encode_activity_output",
]


class PayloadDecodeError(ValueError):
    """An activity input payload does not fit its handler's annotated type."""


class ResultEncodeError(TypeError):
    """A handler returned a value with no JSON representation."""


def decode_activity_input(annotation: object, value: object, *, path: str = "input") -> object:
    """Convert a decoded JSON payload into the handler's annotated input type.

    Annotations the schema layer could not express (``Any``, ``dict``, an
    arbitrary class) pass their value through untouched: the worker advertised
    the permissive input schema for exactly those, so accepting whatever
    arrived is what it promised.

    Raises:
        PayloadDecodeError: naming the field path that did not fit.
    """

    return _decode(annotation, value, path, ())


def encode_activity_output(annotation: object, value: object, *, path: str = "result") -> object:
    """Convert a handler's return value into JSON-native data.

    The annotation guides the walk but the VALUE decides: a dataclass instance
    is expanded whether or not the hint said so, so a handler whose hint is
    looser than its return still encodes correctly.

    Raises:
        ResultEncodeError: naming the field path that has no JSON form.
    """

    return _encode(value, path, ())


# --------------------------------------------------------------------------- #
# decode
# --------------------------------------------------------------------------- #


def _decode(annotation: object, value: object, path: str, visiting: tuple[type, ...]) -> object:
    if annotation is None or annotation is type(None):
        if value is not None:
            raise PayloadDecodeError(f"{path} must be null, got {_kind(value)}")
        return None

    origin = get_origin(annotation)
    if origin is Literal:
        return _decode_literal(get_args(annotation), value, path)
    if origin is Union or origin is types.UnionType:
        return _decode_union(get_args(annotation), value, path, visiting)
    if origin is not None:
        return _decode_generic(annotation, origin, get_args(annotation), value, path, visiting)

    if isinstance(annotation, type):
        return _decode_class(annotation, value, path, visiting)
    # Not a type at all (`Any`, a bare string annotation that never resolved):
    # nothing was advertised about it, so nothing is enforced.
    return value


def _decode_class(annotation: type, value: object, path: str, visiting: tuple[type, ...]) -> object:
    if annotation is bool:
        if not isinstance(value, bool):
            raise PayloadDecodeError(f"{path} must be a boolean, got {_kind(value)}")
        return value
    if annotation is int:
        # `bool` is an `int` subclass in Python; JSON keeps them apart and so
        # does every schema this SDK emits.
        if isinstance(value, bool) or not isinstance(value, int):
            raise PayloadDecodeError(f"{path} must be an integer, got {_kind(value)}")
        return value
    if annotation is float:
        if isinstance(value, bool) or not isinstance(value, (int, float)):
            raise PayloadDecodeError(f"{path} must be a number, got {_kind(value)}")
        return float(value)
    if annotation is str:
        if not isinstance(value, str):
            raise PayloadDecodeError(f"{path} must be a string, got {_kind(value)}")
        return value
    if issubclass(annotation, enum.Enum):
        return _decode_enum(annotation, value, path)
    if dataclasses.is_dataclass(annotation):
        return _decode_dataclass(annotation, value, path, visiting)
    # Bare `dict`/`list`, or a class the schema layer refused: the advertised
    # input schema was permissive, so the raw value is exactly what was promised.
    return value


def _decode_enum(annotation: type[enum.Enum], value: object, path: str) -> object:
    try:
        return annotation(value)
    except ValueError:
        allowed = ", ".join(repr(member.value) for member in annotation)
        raise PayloadDecodeError(f"{path} must be one of {allowed}, got {value!r}") from None


def _decode_literal(values: tuple[object, ...], value: object, path: str) -> object:
    for allowed in values:
        # `1 == True` in Python but not in JSON, so the types must agree too.
        if allowed == value and isinstance(value, type(allowed)):
            return value
    rendered = ", ".join(repr(allowed) for allowed in values)
    raise PayloadDecodeError(f"{path} must be one of {rendered}, got {value!r}")


def _decode_union(args: tuple[object, ...], value: object, path: str, visiting: tuple[type, ...]) -> object:
    failures: list[str] = []
    for branch in args:
        try:
            return _decode(branch, value, path, visiting)
        except PayloadDecodeError as error:
            failures.append(str(error))
    raise PayloadDecodeError(f"{path} matched no branch of the union: " + "; ".join(failures))


def _decode_generic(
    annotation: object,
    origin: object,
    args: tuple[object, ...],
    value: object,
    path: str,
    visiting: tuple[type, ...],
) -> object:
    if origin is dict or (isinstance(origin, type) and issubclass(origin, Mapping)):
        if not isinstance(value, Mapping):
            raise PayloadDecodeError(f"{path} must be an object, got {_kind(value)}")
        return {key: _decode(args[1], item, f"{path}.{key}", visiting) for key, item in value.items()}
    if origin is tuple:
        items = _decode_sequence(args[0], value, path, visiting)
        return tuple(items)
    if origin in (list, set, frozenset) or (isinstance(origin, type) and _is_sequence_origin(origin)):
        items = _decode_sequence(args[0], value, path, visiting)
        return origin(items) if origin in (set, frozenset) else items
    return value


def _decode_sequence(element: object, value: object, path: str, visiting: tuple[type, ...]) -> list[object]:
    if isinstance(value, (str, bytes)) or not isinstance(value, Sequence):
        raise PayloadDecodeError(f"{path} must be an array, got {_kind(value)}")
    return [_decode(element, item, f"{path}[{index}]", visiting) for index, item in enumerate(value)]


def _decode_dataclass(annotation: type, value: object, path: str, visiting: tuple[type, ...]) -> object:
    if annotation in visiting:
        raise PayloadDecodeError(
            f"{path} has a self-referential type ({annotation.__qualname__}) that cannot be decoded"
        )
    if not isinstance(value, Mapping):
        raise PayloadDecodeError(
            f"{path} must be an object with the fields of {annotation.__qualname__}, got {_kind(value)}"
        )
    hints = get_type_hints(annotation)
    fields = [field for field in dataclasses.fields(annotation) if field.init]
    known = {field.name for field in fields}
    unknown = sorted(set(value) - known)
    if unknown:
        # AWL declares its action inputs closed (`additionalProperties: false`),
        # so an unexpected key means the caller and the worker disagree about
        # the shape. Dropping it silently would hide that disagreement until it
        # mattered.
        raise PayloadDecodeError(
            f"{path} has no field {unknown[0]!r} on {annotation.__qualname__} "
            f"(known fields: {', '.join(sorted(known))})"
        )
    nested = (*visiting, annotation)
    arguments: dict[str, object] = {}
    for field in fields:
        if field.name in value:
            arguments[field.name] = _decode(
                hints.get(field.name, Any), value[field.name], f"{path}.{field.name}", nested
            )
        elif field.default is dataclasses.MISSING and field.default_factory is dataclasses.MISSING:
            raise PayloadDecodeError(f"{path} is missing required field {field.name!r} of {annotation.__qualname__}")
    return annotation(**arguments)


# --------------------------------------------------------------------------- #
# encode
# --------------------------------------------------------------------------- #


def _encode(value: object, path: str, visiting: tuple[int, ...]) -> object:
    if value is None or isinstance(value, (bool, int, float, str)):
        if isinstance(value, float) and value != value:  # NaN
            raise ResultEncodeError(f"{path} is NaN, which JSON cannot represent")
        if isinstance(value, float) and value in (float("inf"), float("-inf")):
            raise ResultEncodeError(f"{path} is infinite, which JSON cannot represent")
        return value
    if isinstance(value, enum.Enum):
        return _encode(value.value, path, visiting)
    if id(value) in visiting:
        raise ResultEncodeError(f"{path} is part of a reference cycle and cannot be encoded")
    nested = (*visiting, id(value))
    if dataclasses.is_dataclass(value) and not isinstance(value, type):
        return {
            field.name: _encode(getattr(value, field.name), f"{path}.{field.name}", nested)
            for field in dataclasses.fields(value)
        }
    if isinstance(value, Mapping):
        encoded: dict[str, object] = {}
        for key, item in value.items():
            if not isinstance(key, str):
                raise ResultEncodeError(f"{path} has a non-string key {key!r}; JSON objects are keyed by string")
            encoded[key] = _encode(item, f"{path}.{key}", nested)
        return encoded
    if isinstance(value, (list, tuple, set, frozenset)):
        return [_encode(item, f"{path}[{index}]", nested) for index, item in enumerate(value)]
    raise ResultEncodeError(
        f"{path} is a {type(value).__qualname__}, which has no JSON representation; "
        "return a dataclass, a JSON scalar, or a container of those"
    )


def _is_sequence_origin(origin: type) -> bool:
    return issubclass(origin, Sequence) and not issubclass(origin, (str, bytes))


def _kind(value: object) -> str:
    """Name a value's JSON kind, which is what the author's schema talks about."""

    if value is None:
        return "null"
    if isinstance(value, bool):
        return "a boolean"
    if isinstance(value, int):
        return "an integer"
    if isinstance(value, float):
        return "a number"
    if isinstance(value, str):
        return "a string"
    if isinstance(value, Mapping):
        return "an object"
    if isinstance(value, Sequence):
        return "an array"
    return f"a {type(value).__qualname__}"
