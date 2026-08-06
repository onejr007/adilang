# 🐍 ADILang Standalone Python SDK

> **Pure Python Stdlib SDK for Universal AI-to-AI Communication & Mental State IR.**  
> Lead Developer: **BAGAS ADI PRATAMA S,Kom.**

---

## 🎯 What is ADILang?

ADILang is an **AI-to-AI Communication Protocol and Intermediate Representation (IR)** designed for AI agents to negotiate intent, tasks, long-term memory, execution plans, and runtime state deterministically and with high token efficiency.

---

## 🚀 Quickstart

### 1. Installation

```bash
cd adilang/python
pip install -e .
```

### 2. Python Code Example (Standalone — No External System Required)

```python
import adilang

# 1. Create an Intent IR block (what the agent asks/commands)
intent_ir = adilang.encode_intent(
    mode="MODE_CODE_ENGINEERING",
    payload="Build a Python Fibonacci function",
    verb="command"
)
print("--- Intent Block ---")
print(intent_ir)

# 2. Parse any ADILang IR string into Python dictionary
parsed = adilang.parse_adilang(intent_ir)
print("\n--- Parsed Data ---")
print(parsed["intent"]["payload"])

# 3. Validate ADILang syntax against closed vocabulary
errors = adilang.validate_adilang(intent_ir)
if not errors:
    print("\n[OK] ADILang syntax is valid!")

# 4. Create a Reply IR block
reply_ir = adilang.encode_reply(
    mode="MODE_CODE_ENGINEERING",
    content="def fib(n): return n if n <= 1 else fib(n-1) + fib(n-2)"
)
print("\n--- Reply Block ---")
print(reply_ir)
```

---

## 🛠️ CLI Usage (`adilang-cli`)

```bash
# Validate an .adi file
adilang-cli check my_file.adi

# Parse an .adi file to JSON IR
adilang-cli parse my_file.adi

# Auto-fix invalid syntax or keys
adilang-cli fix my_file.adi
```
