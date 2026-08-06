"""
adilang/a2a_bridge.py — Standalone ADILang ↔ A2A Bridge (Pure Python Stdlib).
===========================================================================
Bidirectional converter between ADILang IR and Google Agent-to-Agent (A2A) Protocol:
- A2A tasks/messages/artifacts ↔ ADILang IR (task, intent, memory, reply, event, state, world)
- Google A2A Agent Card ↔ ADILang Agent Card

Lead Developer: BAGAS ADI PRATAMA S,Kom.
"""
from __future__ import annotations

import datetime as _dt
import json
from typing import Any, Dict, List, Optional

from adilang.protocol import (
    encode_task,
    encode_event,
    encode_intent,
    encode_memory,
    extract_modules,
)
from adilang.agent_card import create_agent_card


def _now_iso() -> str:
    return _dt.datetime.now(_dt.timezone.utc).isoformat()


def _truncate(text: str, limit: int = 1000) -> str:
    if text and len(text) > limit:
        return text[:limit]
    return text or ""


def _a2a_text_parts(obj: Dict[str, Any]) -> str:
    parts = obj.get("parts") or []
    texts = [p.get("text", "") for p in parts if isinstance(p, dict) and p.get("type") == "text"]
    return " ".join(t for t in texts if t)


def a2a_task_to_adilang(a2a_task: Dict[str, Any]) -> List[Dict[str, Any]]:
    task_input = a2a_task.get("input", {})
    input_str = json.dumps(task_input, ensure_ascii=False) if isinstance(task_input, dict) else str(task_input)
    return [
        {
            "module": "task",
            "tag": a2a_task.get("taskId", "unknown"),
            "fields": {
                "assign": a2a_task.get("assignee", "agent"),
                "input": _truncate(input_str),
                "expect": a2a_task.get("expect", "result"),
            },
        },
        {
            "module": "event",
            "tag": "a2a_task_received",
            "fields": {
                "source": "a2a_bridge",
                "at": _now_iso(),
                "key": a2a_task.get("taskId", "unknown"),
            },
        },
    ]


def a2a_message_to_intent(a2a_message: Dict[str, Any]) -> Dict[str, Any]:
    payload = _truncate(_a2a_text_parts(a2a_message)) or "(empty message)"
    role = a2a_message.get("role", "user")
    verb = "ask" if role == "user" else "inform"
    return {
        "module": "intent",
        "tag": verb,
        "fields": {
            "mode": "MODE_CONVERSATION",
            "payload": payload,
            "verb": verb,
        },
    }


def a2a_artifact_to_memory(a2a_artifact: Dict[str, Any], user_key: str = "a2a-user") -> Dict[str, Any]:
    content = _truncate(_a2a_text_parts(a2a_artifact)) or "(empty artifact)"
    return {
        "module": "memory",
        "tag": (a2a_artifact.get("name") or "artifact").replace(" ", "_").lower()[:40],
        "fields": {
            "key": a2a_artifact.get("artifactId", "unknown"),
            "fact": content,
            "topic": "a2a_artifact",
            "confidence": "1.0",
            "source": "a2a_bridge",
            "at": _now_iso(),
        },
    }


def a2a_task_to_adilang_text(a2a_task: Dict[str, Any]) -> str:
    task_input = a2a_task.get("input", {})
    input_str = json.dumps(task_input, ensure_ascii=False) if isinstance(task_input, dict) else str(task_input)
    task_id = a2a_task.get("taskId", "unknown")
    blocks = [
        encode_task(name=task_id, assign=a2a_task.get("assignee", "agent"),
                    input_=_truncate(input_str), expect=a2a_task.get("expect", "result")),
        encode_event(name="a2a_task_received", source="a2a_bridge", key=task_id, at=_now_iso()),
    ]
    return "\n".join(blocks)


def a2a_message_to_intent_text(a2a_message: Dict[str, Any]) -> str:
    payload = _truncate(_a2a_text_parts(a2a_message)) or "(empty message)"
    role = a2a_message.get("role", "user")
    verb = "ask" if role == "user" else "inform"
    return encode_intent(mode="MODE_CONVERSATION", payload=payload, verb=verb)


def a2a_artifact_to_memory_text(a2a_artifact: Dict[str, Any], user_key: str = "a2a-user") -> str:
    content = _truncate(_a2a_text_parts(a2a_artifact)) or "(empty artifact)"
    return encode_memory(
        name=(a2a_artifact.get("name") or "artifact").replace(" ", "_").lower()[:40],
        key=a2a_artifact.get("artifactId", "unknown"),
        fact=content,
        topic="a2a_artifact",
        confidence="1.0",
        source="a2a_bridge",
        at=_now_iso(),
    )


def adilang_task_to_a2a_task(adilang_task: Dict[str, Any]) -> Dict[str, Any]:
    fields = adilang_task.get("fields", {})
    input_str = fields.get("input", "")
    try:
        input_data = json.loads(input_str)
    except (json.JSONDecodeError, TypeError):
        input_data = {"text": input_str}
    return {
        "taskId": adilang_task.get("tag", "unknown"),
        "assignee": fields.get("assign", "agent"),
        "input": input_data,
        "expect": fields.get("expect", "result"),
        "status": "submitted",
    }


def adilang_intent_to_a2a_message(adilang_intent: Dict[str, Any]) -> Dict[str, Any]:
    fields = adilang_intent.get("fields", {})
    return {
        "messageId": f"adilang_{adilang_intent.get('tag', 'unknown')}",
        "role": "user",
        "parts": [{"type": "text", "text": fields.get("payload", "")}],
    }


def adilang_reply_to_a2a_message(adilang_reply: Dict[str, Any]) -> Dict[str, Any]:
    fields = adilang_reply.get("fields", {})
    message = {
        "messageId": f"adilang_reply_{adilang_reply.get('tag', 'unknown')}",
        "role": "assistant",
        "parts": [{"type": "text", "text": fields.get("content", "")}],
    }
    recs = fields.get("recs")
    if isinstance(recs, list) and recs:
        message["parts"].append({"type": "text", "text": "Rekomendasi: " + " | ".join(recs)})
    return message


def adilang_event_to_a2a_message(adilang_event: Dict[str, Any]) -> Dict[str, Any]:
    fields = adilang_event.get("fields", {})
    text = f"[event {adilang_event.get('tag', 'event')}] {fields.get('guidance', '')}".strip()
    return {
        "messageId": f"adilang_event_{adilang_event.get('tag', 'unknown')}",
        "role": "assistant",
        "parts": [{"type": "text", "text": text or "(event)"}],
    }


def adilang_state_to_a2a_message(adilang_state: Dict[str, Any]) -> Dict[str, Any]:
    fields = adilang_state.get("fields", {})
    bits = [f"{k}={v}" for k, v in fields.items() if k != "at" and v]
    text = "[state " + " ".join(bits) + "]" if bits else "[state]"
    return {
        "messageId": f"adilang_state_{adilang_state.get('tag', 'unknown')}",
        "role": "assistant",
        "parts": [{"type": "text", "text": text}],
    }


def adilang_memory_to_a2a_artifact(adilang_memory: Dict[str, Any]) -> Dict[str, Any]:
    fields = adilang_memory.get("fields", {})
    return {
        "artifactId": fields.get("key", "unknown"),
        "name": adilang_memory.get("tag", "memory"),
        "parts": [{"type": "text", "text": fields.get("fact", "")}],
    }


def adilang_world_to_a2a_artifact(adilang_world: Dict[str, Any]) -> Dict[str, Any]:
    return {
        "artifactId": f"world_{adilang_world.get('tag', 'scene')}",
        "name": adilang_world.get("tag", "world"),
        "parts": [{"type": "text", "text": adilang_world.get("fields", {}).get("text", "")}],
    }


_MODULE_TO_A2A = {
    "task": adilang_task_to_a2a_task,
    "intent": adilang_intent_to_a2a_message,
    "reply": adilang_reply_to_a2a_message,
    "event": adilang_event_to_a2a_message,
    "state": adilang_state_to_a2a_message,
    "memory": adilang_memory_to_a2a_artifact,
    "world": adilang_world_to_a2a_artifact,
}


def adilang_text_to_a2a(ir_text: str) -> List[Dict[str, Any]]:
    out = []
    for mod, _tag, fields in extract_modules(ir_text):
        converter = _MODULE_TO_A2A.get(mod)
        if converter is None:
            continue
        try:
            out.append(converter({"module": mod, "tag": _tag, "fields": fields}))
        except Exception:
            continue
    return out


A2A_PROTOCOL_VERSION = "0.2.0"


def adilang_agent_card_to_a2a(adilang_card: Dict[str, Any]) -> Dict[str, Any]:
    caps = adilang_card.get("capabilities", {})
    modules = caps.get("modules", [])
    skills = []
    for i, s in enumerate(adilang_card.get("skills") or []):
        sid = s.get("id") or f"skill_{i + 1}"
        skills.append({
            "id": sid,
            "name": s.get("name") or sid,
            "description": s.get("description") or adilang_card.get("description", ""),
            "tags": list(s.get("tags") or modules),
            "examples": list(s.get("examples") or []),
            "inputModes": list(s.get("inputModes") or ["text"]),
            "outputModes": list(s.get("outputModes") or ["text"]),
        })
    return {
        "name": adilang_card.get("name", "ADI Agent"),
        "description": adilang_card.get("description", ""),
        "url": adilang_card.get("url", ""),
        "version": adilang_card.get("version", "1.0.0"),
        "protocolVersion": A2A_PROTOCOL_VERSION,
        "capabilities": {
            "streaming": bool(caps.get("streaming")),
            "pushNotifications": False,
            "stateTransitionHistory": False,
        },
        "security": adilang_card.get("auth") or {"authenticationSchemes": []},
        "defaultInputModes": ["text"],
        "defaultOutputModes": ["text"],
        "skills": skills,
    }


def a2a_agent_card_to_adilang(a2a_card: Dict[str, Any]) -> Dict[str, Any]:
    modules = []
    for s in a2a_card.get("skills") or []:
        for t in s.get("tags") or []:
            if t in ("intent", "reply", "task", "event", "memory", "plan", "state", "world") and t not in modules:
                modules.append(t)
    return create_agent_card(
        name=a2a_card.get("name", "A2A Agent"),
        version=a2a_card.get("version", "1.0.0"),
        description=a2a_card.get("description", ""),
        url=a2a_card.get("url", ""),
        modules=modules or None,
        transports=["http", "sse"],
        encodings=["text"],
        auth=a2a_card.get("security") or None,
        skills=[
            {
                "id": s.get("id", f"skill_{i + 1}"),
                "name": s.get("name", s.get("id", f"skill_{i + 1}")),
                "description": s.get("description", ""),
                "tags": s.get("tags", []),
                "examples": s.get("examples", []),
            }
            for i, s in enumerate(a2a_card.get("skills") or [])
        ] or None,
        metadata={"a2a_protocol_version": a2a_card.get("protocolVersion", A2A_PROTOCOL_VERSION)},
    )


class A2ABridge:
    @staticmethod
    def a2a_to_adilang(a2a_message: Dict[str, Any]) -> List[Dict[str, Any]]:
        modules = []
        if "taskId" in a2a_message:
            modules.extend(a2a_task_to_adilang(a2a_message))
        elif "messageId" in a2a_message:
            modules.append(a2a_message_to_intent(a2a_message))
        elif "artifactId" in a2a_message:
            modules.append(a2a_artifact_to_memory(a2a_message, "a2a-user"))
        return modules

    @staticmethod
    def a2a_to_adilang_text(a2a_message: Dict[str, Any]) -> str:
        if "taskId" in a2a_message:
            return a2a_task_to_adilang_text(a2a_message)
        elif "messageId" in a2a_message:
            return a2a_message_to_intent_text(a2a_message)
        elif "artifactId" in a2a_message:
            return a2a_artifact_to_memory_text(a2a_message, "a2a-user")
        return ""

    @staticmethod
    def adilang_to_a2a(adilang_module: Dict[str, Any]) -> Optional[Dict[str, Any]]:
        converter = _MODULE_TO_A2A.get(adilang_module.get("module", ""))
        if converter is None:
            return None
        try:
            return converter(adilang_module)
        except Exception:
            return None

    @staticmethod
    def adilang_text_to_a2a(ir_text: str) -> List[Dict[str, Any]]:
        return adilang_text_to_a2a(ir_text)
