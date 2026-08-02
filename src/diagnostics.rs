// ADILang Diagnostics — protokol error mesin AI-ke-AI (v1.14.0).
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).
//
// Tujuan: error dikirim sebagai PASANGAN (err code hex, node id) yang ringkas
// & deterministik — bukan string pesan manusia yang boros token & ambigu.
// Contoh payload: `{err: 0x0E4, node: 12}`. Kode err berada di rentang
// 0x0E1..=0x0EB (dokumentasi di dense_spec & ADILANG_KNOWLEDGE).

use serde_json::json;

use crate::checker::{check_src, Diagnostic, Severity};

// ── Kode error mesin (tetap, deterministik P1) ───────────────────────────────
pub const ERR_SYNTAX: u16 = 0x0E1;
pub const ERR_UNKNOWN_IDENT: u16 = 0x0E2;
pub const ERR_UNDEFINED_FUNC: u16 = 0x0E3;
pub const ERR_BAD_ARITY: u16 = 0x0E4;
pub const ERR_UNKNOWN_PROP: u16 = 0x0E5;
pub const ERR_UNKNOWN_HOOK: u16 = 0x0E6;
pub const ERR_UNKNOWN_EVENT: u16 = 0x0E7;
pub const ERR_DENSE_FORMAT: u16 = 0x0E8;
pub const ERR_GUARD_SIGNATURE: u16 = 0x0E9;
pub const ERR_UNKNOWN_BLOCK: u16 = 0x0EA;
pub const ERR_RUNTIME: u16 = 0x0EB;

/// Nama kode error (untuk payload & telemetri).
pub fn code_name(code: u16) -> &'static str {
    match code {
        ERR_SYNTAX => "syntax",
        ERR_UNKNOWN_IDENT => "unknown_ident",
        ERR_UNDEFINED_FUNC => "undefined_func",
        ERR_BAD_ARITY => "bad_arity",
        ERR_UNKNOWN_PROP => "unknown_prop",
        ERR_UNKNOWN_HOOK => "unknown_hook",
        ERR_UNKNOWN_EVENT => "unknown_event",
        ERR_DENSE_FORMAT => "dense_format",
        ERR_GUARD_SIGNATURE => "guard_signature",
        ERR_UNKNOWN_BLOCK => "unknown_block",
        ERR_RUNTIME => "runtime",
        _ => "unknown",
    }
}

/// Klasifikasikan string error (dari parser/checker/eval) ke kode mesin.
/// Deterministik berbasis kata kunci — urutan penting (guard > dense >
/// hook > event > arity > prop > func > ident > block > syntax).
pub fn classify(msg: &str) -> u16 {
    let m = msg.to_lowercase();
    let has = |kw: &str| m.contains(kw);
    if has("signature") || has("entropy") || has("handshake") || has("guard") {
        return ERR_GUARD_SIGNATURE;
    }
    if has("bytecode") || has("magic") || has("dense") || has("bitstream") {
        return ERR_DENSE_FORMAT;
    }
    if has("hook") {
        return ERR_UNKNOWN_HOOK;
    }
    if has("event") {
        return ERR_UNKNOWN_EVENT;
    }
    if has("argumen") || has("parameter") || has("arity") || has("jumlah args") {
        return ERR_BAD_ARITY;
    }
    if has("prop") || has("property") {
        return ERR_UNKNOWN_PROP;
    }
    if has("fungsi") || has("function") {
        return ERR_UNDEFINED_FUNC;
    }
    if has("ident") || has("tidak dikenal") || has("unknown") {
        return ERR_UNKNOWN_IDENT;
    }
    if has("block") {
        return ERR_UNKNOWN_BLOCK;
    }
    if has("bukan angka") || has("runtime") {
        return ERR_RUNTIME;
    }
    ERR_SYNTAX
}

/// Node id deterministik: untuk kini = nomor baris (0-based) dalam sumber.
pub fn node_from_line(line: usize) -> u32 {
    line as u32
}

/// Payload error ringkas — format yang dipakai kanal AI-to-AI:
/// `{err: 0x0E4, node: 12}`
pub fn machine_error(code: u16, node: u32) -> String {
    format!("{{err: 0x{code:03X}, node: {node}}}")
}

/// Vektor kode+node super ringkas (paling hemat token): `0E4:12`.
pub fn error_vector(code: u16, node: u32) -> String {
    format!("{code:03X}:{node}")
}

/// Konversi daftar Diagnostic checker → payload JSON mesin (array err/node
/// plus konteks minim). Digunakan WASM `adilang_diag_payload`.
pub fn from_checker(diags: &[Diagnostic]) -> String {
    let items = diags
        .iter()
        .map(|d| {
            let code = classify(&d.message);
            json!({
                "err": code,
                "err_hex": format!("0x{code:03X}"),
                "node": node_from_line(d.line),
                "severity": d.severity.as_str(),
                "message": d.message,
                "hint": d.hint,
            })
        })
        .collect::<Vec<_>>();
    json!({ "count": items.len(), "errors": items }).to_string()
}

/// Konversi hasil operasi (Ok/Err) → payload error mesin ringkas.
pub fn from_result(res: Result<(), String>) -> String {
    match res {
        Ok(()) => "{err: 0x000, node: 0}".to_string(),
        Err(e) => machine_error(classify(&e), 0),
    }
}

/// Laporan diagnostik lengkap sumber: parse+check → payload JSON mesin.
pub fn diagnostics_report(src: &str) -> String {
    match check_src(src) {
        Ok(diags) => from_checker(&diags),
        Err(e) => json!({
            "count": 1,
            "errors": [{
                "err": classify(&e),
                "err_hex": format!("0x{:03X}", classify(&e)),
                "node": 0,
                "severity": Severity::Error.as_str(),
                "message": e,
                "hint": "",
            }],
        })
        .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_error_format_ringkas() {
        assert_eq!(machine_error(ERR_BAD_ARITY, 12), "{err: 0x0E4, node: 12}");
        assert_eq!(error_vector(ERR_BAD_ARITY, 12), "0E4:12");
    }

    #[test]
    fn klasifikasi_error_deterministik() {
        assert_eq!(classify("Component 'X' tanpa hook on_mount"), ERR_UNKNOWN_HOOK);
        assert_eq!(classify("Event tak dikenal 'click'"), ERR_UNKNOWN_EVENT);
        assert_eq!(classify("Jumlah argumen salah"), ERR_BAD_ARITY);
        assert_eq!(classify("Bukan angka: Num(5)"), ERR_RUNTIME);
        assert_eq!(classify("bytecode MAGIC tidak cocok"), ERR_DENSE_FORMAT);
        assert_eq!(classify("Fungsi 'f' tidak terdaftar"), ERR_UNDEFINED_FUNC);
        assert_eq!(classify("Ekspektasi '}'"), ERR_SYNTAX);
    }

    #[test]
    fn diagnostics_report_checker() {
        // Sumber valid → payload tanpa error
        let ok = serde_json::from_str::<serde_json::Value>(&diagnostics_report(
            r#"ui_layout "a" { text "hai" }"#,
        ))
        .expect("json valid");
        assert_eq!(ok["count"].as_u64().unwrap(), 0);

        // Sumber dengan entitas prop tidak dikenal → checker menghasilkan
        // diagnostic; payload memuat pasangan err/node.
        let bad = diagnostics_report(
            r#"
            world "T" {
                entity "e" {
                    posi (0 0 0)
                }
            }
        "#,
        );
        assert!(bad.contains("node"), "payload harus memuat node: {bad}");
        assert!(bad.contains("err"), "payload harus memuat err: {bad}");
    }
}
