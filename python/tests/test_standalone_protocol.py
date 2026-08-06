"""
adilang/python/tests/test_standalone_protocol.py
=================================================
Unit tests for the standalone ADILang Python SDK.
Verifies zero external dependencies, compactor, bridges, agent card, and errors.
"""
import pytest
from adilang import (
    VERSION,
    encode_intent,
    encode_reply,
    encode_task,
    encode_memory,
    encode_plan,
    encode_event,
    encode_state,
    parse_adilang,
    validate_adilang,
    auto_fix,
    optimize_src,
    render_pretty,
    get_adilang_knowledge,
    get_adilang_registry,
    create_agent_card,
    validate_agent_card,
    MCPBridge,
    A2ABridge,
    ADILangError,
    classify_message,
)


def test_version():
    assert VERSION == "1.16.0"


def test_encode_and_parse_intent():
    ir = encode_intent(mode="MODE_CONVERSATION", payload="Hello world", verb="ask")
    assert 'intent "ask"' in ir
    assert 'payload "Hello world"' in ir

    parsed = parse_adilang(ir)
    assert "intent" in parsed
    assert parsed["intent"]["mode"] == "MODE_CONVERSATION"
    assert parsed["intent"]["payload"] == "Hello world"
    assert parsed["intent"]["verb"] == "ask"


def test_encode_and_parse_reply():
    ir = encode_reply(mode="MODE_CONVERSATION", content="Hi there!", recs=["Option A", "Option B"])
    assert 'reply "answer"' in ir
    parsed = parse_adilang(ir)
    assert "reply" in parsed
    assert parsed["reply"]["content"] == "Hi there!"
    assert parsed["reply"]["recs"] == ["Option A", "Option B"]


def test_encode_and_parse_task():
    ir = encode_task(name="code_review", assign="agent_1", input_="review main.py", expect="clean diff")
    parsed = parse_adilang(ir)
    assert "task" in parsed
    assert parsed["task"]["_tag"] == "code_review"
    assert parsed["task"]["assign"] == "agent_1"


def test_validate_valid_and_invalid():
    valid_ir = encode_intent(mode="MODE_CONVERSATION", payload="Test")
    errs = validate_adilang(valid_ir)
    assert len(errs) == 0

    invalid_ir = 'intent "ask" { invalid_key "bad" }'
    errs_inv = validate_adilang(invalid_ir)
    assert len(errs_inv) > 0
    assert "Kunci tidak dikenal" in errs_inv[0]


def test_auto_fix():
    bad_ir = 'world Box { mesh sphere'
    fixed_text, fixes = auto_fix(bad_ir)
    assert 'world "Box"' in fixed_text
    assert len(fixes) > 0


def test_token_compactor():
    verbose_ir = """
    # Comments should be stripped
    intent "ask" {
        mode "MODE_CONVERSATION"
        payload "Hello world"
    }
    """
    compacted = optimize_src(verbose_ir)
    assert "# Comments should be stripped" not in compacted
    assert compacted == 'intent "ask"{mode "MODE_CONVERSATION" payload "Hello world"}'


def test_agent_card():
    card = create_agent_card(
        name="TestAgent",
        version="1.0.0",
        description="A test agent",
        url="http://localhost:8000"
    )
    errs = validate_agent_card(card)
    assert len(errs) == 0
    assert card["protocol"] == "adilang"
    assert card["capabilities"]["compactor"] is True


def test_mcp_bridge():
    mcp_call = {
        "name": "search_docs",
        "arguments": {"query": "FastAPI"},
        "id": "123"
    }
    adilang_modules = MCPBridge.mcp_to_adilang(mcp_call)
    assert len(adilang_modules) == 1
    assert adilang_modules[0]["module"] == "task"
    assert adilang_modules[0]["tag"] == "mcp_search_docs"


def test_a2a_bridge():
    a2a_task = {
        "taskId": "task_456",
        "assignee": "agent_alpha",
        "input": {"query": "test"},
        "expect": "json"
    }
    adilang_text = A2ABridge.a2a_to_adilang_text(a2a_task)
    assert 'task "task_456"' in adilang_text
    assert 'event "a2a_task_received"' in adilang_text


def test_error_classification():
    err_msg = "kunci tidak dikenal untuk modul intent"
    code = classify_message(err_msg)
    assert code == "E020"
