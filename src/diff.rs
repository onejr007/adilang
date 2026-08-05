// ADILang — Diffing & Patching level blok (v1.0.0).
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).
//
// Tujuan: LLM menerima perintah Bahasa Alami dari pengguna manusia, lalu
// menghasilkan "ADILang Patch Script" yang hanya mengubah blok yang diminta —
// tanpa perlu mengompilasi/menulis ulang seluruh dokumen.
//
// Model: dokumen ADILang = urutan blok top-level (ui_layout, spatial_3d,
// payload, func, handler, camera, light, entity, let). Setiap blok dikunci
// oleh `block_key` (mis. "ui_layout:hud", "func:tick", "handler:frame").
// `diff_docs` menghasilkan daftar op (add/remove/replace) antar-dua dokumen;
// `apply_doc`/`apply_patch_script` menerapkan op/script ke dokumen sumber.

use std::collections::HashMap;

use crate::ast::{EventKind, Program, TopLevel};
use crate::compactor::{render_program, render_top_level};
use crate::parser;

/// Kind manifest ADILang Patch Script.
pub const PATCH_KIND: &str = "adilang-patch-script";
/// Versi format patch script saat ini.
pub const PATCH_VERSION: &str = "1.0.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpKind {
    Add,
    Remove,
    Replace,
}

impl OpKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            OpKind::Add => "add",
            OpKind::Remove => "remove",
            OpKind::Replace => "replace",
        }
    }

    pub fn from_str(s: &str) -> Option<OpKind> {
        match s {
            "add" => Some(OpKind::Add),
            "remove" => Some(OpKind::Remove),
            "replace" => Some(OpKind::Replace),
            _ => None,
        }
    }
}

/// Satu operasi patch level-blok.
#[derive(Debug, Clone, PartialEq)]
pub struct DiffOp {
    pub kind: OpKind,
    /// Kunci blok (mis. "ui_layout:hud") — dipakai untuk menarget blok.
    pub key: String,
    /// Konten blok ter-render — diisi untuk add/replace; kosong utk remove.
    pub content: String,
}

impl DiffOp {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "kind": self.kind.as_str(),
            "key": self.key,
            "content": self.content,
        })
    }

    pub fn from_json(v: &serde_json::Value) -> Result<DiffOp, String> {
        let kind = OpKind::from_str(
            v.get("kind")
                .and_then(|k| k.as_str())
                .ok_or("patch: op butuh field 'kind'")?,
        )
        .ok_or("patch: 'kind' harus add/remove/replace")?;
        let key = v
            .get("key")
            .and_then(|k| k.as_str())
            .ok_or("patch: op butuh field 'key'")?
            .to_string();
        let content = v
            .get("content")
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string();
        Ok(DiffOp { kind, key, content })
    }

    pub fn from_json_str(s: &str) -> Result<DiffOp, String> {
        let v: serde_json::Value =
            serde_json::from_str(s).map_err(|e| format!("patch: JSON op tidak valid — {e}"))?;
        Self::from_json(&v)
    }
}

/// Kunci unik sebuah blok top-level.
pub fn block_key(item: &TopLevel) -> String {
    match item {
        TopLevel::Payload(_) => "payload:payload".to_string(),
        TopLevel::UILayout(d) => format!("ui_layout:{}", d.name),
        TopLevel::Spatial3D(d) => format!("spatial_3d:{}", d.name),
        TopLevel::World(d) => format!("world:{}", d.name),
        TopLevel::Camera(c) => format!("camera:{}", c.id),
        TopLevel::Light(l) => format!("light:{}", l.id),
        TopLevel::Entity(e) => format!("entity:{}", e.id),
        TopLevel::Let { name, .. } => format!("let:{name}"),
        TopLevel::Func(f) => format!("func:{}", f.name),
        TopLevel::Handler(h) => format!("handler:{}", event_key(&h.event)),
        TopLevel::UseJs(_) => "use_js".to_string(),
        TopLevel::Routes(_) => "routes".to_string(),
        TopLevel::I18n(_) => "i18n".to_string(),
        TopLevel::Component(c) => format!("component:{}", c.name),
    }
}

fn event_key(e: &EventKind) -> String {
    match e {
        EventKind::Frame => "frame",
        EventKind::Speak => "speak",
        EventKind::Silent => "silent",
        EventKind::Click => "click",
    }
    .to_string()
}

/// Pasang kunci→blok, menangani kunci duplikat dgn sufiks `@N`.
fn keyed_items(program: &Program) -> Vec<(String, TopLevel)> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    program
        .items
        .iter()
        .cloned()
        .map(|item| {
            let base = block_key(&item);
            let n = seen.entry(base.clone()).or_insert(0);
            *n += 1;
            let key = if *n == 1 {
                base
            } else {
                format!("{base}@{n}")
            };
            (key, item)
        })
        .collect()
}

/// Diff dua dokumen ADILang → daftar op level-blok.
/// Op remove/replace mengikuti urutan dokumen A; add mengikuti urutan B.
pub fn diff_docs(src_a: &str, src_b: &str) -> Result<Vec<DiffOp>, String> {
    let a = parser::parse(src_a)?;
    let b = parser::parse(src_b)?;
    let a_items = keyed_items(&a);
    let b_items = keyed_items(&b);

    let mut ops = Vec::new();
    for (key, item_a) in &a_items {
        match b_items.iter().find(|(k, _)| k == key) {
            Some((_, item_b)) => {
                let ca = render_top_level(item_a);
                let cb = render_top_level(item_b);
                if ca != cb {
                    ops.push(DiffOp {
                        kind: OpKind::Replace,
                        key: key.clone(),
                        content: cb,
                    });
                }
            }
            None => ops.push(DiffOp {
                kind: OpKind::Remove,
                key: key.clone(),
                content: String::new(),
            }),
        }
    }
    for (key, item_b) in &b_items {
        if !a_items.iter().any(|(k, _)| k == key) {
            ops.push(DiffOp {
                kind: OpKind::Add,
                key: key.clone(),
                content: render_top_level(item_b),
            });
        }
    }
    Ok(ops)
}

/// Diff dua dokumen → JSON (array op) — dipakai oleh WASM/JS/Python.
pub fn diff_docs_json(src_a: &str, src_b: &str) -> Result<String, String> {
    let ops = diff_docs(src_a, src_b)?;
    let arr: Vec<serde_json::Value> = ops.iter().map(DiffOp::to_json).collect();
    Ok(serde_json::Value::Array(arr).to_string())
}

fn parse_block(src: &str) -> Result<TopLevel, String> {
    let prog =
        parser::parse(src).map_err(|e| format!("patch: blok tidak valid — {e}"))?;
    let mut items = prog.items;
    if items.len() != 1 {
        return Err(format!(
            "patch: blok harus berisi tepat satu blok (ditemukan {})",
            items.len()
        ));
    }
    Ok(items.remove(0))
}

/// Terapkan daftar op ke dokumen sumber → dokumen baru.
pub fn apply_doc(src: &str, ops: &[DiffOp]) -> Result<String, String> {
    let program = parser::parse(src)?;
    let mut current = keyed_items(&program);
    let mut adds: Vec<TopLevel> = Vec::new();

    for op in ops {
        match op.kind {
            OpKind::Remove => {
                let pos = current
                    .iter()
                    .position(|(k, _)| k == &op.key)
                    .ok_or_else(|| format!("patch: blok '{}' tidak ditemukan", op.key))?;
                current.remove(pos);
            }
            OpKind::Replace => {
                let pos = current
                    .iter()
                    .position(|(k, _)| k == &op.key)
                    .ok_or_else(|| format!("patch: blok '{}' tidak ditemukan", op.key))?;
                let new_item = parse_block(&op.content)?;
                if block_key(&new_item) != op.key {
                    return Err(format!(
                        "patch: kunci blok ganti ('{}' → '{}') pada op replace — gunakan remove+add",
                        op.key,
                        block_key(&new_item)
                    ));
                }
                current[pos].1 = new_item;
            }
            OpKind::Add => adds.push(parse_block(&op.content)?),
        }
    }

    let mut items: Vec<TopLevel> = current.into_iter().map(|(_, item)| item).collect();
    items.extend(adds);
    let patched = Program {
        name: program.name,
        items,
    };
    Ok(render_program(&patched))
}

/// Terapkan daftar op JSON (array) ke dokumen sumber.
pub fn apply_doc_json(src: &str, ops_json: &str) -> Result<String, String> {
    let arr: Vec<serde_json::Value> = serde_json::from_str(ops_json)
        .map_err(|e| format!("patch: JSON ops tidak valid — {e}"))?;
    let ops: Vec<DiffOp> = arr.iter().map(DiffOp::from_json).collect::<Result<_, _>>()?;
    apply_doc(src, &ops)
}

/// Parse teks ADILang Patch Script → daftar op.
///
/// Format (tiga marker di awal baris; blok mengikuti hingga marker berikutnya):
/// ```text
/// adilang-patch 1.0.0
///
/// + ui_layout "hud" {
///     container {
///         flex column
///         text "Hello"
///     }
/// }
///
/// - func "old"
///
/// ~ spatial_3d "scene" {
///     camera "cam" { pos (0 1.6 7) look (0 0 0) fov 55 }
/// }
/// ```
/// `-` diikuti blok (kunci diturunkan) atau kunci mentah (mis. `- func:old`).
pub fn parse_patch_script(src: &str) -> Result<Vec<DiffOp>, String> {
    let lines: Vec<&str> = src.lines().collect();
    let mut start = 0;
    while start < lines.len() && lines[start].trim().is_empty() {
        start += 1;
    }
    if start >= lines.len() {
        return Err("patch script kosong".to_string());
    }
    let header = lines[start].trim();
    if !header.starts_with("adilang-patch") {
        return Err(format!(
            "patch: header wajib 'adilang-patch <versi>' — ditemukan '{header}'"
        ));
    }
    if let Some(v) = header.split_whitespace().nth(1) {
        if v != PATCH_VERSION {
            return Err(format!(
                "patch: versi tidak didukung '{v}' (harus {PATCH_VERSION})"
            ));
        }
    }

    let mut ops: Vec<DiffOp> = Vec::new();
    let mut idx = start + 1;
    while idx < lines.len() {
        let t = lines[idx].trim_start();
        if t.is_empty() {
            idx += 1;
            continue;
        }
        let marker = t.chars().next().unwrap();
        let kind = match marker {
            '+' => OpKind::Add,
            '-' => OpKind::Remove,
            '~' => OpKind::Replace,
            _ => {
                idx += 1;
                continue;
            }
        };

        let rest = t[marker.len_utf8()..].trim_start();
        let mut block_lines: Vec<String> = Vec::new();
        if !rest.is_empty() {
            block_lines.push(rest.to_string());
        }
        idx += 1;
        while idx < lines.len() {
            let t2 = lines[idx].trim_start();
            if !t2.is_empty()
                && (t2.starts_with('+')
                    || t2.starts_with('-')
                    || t2.starts_with('~'))
            {
                break;
            }
            if t2.starts_with("end patch") {
                idx += 1;
                break;
            }
            if !t2.is_empty() {
                block_lines.push(lines[idx].to_string());
            }
            idx += 1;
        }

        let block = block_lines.join("\n");
        let op = match kind {
            OpKind::Add | OpKind::Replace => {
                let item = parse_block(&block)?;
                DiffOp {
                    kind,
                    key: block_key(&item),
                    content: render_top_level(&item),
                }
            }
            OpKind::Remove => {
                let key = match parse_block(&block) {
                    Ok(item) => block_key(&item),
                    Err(_) => block.trim().to_string(),
                };
                DiffOp {
                    kind,
                    key,
                    content: String::new(),
                }
            }
        };
        ops.push(op);
    }
    Ok(ops)
}

/// Parse patch script → JSON (array op), utk preview/validasi oleh WASM/JS.
pub fn parse_patch_script_json(script: &str) -> Result<String, String> {
    let ops = parse_patch_script(script)?;
    let arr: Vec<serde_json::Value> = ops.iter().map(DiffOp::to_json).collect();
    Ok(serde_json::Value::Array(arr).to_string())
}

/// Terapkan patch script langsung ke dokumen sumber.
pub fn apply_patch_script(src: &str, script: &str) -> Result<String, String> {
    let ops = parse_patch_script(script)?;
    apply_doc(src, &ops)
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn base_doc() -> &'static str {
        r#"
ui_layout "hud" {
    container {
        flex column
        text "Hello"
        button "Go" onClick go
    }
}
spatial_3d "scene" {
    camera "cam" { pos (0 1.6 7) look (0 0 0) fov 55 }
    entity "cube" {
        pos (0 0 0)
        mesh sphere { radius 0.8 segments 3 }
        on frame { rotate(0.35 * t, (0.15 1 0.1)) }
    }
}
"#
    }

    #[test]
    fn diff_detects_add_remove_replace() {
        let a = base_doc();
        let b = r#"
ui_layout "hud" {
    container {
        flex column
        text "Halo"
        button "Go" onClick go
    }
}
entity "extra" { pos (1 2 3) }
"#;
        let ops = diff_docs(a, b).expect("diff ok");
        let kinds: Vec<&str> = ops.iter().map(|o| o.kind.as_str()).collect();
        assert_eq!(kinds, vec!["replace", "remove", "add"]);
        let replace = ops.iter().find(|o| o.kind == OpKind::Replace).unwrap();
        assert_eq!(replace.key, "ui_layout:hud");
        assert!(replace.content.contains("Halo"));
        let remove = ops.iter().find(|o| o.kind == OpKind::Remove).unwrap();
        assert_eq!(remove.key, "spatial_3d:scene");
        // entity "extra" di dokumen B ter-bungkus ke container implisit
        // `spatial_3d "__implicit__"` (parser legacy wrap) → kunci blok tsb.
        let add = ops.iter().find(|o| o.kind == OpKind::Add).unwrap();
        assert_eq!(add.key, "spatial_3d:__implicit__");
        assert!(add.content.contains("entity \"extra\""));
    }

    #[test]
    fn diff_identical_is_empty() {
        let src = base_doc();
        let ops = diff_docs(src, src).expect("diff ok");
        assert!(ops.is_empty());
    }

    #[test]
    fn apply_doc_patches_only_requested_block() {
        let ops = vec![DiffOp {
            kind: OpKind::Replace,
            key: "ui_layout:hud".to_string(),
            content: "ui_layout \"hud\" {\n    container {\n        flex row\n        text \"Halo\"\n    }\n}".to_string(),
        }];
        let out = apply_doc(base_doc(), &ops).expect("apply ok");
        assert!(out.contains("Halo"));
        assert!(out.contains("spatial_3d"));
        assert!(out.contains("rotate(0.35*t"));
        assert!(!out.contains("Hello"));
    }

    #[test]
    fn apply_doc_remove_and_add() {
        let ops = vec![
            DiffOp {
                kind: OpKind::Remove,
                key: "spatial_3d:scene".to_string(),
                content: String::new(),
            },
            DiffOp {
                kind: OpKind::Add,
                key: "func:tick".to_string(),
                content: "func tick() {\n    let x = 1\n}".to_string(),
            },
        ];
        let out = apply_doc(base_doc(), &ops).expect("apply ok");
        // spatial_3d "scene" terhapus; func tick menempel di container implisit
        // `spatial_3d "__implicit__"` — jadi string "spatial_3d" tetap ada.
        assert!(!out.contains("spatial_3d \"scene\""));
        assert!(out.contains("func tick"));
        let reparsed = parser::parse(&out).expect("hasil harus valid ADILang");
        assert_eq!(reparsed.items.len(), 2);
    }

    #[test]
    fn apply_doc_roundtrip_diff() {
        let a = base_doc();
        let b = r#"
ui_layout "hud" {
    container {
        flex column
        text "Halo"
    }
}
"#;
        let ops = diff_docs(a, b).expect("diff ok");
        let out = apply_doc(a, &ops).expect("apply ok");
        let pa = parser::parse(&out).expect("valid");
        let pb = parser::parse(b).expect("valid");
        assert_eq!(render_program(&pa), render_program(&pb));
    }

    #[test]
    fn parse_patch_script_add_replace_remove() {
        let script = r#"
adilang-patch 1.0.0

+ ui_layout "extra" {
    text "Baru"
}

~ ui_layout "hud" {
    container {
        flex column
        text "Halo"
    }
}

- func:old
"#;
        let ops = parse_patch_script(script).expect("script ok");
        let kinds: Vec<&str> = ops.iter().map(|o| o.kind.as_str()).collect();
        assert_eq!(kinds, vec!["add", "replace", "remove"]);
        assert_eq!(ops[1].key, "ui_layout:hud");
        // remove dgn kunci mentah (blok tak valid untuk dibungkus parser)
        assert_eq!(ops[2].key, "func:old");
    }

    #[test]
    fn parse_patch_script_bad_header() {
        let err = parse_patch_script("whatever\n+ ui_layout \"x\" {}\n").unwrap_err();
        assert!(err.contains("header"));
        let err2 = parse_patch_script("adilang-patch 9.9.9\n").unwrap_err();
        assert!(err2.contains("versi"));
    }

    #[test]
    fn apply_patch_script_end_to_end() {
        let script = r#"
adilang-patch 1.0.0

~ ui_layout "hud" {
    container {
        flex column
        text "Halo"
    }
}
"#;
        let out = apply_patch_script(base_doc(), script).expect("apply ok");
        assert!(out.contains("Halo"));
        assert!(!out.contains("Hello"));
        assert!(out.contains("rotate(0.35*t"));
        parser::parse(&out).expect("hasil harus valid ADILang");
    }

    #[test]
    fn patch_script_json_roundtrip() {
        let script = format!("adilang-patch {PATCH_VERSION}\n- spatial_3d:scene\n");
        let json = parse_patch_script_json(&script).expect("json ok");
        assert!(json.contains("\"kind\":\"remove\""));
        assert!(json.contains("\"key\":\"spatial_3d:scene\""));
        let out = apply_doc_json(base_doc(), &json).expect("apply json ok");
        assert!(!out.contains("spatial_3d"));
        parser::parse(&out).expect("hasil harus valid ADILang");
    }
}
