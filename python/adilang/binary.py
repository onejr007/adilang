"""
adilang/binary.py — Standalone Binary Stream Encoding Engine (MessagePack + CBOR).
===================================================================================
Mendefinisikan encoding biner untuk ADILang IR:
- Text: canonical representation (UTF-8)
- MessagePack: compact binary (media type: application/x-adilang+msgpack)
- CBOR: IETF standard binary (media type: application/x-adilang+cbor)

Preserves 100% semantic equivalence dengan text encoding (round-trip: text -> binary -> text = identity).

Lead Developer: BAGAS ADI PRATAMA S,Kom.
"""
from __future__ import annotations

import json
from typing import Any, Dict, List, Optional, Union
from adilang.protocol import parse_adilang

MEDIA_TYPE_TEXT = "text/x-adilang"
MEDIA_TYPE_MSGPACK = "application/x-adilang+msgpack"
MEDIA_TYPE_CBOR = "application/x-adilang+cbor"
MEDIA_TYPE_JSON = "application/x-adilang+json"


def text_to_ir(text: str) -> Dict[str, Any]:
    """Parse ADILang text -> IR dict."""
    parsed = parse_adilang(text)
    return {"modules": parsed, "raw": text}


def encode_msgpack(ir: Dict[str, Any]) -> bytes:
    """Encode IR -> MessagePack bytes."""
    try:
        import msgpack
        return msgpack.packb(ir, use_bin_type=True)
    except ImportError:
        raise RuntimeError("msgpack tidak terinstall. Jalankan: pip install msgpack")


def decode_msgpack(data: bytes) -> Dict[str, Any]:
    """Decode MessagePack bytes -> IR dict."""
    try:
        import msgpack
        return msgpack.unpackb(data, raw=False)
    except ImportError:
        raise RuntimeError("msgpack tidak terinstall. Jalankan: pip install msgpack")


def encode_cbor(ir: Dict[str, Any]) -> bytes:
    """Encode IR -> CBOR bytes."""
    try:
        import cbor2
        return cbor2.dumps(ir)
    except ImportError:
        raise RuntimeError("cbor2 tidak terinstall. Jalankan: pip install cbor2")


def decode_cbor(data: bytes) -> Dict[str, Any]:
    """Decode CBOR bytes -> IR dict."""
    try:
        import cbor2
        return cbor2.loads(data)
    except ImportError:
        raise RuntimeError("cbor2 tidak terinstall. Jalankan: pip install cbor2")


def encode_json(ir: Dict[str, Any]) -> str:
    """Encode IR -> JSON string."""
    return json.dumps(ir, ensure_ascii=False, indent=2)


def decode_json(text: str) -> Dict[str, Any]:
    """Decode JSON string -> IR dict."""
    return json.loads(text)


def compare_encoding_sizes(text: str) -> Dict[str, Any]:
    """Bandingkan ukuran encoding untuk ADILang text."""
    ir = text_to_ir(text)
    text_size = len(text.encode("utf-8"))
    msgpack_size = len(encode_msgpack(ir))
    cbor_size = len(encode_cbor(ir))
    json_size = len(encode_json(ir).encode("utf-8"))
    return {
        "text": text_size,
        "msgpack": msgpack_size,
        "cbor": cbor_size,
        "json": json_size,
        "msgpack_ratio": round(msgpack_size / text_size, 2) if text_size else 1.0,
        "cbor_ratio": round(cbor_size / text_size, 2) if text_size else 1.0,
        "json_ratio": round(json_size / text_size, 2) if text_size else 1.0,
    }
