"""
adilang/agent_card.py — ADILang Agent Card & Capability Discovery (Standalone)
================================================================================
Agent Card adalah JSON descriptor yang advertise capabilities agent AI,
mirip dengan A2A Agent Card tetapi khusus untuk ekosistem ADILang.

Format standar: /.well-known/adilang.json

Lead Developer: BAGAS ADI PRATAMA S,Kom.
"""
from __future__ import annotations

import json
import time
from typing import Any, Dict, List, Optional

from adilang.protocol import VERSION

WELL_KNOWN_PATH = "/.well-known/adilang.json"


def create_agent_card(
    name: str,
    version: str,
    description: str,
    url: str,
    modules: Optional[List[str]] = None,
    transports: Optional[List[str]] = None,
    encodings: Optional[List[str]] = None,
    auth: Optional[Dict[str, Any]] = None,
    skills: Optional[List[Dict[str, Any]]] = None,
    metadata: Optional[Dict[str, Any]] = None,
) -> Dict[str, Any]:
    """Buat Agent Card untuk ADILang agent."""
    if modules is None:
        modules = ["intent", "reply", "task", "event", "memory", "plan", "state", "world"]
    if transports is None:
        transports = ["sse", "http"]
    if encodings is None:
        encodings = ["text", "msgpack", "cbor"]
    
    card = {
        "name": name,
        "version": version,
        "description": description,
        "url": url,
        "protocol": "adilang",
        "protocol_version": VERSION,
        "capabilities": {
            "modules": modules,
            "transports": transports,
            "encodings": encodings,
            "streaming": "sse" in transports,
            "binary": any(e in encodings for e in ["msgpack", "cbor"]),
            "self_heal": True,
            "compactor": True,
        },
        "endpoints": {
            "parse": f"{url.rstrip('/')}/adilang/parse",
            "validate": f"{url.rstrip('/')}/adilang/validate",
            "schema": f"{url.rstrip('/')}/adilang/schema",
            "knowledge": f"{url.rstrip('/')}/api/v1/adilang/knowledge",
            "stream": f"{url.rstrip('/')}/api/v1/chat/stream/{{job_id}}",
        },
        "updated_at": time.time(),
    }
    if auth:
        card["auth"] = auth
    if skills:
        card["skills"] = skills
    if metadata:
        card["metadata"] = metadata
    return card


def validate_agent_card(card: Dict[str, Any]) -> List[str]:
    """Validasi Agent Card."""
    errors = []
    required_fields = ["name", "version", "description", "url", "protocol", "protocol_version"]
    for field in required_fields:
        if field not in card:
            errors.append(f"Field '{field}' wajib diisi")
    if "protocol" in card and card["protocol"] != "adilang":
        errors.append(f"protocol harus 'adilang', bukan '{card['protocol']}'")
    if "capabilities" in card:
        caps = card["capabilities"]
        valid_modules = {"intent", "reply", "task", "event", "memory", "plan", "state", "world"}
        if "modules" in caps:
            invalid = set(caps["modules"]) - valid_modules
            if invalid:
                errors.append(f"Module tidak dikenali: {invalid}")
    return errors


def agent_card_to_json(card: Dict[str, Any], indent: int = 2) -> str:
    """Convert Agent Card → JSON string."""
    return json.dumps(card, ensure_ascii=False, indent=indent)


def agent_card_from_json(text: str) -> Dict[str, Any]:
    """Parse JSON string → Agent Card."""
    return json.loads(text)


def get_well_known_url(base_url: str) -> str:
    """Get well-known URI untuk agent discovery."""
    return f"{base_url.rstrip('/')}{WELL_KNOWN_PATH}"
