// ADILang Schema (adilang_schema) — generator JSON Schema & System Prompt.
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).
//
// Menghasilkan:
//   1) JSON Schema (draft-07) untuk IR ADILang (@payload / ui_layout /
//      spatial_3d / world) — anti-drift: enum mesh/material/verb/protocol
//      diambil dari registry_text() (P6 single source), BUKAN literal lokal.
//   2) System Prompt Template siap suntik ke LLM — mengajarkan kosakata
//      tertutup + aturan Zero-Token-Waste.

use crate::registry::{VERSION, registry_text};

/// Ambil nilai kategori registry ("mesh:", "verb:", ...) → list kata.
/// Kategori tidak ada → list kosong. Sumber tunggal kebenaran = registry_text().
pub fn registry_category(cat: &str) -> Vec<&'static str> {
    registry_text()
        .lines()
        .find_map(|l| {
            let (name, rest) = l.split_once(':')?;
            if name.trim() == cat {
                Some(
                    rest.split_whitespace()
                        .map(|s| Box::leak(s.to_string().into_boxed_str()) as &'static str)
                        .collect::<Vec<_>>(),
                )
            } else {
                None
            }
        })
        .unwrap_or_default()
}

/// JSON Schema lengkap untuk IR ADILang (draft-07).
pub fn json_schema() -> serde_json::Value {
    let mesh = registry_category("mesh");
    let material = registry_category("material");
    let verbs = registry_category("verb");
    let protocol = registry_category("protocol");
    let protocol_keys = registry_category("protocolkey");
    let transform = registry_category("transform");
    let events = registry_category("event");
    let mesh_params = registry_category("meshparam");

    let keyword_enum = {
        let mut k = vec!["true".to_string(), "false".to_string()];
        let mut extra = ["if", "return", "match", "on"].iter().map(|s| s.to_string()).collect::<Vec<_>>();
        k.append(&mut extra);
        serde_json::Value::Array(k.into_iter().map(serde_json::Value::String).collect())
    };

    serde_json::json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "$id": "https://adilang.dev/schema/adilang-ir.schema.json",
        "title": "ADILang IR Protocol Schema",
        "description": "Struktur IR ADILang untuk komunikasi AI-to-AI, UI (ui_layout), dan 3D (spatial_3d). Enum diambil dari registry tertutup resmi (P6).",
        "version": VERSION,
        "type": "object",
        "additionalProperties": false,
        "required": ["version", "module", "intent"],
        "properties": {
            "version": { "const": VERSION },
            "module": { "type": "string", "enum": protocol },
            "intent": { "type": "string", "examples": ["collaborate", "query", "reply", "command"] },
            "verb": { "type": "string", "enum": verbs },
            "sender": { "type": "string" },
            "target_agent": { "type": "string" },
            "content": { "type": "string", "description": "Pesan bebas (maks ~120 token, minimal karakter)." },
            "state_data": { "description": "Peta status bebas (objek/array)." },
            "memory": { "$ref": "#/$defs/memory" },
            "plan": { "$ref": "#/$defs/plan" },
            "recs": { "type": "array", "items": { "$ref": "#/$defs/rec" } },
            "ui_layout": { "$ref": "#/$defs/ui_layout" },
            "spatial_3d": { "$ref": "#/$defs/spatial_3d" },
            "world": { "$ref": "#/$defs/spatial_3d" },
            "protocol_keys": { "type": "array", "items": { "type": "string", "enum": protocol_keys } }
        },
        "$defs": {
            "memory": {
                "type": "object",
                "properties": {
                    "topic": { "type": "string" },
                    "fact": { "type": "string" },
                    "confidence": { "type": "number", "minimum": 0.0, "maximum": 1.0 }
                }
            },
            "plan": {
                "type": "object",
                "properties": {
                    "steps": { "type": "array", "items": { "type": "string" }, "minItems": 1 },
                    "parallel": { "type": "boolean" }
                },
                "required": ["steps"]
            },
            "rec": {
                "type": "object",
                "properties": {
                    "source": { "type": "string" },
                    "key": { "type": "string" },
                    "at": { "type": "number" },
                    "session": { "type": "string" }
                }
            },
            "ui_layout": {
                "type": "object",
                "required": ["name", "root"],
                "properties": {
                    "name": { "type": "string" },
                    "root": { "$ref": "#/$defs/component" }
                }
            },
            "component": {
                "oneOf": [
                    { "$ref": "#/$defs/component_container" },
                    { "$ref": "#/$defs/component_text" },
                    { "$ref": "#/$defs/component_button" },
                    { "$ref": "#/$defs/component_input" }
                ]
            },
            "component_container": {
                "type": "object",
                "required": ["kind"],
                "properties": {
                    "kind": { "const": "container" },
                    "flex": { "type": "string", "enum": ["row", "column"] },
                    "children": { "type": "array", "items": { "$ref": "#/$defs/component" } }
                }
            },
            "component_text": {
                "type": "object",
                "required": ["kind"],
                "properties": { "kind": { "const": "text" }, "content": { "type": "string" } }
            },
            "component_button": {
                "type": "object",
                "required": ["kind"],
                "properties": { "kind": { "const": "button" }, "label": { "type": "string" }, "onClick": { "type": "string" } }
            },
            "component_input": {
                "type": "object",
                "required": ["kind"],
                "properties": { "kind": { "const": "input" }, "name": { "type": "string" }, "placeholder": { "type": "string" } }
            },
            "spatial_3d": {
                "type": "object",
                "required": ["name", "items"],
                "properties": {
                    "name": { "type": "string" },
                    "items": {
                        "type": "array",
                        "items": { "oneOf": [
                            { "$ref": "#/$defs/entity" },
                            { "$ref": "#/$defs/camera" },
                            { "$ref": "#/$defs/light" }
                        ] }
                    }
                }
            },
            "entity": {
                "type": "object",
                "required": ["id"],
                "properties": {
                    "id": { "type": "string" },
                    "props": { "type": "object", "additionalProperties": { "$ref": "#/$defs/expr" } },
                    "handlers": { "type": "array", "items": { "$ref": "#/$defs/handler" } }
                }
            },
            "camera": {
                "type": "object",
                "required": ["id"],
                "properties": {
                    "id": { "type": "string" },
                    "pos": { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3 },
                    "look": { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3 },
                    "fov": { "type": "number" }
                }
            },
            "light": {
                "type": "object",
                "required": ["id"],
                "properties": {
                    "id": { "type": "string" },
                    "type": { "type": "string", "enum": registry_category("lightkind") },
                    "pos": { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3 },
                    "color": { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3 },
                    "intensity": { "type": "number" }
                }
            },
            "handler": {
                "type": "object",
                "required": ["event"],
                "properties": {
                    "event": { "type": "string", "enum": events },
                    "body": { "type": "array", "items": { "$ref": "#/$defs/statement" } }
                }
            },
            "statement": {
                "type": "object",
                "properties": {
                    "kind": { "type": "string", "enum": keyword_enum },
                    "name": { "type": "string" },
                    "expr": { "$ref": "#/$defs/expr" }
                }
            },
            "expr": {
                "oneOf": [
                    { "type": "number" },
                    { "type": "string" },
                    { "type": "boolean" },
                    { "type": "array" },
                    { "type": "object" },
                    { "type": "string", "description": "Nama transform/fungsi tertutup" }
                ],
                "description": "Ekspresi ADILang: angka, string, tuple, panggilan transform."
            },
            "mesh_builders": { "type": "string", "enum": mesh },
            "material_builders": { "type": "string", "enum": material },
            "transform_funcs": { "type": "string", "enum": transform },
            "mesh_params": { "type": "string", "enum": mesh_params }
        }
    })
}

/// JSON Schema sebagai string terindah (untuk CLI / WASM / docs).
pub fn json_schema_string() -> Result<String, String> {
    serde_json::to_string_pretty(&json_schema())
        .map_err(|e| format!("Serialisasi schema gagal: {e}"))
}

/// System Prompt lengkap — suntik ke LLM agar menghasilkan ADILang valid &
/// hemat token. Memuat registry tertutup + aturan + contoh.
pub fn system_prompt() -> String {
    let registry = registry_text();
    format!(
        "# ADILang Generator — System Prompt v{}\n\
         \n\
         Kamu adalah generator ADILang (bahasa AI-to-AI zero-token-waste).\n\
         Hasilkan HANYA kode ADILang dalam fenced code block ```adilang.\n\
         \n\
         ## 1. Kosakata tertutup (P6) — gunakan HANYA kata di bawah ini:\n\
         ```\n\
         {registry}\n\
         ```\n\
         \n\
         ## 2. Struktur dasar\n\
         - Komunikasi agen: blok `@payload {{ ... }}` (sender, target_agent, intent, state_data).\n\
         - Antarmuka 2D: blok `ui_layout \"nama\" {{ container {{ ... }} }}`\n\
           (komponen: container, text, button, input; atribut flex row/column).\n\
         - Dunia 3D: blok `spatial_3d \"nama\" {{ camera / light / entity }}`\n\
           (entity: pos, mesh <sphere|box|torus|icosa|ring|plane|grid> {{ param }},\n\
           material <solid|wire|glow|points> (r g b) alpha, on frame {{ ... }}).\n\
         \n\
         ## 3. Zero-Token-Waste\n\
         - 1 intent <= 120 token; bahasa padat; hindari kata pengisi.\n\
         - Gunakan ident pendek (a, b, e) untuk variabel lokal di handler.\n\
         - String/ident yang sama dipakai ulang → bytecode memampatkannya\n\
           (kata registry = 2 byte).\n\
         \n\
         ## 4. Contoh valid\n\
         ```adilang\n\
         @payload {{ sender \"ai-1\" target_agent \"ai-2\" intent \"query\" }}\n\
         ui_layout \"hud\" {{ container {{ flex column text \"Hello\" button \"Send\" onClick send }} }}\n\
         spatial_3d \"scene\" {{\n\
           camera \"cam\" {{ pos (0 1.6 7) look (0 0 0) fov 55 }}\n\
           entity \"core\" {{ mesh sphere {{ radius 0.8 }} material wire (0.15 0.8 1) 0.9 }}\n\
         }}\n\
         ```\n\
         \n\
         ## 5. Larangan\n\
         - JANGAN mengarang kata di luar registry.\n\
         - JANGAN mengeluarkan JSON, markdown lain, atau penjelasan — hanya ```adilang.\n\
         - ui_layout & spatial_3d wajib punya nama string.\n\
         \n\
         Skema JSON penuh: panggil `json_schema` bila diminta.",
        VERSION
    )
}

/// System Prompt ringkas — untuk konteks terbatas (tanpa dump registry penuh).
pub fn system_prompt_compact() -> String {
    format!(
        "ADILang v{} — bahasa AI-to-AI. Gunakan kosakata tertutup (lihat adilang_registry()). \
         Format: @payload for intent/reply/task; ui_layout for UI; spatial_3d for 3D. \
         Padat, maks 120 token/intent. Output hanya ```adilang block.",
        VERSION
    )
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::MESH_BUILDERS;

    #[test]
    fn schema_valid_json_dengan_versi_registry() {
        let s = json_schema_string().expect("schema json");
        let v: serde_json::Value = serde_json::from_str(&s).expect("parse");
        assert_eq!(v["$schema"], "http://json-schema.org/draft-07/schema#");
        assert_eq!(v["properties"]["version"]["const"], serde_json::Value::String(VERSION.into()));
    }

    #[test]
    fn schema_enum_mesh_sinkron_dengan_registry() {
        let v = json_schema();
        let def = v["$defs"]["mesh_builders"]["enum"].as_array().unwrap();
        let names: Vec<String> = def.iter().map(|e| e.as_str().unwrap().to_string()).collect();
        let expect: Vec<String> = MESH_BUILDERS.iter().map(|(n, _)| n.to_string()).collect();
        assert_eq!(names, expect, "enum mesh di schema harus persis tabel registry");
    }

    #[test]
    fn schema_enum_verb_dan_lightkind_ada() {
        let v = json_schema();
        let verbs = v["properties"]["verb"]["enum"].as_array().unwrap();
        assert!(verbs.iter().any(|e| e == "ask"));
        assert!(verbs.iter().any(|e| e == "inform"));
        let lk = v["$defs"]["light"]["properties"]["type"]["enum"].as_array().unwrap();
        assert!(lk.iter().any(|e| e == "point"));
        assert!(lk.iter().any(|e| e == "ambient"));
    }

    #[test]
    fn prompt_berisi_registry_dan_aturan() {
        let p = system_prompt();
        assert!(p.contains("ADILang Generator"));
        assert!(p.contains(VERSION));
        assert!(p.contains("mesh:"));
        assert!(p.contains("@payload"));
        assert!(p.contains("```adilang"));
        assert!(p.contains("120 token"));
    }

    #[test]
    fn prompt_compact_non_kosong() {
        let c = system_prompt_compact();
        assert!(c.contains("adilang_registry"));
        assert!(c.contains("spatial_3d"));
    }

    #[test]
    fn registry_category_ambil_nilai() {
        let mesh = registry_category("mesh");
        assert!(mesh.contains(&"sphere"));
        assert!(mesh.contains(&"grid"));
        assert_eq!(mesh.len(), 7);
        assert!(registry_category("tidak.ada").is_empty());
    }
}
