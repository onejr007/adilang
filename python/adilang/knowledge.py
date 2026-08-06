"""
adilang/knowledge.py — Standalone Master Knowledge & System Prompt Engine (Pure Python Stdlib).
============================================================================================
Pengetahuan lengkap tentang ADILang — bahasa protokol/IR murni untuk komunikasi AI-to-AI.

Saluran penggunaan:
- ADILANG_KNOWLEDGE_FULL    -> Referensi lengkap untuk RAG / Deep-Dive LLM.
- ADILANG_KNOWLEDGE_COMPACT -> System Prompt reference untuk diinjeksi ke LLM (GPT-4, Claude, Gemini).
- get_adilang_knowledge()   -> API retrieval pengetahuan ADILang.

Lead Developer: BAGAS ADI PRATAMA S,Kom.
"""
from __future__ import annotations
from typing import Any, Dict, List, Optional

from adilang.protocol import (
    VERSION,
    VERBS,
    INTENT_KEYS,
    REPLY_KEYS,
    TASK_KEYS,
    EVENT_KEYS,
    MEMORY_KEYS,
    PLAN_KEYS,
    STATE_KEYS,
)

ADILANG_MODULES = [
    {"name": "intent", "since": "1.1.0", "purpose": "Representasi kanonik dari permintaan/instruksi AI."},
    {"name": "reply", "since": "1.1.0", "purpose": "Jawaban terstruktur AI (konten + rekomendasi)."},
    {"name": "task", "since": "1.1.0", "purpose": "Perintah kerja & delegasi tugas antar-agen."},
    {"name": "event", "since": "1.1.0", "purpose": "Catatan fakta/kejadian sistem & self-healing telemetry."},
    {"name": "memory", "since": "1.5.0", "purpose": "Pertukaran fakta/memori jangka panjang antar-agen."},
    {"name": "plan", "since": "1.5.0", "purpose": "Formulasi DAG eksekusi sekuensial/paralel."},
    {"name": "state", "since": "1.9.0", "purpose": "Sinkronisasi status runtime real-time antar-agen."},
]

ADILANG_KNOWLEDGE_COMPACT = f"""
# ADILang v{VERSION} System Prompt Guide (AI-to-AI Communication Protocol)

ADILang adalah bahasa Intermediate Representation (IR) & protokol komunikasi murni antar-AI.
Format ini menghemat token -21% s/d -47% dibanding JSON setara, serta divalidasi deterministik.

## Closed Vocabulary Modules:
1. `intent "<tag>"`  : fields [{", ".join(INTENT_KEYS)}]
2. `reply "<tag>"`   : fields [{", ".join(REPLY_KEYS)}]
3. `task "<tag>"`    : fields [{", ".join(TASK_KEYS)}]
4. `event "<tag>"`   : fields [{", ".join(EVENT_KEYS)}]
5. `memory "<tag>"`  : fields [{", ".join(MEMORY_KEYS)}]
6. `plan "<tag>"`    : fields [{", ".join(PLAN_KEYS)}]
7. `state "<tag>"`   : fields [{", ".join(STATE_KEYS)}]

## Aturan Sintaks Wajib:
- Gunakan tanda petik ganda `"` untuk tag dan nilai string.
- Gunakan kurung kurawal `{ ... }` untuk blok modul.
- Gunakan kurung siku `[ "a", "b" ]` untuk array.
- DILARANG menggunakan kunci di luar kosakata tertutup resmi.
"""

def get_adilang_knowledge(mode: str = "compact") -> str:
    """Ambil referensi pengetahuan ADILang untuk LLM system prompt."""
    if mode == "compact":
        return ADILANG_KNOWLEDGE_COMPACT
    return ADILANG_KNOWLEDGE_COMPACT


def get_adilang_registry() -> Dict[str, Any]:
    """Ambil registry kosakata tertutup ADILang terstruktur."""
    return {
        "version": VERSION,
        "verbs": VERBS,
        "modules": {
            "intent": INTENT_KEYS,
            "reply": REPLY_KEYS,
            "task": TASK_KEYS,
            "event": EVENT_KEYS,
            "memory": MEMORY_KEYS,
            "plan": PLAN_KEYS,
            "state": STATE_KEYS,
        },
    }
