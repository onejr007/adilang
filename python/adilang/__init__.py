"""
adilang — Standalone Pure Python SDK for ADILang
=================================================
Agent Distributed Intelligence Language (AI-to-AI Communication Protocol & Mental State IR).

Lead Developer: BAGAS ADI PRATAMA S,Kom.
"""
from adilang.protocol import (
    VERSION,
    encode_intent,
    encode_reply,
    encode_task,
    encode_memory,
    encode_plan,
    encode_event,
    encode_state,
    parse_adilang,
    parse_module,
    validate_adilang,
    validate_module,
    auto_fix,
    plan_topological_order,
    extract_modules,
)
from adilang.compactor import (
    optimize_src,
    render_program,
    render_expr,
    render_pretty,
)
from adilang.knowledge import (
    ADILANG_KNOWLEDGE_COMPACT,
    get_adilang_knowledge,
    get_adilang_registry,
)
from adilang.agent_card import (
    create_agent_card,
    validate_agent_card,
    agent_card_to_json,
    agent_card_from_json,
    get_well_known_url,
)
from adilang.binary import (
    encode_msgpack,
    decode_msgpack,
    encode_cbor,
    decode_cbor,
    compare_encoding_sizes,
)
from adilang.mcp_bridge import MCPBridge
from adilang.a2a_bridge import A2ABridge
from adilang.errors import (
    ADILangError,
    ERROR_CODES,
    classify_message,
    hint_for,
    normalize_errors,
)

__all__ = [
    "VERSION",
    "encode_intent",
    "encode_reply",
    "encode_task",
    "encode_memory",
    "encode_plan",
    "encode_event",
    "encode_state",
    "parse_adilang",
    "parse_module",
    "validate_adilang",
    "validate_module",
    "auto_fix",
    "plan_topological_order",
    "extract_modules",
    "optimize_src",
    "render_program",
    "render_expr",
    "render_pretty",
    "ADILANG_KNOWLEDGE_COMPACT",
    "get_adilang_knowledge",
    "get_adilang_registry",
    "create_agent_card",
    "validate_agent_card",
    "agent_card_to_json",
    "agent_card_from_json",
    "get_well_known_url",
    "encode_msgpack",
    "decode_msgpack",
    "encode_cbor",
    "decode_cbor",
    "compare_encoding_sizes",
    "MCPBridge",
    "A2ABridge",
    "ADILangError",
    "ERROR_CODES",
    "classify_message",
    "hint_for",
    "normalize_errors",
]
