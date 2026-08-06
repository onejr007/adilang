"""
ADILang Standalone Python SDK
=============================

ADILang (Agent Distributed Intelligence Language) is an AI-to-AI protocol / Intermediate Representation (IR).
This package is pure Python stdlib with ZERO external dependencies.

Lead Developer: BAGAS ADI PRATAMA S,Kom.
"""

from adilang.protocol import (
    VERSION,
    ADILANG_PROMPT,
    encode_intent,
    encode_reply,
    encode_task,
    encode_event,
    encode_memory,
    encode_plan,
    encode_state,
    parse_adilang,
    validate_adilang,
    auto_fix,
    minify,
)

__version__ = VERSION

__all__ = [
    "VERSION",
    "ADILANG_PROMPT",
    "encode_intent",
    "encode_reply",
    "encode_task",
    "encode_event",
    "encode_memory",
    "encode_plan",
    "encode_state",
    "parse_adilang",
    "validate_adilang",
    "auto_fix",
    "minify",
]
