"""
adilang/errors.py — Standalone Structured Error Model & Self-Healing Engine (Pure Python Stdlib).
=============================================================================================
Machine-readable & deterministic error classification for ADILang toolchain:
    ADILangError(code, severity, message, hint, line, token, module)

Allows any AI (GPT-4, Claude, Llama, Gemini) to perform instant self-correction (self-heal loop).

Lead Developer: BAGAS ADI PRATAMA S,Kom.
"""
from __future__ import annotations

import re
from dataclasses import dataclass, asdict
from typing import Dict, List, Optional, Union

SEVERITY_ERROR = "ERROR"
SEVERITY_WARN = "WARN"
SEVERITIES = (SEVERITY_ERROR, SEVERITY_WARN)
CODE_UNCLASSIFIED = "E099"

_LINE_RE = re.compile(r"(?:baris|posisi)\s+(\d+)", re.IGNORECASE)
_TOKEN_RE = re.compile(r"'([^']{1,40})'")


@dataclass
class ADILangError:
    code: str
    severity: str
    message: str
    hint: str = ""
    line: Optional[int] = None
    token: Optional[str] = None
    module: Optional[str] = None

    def to_dict(self) -> Dict:
        return asdict(self)

    def __str__(self) -> str:
        out = f"[ADILANG-{self.code}] {self.message}"
        if self.hint:
            out += f" — {self.hint}"
        return out


ERROR_CODES: Dict[str, Dict] = {
    "E001": {
        "severity": SEVERITY_ERROR,
        "hint": "Pastikan dokumen dimulai dengan modul tertutup: intent reply task event memory plan world.",
        "matches": ("bukan modul protocol",),
    },
    "E002": {
        "severity": SEVERITY_ERROR,
        "hint": "String ADILang wajib diapit kutip ganda \"...\" — periksa sekitar posisi yang ditunjukkan.",
        "matches": ("ekspektasi string",),
    },
    "E003": {
        "severity": SEVERITY_ERROR,
        "hint": "Tutup string dengan kutip ganda \" sebelum akhir blok.",
        "matches": ("string tidak ditutup",),
    },
    "E004": {
        "severity": SEVERITY_ERROR,
        "hint": "Lengkapi escape setelah backslash (\\n \\t \\\" \\\\) atau hapus backslash yang menggantung.",
        "matches": ("escape",),
    },
    "E005": {
        "severity": SEVERITY_ERROR,
        "hint": "Setelah tag module wajib ada '{' — cek urutan token sesuai adilang.ebnf.",
        "matches": ("ekspektasi '{'",),
    },
    "E006": {
        "severity": SEVERITY_ERROR,
        "hint": "Tutup blok dengan '}' — blok belum ditutup.",
        "matches": ("blok belum ditutup",),
    },
    "E007": {
        "severity": SEVERITY_ERROR,
        "hint": "Nama kunci wajib huruf/underscore di awal: [A-Za-z_][A-Za-z0-9_]*.",
        "matches": ("kunci tidak valid",),
    },
    "E008": {
        "severity": SEVERITY_ERROR,
        "hint": "Hapus kunci duplikat — satu kunci hanya boleh muncul sekali.",
        "matches": ("duplikat",),
    },
    "E009": {
        "severity": SEVERITY_ERROR,
        "hint": "Nilai array wajib diawali '[' — cek posisi yang ditunjukkan.",
        "matches": ("ekspektasi '['",),
    },
    "E010": {
        "severity": SEVERITY_ERROR,
        "hint": "Tutup array dengan ']' — array belum ditutup.",
        "matches": ("array tidak ditutup",),
    },
    "E020": {
        "severity": SEVERITY_ERROR,
        "hint": "Gunakan hanya kunci tertutup modul tersebut — baca adilang_registry() untuk daftar sah.",
        "matches": ("kunci tidak dikenal",),
    },
    "E021": {
        "severity": SEVERITY_ERROR,
        "hint": "Tambahkan semua kunci wajib modul (lihat spec LANGUAGE.md §15 / registry).",
        "matches": ("kunci wajib",),
    },
    "E022": {
        "severity": SEVERITY_ERROR,
        "hint": "verb hanya: ask inform command greet system.",
        "matches": ("verb tidak valid",),
    },
    "E023": {
        "severity": SEVERITY_ERROR,
        "hint": "recs harus array string: recs [ \"a\" \"b\" ].",
        "matches": ("recs harus",),
    },
    "E024": {
        "severity": SEVERITY_ERROR,
        "hint": "content tidak boleh kosong — isi dengan teks jawaban.",
        "matches": ("content tidak boleh kosong",),
    },
    "E025": {
        "severity": SEVERITY_ERROR,
        "hint": "confidence harus angka 0..1 (mis. \"0.98\").",
        "matches": ("confidence",),
    },
    "E026": {
        "severity": SEVERITY_ERROR,
        "hint": "steps harus array string.",
        "matches": ("steps harus",),
    },
    "E027": {
        "severity": SEVERITY_ERROR,
        "hint": "steps format kanonik \"<id>:<action>:<depends_csv>\" — id angka unik, depends boleh kosong, tanpa cycle (DAG).",
        "matches": ("entry steps", "plan memiliki cycle"),
    },
    "E028": {
        "severity": SEVERITY_ERROR,
        "hint": "parallel hanya \"0\" atau \"1\".",
        "matches": ("parallel harus",),
    },
    "E030": {
        "severity": SEVERITY_WARN,
        "hint": "Top-level hanya deklarasi terdaftar (world camera light entity let func on) — cek ejaan.",
        "matches": ("deklarasi tidak dikenal",),
    },
    "E031": {
        "severity": SEVERITY_ERROR,
        "hint": "Gunakan builder mesh/material terdaftar: sphere box torus icosa ring plane grid / solid wire glow points.",
        "matches": ("builder",),
    },
    "E032": {
        "severity": SEVERITY_ERROR,
        "hint": "Periksa pasangan kurung: ( ) [ ] { }.",
        "matches": ("tanpa pasangan", "' belum ditutup"),
    },
    CODE_UNCLASSIFIED: {
        "severity": SEVERITY_ERROR,
        "hint": "Periksa sintaks sesuai adilang.ebnf lalu coba kirim ulang.",
        "matches": (),
    },
}


def _severity(code: str) -> str:
    meta = ERROR_CODES.get(code, ERROR_CODES[CODE_UNCLASSIFIED])
    return meta["severity"]


def hint_for(code: str) -> str:
    return ERROR_CODES.get(code, ERROR_CODES[CODE_UNCLASSIFIED])["hint"]


def classify_message(message: str) -> str:
    low = (message or "").lower()
    for code, meta in ERROR_CODES.items():
        if any(kw in low for kw in meta.get("matches", ())):
            return code
    return CODE_UNCLASSIFIED


def extract_line(message: str) -> Optional[int]:
    m = _LINE_RE.search(message or "")
    return int(m.group(1)) if m else None


def extract_token(message: str) -> Optional[str]:
    m = _TOKEN_RE.search(message or "")
    return m.group(1) if m else None


def to_adilang_error(
    item: Union[ADILangError, str, Exception, Dict],
    module: Optional[str] = None,
) -> ADILangError:
    if isinstance(item, ADILangError):
        return item
    if isinstance(item, dict):
        message = str(item.get("message", ""))
        code = str(item.get("code") or classify_message(message))
        return ADILangError(
            code=code,
            severity=str(item.get("severity") or _severity(code)),
            message=message,
            hint=str(item.get("hint") or hint_for(code)),
            line=item.get("line"),
            token=item.get("token") or extract_token(message),
            module=item.get("module") or module,
        )
    message = str(item)
    code = classify_message(message)
    return ADILangError(
        code=code,
        severity=_severity(code),
        message=message,
        hint=hint_for(code),
        line=extract_line(message),
        token=extract_token(message),
        module=module,
    )


def normalize_errors(
    items: List[Union[ADILangError, str, Exception, Dict]],
    module: Optional[str] = None,
) -> List[ADILangError]:
    return [to_adilang_error(i, module=module) for i in items]


def error_dicts(errors: List[ADILangError]) -> List[Dict]:
    return [e.to_dict() for e in errors]


def error_texts(errors: List[ADILangError]) -> List[str]:
    return [e.message for e in errors]


def get_error_lexicon() -> Dict:
    return {
        "version": "1.0.0",
        "count": len(ERROR_CODES),
        "codes": {
            code: {"severity": meta["severity"], "hint": meta["hint"]}
            for code, meta in ERROR_CODES.items()
        },
    }
