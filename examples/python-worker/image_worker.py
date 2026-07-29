"""A single-file Python worker: real image measurement, no framework.

This is the shape the Python worker story is for — a script that does actual
data handling, decorated and served, with nothing in it about gRPC, streams,
reconnection, or the wire. Everything below the decorator is ordinary Python
that could run standalone.

It reads dimensions out of each file's own header rather than trusting the
extension, using only the standard library, so the example has no dependency
to install and nothing to go stale. Swapping `_measure` for a real model call
is the whole extension path: the worker plumbing does not change.

Serve it against `image_report.awl`, whose `worker python_box` block declares
the queue and the typed action surface this file must satisfy.
"""

from __future__ import annotations

import struct
from dataclasses import dataclass
from pathlib import Path

from aion_worker import TerminalError, activity


class UnreadableImage(TerminalError):
    """The bytes are not an image this worker can measure.

    Terminal by inheritance rather than by convention: the file's contents are
    a fact that a retry cannot change, and saying so in the type means every
    raise site classifies correctly without having to remember to.
    """


@dataclass
class InspectImage:
    """The dispatch's arguments, keyed by the AWL action's parameter names.

    An activity's input arrives as a JSON object keyed by parameter name, so a
    single-parameter action sends `{"path": "..."}` rather than a bare string.
    """

    path: str


@dataclass
class ImageFacts:
    """What was actually measured. Mirrors the `ImageFacts` record in the AWL.

    Both sides of this activity are declared types rather than bare dicts, and
    that is not decoration. The server checks a worker's advertised OUTPUT
    schema covariantly against the deployed contract, so a handler annotated
    `-> dict` cannot describe what it produces and is refused. Annotating the
    shape is what makes the worker admissible.
    """

    path: str
    format: str
    width: int
    height: int
    bytes: int


def _png_dimensions(data: bytes) -> tuple[int, int]:
    """Read width and height from a PNG's IHDR chunk.

    IHDR is required by the spec to be the first chunk, so its position is
    fixed: an 8-byte signature, a 4-byte length, the 4-byte type `IHDR`, then
    two big-endian uint32s.
    """

    if len(data) < 24:
        raise UnreadableImage("PNG signature present but the header is truncated")
    if data[12:16] != b"IHDR":
        raise UnreadableImage("PNG's first chunk is not IHDR, so the header is not readable")
    width, height = struct.unpack(">II", data[16:24])
    return width, height


def _jpeg_dimensions(data: bytes) -> tuple[int, int]:
    """Walk a JPEG's marker segments to the frame header that carries size.

    JPEG has no fixed-offset header. Dimensions live in a Start-Of-Frame marker
    (0xC0-0xCF, excluding the four that are not frame headers), which can sit
    behind any number of variable-length segments, so the segments have to be
    stepped through rather than indexed into.
    """

    offset = 2  # past the SOI marker
    limit = len(data)
    while offset < limit:
        if data[offset] != 0xFF:
            raise UnreadableImage(f"expected a JPEG marker at byte {offset}")
        while offset < limit and data[offset] == 0xFF:
            offset += 1  # fill bytes are legal padding before a marker
        if offset >= limit:
            break
        marker = data[offset]
        offset += 1
        if marker in (0xD8, 0x01) or 0xD0 <= marker <= 0xD7:
            continue  # standalone markers carry no length field
        if offset + 2 > limit:
            raise UnreadableImage("JPEG segment length runs past the end of the file")
        (length,) = struct.unpack(">H", data[offset : offset + 2])
        if length < 2:
            raise UnreadableImage(f"JPEG segment at byte {offset} declares an impossible length")
        # SOF0-SOF15 carry the frame header, except DHT/JPG/DAC which reuse the range.
        if 0xC0 <= marker <= 0xCF and marker not in (0xC4, 0xC8, 0xCC):
            if offset + 7 > limit:
                raise UnreadableImage("JPEG frame header is truncated")
            height, width = struct.unpack(">HH", data[offset + 3 : offset + 7])
            return width, height
        offset += length
    raise UnreadableImage("JPEG has no start-of-frame marker, so it carries no dimensions")


def _gif_dimensions(data: bytes) -> tuple[int, int]:
    """Read the GIF logical screen descriptor, which is little-endian."""

    if len(data) < 10:
        raise UnreadableImage("GIF signature present but the screen descriptor is truncated")
    width, height = struct.unpack("<HH", data[6:10])
    return width, height


def _measure(data: bytes) -> tuple[str, int, int]:
    """Identify the format from its signature and measure it."""

    if data.startswith(b"\x89PNG\r\n\x1a\n"):
        width, height = _png_dimensions(data)
        return "png", width, height
    if data.startswith(b"\xff\xd8"):
        width, height = _jpeg_dimensions(data)
        return "jpeg", width, height
    if data.startswith((b"GIF87a", b"GIF89a")):
        width, height = _gif_dimensions(data)
        return "gif", width, height
    raise UnreadableImage(
        "the file's leading bytes match no format this worker reads (png, jpeg, gif)"
    )


@activity(name="inspect_image")
def inspect_image(input: InspectImage) -> ImageFacts:
    """Measure the image at the requested path and report what is in it.

    A file that is not an image will never become one and a missing path will
    not appear on a retry, so both are raised as `TerminalError`. An I/O error
    reaching the file is genuinely different — a busy or briefly unavailable
    volume can succeed on a second attempt — so it is left to propagate under
    the SDK's retryable default rather than being flattened into the same class.
    """

    location = Path(input.path)
    try:
        data = location.read_bytes()
    except FileNotFoundError as error:
        raise TerminalError(f"no file at {input.path}") from error
    except IsADirectoryError as error:
        raise TerminalError(f"{input.path} is a directory, not an image") from error

    image_format, width, height = _measure(data)
    return ImageFacts(
        path=str(location),
        format=image_format,
        width=width,
        height=height,
        bytes=len(data),
    )
