// ADILang registry — kosakata tertutup bahasa (P6 self-describing).
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).
//
// P6 (LANGUAGE.md §1): "The full vocabulary ... is a closed registry that the
// runtime can enumerate." Modul ini menyediakan enumerasi deterministik seluruh
// registry bahasa (mesh/material builders, fungsi, event, props, ident builtin,
// modul & kunci protocol) sebagai teks ringkas yang bisa diparsing AI mana pun
// TANPA membaca docs. Ekspor WASM-nya: `adilang_registry()`.

use crate::scene::{MaterialKind, MeshKind};

/// Versi kanonik bahasa ADILang — sumber tunggal kebenaran untuk versi.
/// Sinkron dengan: Cargo.toml, docs/LANGUAGE.md, docs/adilang.ebnf,
/// docs/ADILANG_KNOWLEDGE.md, dan core/adilang_protocol.py (backend ADI).
pub const VERSION: &str = "1.9.0";

// ── SUMBER TUNGGAL KEBENARAN untuk builder mesh/material ────────────────────
// Dipakai oleh: parser.rs `is_builder()`, eval.rs `apply_mesh()/apply_material()`,
// DAN registry_text() di bawah. Jangan menduplikasi daftar ini di tempat lain —
// tambahkan builder baru HANYA di sini (tabel (nama, kind) = satu baris), lalu
// parser, evaluator, registry_text, dan script scripts/check_adilang_registry.py
// otomatis ikut — drift kosakata menjadi MUSTAHIL.

/// Builder mesh (tutup): sphere box torus icosa ring plane grid.
/// Tabel (nama → MeshKind) — sumber tunggal untuk parser, evaluator, registry.
pub const MESH_BUILDERS: [(&str, MeshKind); 7] = [
    ("sphere", MeshKind::Sphere),
    ("box", MeshKind::Box),
    ("torus", MeshKind::Torus),
    ("icosa", MeshKind::Icosa),
    ("ring", MeshKind::Ring),
    ("plane", MeshKind::Plane),
    ("grid", MeshKind::Grid),
];

/// Builder material (tutup): solid wire glow points.
/// Tabel (nama → MaterialKind) — sumber tunggal untuk parser, evaluator, registry.
pub const MATERIAL_BUILDERS: [(&str, MaterialKind); 4] = [
    ("solid", MaterialKind::Solid),
    ("wire", MaterialKind::Wire),
    ("glow", MaterialKind::Glow),
    ("points", MaterialKind::Points),
];

/// Daftar nama mesh builder (urutan tabel) — untuk registry_text / parser.
pub fn mesh_builder_names() -> impl Iterator<Item = &'static str> {
    MESH_BUILDERS.iter().map(|(n, _)| *n)
}

/// Daftar nama material builder (urutan tabel) — untuk registry_text / parser.
pub fn material_builder_names() -> impl Iterator<Item = &'static str> {
    MATERIAL_BUILDERS.iter().map(|(n, _)| *n)
}

/// Apakah identifier adalah builder mesh/material? Sumber tunggal kebenaran
/// untuk parser (is_builder) & registry. Tambah builder baru hanya di konstanta.
pub fn is_builder(id: &str) -> bool {
    is_mesh_builder(id) || is_material_builder(id)
}

/// Apakah identifier adalah builder MESH? (sumber tunggal: MESH_BUILDERS)
pub fn is_mesh_builder(id: &str) -> bool {
    MESH_BUILDERS.iter().any(|(n, _)| *n == id)
}

/// Apakah identifier adalah builder MATERIAL? (sumber tunggal: MATERIAL_BUILDERS)
pub fn is_material_builder(id: &str) -> bool {
    MATERIAL_BUILDERS.iter().any(|(n, _)| *n == id)
}

/// Nama mesh builder → MeshKind (sumber tunggal: MESH_BUILDERS).
/// Dipakai eval.rs `apply_mesh()` — tidak ada match literal terpisah di eval.
pub fn mesh_kind(name: &str) -> Option<MeshKind> {
    MESH_BUILDERS.iter().find(|(n, _)| *n == name).map(|(_, k)| *k)
}

/// Nama material builder → MaterialKind (sumber tunggal: MATERIAL_BUILDERS).
/// Dipakai eval.rs `apply_material()` — tidak ada match literal terpisah di eval.
pub fn material_kind(name: &str) -> Option<MaterialKind> {
    MATERIAL_BUILDERS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, k)| *k)
}

/// Enumerasi kosakata tertutup ADILang dalam satu string deterministik.
/// Format: satu kategori per baris, nilai dipisah spasi.
pub fn registry_text() -> String {
    // `mesh`/`material` dibangun dari tabel MESH_BUILDERS/MATERIAL_BUILDERS
    // (sumber tunggal — sama dengan yang dipakai parser.rs is_builder dan
    // eval.rs apply_mesh/apply_material). Kategori lain tetap literal mentah di
    // concat!: concat! adalah NILAI argumen format!, bukan template, sehingga
    // karakter `%` dan `{ }` dikeluarkan apa adanya.
    format!(
        "REGISTRY v{}\nmesh: {}\nmaterial: {}\n{}",
        VERSION,
        mesh_builder_names().collect::<Vec<_>>().join(" "),
        material_builder_names().collect::<Vec<_>>().join(" "),
        concat!(
            "event: frame speak silent click\n",
            "transform: move setPos setScale scaleBy rotate setColor setAlpha\n",
            "math1: sin cos tan asin acos atan sqrt abs floor ceil round\n",
            "math2: pow min max\n",
            "math3: clamp lerp\n",
            "ident: t mouseX mouseY PI\n",
            "meshparam: radius tube inner segments size count\n",
            "cameraprop: pos look fov\n",
            "lightprop: type pos color intensity\n",
            // lightprop.type = enum untuk prop type lampu; lightkind memakai
            // himpunan yang sama point ambient — diverifikasi check script
            // terhadap eval.rs apply_light_prop. Tanpa tanda kurung agar aman
            // bagi scanner penyeimbang paren scripts/check_adilang_registry.py.
            "lightprop.type: point ambient\n",
            "entityprop: pos rot scale mesh material\n",
            "lightkind: point ambient\n",
            "declaration: world camera light entity let func on\n",
            "statement: let if return while for match\n",
            "keyword: true false\n",
            "operator: + - * / % == != < > <= >= = =>\n",
            "delim: ( ) { } , [ ] :\n",
            "protocol: intent reply task event world memory plan state\n",
            "protocolkey: mode payload verb content recs world assign input expect source key session at topic fact confidence steps parallel line token guidance user_key session_id job_id muted speaking mic_active quality status progress provider elapsed\n",
            "verb: ask inform command greet system\n",
            // ADILang Binary/Bytecode (v1.4.0) — API transport real-time
            // (bit-packing + delta, Rust → WASM → WebSocket antar-client).
            "binary: encode_full decode_full encode_delta apply_delta packet_kind packet_version packet_entity_count binary_spec\n",
            // ADILang Tooling (v1.7.0) — linter & token compactor. Sumber
            // aktual: pub fn di checker.rs / compactor.rs + def di
            // core/adilang_protocol.py (diverifikasi scripts/check_adilang_registry.py).
            "checker: check_src\n",
            "compactor: optimize_src render_expr render_program\n",
            "selfheal: syntax_error_event auto_fix check_adilang\n",
        )
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_menyertakan_versi_dan_kategori_kunci() {
        let r = registry_text();
        assert!(r.starts_with("REGISTRY v"), "registry harus diawali header versi");
        assert!(r.contains("mesh:"), "registry harus memuat kategori mesh");
        assert!(r.contains("material:"), "registry harus memuat kategori material");
        assert!(r.contains("protocol:"), "registry harus memuat kategori protocol");
    }

    #[test]
    fn registry_meliputi_kosakata_tertutup_lengkap() {
        let r = registry_text();
        // Mesh builders
        for kw in ["sphere", "box", "torus", "icosa", "ring", "plane", "grid"] {
            assert!(r.contains(kw), "registry harus memuat mesh builder '{kw}'");
        }
        // Material builders (termasuk points yang kini diimplementasikan)
        for kw in ["solid", "wire", "glow", "points"] {
            assert!(r.contains(kw), "registry harus memuat material '{kw}'");
        }
        // Transform & math
        for kw in ["rotate", "setColor", "lerp", "clamp", "pow"] {
            assert!(r.contains(kw), "registry harus memuat fungsi '{kw}'");
        }
        // Protocol modules & keys
        for kw in ["intent", "reply", "task", "event", "world", "payload", "recs", "assign", "at"] {
            assert!(r.contains(kw), "registry harus memuat protocol item '{kw}'");
        }
        // Verbs tertutup
        for kw in ["ask", "inform", "command", "greet", "system"] {
            assert!(r.contains(kw), "registry harus memuat verb '{kw}'");
        }
    }

    #[test]
    fn registry_meliputi_tooling_v170() {
        let r = registry_text();
        // adilang-check (checker.rs)
        assert!(r.contains("checker: check_src"), "registry harus memuat kategori checker");
        // adilang-opt (compactor.rs)
        assert!(r.contains("compactor: optimize_src"), "registry harus memuat kategori compactor");
        // self-heal protocol (core/adilang_protocol.py)
        for kw in ["syntax_error_event", "auto_fix", "check_adilang"] {
            assert!(r.contains(kw), "registry harus memuat self-heal '{kw}'");
        }
    }

    #[test]
    fn registry_meliputi_binary_bytecode_api() {
        let r = registry_text();
        for kw in ["encode_full", "decode_full", "encode_delta", "apply_delta", "binary_spec"] {
            assert!(r.contains(kw), "registry harus memuat binary API '{kw}'");
        }
        assert!(r.contains("binary:"), "registry harus memuat kategori binary");
    }

    #[test]
    fn registry_meliputi_props_deklarasi_dan_simbol() {
        let r = registry_text();
        // Entity props (konsisten dengan eval.rs apply_entity_prop)
        for kw in ["pos", "rot", "scale", "mesh", "material"] {
            assert!(r.contains(kw), "registry harus memuat entity prop '{kw}'");
        }
        // Light kinds
        assert!(r.contains("point"), "registry harus memuat light kind 'point'");
        assert!(r.contains("ambient"), "registry harus memuat light kind 'ambient'");
        // Declarations top-level (konsisten dengan parser.rs parse_top_level)
        for kw in ["world", "camera", "light", "entity", "let", "func", "on"] {
            assert!(r.contains(kw), "registry harus memuat declaration '{kw}'");
        }
        // Keyword ekspresi
        assert!(r.contains("true"), "registry harus memuat keyword 'true'");
        assert!(r.contains("false"), "registry harus memuat keyword 'false'");
        // Operator & delimiter lexer
        for sym in ["==", "!=", "<=", ">="] {
            assert!(r.contains(sym), "registry harus memuat operator '{sym}'");
        }
        // protocolkey lengkap termasuk `world` (reply.world di backend Python) —
        // periksa baris protocolkey secara spesifik (bukan contains global agar
        // tidak trivially true dari kategori lain yang juga memuat 'world').
        let pk_line = r
            .lines()
            .find(|l| l.starts_with("protocolkey:"))
            .unwrap_or("");
        assert!(
            pk_line.split_whitespace().any(|w| w == "world"),
            "protocolkey harus memuat 'world' (reply.world)"
        );
        assert!(!pk_line.is_empty(), "baris protocolkey harus ada di registry");
    }

    #[test]
    fn registry_deterministik() {
        assert_eq!(registry_text(), registry_text(), "registry harus deterministik");
    }

    #[test]
    fn registry_memuat_statement_dan_lightprop_type_enum() {
        // Validasi grammar lengkap: statement (let if return while for) & enum lightprop.type
        let r = registry_text();
        assert!(r.contains("statement: let if return while for"), "registry harus memuat kategori statement (incl. loop)");
        assert!(r.contains("lightprop.type: point ambient"), "registry harus memuat enum lightprop.type");
        // Baris dasar lightprop (type pos color intensity) juga wajib ada —
        // baris inilah yang pernah terhapus tak sengaja dan hanya ketahuan
        // oleh script checker; kunci dengan assertion ini di level cargo test.
        assert!(r.contains("lightprop: type pos color intensity"), "registry harus memuat kategori lightprop");
        // Tepat satu baris (tidak duplikat)
        assert_eq!(r.matches("statement: let if return while for").count(), 1);
        assert_eq!(r.matches("lightprop.type: point ambient").count(), 1);
        assert_eq!(r.matches("lightprop: type pos color intensity").count(), 1);
    }

    #[test]
    fn is_builder_mencakup_semua_builder() {
        // is_builder() = sumber tunggal dari MESH_BUILDERS + MATERIAL_BUILDERS;
        // harus mencakup persis semua builder, tanpa ekstra.
        let semua_builder: Vec<&str> = mesh_builder_names()
            .chain(material_builder_names())
            .collect();
        for b in &semua_builder {
            assert!(is_builder(b), "'{b}' harus dikenali sebagai builder");
        }
        // Bukan builder → false (nama fungsi/prop/ident biasa)
        for not_builder in ["rotate", "frame", "mesh", "material", "sphereX", "Box"] {
            assert!(!is_builder(not_builder), "'{not_builder}' BUKAN builder");
        }
        assert_eq!(semua_builder.len(), 11, "total builder = 7 mesh + 4 material");
    }

    #[test]
    fn mesh_material_lines_dibangun_dari_konstanta() {
        // registry_text() harus memakai konstanta yang sama dengan is_builder
        // (bukan literal duplikat) — satu sumber kebenaran.
        let r = registry_text();
        let mesh_line = format!("mesh: {}", mesh_builder_names().collect::<Vec<_>>().join(" "));
        let material_line = format!("material: {}", material_builder_names().collect::<Vec<_>>().join(" "));
        assert!(r.contains(&mesh_line), "baris mesh harus dibangun dari MESH_BUILDERS");
        assert!(r.contains(&material_line), "baris material harus dibangun dari MATERIAL_BUILDERS");
        // Persis sekali (tidak ada duplikat kategori)
        assert_eq!(r.matches(&mesh_line).count(), 1);
        assert_eq!(r.matches(&material_line).count(), 1);
    }

    #[test]
    fn mesh_kind_material_kind_bijektif_dengan_tabel() {
        // Evaluator memakai mesh_kind()/material_kind() (bukan match literal) —
        // tabel (nama, kind) di sini adalah SATU-SATUNYA pemetaan.
        for (name, kind) in MESH_BUILDERS {
            assert_eq!(mesh_kind(name), Some(kind), "'{name}' harus memetakan ke kind tabel");
        }
        for (name, kind) in MATERIAL_BUILDERS {
            assert_eq!(material_kind(name), Some(kind), "'{name}' harus memetakan ke kind tabel");
        }
        // Nama di luar tabel → None (bukan kind samar)
        assert_eq!(mesh_kind("cube"), None);
        assert_eq!(mesh_kind("Sphere"), None);
        assert_eq!(material_kind("glass"), None);
        assert_eq!(material_kind("solidX"), None);
        // Setiap nama tabel → is_*_builder true; sembarang kata lain → false
        for (name, _) in MESH_BUILDERS {
            assert!(is_mesh_builder(name));
            assert!(!is_material_builder(name));
        }
        for (name, _) in MATERIAL_BUILDERS {
            assert!(is_material_builder(name));
            assert!(!is_mesh_builder(name));
        }
    }
}
