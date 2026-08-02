// ADILang AI Guard — validator & handshake mesin (v1.14.0).
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).
//
// Tujuan: hanya dokumen yang ditulis & ditandatangani MESIN (AI/agent) yang
// valid untuk kanal machine-to-machine. Dokumen yang diubah manusia (berkas
// .adi yang disunting manual) GAGAL verifikasi — menutup celah prompt-injection
// & penyimpangan format. Guard bersifat deterministik (P1): tanpa random seed,
// tanpa source luar; hanya FNV-1a 64-bit (bukan kriptografi — untuk verifikasi
// integritas & determinisme, bukan otentikasi adversarial).

use crate::compactor;

pub const SIGNATURE_PREFIX: &str = "ADILANG-SIG:";
pub const HANDSHAKE_PREFIX: &str = "ADI-HANDSHAKE:";

/// Ambang entropi Shannon (bit/byte). Kode mesin terstruktur & repetitif
/// biasanya di bawah ambang; teks bebas manusia (komentar prosa panjang)
/// cenderung lebih tinggi. Heuristik — verifikasi sejati via signature.
pub const MACHINE_ENTROPY_THRESHOLD: f64 = 6.0;

/// Hasil penilaian guard untuk satu dokumen.
#[derive(Debug, Clone, PartialEq)]
pub struct GuardReport {
    pub valid: bool,
    pub signature: Option<String>,
    pub entropy: f64,
    pub reason: String,
}

/// FNV-1a 64-bit — hash deterministik (P1), portable (std only, tanpa crypto).
pub fn fnv1a64(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Byte kanonik sebuah dokumen: kompak dulu (compactor menghapus komentar &
/// spasi berlebih), lalu byte UTF-8. Suntingan apa pun mengubah byte kanonik.
pub fn canonical_bytes(src: &str) -> Result<Vec<u8>, String> {
    let compact = compactor::optimize_src(src)?;
    Ok(compact.into_bytes())
}

/// Signature hex FNV-1a 64-bit dari dokumen (16 hex).
pub fn signature_hex(src: &str) -> Result<String, String> {
    Ok(format!("{:016x}", fnv1a64(&canonical_bytes(src)?)))
}

/// Baris marker tanda tangan mesin (komentar, jadi tidak mengubah semantik).
pub fn sign(src: &str) -> Result<String, String> {
    Ok(format!("# {} {}", SIGNATURE_PREFIX, signature_hex(src)?))
}

/// Dokumen + marker tanda tangan di baris akhir (untuk dokumen yang akan
/// dikirim antar-agent).
pub fn attach_signature(src: &str) -> Result<String, String> {
    let sig = sign(src)?;
    Ok(format!("{src}\n{sig}\n"))
}

/// Ekstrak hex signature dari marker `# ADILANG-SIG: <hex>`.
pub fn extract_signature(src: &str) -> Option<String> {
    for line in src.lines() {
        if let Some(idx) = line.find(SIGNATURE_PREFIX) {
            let hex = line[idx + SIGNATURE_PREFIX.len()..].trim();
            if !hex.is_empty() {
                return Some(hex.to_string());
            }
        }
    }
    None
}

/// Verifikasi: signature tertanam == signature kanonik (dengan marker
/// dihapus). Dokumen tanpa marker / marker salah → invalid (false).
pub fn verify(src: &str) -> Result<bool, String> {
    let Some(embedded) = extract_signature(src) else {
        return Ok(false);
    };
    let cleaned = src
        .lines()
        .filter(|l| !l.contains(SIGNATURE_PREFIX))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(embedded == signature_hex(&cleaned)?)
}

/// Entropi Shannon (bit/byte) dari byte UTF-8 dokumen.
pub fn machine_entropy(src: &str) -> f64 {
    let bytes = src.as_bytes();
    let len = bytes.len();
    if len == 0 {
        return 0.0;
    }
    let mut counts = [0usize; 256];
    for &b in bytes {
        counts[b as usize] += 1;
    }
    let mut h = 0.0;
    for &c in counts.iter() {
        if c == 0 {
            continue;
        }
        let p = c as f64 / len as f64;
        h -= p * p.log2();
    }
    h
}

/// Penilaian "dihasilkan mesin": signature valid DAN entropi dalam ambang.
/// Suntingan manusia → signature gagal → invalid (reason menjelaskan).
pub fn is_machine_generated(src: &str) -> GuardReport {
    let entropy = machine_entropy(src);
    let sig_ok = verify(src).unwrap_or(false);
    let entropy_ok = entropy <= MACHINE_ENTROPY_THRESHOLD;
    let reason = match (sig_ok, entropy_ok) {
        (true, true) => "tanda tangan mesin valid & entropi dalam ambang",
        (true, false) => {
            "tanda tangan valid tetapi entropi tinggi (kemungkinan suntingan manusia)"
        }
        (false, true) => "tanda tangan mesin tidak valid / tidak ada (suntingan manusia)",
        (false, false) => {
            "tanda tangan mesin tidak valid / tidak ada (suntingan manusia, entropi tinggi)"
        }
    };
    GuardReport {
        valid: sig_ok,
        signature: extract_signature(src),
        entropy,
        reason: reason.to_string(),
    }
}

/// Tantangan handshake (nonce deterministik dari waktu). Mesin lawan harus
/// menjawab dengan `respond(nonce, src)` yang dibuktikan terhadap isi dokumen.
pub fn challenge() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    format!(
        "{HANDSHAKE_PREFIX}-{}",
        format!("{:016x}", fnv1a64(&now.to_le_bytes()))
    )
}

/// Jawaban handshake: FNV-1a(nonce || byte-kanonik dokumen) — bukti bahwa
/// lawan benar-benar memegang dokumen kanonik, tanpa mengirim isi dokumen.
pub fn respond(nonce: &str, src: &str) -> String {
    let mut data = nonce.as_bytes().to_vec();
    if let Ok(canon) = canonical_bytes(src) {
        data.extend_from_slice(&canon);
    }
    format!("{:016x}", fnv1a64(&data))
}

/// Verifikasi jawaban handshake lawan.
pub fn verify_handshake(nonce: &str, src: &str, response: &str) -> bool {
    response.trim() == respond(nonce, src)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = r#"
        ui_layout "main" {
            container {
                flex column
                text "Hello ADILang"
                button "Submit" onClick submit
            }
        }
        world "T" {
            entity "e" { on frame { rotate(0.1, (0 1 0)) } }
        }
    "#;

    #[test]
    fn signature_deterministik_dan_terverifikasi() {
        let signed = attach_signature(SRC).expect("sign ok");
        assert!(verify(&signed).expect("verify ok"), "dokumen bertanda harus valid");
        // Dokumen tanpa tanda tangan → invalid
        assert!(!verify(SRC).expect("verify ok"), "tanpa tanda tangan harus invalid");
    }

    #[test]
    fn suntingan_manusia_membatalkan_signature() {
        let mut signed = attach_signature(SRC).expect("sign ok");
        signed.push_str("\n// disunting oleh manusia\n");
        let report = is_machine_generated(&signed);
        assert!(!report.valid, "suntingan manusia harus membatalkan validitas");
        assert!(report.reason.contains("tanda tangan"), "reason harus menyebut signature");
    }

    #[test]
    fn handshake_dapat_dibuktikan() {
        let nonce = challenge();
        assert!(nonce.starts_with(HANDSHAKE_PREFIX), "nonce harus berprefix handshake");
        let response = respond(&nonce, SRC);
        assert!(verify_handshake(&nonce, SRC, &response), "jawaban benar harus lolos");
        assert!(
            !verify_handshake(&nonce, SRC, &respond(&nonce, &SRC.replacen("Hello", "Halo", 1))),
            "jawaban atas dokumen berbeda harus gagal"
        );
    }

    #[test]
    fn entropi_mesin_di_bawah_ambang() {
        assert!(machine_entropy(SRC) <= MACHINE_ENTROPY_THRESHOLD);
    }

    #[test]
    fn fnv1a64_vektor_uji() {
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a64(b"foobar"), 0x8594_4171_f739_67e8);
    }
}
