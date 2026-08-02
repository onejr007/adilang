// ADILang — Rendering layer abstraction (adilang_target, v1.0.0).
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).
//
// Lapisan abstraksi render cross-platform: menyeleksi backend render
// (WebGL2 / WebGPU / wgpu-native) secara DETERMINISTIK berdasarkan
// kapabilitas yang dibutuhkan adegan + preferensi AI/klien.
//
//   - Backend saat ini (WASM): WebGL2 (glow). WebGPU & wgpu-native adalah
//     backend target (sama-sama dipetakan ke render layer yang sama).
//   - Logika seleksi murni Rust & deterministik → bisa diuji native tanpa
//     browser. Modul ini adalah "seam" agar engine tak terikat ke satu API.

use std::collections::BTreeSet;

/// Backend render yang dikenal ADILang.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Backend {
    WebGl2,
    WebGpu,
    WgpuNative,
}

impl Backend {
    pub fn as_str(&self) -> &'static str {
        match self {
            Backend::WebGl2 => "webgl2",
            Backend::WebGpu => "webgpu",
            Backend::WgpuNative => "wgpu_native",
        }
    }

    pub fn from_str(s: &str) -> Option<Backend> {
        match s {
            "webgl2" => Some(Backend::WebGl2),
            "webgpu" => Some(Backend::WebGpu),
            "wgpu_native" => Some(Backend::WgpuNative),
            _ => None,
        }
    }

    /// Prioritas (lebih tinggi = lebih modern/dimuat native).
    pub fn priority(&self) -> u8 {
        match self {
            Backend::WebGl2 => 1,
            Backend::WebGpu => 2,
            Backend::WgpuNative => 3,
        }
    }
}

/// Kapabilitas render yang mungkin dibutuhkan sebuah adegan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Cap {
    /// Compute shader (partikel/ribuan mesh).
    Compute,
    /// Tekstur float (HDR).
    FloatTextures,
    /// Instanced rendering (ribuan instance 1 draw call).
    Instancing,
    /// Tekstur minimal 4096×4096.
    Texture4096,
}

/// Kapabilitas aktual yang ditawarkan sebuah backend.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderCaps {
    pub backend: Backend,
    pub max_texture_size: u32,
    pub compute: bool,
    pub float_textures: bool,
    pub instancing: bool,
}

impl RenderCaps {
    pub fn has(&self, cap: Cap) -> bool {
        match cap {
            Cap::Compute => self.compute,
            Cap::FloatTextures => self.float_textures,
            Cap::Instancing => self.instancing,
            Cap::Texture4096 => self.max_texture_size >= 4096,
        }
    }
}

/// Kapabilitas default deterministik per backend (minimal jaminan spec).
pub fn default_caps(backend: Backend) -> RenderCaps {
    match backend {
        Backend::WebGl2 => RenderCaps {
            backend,
            max_texture_size: 4096,
            compute: false,
            float_textures: true,
            instancing: true,
        },
        Backend::WebGpu => RenderCaps {
            backend,
            max_texture_size: 8192,
            compute: true,
            float_textures: true,
            instancing: true,
        },
        Backend::WgpuNative => RenderCaps {
            backend,
            max_texture_size: 8192,
            compute: true,
            float_textures: true,
            instancing: true,
        },
    }
}

/// Pilih backend render terbaik secara deterministik.
///
/// - `available`: backend yang benar-benar tersedia di runtime.
/// - `preference`: keinginan AI/klien (None = auto → prioritas tertinggi).
/// - `require`: kapabilitas minimum yang wajib dipenuhi.
///
/// Menghasilkan Err bila tak ada backend memenuhi SEMUA syarat.
pub fn select_backend(
    available: &[Backend],
    preference: Option<Backend>,
    require: &[Cap],
) -> Result<Backend, String> {
    let need: BTreeSet<Cap> = require.iter().copied().collect();

    let meets = |b: &Backend| -> bool {
        let caps = default_caps(*b);
        need.iter().all(|c| caps.has(*c))
    };

    if let Some(pref) = preference {
        if available.contains(&pref) && meets(&pref) {
            return Ok(pref);
        }
    }

    let mut candidates: Vec<Backend> = available.iter().copied().filter(meets).collect();
    candidates.sort_by(|a, b| b.priority().cmp(&a.priority()));
    candidates.into_iter().next().ok_or_else(|| {
        let need_str: Vec<&str> = need.iter().map(|c| cap_str(*c)).collect();
        let avail: Vec<&str> = available.iter().map(|b| b.as_str()).collect();
        format!(
            "target: tidak ada backend ({}) memenuhi syarat [{}]",
            avail.join(", "),
            need_str.join(", ")
        )
    })
}

fn cap_str(c: Cap) -> &'static str {
    match c {
        Cap::Compute => "compute",
        Cap::FloatTextures => "float_textures",
        Cap::Instancing => "instancing",
        Cap::Texture4096 => "texture_4096",
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_auto_memilih_prioritas_tertinggi() {
        let sel = select_backend(&[Backend::WebGl2, Backend::WebGpu], None, &[])
            .expect("webgpu tersedia");
        assert_eq!(sel, Backend::WebGpu);
    }

    #[test]
    fn select_preferensi_menang_bila_tersedia() {
        let sel = select_backend(
            &[Backend::WebGl2, Backend::WebGpu],
            Some(Backend::WebGl2),
            &[],
        )
        .expect("webgl2 tersedia");
        assert_eq!(sel, Backend::WebGl2);
    }

    #[test]
    fn select_preferensi_terabaikan_bila_tidak_tersedia() {
        // prefer webgpu tapi tak tersedia → jatuh ke webgl2 (auto)
        let sel = select_backend(&[Backend::WebGl2], Some(Backend::WebGpu), &[])
            .expect("fallback webgl2");
        assert_eq!(sel, Backend::WebGl2);
    }

    #[test]
    fn select_compute_menolak_webgl2() {
        // WebGL2 tak punya compute → butuh webgpu/wgpu_native
        let sel = select_backend(
            &[Backend::WebGl2],
            None,
            &[Cap::Compute],
        );
        assert!(sel.is_err(), "webgl2 harus ditolak utk compute");
        let sel2 = select_backend(
            &[Backend::WebGl2, Backend::WebGpu],
            None,
            &[Cap::Compute],
        )
        .expect("webgpu memenuhi compute");
        assert_eq!(sel2, Backend::WebGpu);
    }

    #[test]
    fn select_wgpu_native_prioritas_native() {
        let sel = select_backend(
            &[Backend::WebGl2, Backend::WebGpu, Backend::WgpuNative],
            None,
            &[Cap::Instancing, Cap::Texture4096],
        )
        .expect("semua memenuhi");
        assert_eq!(sel, Backend::WgpuNative);
    }

    #[test]
    fn default_caps_konsisten_dengan_has() {
        for b in [Backend::WebGl2, Backend::WebGpu, Backend::WgpuNative] {
            let caps = default_caps(b);
            for c in [Cap::Compute, Cap::FloatTextures, Cap::Instancing, Cap::Texture4096] {
                let expected = match b {
                    Backend::WebGl2 => c != Cap::Compute,
                    Backend::WebGpu | Backend::WgpuNative => true,
                };
                assert_eq!(caps.has(c), expected, "{b:?} harus punya {c:?}");
            }
        }
    }
}
