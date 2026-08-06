"""
adilang/protocol.py — Standalone ADILang Protocol / IR Engine (Pure Python Stdlib).
===================================================================================
This module provides a complete, standalone, zero-dependency implementation of the
ADILang protocol (intent, reply, task, event, memory, plan, state, world).

It can be used in ANY Python application or AI agent independently of the ADI main system.

Lead Developer: BAGAS ADI PRATAMA S,Kom.
"""
import re
from collections import OrderedDict

VERSION = "1.16.0"

# ─── Closed Vocabularies (harus sinkron dengan docs/adilang.ebnf §8-11) ─────
VERBS = ("ask", "inform", "command", "greet", "system")

INTENT_KEYS = ("mode", "payload", "verb")
REPLY_KEYS = ("mode", "content", "recs", "world")
TASK_KEYS = ("assign", "input", "expect")
EVENT_KEYS = ("source", "key", "session", "at", "line", "token", "guidance")
MEMORY_KEYS = ("key", "topic", "fact", "confidence", "source", "at")
PLAN_KEYS = ("steps", "parallel")
STATE_KEYS = (
    "user_key", "session_id", "job_id",
    "muted", "speaking", "mic_active",
    "quality", "status", "progress",
    "provider", "elapsed",
    "at",
)

_MODULE_KEYS = {
    "intent": INTENT_KEYS,
    "reply": REPLY_KEYS,
    "task": TASK_KEYS,
    "event": EVENT_KEYS,
    "memory": MEMORY_KEYS,
    "plan": PLAN_KEYS,
    "state": STATE_KEYS,
    "world": ("text",),
}
_REQUIRED = {
    "intent": ("mode", "payload"),
    "reply": ("content",),
    "task": ("assign", "input", "expect"),
    "event": ("source", "at"),
    "memory": ("key", "fact"),
    "plan": ("steps",),
    "state": ("user_key", "at"),
    "world": (),
}

# verb disimpulkan dari intent-mode (pemetaan tertutup, deterministik)
MODE_VERB = {
    "MODE_CONVERSATION": "ask",
    "MODE_CODE_ENGINEERING": "command",
    "MODE_CALCULATION": "ask",
    "MODE_SYSTEM_DIAGNOSTICS": "command",
    "MODE_TASK_EXECUTION": "command",
    "MODE_JOB_CAREER": "ask",
    "MODE_ECOMMERCE_PRODUCT": "ask",
    "MODE_TRAVEL_LOGISTICS": "ask",
    "MODE_PUBLIC_DATA_ID": "ask",
    "MODE_CULINARY_HEALTH": "ask",
    "MODE_ENTERTAINMENT": "ask",
}

# Referensi ADILang ringkas untuk disuntikkan ke system prompt LLM (hemat token)
ADILANG_PROMPT = (
    "=== ADILANG PROTOCOL/IR (ringkas) ===\n"
    "ADILang adalah bahasa IR utama ekosistem ADI (diciptakan ADI, untuk AI). "
    "Setiap pesan user telah dinormalisasi menjadi satu blok `intent`.\n"
    'intent "<verb>" { mode "<MODE>" payload "<teks>" verb "<verb>" }\n'
    "Modul: intent | reply | task | event | memory | plan | state | world. "
    "Kunci: mode payload verb content recs assign input expect source key session at "
    "topic fact confidence steps parallel line token guidance user_key speaking "
    "muted mic_active quality provider elapsed. "
    "Verbs: ask inform command greet system. Kunci duplikat/tidak dikenal = non-konforman."
    "Verifikasi formal (v1.7.0): syntax error → event \"syntax_error\" { source line token guidance }."
)


def minify(text: str) -> str:
    """Normalisasi teks untuk payload IR: kolaps whitespace, trim, jaga UTF-8."""
    if not text:
        return ""
    collapsed = re.sub(r"[ \t]+", " ", text)
    collapsed = re.sub(r"\s*\n\s*", " ", collapsed)
    return collapsed.strip()


def _q(value) -> str:
    """Encode string sebagai ADILang string literal."""
    s = str(value)
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


# ═══════════════════════════════════════════════════════════════════════════
# ENCODER — menghasilkan blok ADILang deterministik
# ═══════════════════════════════════════════════════════════════════════════

def encode_intent(mode: str, payload: str, verb: str = None) -> str:
    """Terjemahkan input user menjadi blok `intent` (IR kanonik).

    Deterministik: (mode, payload) yang sama selalu menghasilkan blok yang sama.
    """
    verb = verb or MODE_VERB.get(mode, "ask")
    return (
        f"intent {_q(verb)} {{\n"
        f"    mode {_q(mode)}\n"
        f"    payload {_q(minify(payload))}\n"
        f"    verb {_q(verb)}\n"
        f"}}"
    )


def encode_reply(mode: str, content: str, recs=None, world: str = None) -> str:
    """Bungkus jawaban ADI menjadi blok `reply` terstruktur."""
    lines = [
        'reply "answer" {',
        f"    mode {_q(mode)}",
        f"    content {_q(content)}",
    ]
    if recs:
        arr = " ".join(_q(r) for r in recs)
        lines.append(f"    recs [ {arr} ]")
    if world:
        lines.append(f"    world {_q(world)}")
    lines.append("}")
    return "\n".join(lines)


def encode_task(name: str, assign: str, input_: str, expect: str) -> str:
    """Buat blok `task` (work order agent)."""
    return (
        f"task {_q(name)} {{\n"
        f"    assign {_q(assign)}\n"
        f"    input {_q(input_)}\n"
        f"    expect {_q(expect)}\n"
        f"}}"
    )


def encode_memory(
    name: str,
    key: str,
    fact: str,
    topic: str = None,
    confidence: str = None,
    source: str = None,
    at: str = None,
) -> str:
    """Buat blok `memory` (fakta jangka panjang antar-agen, v1.5.0)."""
    import datetime as _dt
    if at is None:
        at = _dt.datetime.now(_dt.timezone.utc).isoformat()
    lines = [
        f"memory {_q(name)} {{",
        f"    key {_q(key)}",
        f"    fact {_q(minify(fact))}",
    ]
    if topic:
        lines.append(f"    topic {_q(topic)}")
    if confidence:
        lines.append(f"    confidence {_q(str(confidence))}")
    if source:
        lines.append(f"    source {_q(source)}")
    lines.append(f"    at {_q(at)}")
    lines.append("}")
    return "\n".join(lines)


def encode_plan(name: str, steps, parallel: str = "0") -> str:
    """Buat blok `plan` (DAG langkah eksekusi CrewAI, v1.5.0)."""
    arr = " ".join(_q(str(s)) for s in steps)
    lines = [
        f"plan {_q(name)} {{",
        f"    steps [ {arr} ]",
        f"    parallel {_q(str(parallel))}",
        "}",
    ]
    return "\n".join(lines)


def encode_event(name: str, source: str, key: str = None, session: str = None, at: str = None) -> str:
    """Buat blok `event` (catatan kejadian)."""
    import datetime as _dt
    if at is None:
        at = _dt.datetime.now(_dt.timezone.utc).isoformat()
    lines = [
        f"event {_q(name)} {{",
        f"    source {_q(source)}",
    ]
    if key:
        lines.append(f"    key {_q(key)}")
    if session:
        lines.append(f"    session {_q(session)}")
    lines.append(f"    at {_q(at)}")
    lines.append("}")
    return "\n".join(lines)


def encode_fact_memory(key: str, source: str, session: str = None, at: str = None, name: str = "fact_memory") -> str:
    """Buat blok `event "fact_memory"` — memori/konteks SEMENTARA antar-agen."""
    return encode_event(name=name, source=source, key=key, session=session, at=at)


def encode_state(user_key: str, **fields) -> str:
    """Buat blok `state` (sinkronisasi status real-time antar-channel, P1.4)."""
    import datetime as _dt
    if fields.get("at") is None:
        fields["at"] = _dt.datetime.now(_dt.timezone.utc).isoformat()
    lines = [
        f'state "stream" {{',
        f'    user_key {_q(user_key)}',
    ]
    for _key in ("session_id", "job_id", "muted", "speaking", "mic_active",
                 "quality", "status", "progress", "provider", "elapsed"):
        val = fields.get(_key)
        if val is not None and val != "":
            lines.append(f'    {_key} {_q(val)}')
    lines.append(f'    at {_q(fields["at"])}')
    lines.append("}")
    return "\n".join(lines)


def encode_world(name: str, content: str) -> str:
    """Buat blok `world` (adegan 3D untuk WASM engine)."""
    name = (name or "ADI World").replace('"', "'")[:40]
    return f'world "{name}" {{\n{content}\n}}\n'


# ═══════════════════════════════════════════════════════════════════════════
# PARSER — pembaca blok protocol deterministik
# ═══════════════════════════════════════════════════════════════════════════

_MOD_RE = re.compile(r"(intent|reply|task|event|memory|plan|state|world)\s+")


def _skip_ws(text, pos):
    while pos < len(text) and text[pos] in " \t\r\n":
        pos += 1
    return pos


def _read_string(text, pos):
    if pos >= len(text) or text[pos] != '"':
        raise ValueError(f"Ekspektasi string di posisi {pos}: {text[pos:pos + 12]!r}")
    pos += 1
    out = []
    esc = {"n": "\n", "t": "\t", '"': '"', "\\": "\\"}
    while True:
        if pos >= len(text):
            raise ValueError("String tidak ditutup")
        ch = text[pos]
        if ch == '"':
            return "".join(out), pos + 1
        if ch == "\\":
            if pos + 1 >= len(text):
                raise ValueError("Escape tidak lengkap")
            nxt = text[pos + 1]
            out.append(esc.get(nxt, nxt))
            pos += 2
            continue
        out.append(ch)
        pos += 1


def _read_array(text, pos):
    if pos >= len(text) or text[pos] != "[":
        raise ValueError(f"Ekspektasi '[' di posisi {pos}")
    pos += 1
    items = []
    while True:
        pos = _skip_ws(text, pos)
        if pos >= len(text):
            raise ValueError("Array tidak ditutup")
        if text[pos] == "]":
            return items, pos + 1
        val, pos = _read_string(text, pos)
        items.append(val)


def parse_module(text: str):
    """Parse dokumen protocol ADILang → (module, tag, OrderedDict)."""
    text = text.strip()
    m = _MOD_RE.match(text)
    if not m:
        raise ValueError(f"Bukan modul protocol ADILang: {text[:40]!r}")
    mod = m.group(1)
    pos = m.end()
    tag, pos = _read_string(text, pos)
    pos = _skip_ws(text, pos)
    if pos >= len(text) or text[pos] != "{":
        raise ValueError("Ekspektasi '{' setelah tag")
    pos += 1

    fields = OrderedDict()
    if mod == "world":
        depth = 1
        start = pos
        while pos < len(text) and depth > 0:
            ch = text[pos]
            if ch == "{":
                depth += 1
            elif ch == "}":
                depth -= 1
            pos += 1
        fields["text"] = text[start:pos - 1].strip()
        return mod, tag, fields

    while True:
        pos = _skip_ws(text, pos)
        if pos >= len(text):
            raise ValueError("Ekspektasi '}' (blok belum ditutup)")
        if text[pos] == "}":
            break
        km = re.match(r"[A-Za-z_][A-Za-z0-9_]*", text[pos:])
        if not km:
            raise ValueError(f"Kunci tidak valid di posisi {pos}: {text[pos:pos + 10]!r}")
        key = km.group(0)
        if key in fields:
            raise ValueError(f"Kunci duplikat: {key!r}")
        pos += len(key)
        pos = _skip_ws(text, pos)
        if pos < len(text) and text[pos] == "[":
            value, pos = _read_array(text, pos)
        else:
            value, pos = _read_string(text, pos)
        fields[key] = value

    return mod, tag, fields


def parse_adilang(text: str) -> dict:
    """Parse string ADILang IR menjadi struktur dict untuk kompatibilitas."""
    if not text:
        return {}
    mods = extract_modules(text)
    if not mods:
        try:
            m, t, f = parse_module(text)
            d = dict(f)
            if t:
                d["_tag"] = t
            return {m: d}
        except ValueError:
            return {}
    res = {}
    for m, t, f in mods:
        d = dict(f)
        if t:
            d["_tag"] = t
        res[m] = d
    return res


def _validate_module_structured(text: str):
    """Validasi konformansi blok protocol → (ok, list[ADILangError])."""
    from core.adilang_errors import ADILangError, classify_message, hint_for

    try:
        mod, tag, fields = parse_module(text)
    except ValueError as e:
        msg = str(e)
        code = classify_message(msg)
        return False, [ADILangError(code=code, severity="ERROR", message=msg,
                                    hint=hint_for(code), module=None)]
    allowed = _MODULE_KEYS[mod]
    errors = []

    def _add(message: str) -> None:
        code = classify_message(message)
        errors.append(ADILangError(code=code, severity="ERROR", message=message,
                                   hint=hint_for(code), module=mod))

    for k in fields:
        if k not in allowed:
            _add(f"Kunci tidak dikenal untuk modul {mod}: {k!r}")
    for k in _REQUIRED[mod]:
        if k not in fields:
            _add(f"Kunci wajib tidak ada: {k!r}")
    if mod == "intent" and "verb" in fields and fields["verb"] not in VERBS:
        _add(f"verb tidak valid: {fields['verb']!r}")
    if mod == "reply":
        recs = fields.get("recs")
        if recs is not None and not isinstance(recs, list):
            _add("recs harus berupa array string")
        content = fields.get("content")
        if content is not None and not str(content).strip():
            _add("content tidak boleh kosong")
    if mod == "memory":
        conf = fields.get("confidence")
        if conf is not None:
            try:
                c = float(conf)
                if not 0.0 <= c <= 1.0:
                    _add(f"confidence harus 0..1: {conf!r}")
            except (TypeError, ValueError):
                _add(f"confidence bukan angka valid: {conf!r}")
    if mod == "plan":
        steps = fields.get("steps")
        if steps is not None and not isinstance(steps, list):
            _add("steps harus berupa array string")
        else:
            try:
                parse_plan_steps(steps or [])
            except ValueError as e:
                _add(str(e))
        par = fields.get("parallel")
        if par is not None and par not in ("0", "1"):
            _add(f"parallel harus '0' atau '1': {par!r}")
    return (len(errors) == 0), errors


def validate_module_structured(text: str):
    """Validasi konformansi → (ok, list[ADILangError])."""
    return _validate_module_structured(text)


def validate_module(text: str):
    """Validasi konformansi blok protocol. Returns (ok: bool, errors: list[str])."""
    ok, structured = _validate_module_structured(text)
    return ok, [e.message for e in structured]


def validate_adilang(text: str) -> list:
    """Validasi string ADILang IR terhadap closed vocabulary."""
    ok, errors = validate_module(text)
    if ok:
        return []
    return errors


# ═══════════════════════════════════════════════════════════════════════════
# PLAN DAG — modul `plan` (v1.5.0): langkah sekuensial/paralel untuk CrewAI
# ═══════════════════════════════════════════════════════════════════════════

def parse_plan_steps(steps: list) -> list:
    """Parse entry steps `"<id>:<action>:<depends_csv>"` menjadi struktur DAG."""
    out = []
    by_id = {}
    seen = set()
    for raw in steps:
        s = str(raw)
        head, _, depends_raw = s.rpartition(":")
        if not head:
            raise ValueError(f"Entry steps tidak valid: {s!r} (butuh id:action:depends)")
        sid_str, _, action = head.partition(":")
        if not sid_str.strip() or not action.strip():
            raise ValueError(f"Entry steps tidak valid: {s!r} (butuh id:action:depends)")
        try:
            sid = int(sid_str)
        except ValueError:
            raise ValueError(f"Entry steps id bukan angka: {sid_str!r}")
        if sid in seen:
            raise ValueError(f"Entry steps id duplikat: {sid}")
        seen.add(sid)
        deps = []
        if depends_raw.strip():
            for d in depends_raw.split(","):
                d = d.strip()
                if not d:
                    continue
                try:
                    deps.append(int(d))
                except ValueError:
                    raise ValueError(
                        f"Entry steps {sid} dependensi bukan angka: {d!r} "
                        f"(format kanonik '<id>:<action>:<depends>' — depends wajib ada, "
                        f"boleh kosong, contoh '1:task:research:')"
                    )
        node = {"id": sid, "action": action, "depends": deps}
        out.append(node)
        by_id[sid] = node
    ids = set(by_id)
    for n in out:
        for d in n["depends"]:
            if d not in ids:
                raise ValueError(f"Entry steps {n['id']} dependensi {d} tidak dikenal")
    state = {nid: 0 for nid in by_id}
    def _visit(nid):
        state[nid] = 1
        for d in by_id[nid]["depends"]:
            if state[d] == 1:
                raise ValueError(f"Plan memiliki cycle pada langkah {nid} → {d}")
            if state[d] == 0:
                _visit(d)
        state[nid] = 2
    for nid in sorted(by_id):
        if state[nid] == 0:
            _visit(nid)
    return out


def plan_topological_order(steps: list) -> list:
    """Urutan eksekusi DAG (topological sort, Kahn) — deterministik."""
    nodes = steps if (steps and isinstance(steps[0], dict)) else parse_plan_steps(steps)
    indeg = {n["id"]: len(n["depends"]) for n in nodes}
    dependents = {n["id"]: [] for n in nodes}
    for n in nodes:
        for d in n["depends"]:
            dependents[d].append(n["id"])
    ready = sorted([i for i, deg in indeg.items() if deg == 0])
    waves = []
    done = set()
    while ready:
        wave = list(ready)
        waves.append(wave)
        done.update(wave)
        ready = []
        for wid in wave:
            for nxt in dependents[wid]:
                indeg[nxt] -= 1
                if indeg[nxt] == 0 and nxt not in done:
                    ready.append(nxt)
        ready = sorted(set(ready) - done)
    total = sum(len(w) for w in waves)
    if total != len(nodes):
        raise ValueError("Plan memiliki cycle — bukan DAG yang valid")
    return waves


# ═══════════════════════════════════════════════════════════════════════════
# UTIL — ekstraksi dari output LLM
# ═══════════════════════════════════════════════════════════════════════════

_RECS_RE = re.compile(r"---RECOMMENDATIONS---\s*(\[.*?\])", re.DOTALL)


def extract_recommendations(output_text: str) -> list:
    """Ambil daftar rekomendasi dari blok ---RECOMMENDATIONS--- (bila ada)."""
    if not output_text:
        return []
    m = _RECS_RE.search(output_text)
    if not m:
        return []
    try:
        import json
        data = json.loads(m.group(1))
        if isinstance(data, list):
            return [str(x) for x in data if str(x).strip()]
    except (json.JSONDecodeError, ValueError):
        pass
    return []


# ═══════════════════════════════════════════════════════════════════════════
# MULTI-MODULE EXTRACTION (v1.15.0) — IR dari output LLM bebas
# ═══════════════════════════════════════════════════════════════════════════

_MODULE_START_RE = re.compile(r'(?m)(?<![A-Za-z0-9_])(intent|reply|task|event|memory|plan|state|world)\s*"')
_ALLOWED_MODULES = set(_MODULE_KEYS)


def _find_block_end(text: str, start: int) -> int:
    """Temukan posisi '}' penutup blok protocol (string-aware, world nested)."""
    i = text.find("{", start)
    if i < 0:
        return -1
    depth = 0
    in_str = False
    esc = False
    n = len(text)
    while i < n:
        ch = text[i]
        if in_str:
            if esc:
                esc = False
            elif ch == "\\":
                esc = True
            elif ch == '"':
                in_str = False
        else:
            if ch == '"':
                in_str = True
            elif ch == "{":
                depth += 1
            elif ch == "}":
                depth -= 1
                if depth == 0:
                    return i
        i += 1
    return -1


def extract_modules(text: str):
    """Ekstrak SEMUA blok protocol ADILang dari teks bebas (output LLM)."""
    if not text:
        return []
    out = []
    for m in _MODULE_START_RE.finditer(text):
        end = _find_block_end(text, m.start())
        if end < 0:
            continue
        block = text[m.start():end + 1]
        try:
            mod, tag, fields = parse_module(block)
        except ValueError:
            continue
        if mod not in _ALLOWED_MODULES:
            continue
        ok, _errs = validate_module(block)
        if not ok:
            continue
        out.append((mod, tag, fields))
    return out


def extract_modules_dict(text: str) -> list:
    """extract_modules → list dict IR (module/tag/fields)."""
    return [{"module": m, "tag": t, "fields": dict(f)} for m, t, f in extract_modules(text)]


# ═══════════════════════════════════════════════════════════════════════════
# SELF-HEAL & FORMAL VERIFICATION (v1.7.0, roadmap §4)
# ═══════════════════════════════════════════════════════════════════════════

def check_adilang(text: str) -> list:
    """Static-check teks ADILang (mirror Python dari checker.rs) — offline."""
    from core.adilang_errors import classify_message
    from core.adilang_knowledge import ADILANG_REGISTRY as REG

    mesh_mats = set(REG.get("mesh", [])) | set(REG.get("material", []))
    declarations = set(REG.get("declaration", []))
    diags = []

    for line_no, msg in _balance_report(text):
        diags.append({
            "line": line_no,
            "severity": "ERROR",
            "message": msg,
            "hint": "Periksa pasangan kurung: ( ) [ ] { }.",
            "code": classify_message(msg),
        })

    depth = 0
    for i, ln in enumerate(text.splitlines(), 1):
        stripped = ln.strip()
        if not stripped or stripped.startswith("#"):
            depth += stripped.count("{") - stripped.count("}")
            continue
        m = re.match(r"^(\w+)", stripped)
        if m:
            word = m.group(1)
            if depth == 0 and word not in declarations and word != "world":
                msg = f"Deklarasi tidak dikenal '{word}'"
                diags.append({
                    "line": i,
                    "severity": "WARN",
                    "message": msg,
                    "hint": "Top-level hanya: " + " ".join(sorted(declarations)),
                    "code": classify_message(msg),
                })
        code = re.sub(r"#.*$", "", stripped)
        for bm in re.finditer(r"\b(mesh|material)\s+(\w+)", code):
            builder = bm.group(2)
            if builder not in mesh_mats:
                msg = f"Builder '{builder}' tidak dikenal"
                diags.append({
                    "line": i,
                    "severity": "ERROR",
                    "message": msg,
                    "hint": "Builder sah: " + " ".join(sorted(mesh_mats)),
                    "code": classify_message(msg),
                })
        depth += stripped.count("{") - stripped.count("}")
    return diags


def check_adilang_structured(text: str) -> list:
    """Static-check → list[ADILangError] (error model terstruktur)."""
    from core.adilang_errors import to_adilang_error

    return [to_adilang_error(d) for d in check_adilang(text)]


def _balance_report(text: str) -> list:
    """Keseimbangan kurung → [(line, message)]."""
    pairs = {")": "(", "]": "[", "}": "{"}
    stack = []
    issues = []
    i = 0
    n = len(text)
    line = 1
    in_str = False
    in_line_comment = False
    in_block_comment = False
    esc = False
    while i < n:
        ch = text[i]
        if ch == "\n":
            line += 1
        if in_line_comment:
            i += 1
            if ch == "\n":
                in_line_comment = False
            continue
        if in_block_comment:
            if ch == "*" and i + 1 < n and text[i + 1] == "/":
                in_block_comment = False
                i += 1
            i += 1
            continue
        if in_str:
            if esc:
                esc = False
            elif ch == "\\":
                esc = True
            elif ch == '"':
                in_str = False
            i += 1
            continue
        if ch == "#":
            in_line_comment = True
        elif ch == "/" and i + 1 < n and text[i + 1] == "*":
            in_block_comment = True
            i += 1
        elif ch == '"':
            in_str = True
        elif ch in "([{":
            stack.append((ch, line))
        elif ch in ")]}":
            if stack and stack[-1][0] == pairs[ch]:
                stack.pop()
            else:
                issues.append((line, f"'{ch}' tanpa pasangan"))
        i += 1
    for ch, open_line in stack:
        issues.append((open_line, f"'{ch}' belum ditutup"))
    return issues


def syntax_error_event(source: str, error: str) -> str:
    """Bangun modul `event "syntax_error"` — return code verifikasi formal (§4)."""
    import datetime as _dt

    line = ""
    m = re.search(r"baris (\d+)", error)
    if m:
        line = m.group(1)
    token = ""
    m = re.search(r"'([^']{1,40})'", error)
    if m:
        token = m.group(1)
    low = error.lower()
    if any(k in low for k in ("tidak dikenal", "bukan", "tidak valid")):
        guidance = (
            "Periksa kosakata tertutup (registry) — baca adilang_registry() "
            "atau get_adilang_registry() sebelum menulis ulang."
        )
    elif "tidak ditutup" in low or "tidak tertutup" in low:
        guidance = "Tutup blok/kurung yang belum ditutup pada baris tersebut."
    elif "ekspektasi" in low or "butuh" in low:
        guidance = "Cek urutan token sesuai grammar adilang.ebnf (mis. butuh '{', '=' atau '=>')."
    else:
        guidance = "Periksa sintaks sesuai adilang.ebnf lalu coba kirim ulang."
    from core.adilang_errors import classify_message

    code = classify_message(error)
    guidance = f"[ADILANG-{code}] {guidance}"
    lines = [
        'event "syntax_error" {',
        f"    source {_q(source)}",
    ]
    if line:
        lines.append(f"    line {_q(line)}")
    if token:
        lines.append(f"    token {_q(token)}")
    lines.append(f"    guidance {_q(guidance)}")
    lines.append(f"    at {_q(_dt.datetime.now(_dt.timezone.utc).isoformat())}")
    lines.append("}")
    return "\n".join(lines)


def auto_fix(text: str):
    """Auto-fix loop (§4): perbaiki kesalahan sintaks umum secara heuristik."""
    fixes = []
    lines = []
    for ln in text.splitlines():
        stripped = ln.strip()
        m = re.match(r"^(World|Entity|Camera|Light|Let|If|Return|While|For|Match|Func|On|Frame|Silent|Speak|Click|Intent|Reply|Task|Event|Memory|Plan|State)(?=\s|\{)", stripped)
        if m:
            kw = m.group(1)
            ln = ln.replace(kw, kw.lower(), 1)
            fixes.append(f"lowercase '{kw}' → '{kw.lower()}'")
            stripped = ln.strip()
        m = re.match(r"^(world|entity|camera|light)\s+([A-Za-z_][A-Za-z0-9_]*)(?=\s|\{)", stripped)
        if m:
            kw, ident = m.group(1), m.group(2)
            if not stripped.startswith(f"{kw} \""):
                ln = re.sub(
                    rf"^(\s*){kw}\s+{re.escape(ident)}(?=\s|\{{)",
                    rf'\1{kw} "{ident}"',
                    ln,
                    count=1,
                )
                fixes.append(f"quote id '{ident}' → \"{ident}\"")
        lines.append(ln)
    fixed = "\n".join(lines)
    fixed = _balance(fixed, fixes)
    return fixed, fixes


def _balance(text: str, fixes: list) -> str:
    """Perbaiki keseimbangan kurung (string & komentar dikecualikan)."""
    pairs = {")": "(", "]": "[", "}": "{"}
    closers = {"{": "}", "(": ")", "[": "]"}
    stack = []
    out = []
    i = 0
    n = len(text)
    in_str = False
    in_line_comment = False
    in_block_comment = False
    esc = False
    while i < n:
        ch = text[i]
        if in_line_comment:
            out.append(ch)
            if ch == "\n":
                in_line_comment = False
            i += 1
            continue
        if in_block_comment:
            out.append(ch)
            if ch == "*" and i + 1 < n and text[i + 1] == "/":
                out.append("/")
                i += 2
                in_block_comment = False
                continue
            i += 1
            continue
        if in_str:
            out.append(ch)
            if esc:
                esc = False
            elif ch == "\\":
                esc = True
            elif ch == '"':
                in_str = False
            i += 1
            continue
        if ch == "#":
            in_line_comment = True
            out.append(ch)
            i += 1
            continue
        if ch == "/" and i + 1 < n and text[i + 1] == "*":
            in_block_comment = True
            out.append(ch)
            i += 1
            continue
        if ch == '"':
            in_str = True
            out.append(ch)
            i += 1
            continue
        if ch in "([{":
            stack.append(ch)
            out.append(ch)
        elif ch in ")]}":
            if stack and stack[-1] == pairs[ch]:
                stack.pop()
                out.append(ch)
            else:
                fixes.append(f"hapus '{ch}' tanpa pasangan")
        else:
            out.append(ch)
        i += 1
    for ch in reversed(stack):
        out.append(closers[ch])
        fixes.append(f"tambahkan penutup '{closers[ch]}' untuk '{ch}' yang belum ditutup")
    return "".join(out)


def estimate_token_count(text: str) -> int:
    """Estimasi jumlah token untuk teks ADILang."""
    if not text:
        return 0
    return max(1, len(text) // 4)
