// ADILang Protocol (adilang_protocol) — transport AI-to-AI zero-token-waste.
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).
//
// Lapisan atas dari bytecode.rs: menyediakan
//   (1) re-export encoder/decoder AST biner,
//   (2) transport teks Base64 untuk channel yang hanya menerima teks
//       (WebSocket, HTTP body, prompt LLM),
//   (3) laporan ukuran (Zero-Token-Waste) untuk verifikasi hemat token.

use crate::ast::Program;
use crate::parser::parse;
use crate::bytecode::{decode_program, encode_program};

pub use crate::bytecode::{
    apply_delta, binary_spec, decode_full, encode_delta, encode_full,
    packet_entity_count, packet_kind, packet_version,
};

const B64_ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encode slice biner → string Base64 standar (RFC 4648) tanpa dependency.
pub fn b64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        let n = (b0 as u32) << 16 | (b1 as u32) << 8 | b2 as u32;
        out.push(B64_ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(B64_ALPHABET[(n >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            out.push(B64_ALPHABET[(n >> 6) as usize & 63] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(B64_ALPHABET[n as usize & 63] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn b64_val(c: u8) -> Option<u8> {
    match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// Decode string Base64 → slice biner. Memvalidasi padding & karakter.
pub fn b64_decode(s: &str) -> Result<Vec<u8>, String> {
    let bytes: Vec<u8> = s.bytes().filter(|b| !b" \t\r\n".contains(b)).collect();
    if bytes.len() % 4 != 0 {
        return Err(format!("Base64 invalid: panjang {} bukan kelipatan 4", bytes.len()));
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks(4) {
        let mut quad = [0u8; 4];
        for (i, b) in chunk.iter().enumerate() {
            if i == 3 && chunk[2] == b'=' {
                quad[i] = 0;
            } else if *b == b'=' {
                quad[i] = 0;
            } else {
                quad[i] = b64_val(*b).ok_or_else(|| format!("Base64 invalid: karakter 0x{:02X}", *b))?;
            }
        }
        let n = ((quad[0] as u32) << 18) | ((quad[1] as u32) << 12) | ((quad[2] as u32) << 6) | quad[3] as u32;
        out.push((n >> 16) as u8);
        if chunk[2] != b'=' {
            out.push((n >> 8) as u8);
        }
        if chunk[3] != b'=' {
            out.push(n as u8);
        }
    }
    Ok(out)
}

/// Parse source ADILang → encode AST → bytecode biner.
pub fn encode_source_to_binary(src: &str) -> Result<Vec<u8>, String> {
    let program = parse(src)?;
    encode_program(&program)
}

/// Parse source ADILang → bytecode biner → Base64 text transport.
pub fn encode_source_to_b64(src: &str) -> Result<String, String> {
    Ok(b64_encode(&encode_source_to_binary(src)?))
}

/// Encode AST yang sudah ada → Base64 text transport.
pub fn encode_ast_to_b64(program: &Program) -> Result<String, String> {
    Ok(b64_encode(&encode_program(program)?))
}

/// Decode Base64 text transport → bytecode biner → AST.
pub fn decode_b64_to_ast(data: &str) -> Result<Program, String> {
    let bin = b64_decode(data)?;
    decode_program(bin)
}

/// Decode Base64 text transport → bytecode biner (mentah).
pub fn decode_b64_to_binary(data: &str) -> Result<Vec<u8>, String> {
    b64_decode(data)
}

/// Laporan Zero-Token-Waste: ukuran source vs binary + rasio hemat + jumlah
/// kata registry terkompresi. Format teks (dokumentasi) & ringkas.
pub fn size_report(src: &str) -> Result<String, String> {
    let program = parse(src)?;
    let bin = encode_program(&program)?;
    let src_bytes = src.len();
    let bin_bytes = bin.len();
    let registry_hits = bin.iter().filter(|&&b| b == 0xFE).count();
    let ratio = if bin_bytes > 0 {
        (1.0 - bin_bytes as f64 / src_bytes.max(1) as f64) * 100.0
    } else {
        0.0
    };
    let mut lines = Vec::new();
    lines.push(format!("ADILang Protocol — Zero-Token-Waste Report"));
    lines.push(format!("  items          : {}", program.items.len()));
    lines.push(format!("  source bytes   : {}", src_bytes));
    lines.push(format!("  binary bytes   : {}", bin_bytes));
    lines.push(format!("  registry words : {} (2-byte tiap kata)", registry_hits));
    lines.push(format!("  saving         : {:.1}%", ratio));
    lines.push(format!("  transport      : {} chars Base64", b64_encode(&bin).len()));
    Ok(lines.join("\n"))
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytecode::binary_spec;

    const SAMPLE: &str = r#"
        @payload {
            sender "ai-1"
            target_agent "ai-2"
            intent "collaborate"
        }
        ui_layout "hud" {
            container {
                flex column
                text "Hello"
                button "Send" onClick send
            }
        }
        spatial_3d "scene" {
            camera "cam" { pos (0 1.6 7) look (0 0 0) fov 55 }
            entity "core" {
                mesh sphere { radius 0.8 segments 3 }
                material wire (0.15 0.8 1) 0.9
                on frame { rotate(0.35 * t, (0.15 1 0.1)) }
            }
        }
    "#;

    #[test]
    fn base64_roundtrip() {
        let data = b"ADI\x00\xFE\xFFproto";
        let enc = b64_encode(data);
        assert_eq!(b64_decode(&enc).expect("decode"), data);
        // roundtrip sembarang bytes
        let mut rand = Vec::new();
        for i in 0..77u8 {
            rand.push(i.wrapping_mul(31));
        }
        assert_eq!(b64_decode(&b64_encode(&rand)).expect("decode"), rand);
    }

    #[test]
    fn base64_menolak_karakter_invalid() {
        assert!(b64_decode("a&b=").is_err());
        assert!(b64_decode("abc").is_err()); // bukan kelipatan 4
    }

    #[test]
    fn protocol_source_to_b64_ke_ast() {
        let transport = encode_source_to_b64(SAMPLE).expect("encode");
        assert!(transport.contains('+') || transport.contains('/') || transport.contains('A'));
        let ast = decode_b64_to_ast(&transport).expect("decode");
        assert_eq!(ast.items.len(), 3);
        // konsisten: decode_b64_to_binary menghasilkan bytecode valid
        let bin = decode_b64_to_binary(&transport).expect("bin");
        assert_eq!(bin[0], 0xAD);
    }

    #[test]
    fn protocol_ast_roundtrip_via_b64() {
        let prog = parse(SAMPLE).expect("parse");
        let b64 = encode_ast_to_b64(&prog).expect("b64");
        let decoded = decode_b64_to_ast(&b64).expect("decode");
        assert_eq!(prog, decoded);
    }

    #[test]
    fn size_report_menampilkan_hemat_token() {
        let report = size_report(SAMPLE).expect("report");
        assert!(report.contains("Zero-Token-Waste"));
        assert!(report.contains("registry words"));
        assert!(report.contains("saving"));
        assert!(report.contains("Base64"));
    }

    #[test]
    fn binary_spec_terdokumentasi() {
        let spec = binary_spec();
        assert!(spec.contains("ADILang Binary Protocol"));
        assert!(spec.contains("0xFE"));
        assert!(spec.contains("0xAF"));
    }
}
