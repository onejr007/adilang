// ADILang — Telemetry & Analytics engine (v1.0.0).
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).
//
// Perekam metrik runtime ADILang yang DETERMINISTIK & murni (tanpa dependensi
// platform): jumlah frame, FPS window berjalan, durasi render rata-rata,
// serta hitungan event (load/speak/silent/action/state_set/error). Dipakai
// WASM (wasm_api.rs) via thread_local singleton; ekspor JSON utk dashboard
// telemetry. Modul sengaja murni Rust agar bisa diuji native di cargo test.

use std::collections::BTreeMap;

/// Panjang window FPS (ms) — setelahnya window di-reset.
pub const WINDOW_MS: f64 = 1000.0;

/// Versi skema snapshot analytics.
pub const ANALYTICS_SCHEMA: &str = "1.0.0";

#[derive(Debug, Clone)]
pub struct Analytics {
    frames: u64,
    loads: u64,
    speaks: u64,
    silences: u64,
    actions: u64,
    state_sets: u64,
    errors: u64,
    total_frame_ms: f64,
    total_render_ms: f64,
    min_frame_ms: f64,
    max_frame_ms: f64,
    window_start_ms: f64,
    window_frames: u64,
    window_ms: f64,
    window_render_ms: f64,
    events: BTreeMap<String, u64>,
}

impl Default for Analytics {
    fn default() -> Self {
        Self::new()
    }
}

impl Analytics {
    pub fn new() -> Self {
        Analytics {
            frames: 0,
            loads: 0,
            speaks: 0,
            silences: 0,
            actions: 0,
            state_sets: 0,
            errors: 0,
            total_frame_ms: 0.0,
            total_render_ms: 0.0,
            min_frame_ms: f64::MAX,
            max_frame_ms: 0.0,
            window_start_ms: 0.0,
            window_frames: 0,
            window_ms: 0.0,
            window_render_ms: 0.0,
            events: BTreeMap::new(),
        }
    }

    /// Rekam satu frame: `frame_ms` = durasi antar-frame (utk FPS),
    /// `render_ms` = durasi render frame tsb.
    pub fn record_frame(&mut self, frame_ms: f64, render_ms: f64) {
        let frame_ms = frame_ms.max(0.0);
        let render_ms = render_ms.max(0.0);
        if self.window_start_ms == 0.0 {
            self.window_start_ms = frame_ms;
        }
        self.frames += 1;
        self.total_frame_ms += frame_ms;
        self.total_render_ms += render_ms;
        self.min_frame_ms = self.min_frame_ms.min(frame_ms);
        self.max_frame_ms = self.max_frame_ms.max(frame_ms);
        self.window_frames += 1;
        self.window_ms += frame_ms;
        self.window_render_ms += render_ms;
        if frame_ms - self.window_start_ms >= WINDOW_MS {
            self.window_start_ms = frame_ms;
            self.window_frames = 0;
            self.window_ms = 0.0;
            self.window_render_ms = 0.0;
        }
    }

    pub fn record_load(&mut self) {
        self.loads += 1;
        *self.events.entry("load".to_string()).or_insert(0) += 1;
    }

    pub fn record_speak(&mut self) {
        self.speaks += 1;
        *self.events.entry("speak".to_string()).or_insert(0) += 1;
    }

    pub fn record_silent(&mut self) {
        self.silences += 1;
        *self.events.entry("silent".to_string()).or_insert(0) += 1;
    }

    pub fn record_action(&mut self) {
        self.actions += 1;
        *self.events.entry("action".to_string()).or_insert(0) += 1;
    }

    pub fn record_state_set(&mut self) {
        self.state_sets += 1;
        *self.events.entry("state_set".to_string()).or_insert(0) += 1;
    }

    pub fn record_error(&mut self) {
        self.errors += 1;
        *self.events.entry("error".to_string()).or_insert(0) += 1;
    }

    pub fn frames(&self) -> u64 {
        self.frames
    }

    pub fn loads(&self) -> u64 {
        self.loads
    }

    pub fn errors(&self) -> u64 {
        self.errors
    }

    /// FPS dari window berjalan (fallback: rata-rata total sejak reset).
    pub fn frame_rate(&self) -> f64 {
        if self.window_ms > 0.0 {
            self.window_frames as f64 / (self.window_ms / 1000.0)
        } else if self.total_frame_ms > 0.0 {
            self.frames as f64 / (self.total_frame_ms / 1000.0)
        } else {
            0.0
        }
    }

    pub fn avg_frame_ms(&self) -> f64 {
        if self.frames > 0 {
            self.total_frame_ms / self.frames as f64
        } else {
            0.0
        }
    }

    pub fn avg_render_ms(&self) -> f64 {
        if self.frames > 0 {
            self.total_render_ms / self.frames as f64
        } else {
            0.0
        }
    }

    pub fn min_frame_ms(&self) -> f64 {
        if self.frames > 0 {
            self.min_frame_ms
        } else {
            0.0
        }
    }

    pub fn max_frame_ms(&self) -> f64 {
        self.max_frame_ms
    }

    /// Snapshot JSON — format tetap utk dashboard telemetry.
    pub fn snapshot_json(&self) -> String {
        serde_json::json!({
            "schema": ANALYTICS_SCHEMA,
            "frames": self.frames,
            "fps": round2(self.frame_rate()),
            "avg_frame_ms": round2(self.avg_frame_ms()),
            "avg_render_ms": round2(self.avg_render_ms()),
            "min_frame_ms": round2(self.min_frame_ms()),
            "max_frame_ms": round2(self.max_frame_ms()),
            "loads": self.loads,
            "speaks": self.speaks,
            "silences": self.silences,
            "actions": self.actions,
            "state_sets": self.state_sets,
            "errors": self.errors,
            "events": self.events,
        })
        .to_string()
    }

    pub fn reset(&mut self) {
        *self = Analytics::new();
    }
}

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analytics_mulai_kosong() {
        let a = Analytics::new();
        assert_eq!(a.frames(), 0);
        assert_eq!(a.frame_rate(), 0.0);
        assert_eq!(a.avg_frame_ms(), 0.0);
    }

    #[test]
    fn analytics_menghitung_fps_window() {
        let mut a = Analytics::new();
        // 60 frame @ ~16.6ms → FPS ≈ 60
        for i in 0..60 {
            a.record_frame(16.67, 5.0);
            let _ = i;
        }
        let fps = a.frame_rate();
        assert!((fps - 60.0).abs() < 2.0, "FPS ≈ 60, dapat {fps}");
        assert_eq!(a.frames(), 60);
        assert!(a.avg_render_ms() > 4.9 && a.avg_render_ms() < 5.1);
    }

    #[test]
    fn analytics_window_reset_setelah_1s() {
        let mut a = Analytics::new();
        for i in 0..120 {
            a.record_frame(8.0, 2.0);
            let _ = i;
        }
        // 120 frame @ 8ms = 960ms < 1000ms window → belum reset, fps≈125
        let fps = a.frame_rate();
        assert!((fps - 125.0).abs() < 3.0, "FPS ≈ 125, dapat {fps}");
        // 1 frame @ 1000ms → reset window (frame ini dihitung sendiri)
        a.record_frame(1000.0, 10.0);
        assert_eq!(a.frames(), 121);
        assert!(a.max_frame_ms() >= 1000.0);
        assert!(a.min_frame_ms() <= 8.0);
    }

    #[test]
    fn analytics_event_counters() {
        let mut a = Analytics::new();
        a.record_load();
        a.record_load();
        a.record_speak();
        a.record_silent();
        a.record_action();
        a.record_state_set();
        a.record_error();
        assert_eq!(a.loads(), 2);
        assert_eq!(a.errors(), 1);
        let snap: serde_json::Value =
            serde_json::from_str(&a.snapshot_json()).expect("snapshot harus JSON valid");
        assert_eq!(snap["loads"], 2);
        assert_eq!(snap["events"]["action"], 1);
        assert_eq!(snap["schema"], ANALYTICS_SCHEMA);
    }

    #[test]
    fn analytics_reset_membersihkan() {
        let mut a = Analytics::new();
        a.record_frame(16.0, 4.0);
        a.record_error();
        a.reset();
        assert_eq!(a.frames(), 0);
        assert_eq!(a.errors(), 0);
        assert_eq!(a.frame_rate(), 0.0);
    }
}
