// ADILang Self-Healing Engine (adilang_self_heal) — v1.11.0.
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).
//
// Menangkap error parsing/runtime ADILang dan mengubahnya menjadi
// "Diagnostic Payload" hemat-token yang siap dikirim kembali ke LLM:
//   - baris & kolom lokasi error
//   - segmen source yang bermasalah (snippet, dipotong di sekitar error)
//   - pesan & hint perbaikan dari parser/checker
//   - saran auto-fix deterministik (berupa patch ops sederhana) bila memungkinkan
//   - payload termuat dalam JSON ringkas + varian Base64 (transport zero-token-waste)
//
// Strategi LLM round-trip:
//   1. AI menulis ADILang → compile gagal (Err).
//   2. self_heal::payload_from_error(...) → Diagnostic Payload (JSON ringkas).
//   3. Payload dikirim balik ke LLM bersama perintah "perbaiki kode ini".
//   4. LLM memperbaiki → loop sampai bersih (mirror core/adilang_self_heal.py).

use crate::checker::{check_src, Diagnostic, Severity};

/// Kategori sumber error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealSource {
    Parse,     // syntax/lexer error
    Check,     // semantic diagnostic (checker)
    Runtime,   // error evaluator/interpreter
}

/// Satu temuan dalam Diagnostic Payload (hemat-token, tanpa struktur besar).
#[derive(Debug, Clone, PartialEq)]
pub struct HealFinding {
    pub severity: String,   // ERROR / WARN / INFO
    pub source: String,     // parse / check / runtime
    pub line: usize,
    pub column: usize,      // 0 = tidak diketahui
    pub message: String,
    pub hint: String,
    pub snippet: String,    // baris source (atau potongan dekat error)
}

/// Diagnostic Payload — siap serialize ke JSON/Base64.
#[derive(Debug, Clone, PartialEq)]
pub struct HealPayload {
    pub lang: String,       // "adilang"
    pub version: String,    // versi payload, bukan versi bahasa
    pub count: usize,
    pub findings: Vec<HealFinding>,
}

impl HealPayload {
    pub fn empty() -> Self {
        Self {
            lang: "adilang".to_string(),
            version: "1.0".to_string(),
            count: 0,
            findings: Vec::new(),
        }
    }

    /// Serialize ke JSON ringkas (keys 1-2 karakter → hemat token LLM).
    pub fn to_json(&self) -> serde_json::Value {
        let findings: Vec<serde_json::Value> = self
            .findings
            .iter()
            .map(|f| {
                serde_json::json!({
                    "s": f.severity,
                    "c": f.source,
                    "l": f.line,
                    "col": f.column,
                    "m": f.message,
                    "f": f.hint,
                    "sn": f.snippet,
                })
            })
            .collect();
        serde_json::json!({
            "lang": self.lang,
            "v": self.version,
            "n": self.count,
            "d": findings,
        })
    }

    /// JSON string ringkas.
    pub fn to_json_string(&self) -> String {
        self.to_json().to_string()
    }

    /// Varian Base64 (transport) — pakai protocol::b64_encode.
    pub fn to_b64(&self) -> String {
        crate::protocol::b64_encode(self.to_json_string().as_bytes())
    }

    /// Rekonstruksi dari JSON (kebalikan to_json).
    pub fn from_json(j: &serde_json::Value) -> Self {
        let findings: Vec<HealFinding> = j
            .get("d")
            .and_then(|d| d.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|f| {
                        Some(HealFinding {
                            severity: f.get("s")?.as_str()?.to_string(),
                            source: f.get("c")?.as_str()?.to_string(),
                            line: f.get("l")?.as_u64()? as usize,
                            column: f.get("col")?.as_u64().unwrap_or(0) as usize,
                            message: f.get("m")?.as_str()?.to_string(),
                            hint: f.get("f")?.as_str().unwrap_or("").to_string(),
                            snippet: f.get("sn")?.as_str().unwrap_or("").to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self {
            lang: j.get("lang").and_then(|v| v.as_str()).unwrap_or("adilang").to_string(),
            version: j.get("v").and_then(|v| v.as_str()).unwrap_or("1.0").to_string(),
            count: findings.len(),
            findings,
        }
    }

    pub fn from_json_string(s: &str) -> Result<Self, String> {
        let v: serde_json::Value =
            serde_json::from_str(s).map_err(|e| format!("JSON invalid: {e}"))?;
        Ok(Self::from_json(&v))
    }

    pub fn from_b64(s: &str) -> Result<Self, String> {
        let bytes = crate::protocol::b64_decode(s)?;
        let text = String::from_utf8(bytes).map_err(|e| format!("UTF-8 invalid: {e}"))?;
        Self::from_json_string(&text)
    }
}

/// Ambil baris source (1-based). Baris di luar rentang → "".
fn line_at(src: &str, line: usize) -> String {
    src.lines().nth(line.saturating_sub(1)).unwrap_or("").trim_end().to_string()
}

/// Snippet terpotong di sekitar error (maks 72 karakter, potong di tengah
/// bila baris panjang → hemat token).
fn snippet_for(src: &str, line: usize, max: usize) -> String {
    let raw = line_at(src, line);
    if raw.chars().count() <= max {
        return raw;
    }
    let chars: Vec<char> = raw.chars().collect();
    let half = max / 2;
    let mut out: String = chars[..half].iter().collect();
    out.push_str("…");
    out.push_str(&chars[chars.len() - half..].iter().collect::<String>());
    out
}

/// Parse baris & kolom dari pesan error parser (format: "...baris N kolom M"
/// atau "...di baris N") tanpa dependency regex.
fn extract_baris_kolom(msg: &str) -> (usize, usize) {
    // "baris N kolom M"
    let mut words = msg.split_whitespace().peekable();
    let mut line: usize = 0;
    let mut col: usize = 0;
    while let Some(w) = words.next() {
        if w == "baris" {
            if let Some(n) = words.next().and_then(|s| s.parse::<usize>().ok()) {
                line = n;
            }
        } else if w == "kolom" {
            if let Some(n) = words.next().and_then(|s| s.parse::<usize>().ok()) {
                col = n;
            }
        }
    }
    (line, col)
}

/// Bangun payload dari pesan error tunggal (parse/runtime) + source.
pub fn payload_from_error(src: &str, err: &str, source_kind: HealSource) -> HealPayload {
    let (line, col) = extract_baris_kolom(err);
    let (line, col) = if line > 0 { (line, col) } else { parse_line_col_fallback(err) };
    let finding = HealFinding {
        severity: "ERROR".to_string(),
        source: match source_kind {
            HealSource::Parse => "parse".to_string(),
            HealSource::Check => "check".to_string(),
            HealSource::Runtime => "runtime".to_string(),
        },
        line,
        column: col,
        message: err.to_string(),
        hint: auto_fix_hint(err),
        snippet: snippet_for(src, line, 72),
    };
    HealPayload {
        lang: "adilang".to_string(),
        version: "1.0".to_string(),
        count: 1,
        findings: vec![finding],
    }
}

fn parse_line_col_fallback(err: &str) -> (usize, usize) {
    let mut line = 0usize;
    for w in err.split_whitespace() {
        if let Ok(n) = w.trim_matches(|c: char| !c.is_ascii_digit()).parse::<usize>() {
            if n > 0 {
                line = n;
                break;
            }
        }
    }
    (line, 0)
}

/// Bangun payload dari seluruh hasil checker (Vec<Diagnostic>).
pub fn payload_from_check(src: &str, diags: &[Diagnostic]) -> HealPayload {
    let findings: Vec<HealFinding> = diags
        .iter()
        .map(|d| HealFinding {
            severity: match d.severity {
                Severity::Error => "ERROR".to_string(),
                Severity::Warning => "WARN".to_string(),
                Severity::Info => "INFO".to_string(),
            },
            source: "check".to_string(),
            line: d.line,
            column: 0,
            message: d.message.clone(),
            hint: d.hint.clone(),
            snippet: snippet_for(src, d.line, 72),
        })
        .collect();
    HealPayload {
        lang: "adilang".to_string(),
        version: "1.0".to_string(),
        count: findings.len(),
        findings,
    }
}

/// Jalur lengkap: parse → check → payload (0 call jika bersih → empty payload).
/// Bila parse gagal → payload 1 temuan parse.
pub fn heal_check(src: &str) -> Result<HealPayload, String> {
    match check_src(src) {
        Ok(diags) => Ok(payload_from_check(src, &diags)),
        Err(e) => Ok(payload_from_error(src, &e, HealSource::Parse)),
    }
}

/// Auto-fix hint deterministik dari pesan error (dipakai LLM & auto-heal loop).
fn auto_fix_hint(err: &str) -> String {
    let e = err.to_lowercase();
    if e.contains("mesh builder tidak dikenal")
        || (e.contains("tidak dikenal") && e.contains("sphre"))
    {
        "Periksa ejaan mesh builder: sphere box torus icosa ring plane grid.".to_string()
    } else if e.contains("material builder tidak dikenal") {
        "Periksa ejaan material: solid wire glow points.".to_string()
    } else if e.contains("string tidak ditutup") {
        "Tutup string dengan kutip ganda \" ... \".".to_string()
    } else if e.contains("variabel tidak dikenal") {
        "Deklarasikan dengan let, atau gunakan builtin (t mouseX mouseY PI).".to_string()
    } else if e.contains("fungsi tidak dikenal") {
        "Gunakan builtin (move setPos rotate sin cos clamp) atau definisikan func.".to_string()
    } else if e.contains("tidak ditutup") {
        "Tutup blok dengan '}' (atau ')' / ']').".to_string()
    } else if e.contains("event tidak dikenal") {
        "Event sah: frame speak silent click.".to_string()
    } else if e.contains("payload membutuhkan") {
        "Isi sender, target_agent, dan intent pada blok @payload.".to_string()
    } else if e.contains("wildcard") {
        "Wildcard '_' wajib menjadi arm TERAKHIR di match.".to_string()
    } else if e.contains("bukan angka") {
        "Berikan angka, bukan teks/ident, pada posisi ini.".to_string()
    } else {
        "Periksa sintaks ADILang di sekitar lokasi yang ditunjukkan.".to_string()
    }
}

/// Saran perbaikan kompak (ringkasan 1 baris untuk prompt LLM).
pub fn summarize(payload: &HealPayload) -> String {
    if payload.count == 0 {
        return "bersih".to_string();
    }
    let first = &payload.findings[0];
    format!(
        "{}/{} baris {}: {} — {}",
        first.source, first.line, first.severity, first.message, first.hint
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_dari_error_parse_memuat_baris() {
        let src = "world \"w\" {\n  entity \"e\" {\n    mesh sphre { radius 1 }\n  }\n}\n";
        // parse gagal karena sphre bukan builder → Err dari parser
        let res = heal_check(src);
        assert!(res.is_ok(), "heal_check harus Ok bahkan saat parse gagal");
        let p = res.unwrap();
        assert_eq!(p.count, 1);
        assert_eq!(p.findings[0].source, "parse");
        assert_eq!(p.findings[0].severity, "ERROR");
        assert!(p.findings[0].line >= 3, "baris error = {}", p.findings[0].line);
    }

    #[test]
    fn payload_dari_error_runtime_manual() {
        let src = "entity \"e\" { pos (0 0 0) }";
        let p = payload_from_error(src, "Variabel tidak dikenal 'foo' di baris 7", HealSource::Runtime);
        assert_eq!(p.count, 1);
        assert_eq!(p.findings[0].line, 7);
        assert_eq!(p.findings[0].source, "runtime");
        assert!(p.findings[0].hint.contains("let"));
    }

    #[test]
    fn payload_bersih_untuk_code_sehat() {
        let src = include_str!("../worlds/default.adi");
        let p = heal_check(src).expect("heal");
        assert_eq!(p.count, 0, "default.adi harus bersih");
        assert_eq!(summarize(&p), "bersih");
    }

    #[test]
    fn json_roundtrip() {
        let src = "world \"w\" {\n entity \"e\" {\n mesh sphre { radius 1 }\n}\n}\n";
        let p = heal_check(src).unwrap();
        let json = p.to_json_string();
        let back = HealPayload::from_json_string(&json).unwrap();
        assert_eq!(p.count, back.count);
        assert_eq!(p.findings[0].line, back.findings[0].line);
        assert_eq!(p.findings[0].message, back.findings[0].message);
    }

    #[test]
    fn b64_roundtrip() {
        let src = "world \"w\" {\n entity \"e\" {\n mesh sphre { radius 1 }\n}\n}\n";
        let p = heal_check(src).unwrap();
        let b64 = p.to_b64();
        let back = HealPayload::from_b64(&b64).unwrap();
        assert_eq!(p.count, back.count);
        assert_eq!(p.findings[0].message, back.findings[0].message);
    }

    #[test]
    fn extract_baris_kolom_pola_lexer() {
        let (l, c) = extract_baris_kolom("Karakter tidak dikenal '#' baris 3 kolom 5");
        assert_eq!(l, 3);
        assert_eq!(c, 5);
    }

    #[test]
    fn snippet_dipotong_di_tengah() {
        let long = format!("x = {}\n", "1.0 * ".repeat(40));
        let sn = snippet_for(&long, 1, 72);
        assert!(sn.chars().count() <= 76, "snippet terlalu panjang: {}", sn.chars().count());
        assert!(sn.contains('…'));
    }

    #[test]
    fn hint_menutup_blok() {
        let h = auto_fix_hint("'ui_layout' tidak ditutup, baris 5");
        assert!(h.contains("}"));
    }

    #[test]
    fn semantik_diagnosa_diubah_menjadi_payload() {
        let src = "world \"T\" {\n entity \"e\" {\n  on frame {\n   let a = 1\n   setPos(unknownVar, a, 0)\n  }\n }\n}\n";
        let p = heal_check(src).unwrap();
        assert!(p.count >= 1);
        assert!(p.findings.iter().any(|f| f.message.contains("unknownVar")));
        assert!(p.findings.iter().all(|f| f.source == "check"));
    }
}
