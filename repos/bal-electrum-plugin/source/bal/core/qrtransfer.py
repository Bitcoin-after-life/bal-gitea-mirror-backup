"""
bal.core.qrtransfer
===================

GUI-free helpers for moving BAL will data between devices via QR codes or
the Electrum ``audio_modem`` plugin (see ``PLAN_QR_TRANSFER.md``).

Scope
-----
* converts will transactions into a compact ``transfer_string``
  (newline-joined serialized transactions, optionally zlib + base64
  compressed);
* splits that string into fixed-size ``BALQR1|N|i|flags|payload`` frames for
  multi-QR export, and reassembles/validates them on import.

The audio-modem channel deliberately bypasses the framing helpers here
(PLAN_QR_TRANSFER.md section 4.4): its transport compresses internally and
carries the whole transfer string in a single blob, so callers only use
:func:`encode_transfer` / :func:`decode_transfer`.

This module never imports Qt or any Electrum GUI code (house rule).
"""

from __future__ import annotations

import base64
import zlib

MAGIC = "BALQR"
VERSION = 1
FLAG_COMPRESSED = "Z"

# 4 standard presets (label, payload budget in bytes per QR). Ordered from
# low-resolution cameras to high-resolution cameras (owner decision D5).
CHUNK_PRESETS = (
    ("Small - ~150 bytes/QR (low-res cameras)", 150),
    ("Medium - ~400 bytes/QR", 400),
    ("Large - ~900 bytes/QR", 900),
    ("XL - ~1800 bytes/QR (high-res cameras)", 1800),
)

# Smallest allowed payload budget per frame, below which the frame header
# could consume the whole budget.
MIN_CHUNK_SIZE = 40

_FRAME_MAGIC = MAGIC + str(VERSION)


class QrTransferError(ValueError):
    """Base error for will QR / audio transfer processing."""


class MissingFramesError(QrTransferError):
    """Some frame indices of a multi-QR transfer are missing."""

    def __init__(self, missing):
        self.missing = list(missing)
        super().__init__("Missing QR frames: {}".format(self.missing))


class InconsistentTotalError(QrTransferError):
    """Frames disagree about the advertised frame total."""


def encode_transfer(tx_strings, compress=False):
    """Join serialized transaction strings into a transfer string.

    ``compress=True`` wraps the joined text in zlib + base64 (ASCII-safe) so
    the whole bundle shrinks before being printed/scanned. The optional flags
    of the frame header let the importer reverse this automatically.
    """
    return __compress("\n".join(tx_strings), enabled=compress)


def decode_transfer(transfer_string, compressed):
    """Inverse of :func:`encode_transfer`.

    Returns the list of serialized transaction strings; empty frames are
    dropped so a trailing newline (or an empty payload) cannot produce an
    empty trailing element.
    """
    text = __decompress(transfer_string, enabled=compressed)
    return [part for part in text.split("\n") if part]


def split_frames(transfer_string, chunk_size, compressed=False):
    """Split ``transfer_string`` into full ``BALQR`` frames.

    Every returned frame is at most ``chunk_size`` characters long (header
    included). ``compressed`` propagates the ``Z`` flag into every frame so
    the importer knows how to reverse the encoding.

    Raises :class:`QrTransferError` when ``chunk_size`` is too small to hold
    the header plus any payload.
    """
    flags = FLAG_COMPRESSED if compressed else ""
    total = __compute_total(len(transfer_string), chunk_size, flags)
    frames = []
    pos = 0
    length = len(transfer_string)
    for index in range(1, total + 1):
        overhead = len(__frame_header(total, index, flags))
        budget = chunk_size - overhead
        end = min(pos + budget, length)
        frames.append(__build_frame(total, index, flags, transfer_string[pos:end]))
        pos = end
        if pos >= length:
            break
    if pos < length:
        # __compute_total guarantees this cannot happen; keep a safety net.
        raise QrTransferError("internal error: frames did not cover the transfer string")
    return frames


def parse_frame(frame):
    """Parse a single frame.

    Returns ``(total, index, compressed: bool, payload: str)``. Raises
    :class:`QrTransferError` on malformed input (bad magic/version, wrong
    arity, non-integer or out-of-range frame numbers, unknown flags).
    """
    parts = frame.split("|", maxsplit=4)
    if len(parts) != 5:
        raise QrTransferError("Not a BAL will QR (bad frame structure)")
    magic_seen, total_s, index_s, flags, payload = parts
    if magic_seen != _FRAME_MAGIC:
        raise QrTransferError("Not a BAL will QR (unknown magic/version)")
    try:
        total = int(total_s)
        index = int(index_s)
    except ValueError as e:
        raise QrTransferError("Not a BAL will QR (bad frame numbers)") from e
    if total < 1 or not 1 <= index <= total:
        raise QrTransferError("Not a BAL will QR (frame numbering out of range)")
    if flags not in ("", FLAG_COMPRESSED):
        raise QrTransferError("Not a BAL will QR (unknown flags)")
    return total, index, flags == FLAG_COMPRESSED, payload


def assemble(frames, total):
    """Concatenate frame payloads back into a transfer string.

    ``frames`` maps 1-based index -> payload. Every index ``1..total`` must
    be present (else :class:`MissingFramesError`) and no index may exceed
    ``total`` (else :class:`InconsistentTotalError`).
    """
    if total < 1:
        raise QrTransferError("invalid frame total")
    missing = [index for index in range(1, total + 1) if index not in frames]
    if missing:
        raise MissingFramesError(missing)
    extra = [index for index in frames if index > total]
    if extra:
        raise InconsistentTotalError()
    return "".join(frames[index] for index in range(1, total + 1))


def preset_index_for_chunk_size(chunk_size):
    """Return the :data:`CHUNK_PRESETS` index whose budget best matches a size."""
    best, best_diff = 0, abs(chunk_size - CHUNK_PRESETS[0][1])
    for index, (_label, budget) in enumerate(CHUNK_PRESETS):
        diff = abs(chunk_size - budget)
        if diff < best_diff:
            best, best_diff = index, diff
    return best


# --------------------------------------------------------------------------- #
# Internals
# --------------------------------------------------------------------------- #

def __compress(text, *, enabled):
    if not enabled:
        return text
    return base64.b64encode(zlib.compress(text.encode("utf-8"))).decode("ascii")


def __decompress(text, *, enabled):
    if not enabled:
        return text
    return zlib.decompress(base64.b64decode(text.encode("ascii"))).decode("utf-8")


def __frame_header(total, index, flags):
    return "{}|{}|{}|{}|".format(_FRAME_MAGIC, total, index, flags)


def __build_frame(total, index, flags, payload):
    return __frame_header(total, index, flags) + payload


def __compute_total(transfer_len, chunk_size, flags):
    """Smallest frame count whose budget covers the whole transfer string.

    The budget shrinks as ``total`` gains digits (wider header), so the count
    is recomputed iteratively until it converges.
    """
    if chunk_size < MIN_CHUNK_SIZE:
        raise QrTransferError(
            "chunk size too small to hold a BAL QR frame: {}".format(chunk_size)
        )
    total = 1
    while True:
        overhead = len(__frame_header(total, total, flags))
        budget = chunk_size - overhead
        if budget <= 0:
            raise QrTransferError(
                "chunk size too small for the BAL QR frame header: {}".format(chunk_size)
            )
        if transfer_len <= budget * total:
            return total
        total += 1
