// ADILang State (adilang_state) — reactive state engine.
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).
//
// Mirip Redux + Signals:
//   - dot-path get/set ("ui.player.health", "payload.intent", "scene.cam.0")
//   - revision counter naik pada setiap mutasi (untuk diff/re-render)
//   - subscribe(path-prefix, cb) → dipanggil saat path terkait berubah
//   - snapshot/load JSON penuh (untuk sinkronisasi store ↔ DOM/WebGL ↔ WASM)
//   - konvensi: prefix "payload" (dari blok @payload) & "ui" (ui_layout)
//     → is_render_relevant() untuk auto re-render pada perubahan atribut.

use std::collections::BTreeMap;

/// Prefix state yang memicu re-render UI/WebGL bila berubah.
pub const PAYLOAD_PREFIX: &str = "payload";
pub const UI_PREFIX: &str = "ui";

#[derive(Debug, Clone, PartialEq)]
pub enum StateValue {
    Null,
    Num(f64),
    Str(String),
    Bool(bool),
    Arr(Vec<StateValue>),
    Map(BTreeMap<String, StateValue>),
}

impl StateValue {
    pub fn type_name(&self) -> &'static str {
        match self {
            StateValue::Null => "null",
            StateValue::Num(_) => "number",
            StateValue::Str(_) => "string",
            StateValue::Bool(_) => "boolean",
            StateValue::Arr(_) => "array",
            StateValue::Map(_) => "object",
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        match self {
            StateValue::Null => serde_json::Value::Null,
            StateValue::Num(n) => serde_json::Value::from(*n),
            StateValue::Str(s) => serde_json::Value::from(s.as_str()),
            StateValue::Bool(b) => serde_json::Value::Bool(*b),
            StateValue::Arr(items) => serde_json::Value::Array(items.iter().map(|v| v.to_json()).collect()),
            StateValue::Map(map) => {
                serde_json::Value::Object(map.iter().map(|(k, v)| (k.clone(), v.to_json())).collect())
            }
        }
    }

    pub fn from_json(j: &serde_json::Value) -> StateValue {
        match j {
            serde_json::Value::Null => StateValue::Null,
            serde_json::Value::Bool(b) => StateValue::Bool(*b),
            serde_json::Value::Number(n) => StateValue::Num(n.as_f64().unwrap_or(0.0)),
            serde_json::Value::String(s) => StateValue::Str(s.clone()),
            serde_json::Value::Array(items) => StateValue::Arr(items.iter().map(Self::from_json).collect()),
            serde_json::Value::Object(map) => StateValue::Map(
                map.iter().map(|(k, v)| (k.clone(), Self::from_json(v))).collect(),
            ),
        }
    }
}

impl From<&str> for StateValue {
    fn from(s: &str) -> Self {
        StateValue::Str(s.to_string())
    }
}

impl From<f64> for StateValue {
    fn from(n: f64) -> Self {
        StateValue::Num(n)
    }
}

impl From<bool> for StateValue {
    fn from(b: bool) -> Self {
        StateValue::Bool(b)
    }
}

type SubId = usize;

pub struct StateStore {
    root: StateValue,
    revision: u64,
    subs: Vec<(SubId, Vec<String>, Box<dyn Fn(&str, &StateValue) + 'static>)>,
    next_id: SubId,
}

/// Normalisasi path dot → segmen. "a.b.0" → ["a","b","0"]. Kosong → [].
fn split_path(path: &str) -> Vec<String> {
    path.split('.')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn descend_map<'a>(node: &'a mut StateValue, seg: &str) -> Option<&'a mut StateValue> {
    match node {
        StateValue::Map(m) => Some(
            m.entry(seg.to_string()).or_insert_with(|| StateValue::Map(BTreeMap::new())),
        ),
        StateValue::Arr(items) => {
            if let Ok(idx) = seg.parse::<usize>() {
                while items.len() <= idx {
                    items.push(StateValue::Map(BTreeMap::new()));
                }
                Some(&mut items[idx])
            } else {
                None
            }
        }
        _ => None,
    }
}

fn get_path_value<'a>(node: &'a StateValue, segs: &[String]) -> Option<&'a StateValue> {
    let mut cur = node;
    for seg in segs {
        cur = match cur {
            StateValue::Map(m) => m.get(seg)?,
            StateValue::Arr(items) => items.get(seg.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(cur)
}

impl StateStore {
    pub fn new() -> Self {
        Self { root: StateValue::Map(BTreeMap::new()), revision: 0, subs: Vec::new(), next_id: 1 }
    }

    /// Revisi terakhir — naik hanya saat ada mutasi nyata (nilai berubah).
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Ambil nilai pada dot-path. Err bila path tidak ada.
    pub fn get(&self, path: &str) -> Result<&StateValue, String> {
        let segs = split_path(path);
        if segs.is_empty() {
            return Ok(&self.root);
        }
        get_path_value(&self.root, &segs).ok_or_else(|| format!("Path tidak ada: '{path}'"))
    }

    /// Set nilai pada dot-path (intermediate map dibuat otomatis).
    /// Return true bila nilai benar-benar berubah (revision +1 & notifikasi).
    pub fn set(&mut self, path: &str, value: StateValue) -> Result<bool, String> {
        let segs = split_path(path);
        if segs.is_empty() {
            return Err("Path kosong untuk set".into());
        }
        let target = self.navigate_create(&segs)?;
        if *target == value {
            return Ok(false);
        }
        *target = value;
        self.bump_and_notify(&segs);
        Ok(true)
    }

    pub fn set_json(&mut self, path: &str, json: &serde_json::Value) -> Result<bool, String> {
        self.set(path, StateValue::from_json(json))
    }

    /// Hapus key/path. Return true bila ada yang terhapus.
    pub fn delete(&mut self, path: &str) -> Result<bool, String> {
        let segs = split_path(path);
        if segs.is_empty() || segs.len() == 1 && segs[0].is_empty() {
            return Err("Path kosong untuk delete".into());
        }
        let parent_segs = &segs[..segs.len() - 1];
        let last = &segs[segs.len() - 1];
        let mut removed = false;
        if let Some(parent) = self.navigate_create(parent_segs).ok() {
            if !matches!(*parent, StateValue::Null) {
                match parent {
                    StateValue::Map(m) => removed = m.remove(last).is_some(),
                    StateValue::Arr(items) => {
                        if let Ok(idx) = last.parse::<usize>() {
                            if idx < items.len() {
                                items.remove(idx);
                                removed = true;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        if removed {
            self.bump_and_notify(&segs);
        }
        Ok(removed)
    }

    /// Increment numerik pada path. Path dibuat (0.0) bila belum ada.
    pub fn incr(&mut self, path: &str, by: f64) -> Result<f64, String> {
        let current = match self.get(path) {
            Ok(StateValue::Num(n)) => *n,
            Ok(_) => return Err(format!("'{path}' bukan angka")),
            Err(_) => 0.0,
        };
        let next = current + by;
        self.set(path, StateValue::Num(next))?;
        Ok(next)
    }

    /// Subscribe ke prefix path. Callback dipanggil (path, nilai baru) saat
    /// path yang berubah berada di dalam prefix. Prefix kosong = semua.
    pub fn subscribe(&mut self, prefix: &str, cb: Box<dyn Fn(&str, &StateValue) + 'static>) -> SubId {
        let id = self.next_id;
        self.next_id += 1;
        self.subs.push((id, split_path(prefix), cb));
        id
    }

    pub fn unsubscribe(&mut self, id: SubId) -> bool {
        let before = self.subs.len();
        self.subs.retain(|(sid, _, _)| *sid != id);
        self.subs.len() != before
    }

    /// Snapshot seluruh state → JSON (untuk JS bridge / persistensi).
    pub fn snapshot_json(&self) -> serde_json::Value {
        self.root.to_json()
    }

    /// Ganti seluruh state dari JSON (revision +1, notifikasi ke semua).
    pub fn load_json(&mut self, json: &serde_json::Value) {
        self.root = StateValue::from_json(json);
        self.bump_and_notify(&[]);
    }

    /// Flatten menjadi daftar (dot-path, nilai) — siap dipakai binding UI.
    pub fn snapshot_flat(&self) -> Vec<(String, StateValue)> {
        let mut out = Vec::new();
        self.flatten_into(&mut Vec::new(), &self.root, &mut out);
        out
    }

    fn flatten_into(&self, prefix: &mut Vec<String>, node: &StateValue, out: &mut Vec<(String, StateValue)>) {
        match node {
            StateValue::Map(m) => {
                for (k, v) in m {
                    prefix.push(k.clone());
                    self.flatten_into(prefix, v, out);
                    prefix.pop();
                }
            }
            StateValue::Arr(items) => {
                for (i, v) in items.iter().enumerate() {
                    prefix.push(i.to_string());
                    self.flatten_into(prefix, v, out);
                    prefix.pop();
                }
            }
            leaf => out.push((prefix.join("."), leaf.clone())),
        }
    }

    fn navigate_create(&mut self, segs: &[String]) -> Result<&mut StateValue, String> {
        if segs.is_empty() {
            return Ok(&mut self.root);
        }
        let mut cur = &mut self.root;
        for seg in &segs[..segs.len() - 1] {
            cur = descend_map(cur, seg).ok_or_else(|| {
                format!("Tidak bisa menembus nilai non-objek/array di segmen '{seg}'")
            })?;
        }
        let last = &segs[segs.len() - 1];
        descend_map(cur, last).ok_or_else(|| format!("Gagal membuat segmen '{last}'"))
    }

    fn bump_and_notify(&mut self, changed: &[String]) {
        self.revision += 1;
        let rev = self.revision;
        let changed_path = changed.join(".");
        let changed_value = get_path_value(&self.root, changed)
            .cloned()
            .unwrap_or(StateValue::Null);
        let targets: Vec<(String, StateValue)> = self
            .subs
            .iter()
            .filter(|(_, prefix, _)| prefix.is_empty() || {
                changed.len() >= prefix.len() && changed[..prefix.len()] == prefix[..]
            })
            .map(|_| (changed_path.clone(), changed_value.clone()))
            .collect();
        // panggil callback dengan snapshot agar tidak ada borrow clash
        for (i, (path, val)) in targets.iter().enumerate() {
            if let Some((_, _, cb)) = self.subs.get(i) {
                cb(path, val);
            }
        }
        let _ = rev;
    }
}

/// Apakah perubahan path ini relevan untuk re-render (@payload / ui_layout)?
pub fn is_render_relevant(path: &str) -> bool {
    let p = path.trim_start_matches('.');
    p == PAYLOAD_PREFIX
        || p == UI_PREFIX
        || p.starts_with(&format!("{}.", PAYLOAD_PREFIX))
        || p.starts_with(&format!("{}.", UI_PREFIX))
}

/// Nilai binding yang dirender dari path ("" bila tidak ada).
pub fn render_binding(store: &StateStore, path: &str) -> String {
    match store.get(path) {
        Ok(StateValue::Str(s)) => s.clone(),
        Ok(StateValue::Num(n)) => {
            if n.fract() == 0.0 {
                format!("{}", *n as i64)
            } else {
                format!("{n}")
            }
        }
        Ok(StateValue::Bool(b)) => format!("{b}"),
        Ok(StateValue::Null) => String::new(),
        Ok(v) => serde_json::to_string(&v.to_json()).unwrap_or_default(),
        Err(_) => String::new(),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn set_get_dan_revisi() {
        let mut s = StateStore::new();
        assert_eq!(s.revision(), 0);
        s.set("payload.intent", StateValue::Str("collaborate".into())).unwrap();
        assert_eq!(s.revision(), 1);
        assert_eq!(s.get("payload.intent").unwrap(), &StateValue::Str("collaborate".into()));
        // set nilai sama → tidak mutasi
        assert!(!s.set("payload.intent", StateValue::Str("collaborate".into())).unwrap());
        assert_eq!(s.revision(), 1);
        // set nilai baru → mutasi
        assert!(s.set("payload.intent", StateValue::Str("query".into())).unwrap());
        assert_eq!(s.revision(), 2);
    }

    #[test]
    fn array_index_dan_intermediate_map() {
        let mut s = StateStore::new();
        s.set("ui.slots.2.health", StateValue::Num(100.0)).unwrap();
        assert_eq!(s.get("ui.slots.2.health").unwrap(), &StateValue::Num(100.0));
        assert_eq!(s.snapshot_flat().len(), 1);
    }

    #[test]
    fn subscribe_prefix_trigger() {
        let mut s = StateStore::new();
        let hits = Rc::new(RefCell::new(Vec::new()));
        let h = hits.clone();
        s.subscribe("ui", Box::new(move |p, v| {
            h.borrow_mut().push((p.to_string(), v.type_name().to_string()));
        }));
        s.set("ui.player.health", StateValue::Num(50.0)).unwrap();
        s.set("payload.intent", StateValue::Str("hi".into())).unwrap(); // di luar prefix ui
        let hits = hits.borrow();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, "ui.player.health");
    }

    #[test]
    fn subscribe_exact_path_tidak_kena_anak() {
        let mut s = StateStore::new();
        let hits = Rc::new(RefCell::new(Vec::new()));
        let h = hits.clone();
        s.subscribe("ui.player", Box::new(move |p, _| h.borrow_mut().push(p.to_string())));
        s.set("ui.player.health", StateValue::Num(10.0)).unwrap();
        s.set("ui.menu.open", StateValue::Bool(true)).unwrap();
        assert_eq!(*hits.borrow(), vec!["ui.player.health".to_string()]);
    }

    #[test]
    fn subscribe_kosong_semua() {
        let mut s = StateStore::new();
        let count = Rc::new(RefCell::new(0u32));
        let c = count.clone();
        s.subscribe("", Box::new(move |_, _| *c.borrow_mut() += 1));
        s.set("a", StateValue::Num(1.0)).unwrap();
        s.set("payload.intent", StateValue::Str("x".into())).unwrap();
        s.delete("a").unwrap();
        assert_eq!(*count.borrow(), 3);
    }

    #[test]
    fn unsubscribe_menghentikan_notifikasi() {
        let mut s = StateStore::new();
        let count = Rc::new(RefCell::new(0u32));
        let c = count.clone();
        let id = s.subscribe("", Box::new(move |_, _| *c.borrow_mut() += 1));
        s.set("a", StateValue::Num(1.0)).unwrap();
        assert!(s.unsubscribe(id));
        s.set("a", StateValue::Num(2.0)).unwrap();
        assert_eq!(*count.borrow(), 1);
        assert!(!s.unsubscribe(id));
    }

    #[test]
    fn incr_membuat_path_baru() {
        let mut s = StateStore::new();
        assert_eq!(s.incr("scene.frame", 1.0).unwrap(), 1.0);
        assert_eq!(s.incr("scene.frame", 2.0).unwrap(), 3.0);
    }

    #[test]
    fn snapshot_dan_load_json() {
        let mut s = StateStore::new();
        s.set("payload.sender", StateValue::Str("ai-1".into())).unwrap();
        s.set("ui.score", StateValue::Num(7.0)).unwrap();
        let snap = s.snapshot_json();
        let json_str = serde_json::to_string(&snap).unwrap();
        let mut s2 = StateStore::new();
        s2.load_json(&serde_json::from_str(&json_str).unwrap());
        assert_eq!(s2.get("payload.sender").unwrap(), &StateValue::Str("ai-1".into()));
        assert_eq!(s2.get("ui.score").unwrap(), &StateValue::Num(7.0));
        assert!(s2.revision() > 0);
    }

    #[test]
    fn render_relevan_hanya_payload_dan_ui() {
        assert!(is_render_relevant("payload.intent"));
        assert!(is_render_relevant("ui.button.label"));
        assert!(is_render_relevant("payload"));
        assert!(!is_render_relevant("scene.cam.pos"));
        assert!(!is_render_relevant("memory"));
    }

    #[test]
    fn render_binding_format() {
        let mut s = StateStore::new();
        s.set("ui.title", StateValue::Str("Status: OK".into())).unwrap();
        s.set("ui.count", StateValue::Num(3.0)).unwrap();
        s.set("ui.ratio", StateValue::Num(0.5)).unwrap();
        assert_eq!(render_binding(&s, "ui.title"), "Status: OK");
        assert_eq!(render_binding(&s, "ui.count"), "3");
        assert_eq!(render_binding(&s, "ui.ratio"), "0.5");
        assert_eq!(render_binding(&s, "ui.tidak.ada"), "");
    }

    #[test]
    fn nilai_tidak_mungkin_menembus_non_objek() {
        let mut s = StateStore::new();
        s.set("a", StateValue::Num(1.0)).unwrap();
        assert!(s.set("a.b", StateValue::Num(2.0)).is_err());
        assert!(s.set("a.0", StateValue::Num(2.0)).is_err());
    }

    #[test]
    fn delete_hapus_key_dan_naikkan_revisi() {
        let mut s = StateStore::new();
        s.set("ui.menu.open", StateValue::Bool(true)).unwrap();
        let rev = s.revision();
        assert!(s.delete("ui.menu.open").unwrap());
        assert_eq!(s.revision(), rev + 1);
        assert!(s.get("ui.menu.open").is_err());
    }
}
