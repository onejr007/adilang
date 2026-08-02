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
pub const VERSION: &str = "1.14.0";

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
            // v1.12.0 — builtin i18n translate t(kunci) (bukan math; diklasifikasi
            // khusus oleh scripts/check_adilang_registry.py dari call_builtin).
            "i18n: t\n",
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
            "declaration: world camera light entity let func on ui_layout spatial_3d routes payload use_js i18n url sender target_agent intent state_data container flex row column text button input card navbar footer component\n",
            "statement: let if return while for match\n",
            "keyword: true false\n",
            "operator: + - * / % == != < > <= >= = =>\n",
            "delim: ( ) { } , [ ] :\n",
            "protocol: intent reply task event world memory plan state\n",
            "protocolkey: mode payload verb content recs world assign input expect source key session at topic fact confidence steps parallel line token guidance user_key session_id job_id muted speaking mic_active quality status progress provider elapsed\n",
            "verb: ask inform command greet system\n",
            // ADILang Binary/Bytecode (v1.4.0) — API transport real-time
            // (bit-packing + delta, Rust → WASM → WebSocket antar-client).
            "binary: encode_program decode_program encode_full decode_full encode_delta apply_delta packet_kind packet_version packet_entity_count binary_spec\n",
            // ADILang Protocol (v1.10.0) — text transport Base64 (adilang_protocol.rs).
            "protocol_rs: b64_encode b64_decode encode_source_to_binary encode_source_to_b64 encode_ast_to_b64 decode_b64_to_ast decode_b64_to_binary size_report\n",
            // ADILang Tooling (v1.7.0) — linter & token compactor. Sumber
            // aktual: pub fn di checker.rs / compactor.rs + def di
            // core/adilang_protocol.py (diverifikasi scripts/check_adilang_registry.py).
            "checker: check_src\n",
            "compactor: optimize_src render_expr render_program\n",
            "selfheal: syntax_error_event auto_fix check_adilang\n",
            // ADILang Spatial (v1.11.0) — procedural 3D + spatial UI rasterizer
            // (adilang_spatial.rs). Generate mesh tanpa aset eksternal + render
            // ui_layout ke tekstur RGBA untuk dipetakan ke permukaan objek.
            "spatial: generate_shape generate_all render_layout_to_texture\n",
            // ADILang CRDT (v1.11.0) — multi-agent collaborative state
            // (adilang_crdt.rs). Register CRDT per path sel AST; merge konvergen
            // & delta transfer antar-agen. Cermin pub fn crdt.rs.
            "crdt: new paths get get_value live_count max_lamport total_count make_set make_delete apply conflicts merge snapshot_json snapshot_string load_snapshot_json load_snapshot_string missing_ops is_tombstone join_path\n",
            // ADILang Diff/Patch (v1.11.0) — diffing & patching level-blok AST
            // (adilang_diff.rs). LLM menghasilkan 'ADILang Patch Script' yang
            // hanya mengubah blok yang diminta — tanpa compile ulang dokumen.
            "diff: block_key diff_docs diff_docs_json apply_doc apply_doc_json parse_patch_script parse_patch_script_json apply_patch_script\n",
            // ADILang Analytics/Telemetry (v1.11.0) — metrik runtime deterministik
            // (adilang_analytics.rs): FPS window, durasi render, hitungan event.
            "analytics: new reset record_frame record_load record_speak record_silent record_action record_state_set record_error frame_rate avg_frame_ms avg_render_ms min_frame_ms max_frame_ms snapshot_json frames loads errors\n",
            // ADILang Render Target (v1.11.0) — abstraksi rendering layer
            // cross-platform (adilang_target.rs): seleksi backend WebGL2/
            // WebGPU/wgpu-native secara deterministik per kapabilitas.
            "target: default_caps select_backend\n",
            // ADILang Package Manager (v1.12.0) — adipm (adilang_pkg.rs):
            // manifest `adi.toml` + CLI `adi add/install`. Cermin pub fn pkg.rs.
            "pkg: parse_manifest render_manifest add_dependency remove_dependency has_dependency\n",
            // ADILang Headless Tester (v1.12.0) — `adi test` (adilang_tester.rs):
            // uji parse/check/struktur/simulasi event tanpa browser.
            "tester: test_program\n",
            // ADILang Build Exporter (v1.12.0) — `adilang-build --target gh-pages`
            // (adilang_exporter.rs): situs statis + PWA (manifest.json/sw.js).
            "exporter: export_gh_pages\n",
            // ADILang Lifecycle Hooks (v1.13.0) — komponen `component Name {
            // on_mount: @fetch_data() ... }` (ast.rs LifecycleHookKind.as_str()).
            "lifecycle: on_mount on_update on_unmount\n",
            // ADILang CLI Scaffolder (v1.13.0) — `adi new` (adilang_scaffolder.rs):
            // template minimal/spatial-3d/fullstack-agent. Cermin pub fn scaffolder.rs.
            "scaffolder: template_source validate_template scaffold\n",
            // ADILang DevServer+HMR (v1.13.0) — `adi dev` (adilang_devserver.rs):
            // server HTTP statis + WebSocket HMR (frame HMR_CONNECT/HMR_RELOAD).
            "devserver: serve HMR_CONNECT HMR_RELOAD WS_PATH\n",
            // ADILang Build Optimizer (v1.13.0) — `adi build` (adilang_build.rs):
            // gabung + DCE (compact + .adib) + ekspor gh-pages + PWA + wasm-opt.
            "build: build_project savings_percent\n",
            // ADILang Dense Compact AST (v1.14.0) — opcode map & bitstream
            // (adilang_dense.rs): UI 2D / layout / mesh WebGL 3D sebagai opcode
            // satu-byte; laporan penghematan >80% vs HTML/JS/JSON. Cermin pub fn dense.rs.
            "dense: dense_spec opcode_name opcode_of_ui opcode_of_mesh mesh_name material_name light_name encode_program decode_program opcode_histogram json_equivalent html_equivalent savings_percent size_report\n",
            // ADILang AI Guard (v1.14.0) — validator & handshake mesin
            // (adilang_ai_guard.rs): dokumen yang diedit manusia gagal verifikasi.
            "ai_guard: fnv1a64 canonical_bytes signature_hex sign attach_signature extract_signature verify machine_entropy is_machine_generated challenge respond verify_handshake\n",
            // ADILang Diagnostics (v1.14.0) — protokol error mesin AI-ke-AI
            // (adilang_diagnostics.rs): pasangan {err: 0x0E4, node: 12}, bukan
            // string pesan manusia.
            "diagnostics: code_name classify machine_error error_vector node_from_line from_checker from_result diagnostics_report\n",
            // ADILang Machine Runner (v1.14.0) — interpreter bitstream langsung
            // (adilang_machine_runner.rs): decode → evaluasi → operasi DOM/WebGL2.
            "machine: from_dense from_source program dense_bytes run_lifecycle fire_event run_frame dom_ops_json webgl_ops_json components_json spec\n",
            // ADILang WASM API (v1.10.0) — exports wasm-bindgen (wasm_api.rs)
            // untuk Web SDK adilang_web.js.
            "wasm: adilang_start adilang_load adilang_speak adilang_silent adilang_check adilang_check_diagnostics adilang_optimize adilang_debug_count adilang_version adilang_registry adilang_binary_encode_full adilang_binary_encode_delta adilang_binary_decode_full adilang_binary_spec adilang_parse_and_render adilang_export_agent_payload adilang_update_state adilang_get_state adilang_protocol_encode_source_to_b64 adilang_protocol_decode_b64_to_ast adilang_protocol_size_report adilang_schema_json adilang_schema_prompt adilang_schema_prompt_compact adilang_state_set_json adilang_state_get adilang_state_snapshot adilang_state_revision adilang_state_incr adilang_state_is_render_relevant adilang_spatial_generate adilang_spatial_shapes adilang_spatial_ui_texture adilang_crdt_set adilang_crdt_delete adilang_crdt_get adilang_crdt_snapshot adilang_crdt_load_snapshot adilang_crdt_merge adilang_crdt_count adilang_crdt_missing_ops adilang_patch_info adilang_diff adilang_apply_patch adilang_parse_patch_script adilang_apply_patch_script adilang_analytics_record_frame adilang_analytics_record_event adilang_analytics_snapshot adilang_analytics_reset adilang_capture_viewport_snapshot adilang_target_info adilang_target_select adilang_test_report adilang_parse_components adilang_run_lifecycle adilang_dense_spec adilang_dense_size_report adilang_ai_guard_sign adilang_ai_guard_verify adilang_diag_payload adilang_machine_run_lifecycle adilang_machine_dom_ops adilang_machine_webgl_ops adilang_machine_fire_event adilang_machine_components\n",
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
    fn registry_meliputi_protocol_rs_api() {
        let r = registry_text();
        for kw in [
            "b64_encode",
            "b64_decode",
            "encode_source_to_b64",
            "decode_b64_to_ast",
            "size_report",
        ] {
            assert!(r.contains(kw), "registry harus memuat protocol_rs API '{kw}'");
        }
        assert!(r.contains("protocol_rs:"), "registry harus memuat kategori protocol_rs");
    }

    #[test]
    fn registry_meliputi_wasm_api() {
        let r = registry_text();
        for kw in [
            "adilang_start",
            "adilang_parse_and_render",
            "adilang_protocol_encode_source_to_b64",
            "adilang_schema_json",
            "adilang_state_set_json",
            "adilang_state_revision",
            "adilang_get_state",
        ] {
            assert!(r.contains(kw), "registry harus memuat wasm API '{kw}'");
        }
        assert!(r.contains("wasm:"), "registry harus memuat kategori wasm");
    }

    #[test]
    fn registry_meliputi_spatial_api() {
        let r = registry_text();
        for kw in ["generate_shape", "render_layout_to_texture", "torus", "grid"] {
            assert!(r.contains(kw), "registry harus memuat spatial API '{kw}'");
        }
        assert!(r.contains("spatial:"), "registry harus memuat kategori spatial");
        // Export WASM spatial ikut tercantum
        for kw in ["adilang_spatial_generate", "adilang_spatial_shapes", "adilang_spatial_ui_texture"] {
            assert!(r.contains(kw), "registry harus memuat wasm spatial '{kw}'");
        }
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
    fn registry_meliputi_diff_api() {
        let r = registry_text();
        for kw in ["diff_docs", "apply_doc", "parse_patch_script", "apply_patch_script"] {
            assert!(r.contains(kw), "registry harus memuat diff API '{kw}'");
        }
        assert!(r.contains("diff:"), "registry harus memuat kategori diff");
        // Export WASM diff ikut tercantum
        for kw in [
            "adilang_diff",
            "adilang_apply_patch",
            "adilang_parse_patch_script",
            "adilang_apply_patch_script",
        ] {
            assert!(r.contains(kw), "registry harus memuat wasm diff '{kw}'");
        }
    }

    #[test]
    fn registry_meliputi_analytics_api() {
        let r = registry_text();
        for kw in [
            "record_frame",
            "record_load",
            "frame_rate",
            "avg_render_ms",
            "snapshot_json",
        ] {
            assert!(r.contains(kw), "registry harus memuat analytics API '{kw}'");
        }
        assert!(r.contains("analytics:"), "registry harus memuat kategori analytics");
        for kw in [
            "adilang_analytics_record_frame",
            "adilang_analytics_record_event",
            "adilang_analytics_snapshot",
            "adilang_analytics_reset",
        ] {
            assert!(r.contains(kw), "registry harus memuat wasm analytics '{kw}'");
        }
    }

    #[test]
    fn registry_meliputi_target_api() {
        let r = registry_text();
        for kw in ["default_caps", "select_backend"] {
            assert!(r.contains(kw), "registry harus memuat target API '{kw}'");
        }
        assert!(r.contains("target:"), "registry harus memuat kategori target");
        for kw in ["adilang_target_info", "adilang_target_select"] {
            assert!(r.contains(kw), "registry harus memuat wasm target '{kw}'");
        }
    }

    #[test]
    fn registry_meliputi_v1130_lifecycle_tooling() {
        let r = registry_text();
        // Lifecycle hooks (v1.13.0) — component + hook kinds
        for kw in ["on_mount", "on_update", "on_unmount"] {
            assert!(r.contains(kw), "registry harus memuat lifecycle hook '{kw}'");
        }
        assert!(r.contains("lifecycle:"), "registry harus memuat kategori lifecycle");
        assert!(r.contains("component"), "registry harus memuat declaration 'component'");
        // Scaffolder (v1.13.0) — `adi new`
        for kw in ["template_source", "validate_template", "scaffold"] {
            assert!(r.contains(kw), "registry harus memuat scaffolder API '{kw}'");
        }
        assert!(r.contains("scaffolder:"), "registry harus memuat kategori scaffolder");
        // DevServer+HMR (v1.13.0)
        for kw in ["serve", "HMR_CONNECT", "HMR_RELOAD", "WS_PATH"] {
            assert!(r.contains(kw), "registry harus memuat devserver API '{kw}'");
        }
        assert!(r.contains("devserver:"), "registry harus memuat kategori devserver");
        // Build optimizer (v1.13.0)
        for kw in ["build_project", "savings_percent"] {
            assert!(r.contains(kw), "registry harus memuat build API '{kw}'");
        }
        assert!(r.contains("build:"), "registry harus memuat kategori build");
        // WASM lifecycle exports ikut tercantum
        for kw in ["adilang_parse_components", "adilang_run_lifecycle"] {
            assert!(r.contains(kw), "registry harus memuat wasm lifecycle '{kw}'");
        }
    }

    #[test]
    fn registry_meliputi_v1140_dense_ai_guard_diagnostics_machine() {
        let r = registry_text();
        // Dense Compact AST (v1.14.0)
        for kw in ["dense_spec", "opcode_of_mesh", "size_report", "savings_percent"] {
            assert!(r.contains(kw), "registry harus memuat dense API '{kw}'");
        }
        assert!(r.contains("dense:"), "registry harus memuat kategori dense");
        // AI Guard (v1.14.0)
        for kw in ["is_machine_generated", "machine_entropy", "verify_handshake"] {
            assert!(r.contains(kw), "registry harus memuat ai_guard API '{kw}'");
        }
        assert!(r.contains("ai_guard:"), "registry harus memuat kategori ai_guard");
        // Diagnostics (v1.14.0)
        for kw in ["machine_error", "error_vector", "diagnostics_report"] {
            assert!(r.contains(kw), "registry harus memuat diagnostics API '{kw}'");
        }
        assert!(r.contains("diagnostics:"), "registry harus memuat kategori diagnostics");
        // Machine Runner (v1.14.0)
        for kw in ["from_dense", "dom_ops_json", "webgl_ops_json"] {
            assert!(r.contains(kw), "registry harus memuat machine API '{kw}'");
        }
        assert!(r.contains("machine:"), "registry harus memuat kategori machine");
        // WASM exports v1.14.0 ikut tercantum
        for kw in [
            "adilang_dense_spec",
            "adilang_ai_guard_verify",
            "adilang_diag_payload",
            "adilang_machine_dom_ops",
            "adilang_machine_webgl_ops",
            "adilang_machine_run_lifecycle",
        ] {
            assert!(r.contains(kw), "registry harus memuat wasm v1.14.0 '{kw}'");
        }
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
