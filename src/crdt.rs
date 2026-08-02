// ADILang CRDT (adilang_crdt) — Conflict-free Replicated Data Type v1.11.0.
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).
//
// Memungkinkan beberapa agen AI mengedit satu dokumen ADILang secara
// bersamaan tanpa kehilangan state:
//   - AI 'A' mengerjakan ui_layout (2D UI)
//   - AI 'B' mengerjakan spatial_3d (3D scene)
//   - AI 'C' mengedit @payload (komunikasi)
// tanpa menimpa satu sama lain.
//
// Model: Register CRDT per "path" (sel AST):
//   path = "/ui/main/container/0/children/1/text"
//   value = JSON ringkas dari node (atau DELETED tombstone)
//   (value, lamport, agent) — merge: Lamport terbesar menang; jika imbang,
//   agent-id lebih besar menang (deterministik).
//
// Operasi:
//   - set(path, value) → op (add/update register)
//   - delete(path) → tombstone
//   - merge(replica) → gabungkan 2 state konvergen (commutative, idempotent)
//   - conflicts() → daftar path yang masih konflik (bila nilai berbeda)
//
// Sifat:
//   - Commutative: urutan merge tidak memengaruhi hasil.
//   - Idempotent:  merge(self) tidak mengubah apa-apa.
//   - Convergent:  merge(A,B) == merge(B,A) == merge(A,B,A).

use std::cmp::Ordering;
use std::collections::BTreeMap;

/// Tombstone: nilai khusus yang menandakan node telah dihapus.
pub const TOMBSTONE: &str = "\u{0}DELETED";

/// Nilai register — teks JSON ringkas dari node AST + metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct RegValue {
    /// Representasi JSON node (mis. `{"t":"button","label":"OK"}`).
    pub value: String,
    /// Waktu Lamport (dapat dari jam logika atau timestamp).
    pub lamport: u64,
    /// ID agen penulis (A, B, C, ...).
    pub agent: String,
}

impl RegValue {
    pub fn is_tombstone(&self) -> bool {
        self.value == TOMBSTONE
    }
}

/// State CRDT: path → register.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CrdtState {
    regs: BTreeMap<String, RegValue>,
}

/// Operasi CRDT (aksi yang bisa dikirim antar-agen).
#[derive(Debug, Clone, PartialEq)]
pub struct CrdtOp {
    pub path: String,
    pub value: String,
    pub lamport: u64,
    pub agent: String,
}

impl CrdtState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Path terdaftar (termasuk tombstone).
    pub fn paths(&self) -> Vec<String> {
        self.regs.keys().cloned().collect()
    }

    /// Ambil nilai register pada path (None bila belum ada).
    pub fn get(&self, path: &str) -> Option<&RegValue> {
        self.regs.get(path)
    }

    /// Ambil nilai murni (bukan tombstone) → None bila dihapus/absen.
    pub fn get_value(&self, path: &str) -> Option<&str> {
        self.regs.get(path).and_then(|r| {
            if r.is_tombstone() {
                None
            } else {
                Some(r.value.as_str())
            }
        })
    }

    /// Jumlah register hidup (non-tombstone).
    pub fn live_count(&self) -> usize {
        self.regs.values().filter(|r| !r.is_tombstone()).count()
    }

    /// Lamport maksimum di seluruh register (untuk jam logika otomatis).
    pub fn max_lamport(&self) -> u64 {
        self.regs.values().map(|r| r.lamport).max().unwrap_or(0)
    }

    pub fn total_count(&self) -> usize {
        self.regs.len()
    }

    /// Buat operasi set. `lamport` harus > semua yang sudah ada di path ini
    /// bila ingin menang; library TIDAK mengatur jam — pemanggil yang
    /// memastikan. Untuk kemudahan: `set_auto` memakai jam internal.
    pub fn make_set(&self, path: &str, value: String, lamport: u64, agent: &str) -> CrdtOp {
        CrdtOp {
            path: path.to_string(),
            value,
            lamport,
            agent: agent.to_string(),
        }
    }

    pub fn make_delete(&self, path: &str, lamport: u64, agent: &str) -> CrdtOp {
        self.make_set(path, TOMBSTONE.to_string(), lamport, agent)
    }

    /// Terapkan satu op ke state (LWW: lamport terbesar menang).
    pub fn apply(&mut self, op: &CrdtOp) -> bool {
        let new_reg = RegValue {
            value: op.value.clone(),
            lamport: op.lamport,
            agent: op.agent.clone(),
        };
        match self.regs.get(&op.path) {
            None => {
                self.regs.insert(op.path.clone(), new_reg);
                true
            }
            Some(existing) => {
                if wins(&new_reg, existing) {
                    self.regs.insert(op.path.clone(), new_reg);
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Konflik: path yang memiliki >1 nilai kandidat tidak mungkin terjadi di
    /// LWW (selalu ada pemenang deterministik). Fungsi ini mengembalikan path
    /// dengan nilai hidup yang mungkin SALAH dari perspektif agen lain —
    /// berguna untuk audit/visualisasi.
    pub fn conflicts(&self) -> Vec<String> {
        // LWW deterministik → tidak ada konflik permanen. Disediakan API untuk
        // kompatibilitas; kembalikan tombstone yang menutupi nilai (edge case).
        let mut out = Vec::new();
        for (path, r) in &self.regs {
            if r.is_tombstone() {
                out.push(path.clone());
            }
        }
        out
    }

    /// Merge: gabungkan state lain (konvergen). Idempotent & commutative.
    pub fn merge(&mut self, other: &CrdtState) {
        for (path, reg) in &other.regs {
            let new_reg = reg.clone();
            match self.regs.get(path) {
                None => {
                    self.regs.insert(path.clone(), new_reg);
                }
                Some(existing) => {
                    if wins(&new_reg, existing) {
                        self.regs.insert(path.clone(), new_reg);
                    }
                }
            }
        }
    }

    /// Snapshot seluruh register → JSON (untuk sinkronisasi penuh antar-agen).
    pub fn snapshot_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        for (path, r) in &self.regs {
            map.insert(
                path.clone(),
                serde_json::json!({
                    "v": r.value,
                    "t": r.lamport,
                    "a": r.agent,
                }),
            );
        }
        serde_json::Value::Object(map)
    }

    pub fn snapshot_string(&self) -> String {
        self.snapshot_json().to_string()
    }

    /// Muat snapshot JSON (kebalikan snapshot_json) — replace penuh.
    pub fn load_snapshot_json(&mut self, j: &serde_json::Value) {
        self.regs.clear();
        if let Some(obj) = j.as_object() {
            for (path, node) in obj {
                if let Some(r) = node.get("v").and_then(|v| v.as_str()) {
                    let lamport = node.get("t").and_then(|v| v.as_u64()).unwrap_or(0);
                    let agent = node
                        .get("a")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    self.regs.insert(
                        path.clone(),
                        RegValue {
                            value: r.to_string(),
                            lamport,
                            agent,
                        },
                    );
                }
            }
        }
    }

    pub fn load_snapshot_string(&mut self, s: &str) -> Result<(), String> {
        let v: serde_json::Value =
            serde_json::from_str(s).map_err(|e| format!("JSON invalid: {e}"))?;
        self.load_snapshot_json(&v);
        Ok(())
    }

    /// Daftar op yang belum ada di state ini (untuk transfer delta antar-agen).
    pub fn missing_ops(&self, other: &CrdtState) -> Vec<CrdtOp> {
        let mut out = Vec::new();
        for (path, reg) in &other.regs {
            match self.regs.get(path) {
                None => out.push(CrdtOp {
                    path: path.clone(),
                    value: reg.value.clone(),
                    lamport: reg.lamport,
                    agent: reg.agent.clone(),
                }),
                Some(existing) => {
                    if wins(reg, existing) {
                        out.push(CrdtOp {
                            path: path.clone(),
                            value: reg.value.clone(),
                            lamport: reg.lamport,
                            agent: reg.agent.clone(),
                        });
                    }
                }
            }
        }
        out
    }
}

/// Aturan menang (deterministik, tidak ambigu):
/// lamport lebih besar menang; imbang → agent lexicographically lebih besar.
fn wins(a: &RegValue, b: &RegValue) -> bool {
    match a.lamport.cmp(&b.lamport) {
        Ordering::Greater => true,
        Ordering::Less => false,
        Ordering::Equal => a.agent > b.agent,
    }
}

/// Helper membangun path AST ADILang yang stabil.
/// Contoh: entity "core" → `spatial/entity:core`; text di ui → `ui/0/children/1`.
pub fn join_path(parts: &[&str]) -> String {
    format!("/{}", parts.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_dan_get_dot_path() {
        let mut s = CrdtState::new();
        let op = s.make_set("/ui/main/text", "{\"t\":\"text\",\"v\":\"Halo\"}".into(), 1, "A");
        assert!(s.apply(&op));
        assert_eq!(s.get_value("/ui/main/text"), Some("{\"t\":\"text\",\"v\":\"Halo\"}"));
        assert_eq!(s.live_count(), 1);
    }

    #[test]
    fn lww_lamport_lebih_besar_menang() {
        let mut s = CrdtState::new();
        s.apply(&s.make_set("/ui/btn", "label:A".into(), 5, "A"));
        s.apply(&s.make_set("/ui/btn", "label:B".into(), 9, "B"));
        assert_eq!(s.get_value("/ui/btn"), Some("label:B"));
    }

    #[test]
    fn lww_imbang_agent_lebih_besar_menang() {
        let mut s = CrdtState::new();
        s.apply(&s.make_set("/ui/btn", "fromA".into(), 7, "A"));
        s.apply(&s.make_set("/ui/btn", "fromB".into(), 7, "B"));
        assert_eq!(s.get_value("/ui/btn"), Some("fromB"));
    }

    #[test]
    fn tombstone_menghapus_nilai() {
        let mut s = CrdtState::new();
        s.apply(&s.make_set("/ui/btn", "value".into(), 1, "A"));
        assert!(s.get_value("/ui/btn").is_some());
        s.apply(&s.make_delete("/ui/btn", 2, "A"));
        assert!(s.get_value("/ui/btn").is_none());
        assert_eq!(s.live_count(), 0);
        assert_eq!(s.total_count(), 1);
    }

    #[test]
    fn delete_tidak_menang_atas_lamport_baru() {
        let mut s = CrdtState::new();
        s.apply(&s.make_set("/ui/btn", "final".into(), 10, "A"));
        s.apply(&s.make_delete("/ui/btn", 3, "B")); // lamport lebih kecil → kalah
        assert_eq!(s.get_value("/ui/btn"), Some("final"));
    }

    #[test]
    fn merge_konvergen_commutative_idempotent() {
        // Agen A edit UI, agen B edit spatial — tidak saling timpa.
        let mut a = CrdtState::new();
        a.apply(&a.make_set("/ui/main/text", "A-UI".into(), 4, "A"));
        a.apply(&a.make_set("/ui/main/button", "A-BTN".into(), 2, "A"));

        let mut b = CrdtState::new();
        b.apply(&b.make_set("/spatial/scene/entity:core", "B-3D".into(), 5, "B"));
        b.apply(&b.make_set("/ui/main/text", "B-UI".into(), 3, "B")); // kalah dari A (4)

        let mut ab = a.clone();
        ab.merge(&b);
        let mut ba = b.clone();
        ba.merge(&a);

        assert_eq!(ab, ba, "merge harus commutative");
        // UI dimenangkan A, spatial dimenangkan B
        assert_eq!(ab.get_value("/ui/main/text"), Some("A-UI"));
        assert_eq!(ab.get_value("/spatial/scene/entity:core"), Some("B-3D"));
        // idempotent
        let mut aba = ab.clone();
        aba.merge(&b);
        assert_eq!(ab, aba);
    }

    #[test]
    fn merge_dengan_konflik_jam_imbang_deterministik() {
        let mut a = CrdtState::new();
        a.apply(&a.make_set("/ui/x", "A".into(), 9, "A"));
        let mut b = CrdtState::new();
        b.apply(&b.make_set("/ui/x", "B".into(), 9, "B"));
        let mut ab = a.clone();
        ab.merge(&b);
        assert_eq!(ab.get_value("/ui/x"), Some("B")); // B > A lexicographic
    }

    #[test]
    fn snapshot_roundtrip() {
        let mut s = CrdtState::new();
        s.apply(&s.make_set("/ui/main/container/0", "{\"t\":\"container\"}".into(), 1, "A"));
        s.apply(&s.make_set("/spatial/scene/entity:core", "{\"t\":\"entity\"}".into(), 2, "B"));
        let snap = s.snapshot_string();
        let mut t = CrdtState::new();
        t.load_snapshot_string(&snap).unwrap();
        assert_eq!(s, t);
    }

    #[test]
    fn missing_ops_menghasilkan_delta_transfer() {
        let mut a = CrdtState::new();
        a.apply(&a.make_set("/ui/a", "1".into(), 1, "A"));
        let mut b = CrdtState::new();
        b.apply(&b.make_set("/ui/a", "1".into(), 1, "A"));
        b.apply(&b.make_set("/ui/b", "2".into(), 2, "B"));
        let ops = a.missing_ops(&b);
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].path, "/ui/b");
        for op in &ops {
            a.apply(op);
        }
        assert_eq!(a, b);
    }

    #[test]
    fn path_helper() {
        assert_eq!(join_path(&["ui", "main", "container", "0", "children", "1"]), "/ui/main/container/0/children/1");
    }

    #[test]
    fn max_lamport_menjaga_jam_logika() {
        let mut s = CrdtState::new();
        assert_eq!(s.max_lamport(), 0);
        s.apply(&s.make_set("/a", "1".into(), 5, "A"));
        s.apply(&s.make_set("/b", "2".into(), 12, "B"));
        assert_eq!(s.max_lamport(), 12);
        // Set dengan lamport lebih kecil tidak menaikkan jam.
        s.apply(&s.make_set("/c", "3".into(), 3, "A"));
        assert_eq!(s.max_lamport(), 12);
    }
}
