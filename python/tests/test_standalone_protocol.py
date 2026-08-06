"""
adilang/python/tests/test_standalone_protocol.py
=================================================
Unit tests for the standalone ADILang Python SDK.
Verifies zero external dependencies and standalone functionality.
"""
import pytest
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
    validate_adilang,
    auto_fix,
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
    assert "Kunci tidak valid" in errs_inv[0]


def test_auto_fix():
    bad_ir = 'intent "ask" { mode "MODE_CONVERSATION" payload "Hello" bad_key "x" }'
    fixed = auto_fix(bad_ir)
    errs = validate_adilang(fixed)
    assert len(errs) == 0
