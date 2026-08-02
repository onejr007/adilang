// ADILang Spatial Engine (adilang_spatial) — v1.11.0.
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).
//
// Dua kemampuan:
//   1. Generator bentuk 3D PROSEDURAL (tanpa aset .gltf eksternal):
//      sphere, box, torus, icosa, ring, plane, grid → vertex/index buffer
//      siap GL. Bisa dipakai native (tests) maupun WASM/WebGL2.
//   2. Rasterisasi UI 2D ke buffer tekstur RGBA (software renderer + bitmap
//      font 5x7 bawaan) → dipakai untuk merender elemen ui_layout DI ATAS
//      permukaan objek 3D di WebGL2 (texture mapping, spatial UI).

use crate::ast::{FlexDirection, UIComponent, UILayoutDef};

// ═══════════════════════════════════════════════════════════════════════════
// 1. PROCEDURAL 3D
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeKind {
    Sphere,
    Box,
    Torus,
    Icosa,
    Ring,
    Plane,
    Grid,
}

impl ShapeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ShapeKind::Sphere => "sphere",
            ShapeKind::Box => "box",
            ShapeKind::Torus => "torus",
            ShapeKind::Icosa => "icosa",
            ShapeKind::Ring => "ring",
            ShapeKind::Plane => "plane",
            ShapeKind::Grid => "grid",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "sphere" => Some(ShapeKind::Sphere),
            "box" => Some(ShapeKind::Box),
            "torus" => Some(ShapeKind::Torus),
            "icosa" => Some(ShapeKind::Icosa),
            "ring" => Some(ShapeKind::Ring),
            "plane" => Some(ShapeKind::Plane),
            "grid" => Some(ShapeKind::Grid),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShapeParams {
    pub radius: f32,
    pub tube: f32,
    pub inner: f32,
    pub segments: f32,
    pub size: f32,
    pub count: f32,
}

impl Default for ShapeParams {
    fn default() -> Self {
        Self {
            radius: 1.0,
            tube: 0.05,
            inner: 1.0,
            segments: 2.0,
            size: 10.0,
            count: 16.0,
        }
    }
}

/// Mesh prosedural siap upload ke GPU (layout: position(n3) normal(n3) uv(n2)).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SpatialMesh {
    pub positions: Vec<f32>,
    pub normals: Vec<f32>,
    pub uvs: Vec<f32>,
    pub indices: Vec<u32>,
}

impl SpatialMesh {
    pub fn vertex_count(&self) -> usize {
        self.positions.len() / 3
    }
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
    /// Interleaved [pos3, normal3, uv2] per vertex — format siap GL upload.
    pub fn interleaved(&self) -> Vec<f32> {
        let n = self.vertex_count();
        let mut out = Vec::with_capacity(n * 8);
        for i in 0..n {
            out.extend_from_slice(&self.positions[i * 3..i * 3 + 3]);
            out.extend_from_slice(&self.normals[i * 3..i * 3 + 3]);
            out.extend_from_slice(&self.uvs[i * 2..i * 2 + 2]);
        }
        out
    }
}

/// Generator prosedural — pilih bentuk, dapat mesh.
pub fn generate_shape(kind: ShapeKind, p: &ShapeParams) -> SpatialMesh {
    match kind {
        ShapeKind::Sphere => sphere(p.radius, p.segments),
        ShapeKind::Box => box_mesh(p.size),
        ShapeKind::Torus => torus(p.radius, p.tube),
        ShapeKind::Icosa => icosa(p.radius, p.inner),
        ShapeKind::Ring => ring(p.radius, p.tube),
        ShapeKind::Plane => plane(p.size),
        ShapeKind::Grid => grid(p.size, p.count),
    }
}

/// Generator semua bentuk default (untuk preview/tests).
pub fn generate_all() -> Vec<(String, SpatialMesh)> {
    ShapeKind::from_str_all()
        .iter()
        .map(|k| (k.as_str().to_string(), generate_shape(*k, &ShapeParams::default())))
        .collect()
}

impl ShapeKind {
    fn from_str_all() -> Vec<ShapeKind> {
        vec![
            ShapeKind::Sphere,
            ShapeKind::Box,
            ShapeKind::Torus,
            ShapeKind::Icosa,
            ShapeKind::Ring,
            ShapeKind::Plane,
            ShapeKind::Grid,
        ]
    }
}

fn push_vert(m: &mut SpatialMesh, pos: [f32; 3], nrm: [f32; 3], uv: [f32; 2]) {
    m.positions.extend_from_slice(&pos);
    m.normals.extend_from_slice(&nrm);
    m.uvs.extend_from_slice(&uv);
}

fn sphere(radius: f32, segs_f: f32) -> SpatialMesh {
    let mut m = SpatialMesh::default();
    let segs = (segs_f as usize).clamp(2, 64);
    let stride = segs + 1;
    for lat in 0..=segs {
        let theta = std::f32::consts::PI * lat as f32 / segs as f32;
        let sy = theta.cos();
        let sxy = theta.sin();
        for lon in 0..=segs {
            let phi = 2.0 * std::f32::consts::PI * lon as f32 / segs as f32;
            let n = [sxy * phi.cos(), sy, sxy * phi.sin()];
            let uv = [lon as f32 / segs as f32, lat as f32 / segs as f32];
            push_vert(&mut m, [n[0] * radius, n[1] * radius, n[2] * radius], n, uv);
        }
    }
    for lat in 0..segs {
        for lon in 0..segs {
            let a = (lat * stride + lon) as u32;
            let b = a + 1;
            let c = a + stride as u32;
            let d = c + 1;
            m.indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }
    m
}

fn box_mesh(size: f32) -> SpatialMesh {
    let mut m = SpatialMesh::default();
    let h = size / 2.0;
    let corners: [[f32; 3]; 8] = [
        [-h, -h, -h], [h, -h, -h], [h, -h, h], [-h, -h, h],
        [-h, h, -h], [h, h, -h], [h, h, h], [-h, h, h],
    ];
    let faces: [([usize; 4], [f32; 3]); 6] = [
        ([0, 1, 2, 3], [0.0, -1.0, 0.0]),
        ([4, 5, 6, 7], [0.0, 1.0, 0.0]),
        ([0, 4, 5, 1], [0.0, 0.0, -1.0]),
        ([2, 6, 7, 3], [0.0, 0.0, 1.0]),
        ([1, 5, 6, 2], [1.0, 0.0, 0.0]),
        ([0, 3, 7, 4], [-1.0, 0.0, 0.0]),
    ];
    for (idx, n) in faces {
        let base = m.vertex_count() as u32;
        for (i, &ci) in idx.iter().enumerate() {
            let uv = [
                if i % 2 == 0 { 0.0 } else { 1.0 },
                if i < 2 { 0.0 } else { 1.0 },
            ];
            push_vert(&mut m, corners[ci], n, uv);
        }
        m.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    m
}

fn torus(r: f32, tube: f32) -> SpatialMesh {
    let mut m = SpatialMesh::default();
    let ms = 24usize;
    let ns = 12usize;
    let stride = ns + 1;
    for i in 0..=ms {
        let u = 2.0 * std::f32::consts::PI * i as f32 / ms as f32;
        for j in 0..=ns {
            let v = 2.0 * std::f32::consts::PI * j as f32 / ns as f32;
            let n = [v.cos() * u.cos(), v.sin(), v.cos() * u.sin()];
            let cx = r + tube * v.cos();
            let pos = [cx * u.cos(), tube * v.sin(), cx * u.sin()];
            let uv = [i as f32 / ms as f32, j as f32 / ns as f32];
            push_vert(&mut m, pos, n, uv);
        }
    }
    for i in 0..ms {
        for j in 0..ns {
            let a = (i * stride + j) as u32;
            let b = a + 1;
            let c = a + stride as u32;
            let d = c + 1;
            m.indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }
    m
}

fn icosa(r: f32, inner: f32) -> SpatialMesh {
    let t = (1.0 + 5.0f32.sqrt()) / 2.0;
    let base_verts: [[f32; 3]; 12] = [
        [-1.0, t, 0.0], [1.0, t, 0.0], [-1.0, -t, 0.0], [1.0, -t, 0.0],
        [0.0, -1.0, t], [0.0, 1.0, t], [0.0, -1.0, -t], [0.0, 1.0, -t],
        [t, 0.0, -1.0], [t, 0.0, 1.0], [-t, 0.0, -1.0], [-t, 0.0, 1.0],
    ];
    let faces: [[usize; 3]; 20] = [
        [0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11],
        [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8],
        [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9],
        [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1],
    ];
    let norm = |v: [f32; 3]| -> [f32; 3] {
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-9);
        [v[0] / l, v[1] / l, v[2] / l]
    };
    let mut m = SpatialMesh::default();
    let base = m.vertex_count() as u32;
    for p in base_verts {
        let n = norm(p);
        push_vert(&mut m, [n[0] * r, n[1] * r, n[2] * r], n, [0.0, 0.0]);
    }
    for f in faces {
        m.indices.extend_from_slice(&[(base + f[0] as u32), (base + f[1] as u32), (base + f[2] as u32)]);
    }
    if inner > 0.05 && inner < 1.0 {
        let ibase = m.vertex_count() as u32;
        for p in base_verts {
            let n = norm(p);
            push_vert(&mut m, [n[0] * r * inner, n[1] * r * inner, n[2] * r * inner], n, [0.0, 0.0]);
        }
        // faces inner dibalik normal → buat indeks dengan arah terbalik
        for f in faces {
            m.indices.extend_from_slice(&[(ibase + f[0] as u32), (ibase + f[2] as u32), (ibase + f[1] as u32)]);
        }
    }
    m
}

fn ring(r: f32, tube: f32) -> SpatialMesh {
    let mut m = SpatialMesh::default();
    let n = 64usize;
    for i in 0..=n {
        let a = 2.0 * std::f32::consts::PI * i as f32 / n as f32;
        let idx = m.vertex_count() as u32;
        push_vert(&mut m, [r * a.cos(), 0.0, r * a.sin()], [0.0, 1.0, 0.0], [i as f32 / n as f32, 0.0]);
        push_vert(&mut m, [(r - tube) * a.cos(), 0.0, (r - tube) * a.sin()], [0.0, 1.0, 0.0], [i as f32 / n as f32, 1.0]);
        if i < n {
            m.indices.extend_from_slice(&[idx, idx + 1, idx + 2, idx + 1, idx + 3, idx + 2]);
        }
    }
    m
}

fn plane(size: f32) -> SpatialMesh {
    let mut m = SpatialMesh::default();
    let h = size / 2.0;
    push_vert(&mut m, [-h, 0.0, -h], [0.0, 1.0, 0.0], [0.0, 0.0]);
    push_vert(&mut m, [h, 0.0, -h], [0.0, 1.0, 0.0], [1.0, 0.0]);
    push_vert(&mut m, [h, 0.0, h], [0.0, 1.0, 0.0], [1.0, 1.0]);
    push_vert(&mut m, [-h, 0.0, h], [0.0, 1.0, 0.0], [0.0, 1.0]);
    m.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    m
}

fn grid(size: f32, count_f: f32) -> SpatialMesh {
    let mut m = SpatialMesh::default();
    let count = (count_f as usize).clamp(2, 128);
    let half = size / 2.0;
    let step = size / count as f32;
    for i in 0..=count {
        let x = -half + step * i as f32;
        let z = -half + step * i as f32;
        let ux = i as f32 / count as f32;
        push_vert(&mut m, [x, 0.0, -half], [0.0, 1.0, 0.0], [ux, 0.0]);
        push_vert(&mut m, [x, 0.0, half], [0.0, 1.0, 0.0], [ux, 1.0]);
        push_vert(&mut m, [-half, 0.0, z], [0.0, 1.0, 0.0], [0.0, ux]);
        push_vert(&mut m, [half, 0.0, z], [0.0, 1.0, 0.0], [1.0, ux]);
    }
    // grid = garis, bukan segitiga; demi mesh triangulasi kita buat strip tipis
    for i in 0..count {
        let x = -half + step * i as f32;
        let z = -half + step * i as f32;
        let base = m.vertex_count() as u32;
        let e = 0.002;
        push_vert(&mut m, [x, 0.0, -half], [0.0, 1.0, 0.0], [0.0, 0.0]);
        push_vert(&mut m, [x + e, 0.0, -half], [0.0, 1.0, 0.0], [0.0, 0.0]);
        push_vert(&mut m, [x, 0.0, half], [0.0, 1.0, 0.0], [0.0, 0.0]);
        push_vert(&mut m, [x + e, 0.0, half], [0.0, 1.0, 0.0], [0.0, 0.0]);
        m.indices.extend_from_slice(&[base, base + 1, base + 3, base, base + 3, base + 2]);
        let base = m.vertex_count() as u32;
        push_vert(&mut m, [-half, 0.0, z], [0.0, 1.0, 0.0], [0.0, 0.0]);
        push_vert(&mut m, [-half, 0.0, z + e], [0.0, 1.0, 0.0], [0.0, 0.0]);
        push_vert(&mut m, [half, 0.0, z], [0.0, 1.0, 0.0], [0.0, 0.0]);
        push_vert(&mut m, [half, 0.0, z + e], [0.0, 1.0, 0.0], [0.0, 0.0]);
        m.indices.extend_from_slice(&[base, base + 1, base + 3, base, base + 3, base + 2]);
    }
    m
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. RASTERIZER UI → TEXTURE RGBA (spatial UI)
// ═══════════════════════════════════════════════════════════════════════════

/// Buffer tekstur RGBA (8-bit per channel). Siap upload ke WebGL2
/// (gl.texImage2D) untuk dipetakan ke permukaan objek 3D.
#[derive(Debug, Clone, PartialEq)]
pub struct Texture {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

impl Texture {
    pub fn new(width: usize, height: usize, bg: [u8; 4]) -> Self {
        let mut rgba = vec![0u8; width * height * 4];
        for px in rgba.chunks_exact_mut(4) {
            px.copy_from_slice(&bg);
        }
        Self { width, height, rgba }
    }

    pub fn set_pixel(&mut self, x: usize, y: usize, color: [u8; 4]) {
        if x >= self.width || y >= self.height {
            return;
        }
        let idx = (y * self.width + x) * 4;
        self.rgba[idx..idx + 4].copy_from_slice(&color);
    }

    pub fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: [u8; 4]) {
        for yy in y..y.saturating_add(h) {
            for xx in x..x.saturating_add(w) {
                self.set_pixel(xx, yy, color);
            }
        }
    }

    pub fn stroke_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: [u8; 4], th: usize) {
        self.fill_rect(x, y, w, th, color);
        self.fill_rect(x, y.saturating_add(h.saturating_sub(th)), w, th, color);
        self.fill_rect(x, y, th, h, color);
        self.fill_rect(x.saturating_add(w.saturating_sub(th)), y, th, h, color);
    }

    /// Gambar teks memakai bitmap font 5x7 bawaan (ASCII printable).
    pub fn draw_text(&mut self, x: usize, y: usize, text: &str, color: [u8; 4]) {
        let mut cx = x;
        for ch in text.chars() {
            let glyph = glyph(ch);
            for (gy, row) in glyph.iter().enumerate() {
                for (gx, bit) in row.iter().enumerate() {
                    if *bit != 0 {
                        self.set_pixel(cx + gx, y + gy, color);
                    }
                }
            }
            cx += 6; // 5 px + 1 spasi
        }
    }
}

/// Warna default tema ADILang (dark + neon).
pub const COLOR_BG: [u8; 4] = [6, 10, 16, 255];
pub const COLOR_PANEL: [u8; 4] = [16, 34, 50, 255];
pub const COLOR_TEXT: [u8; 4] = [200, 246, 255, 255];
pub const COLOR_ACCENT: [u8; 4] = [80, 200, 255, 255];
pub const COLOR_BTN: [u8; 4] = [20, 60, 90, 255];
pub const COLOR_BTN_EDGE: [u8; 4] = [90, 200, 255, 200];

/// Rasterisasi ui_layout ke tekstur RGBA ukuran tertentu (spatial UI).
/// Text di-encode memakai bitmap font → terlihat jelas di surface objek 3D.
pub fn render_layout_to_texture(layout: &UILayoutDef, width: usize, height: usize) -> Texture {
    let mut tex = Texture::new(width, height, COLOR_BG);
    let pad = 8usize;
    // Header layout name
    tex.draw_text(pad, pad, &layout.name, COLOR_ACCENT);
    let mut cursor_y = pad + 12;
    let mut used = width.saturating_sub(pad * 2);
    raster_ui(&mut tex, &layout.root, pad, cursor_y, &mut used, &mut cursor_y);
    tex
}

/// Rasterisasi komponen (stacked vertically) ke texture.
fn raster_ui(
    tex: &mut Texture,
    comp: &UIComponent,
    x: usize,
    y: usize,
    used_w: &mut usize,
    cursor_y: &mut usize,
) {
    let panel_w = *used_w;
    let line_h = 14usize;
    match comp {
        UIComponent::Container { flex, children } => {
            tex.stroke_rect(x, y, panel_w, children.len().saturating_mul(line_h).saturating_add(8), COLOR_PANEL, 1);
            let mut inner_y = y + 4;
            match flex {
                Some(FlexDirection::Row) => {
                    // horizontal: setiap child di kolom sendiri
                    let n = children.len().max(1);
                    let col_w = panel_w.saturating_sub(8) / n;
                    for (i, child) in children.iter().enumerate() {
                        let cx = x + 4 + i * col_w;
                        raster_ui(tex, child, cx, inner_y, &mut (col_w.saturating_sub(8)), &mut inner_y);
                    }
                }
                _ => {
                    for child in children {
                        raster_ui(tex, child, x + 4, inner_y, used_w, &mut inner_y);
                        inner_y += line_h;
                    }
                }
            }
            *cursor_y = inner_y.saturating_add(4);
        }
        UIComponent::Text { content } => {
            tex.draw_text(x, y, content, COLOR_TEXT);
            *cursor_y = y.saturating_add(line_h);
        }
        UIComponent::Button { label, onClick } => {
            let w = label.chars().count() * 6 + 12;
            tex.fill_rect(x, y, w, 12, COLOR_BTN);
            tex.stroke_rect(x, y, w, 12, COLOR_BTN_EDGE, 1);
            tex.draw_text(x + 6, y + 3, label, COLOR_TEXT);
            if let Some(action) = onClick {
                tex.draw_text(x + 6, y.saturating_sub(6), &format!("[{}]", action), COLOR_ACCENT);
            }
            *cursor_y = y.saturating_add(line_h);
        }
        UIComponent::Input { name, placeholder, .. } => {
            let ph = placeholder.as_deref().unwrap_or(name);
            let w = (ph.chars().count() * 6).max(20) + 12;
            tex.stroke_rect(x, y, w, 12, COLOR_ACCENT, 1);
            tex.draw_text(x + 4, y + 3, ph, COLOR_TEXT);
            *cursor_y = y.saturating_add(line_h);
        }
        UIComponent::Card { title, children } => {
            let mut inner_y = y;
            if let Some(t) = title {
                tex.fill_rect(x, y, panel_w, line_h, COLOR_PANEL);
                tex.draw_text(x + 4, y + 3, t, COLOR_ACCENT);
                inner_y += line_h;
            }
            for child in children {
                raster_ui(tex, child, x + 4, inner_y, used_w, &mut inner_y);
                inner_y += line_h;
            }
            tex.stroke_rect(x, y, panel_w, inner_y.saturating_sub(y).saturating_add(4), COLOR_PANEL, 1);
            *cursor_y = inner_y.saturating_add(4);
        }
        UIComponent::Modal { title, children } => {
            let mut inner_y = y;
            if let Some(t) = title {
                tex.fill_rect(x, y, panel_w, line_h, COLOR_BTN);
                tex.draw_text(x + 4, y + 3, t, COLOR_TEXT);
                inner_y += line_h;
            }
            for child in children {
                raster_ui(tex, child, x + 4, inner_y, used_w, &mut inner_y);
                inner_y += line_h;
            }
            tex.stroke_rect(x, y, panel_w, inner_y.saturating_sub(y).saturating_add(4), COLOR_ACCENT, 1);
            *cursor_y = inner_y.saturating_add(4);
        }
        UIComponent::Navbar { title } => {
            tex.fill_rect(x, y, panel_w, line_h, COLOR_PANEL);
            if let Some(t) = title {
                tex.draw_text(x + 4, y + 3, t, COLOR_ACCENT);
            }
            *cursor_y = y.saturating_add(line_h);
        }
        UIComponent::Footer { content } => {
            tex.draw_text(x, y, content, COLOR_TEXT);
            *cursor_y = y.saturating_add(line_h);
        }
    }
}

/// Bitmap font 5x7 untuk karakter printable ASCII.
/// Representasi: 7 baris × 5 bit (MSB = kiri).
fn glyph(c: char) -> [[u8; 5]; 7] {
    // Default: kotak kosong
    let empty = [[0u8; 5]; 7];
    match c {
        'A' => [[0,1,1,1,0],[1,0,0,0,1],[1,0,0,0,1],[1,1,1,1,1],[1,0,0,0,1],[1,0,0,0,1],[1,0,0,0,1]],
        'B' => [[1,1,1,1,0],[1,0,0,0,1],[1,0,0,0,1],[1,1,1,1,0],[1,0,0,0,1],[1,0,0,0,1],[1,1,1,1,0]],
        'C' => [[0,1,1,1,1],[1,0,0,0,0],[1,0,0,0,0],[1,0,0,0,0],[1,0,0,0,0],[1,0,0,0,0],[0,1,1,1,1]],
        'D' => [[1,1,1,1,0],[1,0,0,0,1],[1,0,0,0,1],[1,0,0,0,1],[1,0,0,0,1],[1,0,0,0,1],[1,1,1,1,0]],
        'E' => [[1,1,1,1,1],[1,0,0,0,0],[1,0,0,0,0],[1,1,1,1,0],[1,0,0,0,0],[1,0,0,0,0],[1,1,1,1,1]],
        'F' => [[1,1,1,1,1],[1,0,0,0,0],[1,0,0,0,0],[1,1,1,1,0],[1,0,0,0,0],[1,0,0,0,0],[1,0,0,0,0]],
        'G' => [[0,1,1,1,1],[1,0,0,0,0],[1,0,0,0,0],[1,0,0,1,1],[1,0,0,0,1],[1,0,0,0,1],[0,1,1,1,0]],
        'H' => [[1,0,0,0,1],[1,0,0,0,1],[1,0,0,0,1],[1,1,1,1,1],[1,0,0,0,1],[1,0,0,0,1],[1,0,0,0,1]],
        'I' => [[1,1,1,1,1],[0,0,1,0,0],[0,0,1,0,0],[0,0,1,0,0],[0,0,1,0,0],[0,0,1,0,0],[1,1,1,1,1]],
        'J' => [[0,0,1,1,1],[0,0,0,1,0],[0,0,0,1,0],[0,0,0,1,0],[0,0,0,1,0],[1,0,0,1,0],[0,1,1,0,0]],
        'K' => [[1,0,0,0,1],[1,0,0,1,0],[1,0,1,0,0],[1,1,0,0,0],[1,0,1,0,0],[1,0,0,1,0],[1,0,0,0,1]],
        'L' => [[1,0,0,0,0],[1,0,0,0,0],[1,0,0,0,0],[1,0,0,0,0],[1,0,0,0,0],[1,0,0,0,0],[1,1,1,1,1]],
        'M' => [[1,0,0,0,1],[1,1,0,1,1],[1,0,1,0,1],[1,0,0,0,1],[1,0,0,0,1],[1,0,0,0,1],[1,0,0,0,1]],
        'N' => [[1,0,0,0,1],[1,1,0,0,1],[1,0,1,0,1],[1,0,0,1,1],[1,0,0,0,1],[1,0,0,0,1],[1,0,0,0,1]],
        'O' => [[0,1,1,1,0],[1,0,0,0,1],[1,0,0,0,1],[1,0,0,0,1],[1,0,0,0,1],[1,0,0,0,1],[0,1,1,1,0]],
        'P' => [[1,1,1,1,0],[1,0,0,0,1],[1,0,0,0,1],[1,1,1,1,0],[1,0,0,0,0],[1,0,0,0,0],[1,0,0,0,0]],
        'Q' => [[0,1,1,1,0],[1,0,0,0,1],[1,0,0,0,1],[1,0,0,0,1],[1,0,1,0,1],[1,0,0,1,1],[0,1,1,1,1]],
        'R' => [[1,1,1,1,0],[1,0,0,0,1],[1,0,0,0,1],[1,1,1,1,0],[1,0,1,0,0],[1,0,0,1,0],[1,0,0,0,1]],
        'S' => [[0,1,1,1,1],[1,0,0,0,0],[1,0,0,0,0],[0,1,1,1,0],[0,0,0,0,1],[0,0,0,0,1],[1,1,1,1,0]],
        'T' => [[1,1,1,1,1],[0,0,1,0,0],[0,0,1,0,0],[0,0,1,0,0],[0,0,1,0,0],[0,0,1,0,0],[0,0,1,0,0]],
        'U' => [[1,0,0,0,1],[1,0,0,0,1],[1,0,0,0,1],[1,0,0,0,1],[1,0,0,0,1],[1,0,0,0,1],[0,1,1,1,0]],
        'V' => [[1,0,0,0,1],[1,0,0,0,1],[1,0,0,0,1],[1,0,0,0,1],[1,0,0,0,1],[0,1,0,1,0],[0,0,1,0,0]],
        'W' => [[1,0,0,0,1],[1,0,0,0,1],[1,0,0,0,1],[1,0,0,0,1],[1,0,1,0,1],[1,0,1,0,1],[0,1,1,1,0]],
        'X' => [[1,0,0,0,1],[1,0,0,0,1],[0,1,0,1,0],[0,0,1,0,0],[0,1,0,1,0],[1,0,0,0,1],[1,0,0,0,1]],
        'Y' => [[1,0,0,0,1],[1,0,0,0,1],[0,1,0,1,0],[0,0,1,0,0],[0,0,1,0,0],[0,0,1,0,0],[0,0,1,0,0]],
        'Z' => [[1,1,1,1,1],[0,0,0,0,1],[0,0,0,1,0],[0,0,1,0,0],[0,1,0,0,0],[1,0,0,0,0],[1,1,1,1,1]],
        '0' => [[0,1,1,1,0],[1,0,0,0,1],[1,0,0,1,1],[1,0,1,0,1],[1,1,0,0,1],[1,0,0,0,1],[0,1,1,1,0]],
        '1' => [[0,0,1,0,0],[0,1,1,0,0],[0,0,1,0,0],[0,0,1,0,0],[0,0,1,0,0],[0,0,1,0,0],[0,1,1,1,0]],
        '2' => [[0,1,1,1,0],[1,0,0,0,1],[0,0,0,0,1],[0,0,0,1,0],[0,0,1,0,0],[0,1,0,0,0],[1,1,1,1,1]],
        '3' => [[1,1,1,1,1],[0,0,0,0,1],[0,0,0,1,0],[0,0,1,1,0],[0,0,0,0,1],[1,0,0,0,1],[0,1,1,1,0]],
        '4' => [[0,0,0,1,0],[0,0,1,1,0],[0,1,0,1,0],[1,0,0,1,0],[1,1,1,1,1],[0,0,0,1,0],[0,0,0,1,0]],
        '5' => [[1,1,1,1,1],[1,0,0,0,0],[1,1,1,1,0],[0,0,0,0,1],[0,0,0,0,1],[1,0,0,0,1],[0,1,1,1,0]],
        '6' => [[0,1,1,1,1],[1,0,0,0,0],[1,0,0,0,0],[1,1,1,1,0],[1,0,0,0,1],[1,0,0,0,1],[0,1,1,1,0]],
        '7' => [[1,1,1,1,1],[0,0,0,0,1],[0,0,0,1,0],[0,0,1,0,0],[0,1,0,0,0],[0,1,0,0,0],[0,1,0,0,0]],
        '8' => [[0,1,1,1,0],[1,0,0,0,1],[1,0,0,0,1],[0,1,1,1,0],[1,0,0,0,1],[1,0,0,0,1],[0,1,1,1,0]],
        '9' => [[0,1,1,1,0],[1,0,0,0,1],[1,0,0,0,1],[0,1,1,1,1],[0,0,0,0,1],[0,0,0,0,1],[1,1,1,1,0]],
        ' ' => empty,
        '-' => [[0,0,0,0,0],[0,0,0,0,0],[0,0,0,0,0],[1,1,1,1,1],[0,0,0,0,0],[0,0,0,0,0],[0,0,0,0,0]],
        '_' => [[0,0,0,0,0],[0,0,0,0,0],[0,0,0,0,0],[0,0,0,0,0],[0,0,0,0,0],[0,0,0,0,0],[1,1,1,1,1]],
        '.' => [[0,0,0,0,0],[0,0,0,0,0],[0,0,0,0,0],[0,0,0,0,0],[0,0,0,0,0],[0,1,1,0,0],[0,1,1,0,0]],
        ':' => [[0,0,0,0,0],[0,1,1,0,0],[0,1,1,0,0],[0,0,0,0,0],[0,1,1,0,0],[0,1,1,0,0],[0,0,0,0,0]],
        '!' => [[0,0,1,0,0],[0,0,1,0,0],[0,0,1,0,0],[0,0,1,0,0],[0,0,1,0,0],[0,0,0,0,0],[0,0,1,0,0]],
        '?' => [[0,1,1,1,0],[1,0,0,0,1],[0,0,0,0,1],[0,0,0,1,0],[0,0,1,0,0],[0,0,0,0,0],[0,0,1,0,0]],
        '/' => [[0,0,0,0,1],[0,0,0,0,1],[0,0,0,1,0],[0,0,1,0,0],[0,1,0,0,0],[1,0,0,0,0],[1,0,0,0,0]],
        '+' => [[0,0,0,0,0],[0,0,1,0,0],[0,0,1,0,0],[1,1,1,1,1],[0,0,1,0,0],[0,0,1,0,0],[0,0,0,0,0]],
        '=' => [[0,0,0,0,0],[0,0,0,0,0],[1,1,1,1,1],[0,0,0,0,0],[1,1,1,1,1],[0,0,0,0,0],[0,0,0,0,0]],
        '(' => [[0,0,0,1,0],[0,0,1,0,0],[0,1,0,0,0],[0,1,0,0,0],[0,1,0,0,0],[0,0,1,0,0],[0,0,0,1,0]],
        ')' => [[0,1,0,0,0],[0,0,1,0,0],[0,0,0,1,0],[0,0,0,1,0],[0,0,0,1,0],[0,0,1,0,0],[0,1,0,0,0]],
        '[' => [[0,0,1,1,1],[0,0,1,0,0],[0,0,1,0,0],[0,0,1,0,0],[0,0,1,0,0],[0,0,1,0,0],[0,0,1,1,1]],
        ']' => [[1,1,1,0,0],[0,0,1,0,0],[0,0,1,0,0],[0,0,1,0,0],[0,0,1,0,0],[0,0,1,0,0],[1,1,1,0,0]],
        _ => empty,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semua_bentuk_dihasilkan_tanpa_asset() {
        for (name, mesh) in generate_all() {
            assert!(mesh.vertex_count() > 0, "{name} harus punya vertex");
            assert!(mesh.triangle_count() > 0, "{name} harus punya triangle");
            assert_eq!(mesh.interleaved().len(), mesh.vertex_count() * 8);
        }
    }

    #[test]
    fn sphere_berisi_normal_unit() {
        let m = generate_shape(ShapeKind::Sphere, &ShapeParams { radius: 2.0, ..Default::default() });
        for i in 0..m.vertex_count() {
            let n = [m.normals[i * 3], m.normals[i * 3 + 1], m.normals[i * 3 + 2]];
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            assert!((len - 1.0).abs() < 1e-3, "normal harus unit, dapat {len}");
        }
    }

    #[test]
    fn shape_kind_roundtrip_string() {
        for k in ShapeKind::from_str_all() {
            assert_eq!(ShapeKind::from_str(k.as_str()), Some(k));
        }
        assert_eq!(ShapeKind::from_str("tidak_ada"), None);
    }

    #[test]
    fn texture_fill_dan_stroke() {
        let mut t = Texture::new(16, 16, COLOR_BG);
        t.fill_rect(2, 2, 4, 4, [255, 0, 0, 255]);
        assert_eq!(t.rgba[((2 * 16 + 2) * 4)..((2 * 16 + 2) * 4 + 4)], [255, 0, 0, 255]);
        assert_eq!(t.rgba[(0 * 4)..(0 * 4 + 4)], COLOR_BG);
        t.stroke_rect(8, 8, 6, 6, COLOR_ACCENT, 1);
        // pixel di border terisi
        assert_eq!(t.rgba[((8 * 16 + 8) * 4)..((8 * 16 + 8) * 4 + 4)], COLOR_ACCENT);
    }

    #[test]
    fn draw_text_menulis_pixel() {
        let mut t = Texture::new(64, 16, COLOR_BG);
        t.draw_text(1, 1, "A", COLOR_TEXT);
        // 'A' punya bit nyala di (1,0) pada baris pertama → pixel (2,1) terisi
        let i = (1 * 64 + 2) * 4;
        assert_eq!(t.rgba[i..i + 4], COLOR_TEXT);
        // Bit kosong (0,0) tetap bg → pixel (1,1) tidak terisi
        let j = (1 * 64 + 1) * 4;
        assert_eq!(t.rgba[j..j + 4], COLOR_BG);
    }

    #[test]
    fn ui_layout_dirender_ke_texture() {
        use crate::ast::{FlexDirection, UIComponent, UILayoutDef};
        let layout = UILayoutDef {
            name: "HUD".to_string(),
            root: UIComponent::Container {
                flex: Some(FlexDirection::Column),
                children: vec![
                    UIComponent::Text { content: "Status".to_string() },
                    UIComponent::Button { label: "Send".to_string(), onClick: Some("send".to_string()) },
                    UIComponent::Input { name: "usr".to_string(), placeholder: Some("Name".to_string()), bind: None, validate: None },
                ],
            },
        };
        let tex = render_layout_to_texture(&layout, 128, 128);
        assert_eq!(tex.width, 128);
        assert_eq!(tex.height, 128);
        // Pastikan ada pixel non-bg (panel/teks ditulis)
        let has_ink = tex.rgba.chunks_exact(4).any(|px| px != COLOR_BG);
        assert!(has_ink, "texture harus memuat konten");
    }

    #[test]
    fn grid_dan_plane_jumlah_triangle() {
        let g = generate_shape(ShapeKind::Grid, &ShapeParams { size: 10.0, count: 4.0, ..Default::default() });
        assert!(g.triangle_count() > 0);
        let p = generate_shape(ShapeKind::Plane, &ShapeParams::default());
        assert_eq!(p.triangle_count(), 2);
    }

    #[test]
    fn icosa_inner_membuat_lebih_banyak_vertex() {
        let tanpa = generate_shape(ShapeKind::Icosa, &ShapeParams { inner: 1.0, ..Default::default() });
        let dengan = generate_shape(ShapeKind::Icosa, &ShapeParams { inner: 0.5, ..Default::default() });
        assert!(dengan.vertex_count() > tanpa.vertex_count());
    }
}
