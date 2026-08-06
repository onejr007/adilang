"""
adilang/mcp_bridge.py — Standalone ADILang ↔ MCP Bridge (Pure Python Stdlib).
=============================================================================
Bidirectional converter between ADILang IR and Anthropic Model Context Protocol (MCP):
1. MCP tool calls → ADILang task modules
2. ADILang intent → MCP tool invocations
3. MCP resources → ADILang memory modules

Lead Developer: BAGAS ADI PRATAMA S,Kom.
"""
from __future__ import annotations
from typing import Any, Dict, List, Optional


def mcp_tool_call_to_task(mcp_call: Dict[str, Any], agent_key: str = "agent") -> Dict[str, Any]:
    """Convert MCP tool call → ADILang task module."""
    tool_name = mcp_call.get("name", "unknown_tool")
    arguments = mcp_call.get("arguments", {})
    call_id = mcp_call.get("id", "unknown")
    
    if isinstance(arguments, dict):
        input_parts = [f"{k}={v}" for k, v in arguments.items()]
        input_str = ", ".join(input_parts)
    else:
        input_str = str(arguments)
    
    return {
        "module": "task",
        "tag": f"mcp_{tool_name}",
        "fields": {
            "assign": agent_key,
            "input": input_str,
            "expect": f"result dari tool {tool_name}",
            "depends": [f"mcp_call_{call_id}"],
            "parallel": "0",
        },
    }


def mcp_tool_result_to_reply(mcp_result: Dict[str, Any], task_tag: str = "mcp_tool") -> Dict[str, Any]:
    """Convert MCP tool result → ADILang reply module."""
    content = mcp_result.get("content", [])
    is_error = mcp_result.get("isError", False)
    
    text_parts = []
    for item in content:
        if isinstance(item, dict) and item.get("type") == "text":
            text_parts.append(item.get("text", ""))
        elif isinstance(item, str):
            text_parts.append(item)
    
    reply_text = "\n".join(text_parts) if text_parts else "(no content)"
    kind = "error" if is_error else "inform"
    
    return {
        "module": "reply",
        "tag": kind,
        "fields": {
            "content": reply_text,
        },
    }


def mcp_resource_to_memory(mcp_resource: Dict[str, Any], user_key: str = "mcp-user") -> Dict[str, Any]:
    """Convert MCP resource → ADILang memory module."""
    uri = mcp_resource.get("uri", "unknown")
    name = mcp_resource.get("name", uri)
    content = mcp_resource.get("content", "")
    
    return {
        "module": "memory",
        "tag": name.replace(" ", "_").lower()[:40],
        "fields": {
            "key": uri,
            "fact": content[:500],
            "topic": "mcp_resource",
            "confidence": "1.0",
            "source": "mcp_bridge",
            "at": "2026-08-02T00:00:00Z",
        },
    }


def adilang_intent_to_mcp_tool_call(intent_module: Dict[str, Any]) -> Dict[str, Any]:
    """Convert ADILang intent module → MCP tool call."""
    fields = intent_module.get("fields", {})
    payload = fields.get("payload", "")
    
    parts = payload.split()
    tool_name = parts[0] if parts else "unknown"
    arguments = {}
    
    for part in parts[1:]:
        if "=" in part:
            key, value = part.split("=", 1)
            arguments[key] = value
    
    return {
        "name": tool_name,
        "arguments": arguments,
        "id": f"adilang_{intent_module.get('tag', 'unknown')}",
    }


def adilang_task_to_mcp_task(adilang_task: Dict[str, Any]) -> Dict[str, Any]:
    """Convert ADILang task module → MCP task."""
    fields = adilang_task.get("fields", {})
    return {
        "taskId": adilang_task.get("tag", "unknown"),
        "assignee": fields.get("assign", "agent"),
        "input": fields.get("input", ""),
        "expect": fields.get("expect", ""),
        "depends": fields.get("depends", []),
        "parallel": fields.get("parallel", "0"),
    }


def adilang_memory_to_mcp_resource(memory_module: Dict[str, Any]) -> Dict[str, Any]:
    """Convert ADILang memory module → MCP resource."""
    fields = memory_module.get("fields", {})
    return {
        "uri": fields.get("key", "unknown"),
        "name": memory_module.get("tag", "unknown"),
        "content": fields.get("fact", ""),
        "mimeType": "text/plain",
    }


class MCPBridge:
    """Bridge untuk MCP ↔ ADILang conversion."""
    
    @staticmethod
    def mcp_to_adilang(mcp_message: Dict[str, Any]) -> List[Dict[str, Any]]:
        modules = []
        if "uri" in mcp_message:
            modules.append(mcp_resource_to_memory(mcp_message))
        elif "name" in mcp_message and "arguments" in mcp_message:
            modules.append(mcp_tool_call_to_task(mcp_message))
        elif "content" in mcp_message:
            modules.append(mcp_tool_result_to_reply(mcp_message))
        return modules
    
    @staticmethod
    def adilang_to_mcp(adilang_module: Dict[str, Any]) -> Optional[Dict[str, Any]]:
        module_type = adilang_module.get("module", "")
        if module_type == "intent":
            return adilang_intent_to_mcp_tool_call(adilang_module)
        elif module_type == "task":
            return adilang_task_to_mcp_task(adilang_module)
        elif module_type == "memory":
            return adilang_memory_to_mcp_resource(adilang_module)
        return None
