// ADILang binary / bytecode — bit-packed transport format (v1.4.0).
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).
//
// Modul ini adalah API transport WASM-only: pub item-nya dipakai oleh
// wasm_api.rs (target wasm32) dan unit test. Pada build NATIVE (rlib,
// cargo test), tidak ada caller non-test sehingga dead_code warning muncul
// untuk konstanta/fungsi yang sah — allow di sini lebih jujur daripada
// menambahkan #[allow] per item (pattern sama untuk engine/wasm_api).
//
// Teks ADILang ringkas & mudah dibaca AI. Untuk komunikasi antar-client
// real-time (ribuan paket/detik, multiplayer via WebSocket), source text
// dikompilasi oleh Rust menjadi BINER RINGKAS (bit-packing + delta encoding):
//
//   FULL  snapshot  → dikirim saat client join / struktur berubah.
//   DELTA packet    → per-frame, HANYA field yang berubah (mask-based).
//
// Format (deterministik, P1; closed-vocabulary, P6):
//   Header 4 byte:
//     [0] 0xAD            (magic "ADI")
//     [1] 0x01            (format version)
//     [2] flags: bit0 = 1 → DELTA, 0 → FULL
//     [3] entity_count u8 (< 256 entity)
//   FULL — per entity 21 byte:
//     [0] mesh(3b) | material(2b) | reserved(3b)   ← bit-packing
//     [1] id u8 (indeks entity pada snapshot)
//     [2..8]   pos  i16 × 3 (kuantisasi 0.01, rentang ±327.67)
//     [8..14]  rot  i16 × 3 (kuantisasi 0.001 rad)
//     [14..17] scale u8 × 3 (kuantisasi 0.02, rentang 0..5.1)
//     [17..21] color u8 × 4 (rgba 0..255)
//   DELTA — per entity berubah:
//     [0] id u8
//     [1] mask: bit0 pos, bit1 rot, bit2 scale, bit3 color, bit4 mesh/material
//     lalu field sesuai bit yang aktif (pos 6B, rot 6B, scale 3B, color 4B,
//     mesh/material 1B). DELTA hanya valid bila jumlah entity sama dengan
//     baseline (struktur berubah = kirim FULL baru).

#![allow(dead_code)]

use crate::scene::{EntityState, MaterialKind, MeshKind};

/// Magic byte pertama header ("ADI").
pub const MAGIC: u8 = 0xAD;
/// Versi format biner (independent dari versi bahasa; v1).
pub const BIN_VERSION: u8 = 0x01;
/// Flags: bit0 — DELTA (1) vs FULL (0).
pub const FLAG_DELTA: u8 = 0x01;
/// Jumlah entity maksimal per paket (id u8).
pub const MAX_ENTITIES: usize = 255;

// ── Kuantisasi (deterministik, P1) ───────────────────────────────────────────
const POS_SCALE: f64 = 100.0;   // i16 → presisi 0.01
const ROT_SCALE: f64 = 1000.0;  // i16 → presisi 0.001 rad
const SCALE_DIV: f64 = 50.0;    // u8 → presisi 0.02, rentang 0..5.1
const COLOR_SCALE: f64 = 255.0;

fn q_pos(v: f64) -> i16 {
    (v * POS_SCALE).round().clamp(i16::MIN as f64, i16::MAX as f64) as i16
}
fn dq_pos(q: i16) -> f64 {
    q as f64 / POS_SCALE
}
fn q_rot(v: f64) -> i16 {
    (v * ROT_SCALE).round().clamp(i16::MIN as f64, i16::MAX as f64) as i16
}
fn dq_rot(q: i16) -> f64 {
    q as f64 / ROT_SCALE
}
fn q_scale(v: f64) -> u8 {
    (v * SCALE_DIV).round().clamp(0.0, u8::MAX as f64) as u8
}
fn dq_scale(q: u8) -> f64 {
    q as f64 / SCALE_DIV
}
fn q_color(v: f64) -> u8 {
    (v * COLOR_SCALE).round().clamp(0.0, 255.0) as u8
}
fn dq_color(q: u8) -> f64 {
    q as f64 / COLOR_SCALE
}

fn mesh_bits(m: MeshKind) -> u8 {
    match m {
        MeshKind::Sphere => 0,
        MeshKind::Box => 1,
        MeshKind::Torus => 2,
        MeshKind::Icosa => 3,
        MeshKind::Ring => 4,
        MeshKind::Plane => 5,
        MeshKind::Grid => 6,
    }
}
fn material_bits(m: MaterialKind) -> u8 {
    match m {
        MaterialKind::Solid => 0,
        MaterialKind::Wire => 1,
        MaterialKind::Glow => 2,
        MaterialKind::Points => 3,
    }
}
fn bits_mesh(b: u8) -> MeshKind {
    match b & 0x07 {
        1 => MeshKind::Box,
        2 => MeshKind::Torus,
        3 => MeshKind::Icosa,
        4 => MeshKind::Ring,
        5 => MeshKind::Plane,
        6 => MeshKind::Grid,
        _ => MeshKind::Sphere,
    }
}
fn bits_material(b: u8) -> MaterialKind {
    match (b >> 3) & 0x03 {
        1 => MaterialKind::Wire,
        2 => MaterialKind::Glow,
        3 => MaterialKind::Points,
        _ => MaterialKind::Solid,
    }
}

// ── PUBLIC API (terverifikasi registry scripts/check_adilang_registry.py) ────

/// Encode FULL snapshot seluruh entity → bytecode deterministik.
pub fn encode_full(entities: &[EntityState]) -> Result<Vec<u8>, String> {
    if entities.len() > MAX_ENTITIES {
        return Err(format!(
            "Terlalu banyak entity: {} (maks {})",
            entities.len(),
            MAX_ENTITIES
        ));
    }
    let mut out = vec![MAGIC, BIN_VERSION, 0, entities.len() as u8];
    for (idx, e) in entities.iter().enumerate() {
        let header = mesh_bits(e.mesh) | (material_bits(e.material) << 3);
        out.push(header);
        out.push(idx as u8); // id = indeks snapshot (stabil antar frame)
        for c in &e.transform.pos {
            out.extend_from_slice(&q_pos(*c).to_le_bytes());
        }
        for c in &e.transform.rot {
            out.extend_from_slice(&q_rot(*c).to_le_bytes());
        }
        for c in &e.transform.scale {
            out.push(q_scale(*c));
        }
        for c in &e.color {
            out.push(q_color(*c));
        }
    }
    Ok(out)
}

/// Validasi header FULL & kembalikan jumlah entity (magic/version/flag DELTA).
fn parse_full_header(bytes: &[u8]) -> Result<usize, String> {
    if bytes.len() < 4 {
        return Err("Paket ADILang binary terlalu pendek (< 4 byte)".into());
    }
    if bytes[0] != MAGIC {
        return Err(format!("Magic salah: 0x{:02X} (harus 0x{:02X})", bytes[0], MAGIC));
    }
    if bytes[1] != BIN_VERSION {
        return Err(format!(
            "Versi format biner salah: {} (harus {})",
            bytes[1], BIN_VERSION
        ));
    }
    if bytes[2] & FLAG_DELTA != 0 {
        return Err("Ini paket DELTA, bukan FULL".into());
    }
    Ok(bytes[3] as usize)
}

/// Decode FULL snapshot → daftar EntityState (id = "e{index}").
pub fn decode_full(bytes: &[u8]) -> Result<Vec<EntityState>, String> {
    let n = parse_full_header(bytes)?;
    let mut rest = &bytes[4..];
    let mut out = Vec::with_capacity(n);
    for idx in 0..n {
        if rest.len() < 21 {
            return Err(format!("Paket FULL terpotong: entity #{idx}"));
        }
        let h = rest[0];
        let mesh = bits_mesh(h);
        let material = bits_material(h);
        let pos = [
            dq_pos(i16::from_le_bytes([rest[2], rest[3]])),
            dq_pos(i16::from_le_bytes([rest[4], rest[5]])),
            dq_pos(i16::from_le_bytes([rest[6], rest[7]])),
        ];
        let rot = [
            dq_rot(i16::from_le_bytes([rest[8], rest[9]])),
            dq_rot(i16::from_le_bytes([rest[10], rest[11]])),
            dq_rot(i16::from_le_bytes([rest[12], rest[13]])),
        ];
        let scale = [
            dq_scale(rest[14]),
            dq_scale(rest[15]),
            dq_scale(rest[16]),
        ];
        let color = [
            dq_color(rest[17]),
            dq_color(rest[18]),
            dq_color(rest[19]),
            dq_color(rest[20]),
        ];
        out.push(EntityState {
            id: format!("e{idx}"),
            transform: crate::scene::Transform { pos, rot, scale },
            color,
            material,
            mesh,
            mesh_params: crate::scene::MeshParams::default(),
            handlers: Vec::new(),
        });
        rest = &rest[21..];
    }
    Ok(out)
}

/// Encode DELTA (perubahan field) dari baseline `prev` ke `next`.
///
/// Mengembalikan None bila jumlah entity berbeda — struktur berubah → caller
/// harus mengirim FULL snapshot (dokumentasi format). `next`/`prev` harus
/// urutannya konsisten (id = indeks).
pub fn encode_delta(prev: &[EntityState], next: &[EntityState]) -> Option<Vec<u8>> {
    if prev.len() != next.len() || next.len() > MAX_ENTITIES {
        return None;
    }
    let mut out = vec![MAGIC, BIN_VERSION, FLAG_DELTA, next.len() as u8];
    for (idx, (p, n)) in prev.iter().zip(next.iter()).enumerate() {
        let mut mask = 0u8;
        // Bandingkan pada RESOLUSI KUANTISASI (bukan f64 mentah) — entity yang
        // tidak berubah secara visual (delta < presisi format) → tanpa mask.
        // `prev` (decode_full) sudah terkuantisasi; `next` bisa f64 penuh.
        if q_pos(p.transform.pos[0]) != q_pos(n.transform.pos[0])
            || q_pos(p.transform.pos[1]) != q_pos(n.transform.pos[1])
            || q_pos(p.transform.pos[2]) != q_pos(n.transform.pos[2])
        {
            mask |= 0x01;
        }
        if q_rot(p.transform.rot[0]) != q_rot(n.transform.rot[0])
            || q_rot(p.transform.rot[1]) != q_rot(n.transform.rot[1])
            || q_rot(p.transform.rot[2]) != q_rot(n.transform.rot[2])
        {
            mask |= 0x02;
        }
        if q_scale(p.transform.scale[0]) != q_scale(n.transform.scale[0])
            || q_scale(p.transform.scale[1]) != q_scale(n.transform.scale[1])
            || q_scale(p.transform.scale[2]) != q_scale(n.transform.scale[2])
        {
            mask |= 0x04;
        }
        if q_color(p.color[0]) != q_color(n.color[0])
            || q_color(p.color[1]) != q_color(n.color[1])
            || q_color(p.color[2]) != q_color(n.color[2])
            || q_color(p.color[3]) != q_color(n.color[3])
        {
            mask |= 0x08;
        }
        if p.mesh != n.mesh || p.material != n.material {
            mask |= 0x10;
        }
        if mask == 0 {
            continue;
        }
        out.push(idx as u8);
        out.push(mask);
        if mask & 0x01 != 0 {
            for c in &n.transform.pos {
                out.extend_from_slice(&q_pos(*c).to_le_bytes());
            }
        }
        if mask & 0x02 != 0 {
            for c in &n.transform.rot {
                out.extend_from_slice(&q_rot(*c).to_le_bytes());
            }
        }
        if mask & 0x04 != 0 {
            for c in &n.transform.scale {
                out.push(q_scale(*c));
            }
        }
        if mask & 0x08 != 0 {
            for c in &n.color {
                out.push(q_color(*c));
            }
        }
        if mask & 0x10 != 0 {
            let h = mesh_bits(n.mesh) | (material_bits(n.material) << 3);
            out.push(h);
        }
    }
    Some(out)
}

/// Terapkan DELTA ke snapshot `prev` → snapshot terbaru.
pub fn apply_delta(prev: &[EntityState], bytes: &[u8]) -> Result<Vec<EntityState>, String> {
    if bytes.len() < 4 || bytes[0] != MAGIC || bytes[1] != BIN_VERSION {
        return Err("Bukan paket ADILang binary (magic/version salah)".into());
    }
    if bytes[2] & FLAG_DELTA == 0 {
        return Err("Bukan paket DELTA (flags bit0 = 0)".into());
    }
    let count = bytes[3] as usize;
    if count != prev.len() {
        return Err(format!(
            "DELTA count ({count}) ≠ baseline ({}) — kirim FULL snapshot",
            prev.len()
        ));
    }
    let mut out = prev.to_vec();
    let mut rest = &bytes[4..];
    while !rest.is_empty() {
        if rest.len() < 2 {
            return Err("Paket DELTA terpotong (butuh id+mask)".into());
        }
        let idx = rest[0] as usize;
        let mask = rest[1];
        rest = &rest[2..];
        if mask == 0 {
            // Encoder TIDAK pernah meng-emit entry dengan mask 0 (entity tak
            // berubah = dilewati). Entry mask 0 = byte sisa/garbage → tolak.
            return Err(format!("Paket DELTA rusak: entry #{idx} mask 0 (byte sisa?)"));
        }
        if idx >= out.len() {
            return Err(format!("DELTA id {idx} di luar baseline ({})", out.len()));
        }
        if mask & 0x01 != 0 {
            if rest.len() < 6 {
                return Err("DELTA terpotong: pos".into());
            }
            out[idx].transform.pos = [
                dq_pos(i16::from_le_bytes([rest[0], rest[1]])),
                dq_pos(i16::from_le_bytes([rest[2], rest[3]])),
                dq_pos(i16::from_le_bytes([rest[4], rest[5]])),
            ];
            rest = &rest[6..];
        }
        if mask & 0x02 != 0 {
            if rest.len() < 6 {
                return Err("DELTA terpotong: rot".into());
            }
            out[idx].transform.rot = [
                dq_rot(i16::from_le_bytes([rest[0], rest[1]])),
                dq_rot(i16::from_le_bytes([rest[2], rest[3]])),
                dq_rot(i16::from_le_bytes([rest[4], rest[5]])),
            ];
            rest = &rest[6..];
        }
        if mask & 0x04 != 0 {
            if rest.len() < 3 {
                return Err("DELTA terpotong: scale".into());
            }
            out[idx].transform.scale = [dq_scale(rest[0]), dq_scale(rest[1]), dq_scale(rest[2])];
            rest = &rest[3..];
        }
        if mask & 0x08 != 0 {
            if rest.len() < 4 {
                return Err("DELTA terpotong: color".into());
            }
            out[idx].color = [
                dq_color(rest[0]),
                dq_color(rest[1]),
                dq_color(rest[2]),
                dq_color(rest[3]),
            ];
            rest = &rest[4..];
        }
        if mask & 0x10 != 0 {
            if rest.is_empty() {
                return Err("DELTA terpotong: mesh/material".into());
            }
            let h = rest[0];
            out[idx].mesh = bits_mesh(h);
            out[idx].material = bits_material(h);
            rest = &rest[1..];
        }
    }
    // Loop berakhir HANYA bila `rest` kosong (kondisi while). Entry mask 0 yang
    // menandakan byte sisa sudah ditolak di atas — format ketat, tanpa celah.
    Ok(out)
}

/// Jenis paket: FULL (0) / DELTA (1) — dari byte flags.
pub fn packet_kind(bytes: &[u8]) -> u8 {
    if bytes.len() >= 4 {
        bytes[2] & FLAG_DELTA
    } else {
        0
    }
}

/// Versi format biner dari header.
pub fn packet_version(bytes: &[u8]) -> u8 {
    if bytes.len() >= 2 {
        bytes[1]
    } else {
        0
    }
}

/// Jumlah entity pada paket (dari header).
pub fn packet_entity_count(bytes: &[u8]) -> u8 {
    if bytes.len() >= 4 {
        bytes[3]
    } else {
        0
    }
}

/// Spesifikasi format biner (teks, untuk registry/docs/AI).
pub fn binary_spec() -> String {
    format!(
        "ADILangBinary v{} — magic 0x{:02X}, FULL 21B/entity (mesh3b|mat2b|id|pos i16x3|rot i16x3|scale u8x3|color u8x4), DELTA mask-based (pos|rot|scale|color|mesh), kuantisasi deterministik, max {} entity",
        BIN_VERSION, MAGIC, MAX_ENTITIES
    )
}

// ── Tests ────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{EntityState, MaterialKind, MeshKind, Transform};

    fn entity(idx: usize, pos: [f64; 3], color: [f64; 4]) -> EntityState {
        EntityState {
            id: format!("e{idx}"),
            transform: Transform { pos, rot: [0.0, 0.3, 0.0], scale: [1.0, 1.0, 1.0] },
            color,
            material: MaterialKind::Wire,
            mesh: MeshKind::Sphere,
            mesh_params: Default::default(),
            handlers: Vec::new(),
        }
    }

    fn sample_world() -> Vec<EntityState> {
        vec![
            entity(0, [1.5, -0.25, 3.75], [0.2, 0.8, 1.0, 0.9]),
            entity(1, [-2.0, 0.5, -1.2], [1.0, 0.45, 0.15, 1.0]),
            entity(2, [0.0, 1.0, 0.0], [0.3, 1.0, 0.45, 0.5]),
        ]
    }

    #[test]
    fn full_roundtrip_eksak() {
        let w = sample_world();
        let bytes = encode_full(&w).unwrap();
        let back = decode_full(&bytes).unwrap();
        assert_eq!(back.len(), w.len());
        for (a, b) in w.iter().zip(back.iter()) {
            // id di-regenerate dari indeks — cocok
            assert_eq!(a.id, b.id);
            // mesh/material eksak (bit-packing tanpa loss)
            assert_eq!(a.mesh, b.mesh);
            assert_eq!(a.material, b.material);
            // nilai kuantisasi: toleransi presisi format
            for k in 0..3 {
                assert!((a.transform.pos[k] - b.transform.pos[k]).abs() < 0.01, "pos[{k}]");
                assert!((a.transform.rot[k] - b.transform.rot[k]).abs() < 0.001, "rot[{k}]");
                assert!((a.transform.scale[k] - b.transform.scale[k]).abs() < 0.02, "scale[{k}]");
            }
            for k in 0..4 {
                assert!((a.color[k] - b.color[k]).abs() < 0.005, "color[{k}]");
            }
        }
    }

    #[test]
    fn full_roundtrip_semua_mesh_dan_material() {
        let meshes = [
            MeshKind::Sphere,
            MeshKind::Box,
            MeshKind::Torus,
            MeshKind::Icosa,
            MeshKind::Ring,
            MeshKind::Plane,
            MeshKind::Grid,
        ];
        let mats = [
            MaterialKind::Solid,
            MaterialKind::Wire,
            MaterialKind::Glow,
            MaterialKind::Points,
        ];
        let w: Vec<EntityState> = meshes
            .iter()
            .flat_map(|&m| {
                mats.iter().map(move |&mat| EntityState {
                    id: "x".into(),
                    transform: Transform::default(),
                    color: [0.5, 0.5, 0.5, 1.0],
                    material: mat,
                    mesh: m,
                    mesh_params: Default::default(),
                    handlers: Vec::new(),
                })
            })
            .collect();
        let back = decode_full(&encode_full(&w).unwrap()).unwrap();
        for (a, b) in w.iter().zip(back.iter()) {
            assert_eq!(a.mesh, b.mesh, "mesh must roundtrip eksak");
            assert_eq!(a.material, b.material, "material must roundtrip eksak");
        }
    }

    #[test]
    fn delta_roundtrip_hanya_field_berubah() {
        let prev = sample_world();
        let mut next = prev.clone();
        next[0].transform.pos = [9.5, -4.0, 2.25];
        next[2].color = [1.0, 1.0, 0.0, 0.8];
        let delta = encode_delta(&prev, &next).expect("delta harus ada (count sama)");
        // Harus DELTA, dan jauh lebih kecil dari FULL (hanya 2 entity berubah)
        assert_eq!(packet_kind(&delta), FLAG_DELTA);
        let applied = apply_delta(&prev, &delta).unwrap();
        assert_eq!(applied[0].transform.pos[0], 9.5);
        assert_eq!(applied[2].color[0], 1.0);
        // Entity yang tidak berubah tetap utuh
        assert_eq!(applied[1].transform.pos, prev[1].transform.pos);
    }

    #[test]
    fn delta_none_bila_jumlah_entity_berbeda() {
        let prev = sample_world();
        let mut next = prev.clone();
        next.push(entity(3, [0.0, 0.0, 0.0], [1.0, 1.0, 1.0, 1.0]));
        assert!(encode_delta(&prev, &next).is_none(), "struktur berubah → wajib FULL");
    }

    #[test]
    fn delta_identik_menghasilkan_paket_kecil() {
        let prev = sample_world();
        let next = prev.clone(); // tidak ada perubahan
        let delta = encode_delta(&prev, &next).unwrap();
        assert_eq!(delta.len(), 4, "tanpa perubahan → hanya header 4 byte");
        let applied = apply_delta(&prev, &delta).unwrap();
        assert_eq!(applied.len(), prev.len());
    }

    #[test]
    fn delta_menekan_noise_bawah_resolusi_kuantisasi() {
        // Regression test: `prev` hasil decode_full sudah terkuantisasi,
        // `next` dari world live f64 penuh. Noise < presisi format (0.01 pos,
        // 0.001 rot, 0.02 scale, 0.004 color) TIDAK boleh menandai field
        // sebagai berubah — jika tidak, DELTA membengkak tiap frame.
        let quantized = decode_full(&encode_full(&sample_world()).unwrap()).unwrap();
        let mut noisy = quantized.clone();
        noisy[0].transform.pos[0] += 0.001; // < 0.01 → tidak terlihat
        noisy[1].transform.rot[1] += 0.0001; // < 0.001 → tidak terlihat
        noisy[2].transform.scale[0] += 0.001; // < 0.02 → tidak terlihat
        noisy[2].color[0] += 0.001; // < 0.004 → tidak terlihat
        let delta = encode_delta(&quantized, &noisy).unwrap();
        assert_eq!(delta.len(), 4, "noise sub-resolusi harus menghasilkan DELTA kosong (4 byte)");

        // Perubahan ≥ presisi format → field ditandai
        let mut changed = quantized.clone();
        changed[0].transform.pos[0] += 0.05;
        let delta2 = encode_delta(&quantized, &changed).unwrap();
        assert!(delta2.len() > 4, "perubahan nyata harus menghasilkan DELTA berisi field");
        let applied = apply_delta(&quantized, &delta2).unwrap();
        assert!((applied[0].transform.pos[0] - changed[0].transform.pos[0]).abs() < 0.01);
    }

    #[test]
    fn binary_jauh_lebih_kecil_dari_teks() {
        let w = sample_world();
        let bin = encode_full(&w).unwrap();
        // Teks ADILang dunia serupa (perkiraan konservatif: ~120 byte/entity)
        let text_est = w.len() * 120;
        assert!(
            bin.len() < text_est,
            "biner ({}) harus < teks (~{})",
            bin.len(),
            text_est
        );
        // FULL 3 entity = 4 header + 3×21 = 67 byte
        assert_eq!(bin.len(), 4 + w.len() * 21);
    }

    #[test]
    fn deterministik_sama_input_sama_output() {
        let w = sample_world();
        assert_eq!(encode_full(&w).unwrap(), encode_full(&w).unwrap());
        assert_eq!(encode_full(&w).unwrap(), encode_full(&w.clone()).unwrap());
    }

    #[test]
    fn tolak_terlalu_banyak_entity() {
        let w: Vec<EntityState> = (0..256)
            .map(|i| entity(i, [0.0, 0.0, 0.0], [1.0, 1.0, 1.0, 1.0]))
            .collect();
        assert!(encode_full(&w).is_err(), ">255 entity harus ditolak");
    }

    #[test]
    fn decode_tolak_paket_rusak() {
        // magic salah
        assert!(decode_full(&[0x00, 0x01, 0x00, 0x01]).is_err());
        // FULL terpotong (header bilang 1 entity, data < 21 byte)
        assert!(decode_full(&[0xAD, 0x01, 0x00, 0x01, 0x00]).is_err());
        // DELTA tanpa baseline
        let delta = [0xAD, 0x01, 0x01, 0x02, 0x00, 0x01];
        assert!(apply_delta(&sample_world(), &delta).is_err());
    }

    #[test]
    fn delta_tolak_entry_mask_nol_byte_sisa() {
        // Regression: entry dengan mask 0 TIDAK pernah di-emit encoder — bila
        // muncul, itu byte sisa/garbage. Dulu byte sisa 2 (idx, 0x00) ditelan
        // diam-diam karena check trailing-bytes berada DI LUAR loop (dead code).
        let prev = sample_world(); // 3 entity → header count 3
        // header count=3 + garbage (idx=0, mask=0) → harus ditolak
        let garbage = [0xAD, 0x01, FLAG_DELTA, 0x03, 0x00, 0x00];
        assert!(
            apply_delta(&prev, &garbage).is_err(),
            "entry mask 0 (byte sisa) harus ditolak"
        );
        // garbage dengan mask valid tapi idx di luar baseline → ditolak
        let garbage2 = [0xAD, 0x01, FLAG_DELTA, 0x03, 0x63, 0x01];
        assert!(apply_delta(&prev, &garbage2).is_err(), "idx di luar baseline harus ditolak");
        // DELTA kosong yang sah (4 byte header) tetap diterima
        let empty = [0xAD, 0x01, FLAG_DELTA, 0x03];
        assert!(apply_delta(&prev, &empty).is_ok(), "DELTA kosong 4 byte harus diterima");
    }

    #[test]
    fn spec_memuat_poin_kunci() {
        let s = binary_spec();
        assert!(s.contains("ADILangBinary v"));
        assert!(s.contains("FULL"));
        assert!(s.contains("DELTA"));
        assert!(s.contains("kuantisasi"));
    }
}
