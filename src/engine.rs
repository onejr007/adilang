// ADILang engine — renderer WebGL2 via glow.
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).

use glow::HasContext;
use std::collections::HashMap;
use wasm_bindgen::JsCast;

use crate::eval::Interpreter;
use crate::math3d::{self, Mat4, Vec3};
use crate::scene::{EntityState, LightKind, MaterialKind, MeshKind, MeshParams};

const VERTEX_SRC: &str = r#"#version 300 es
layout(location=0) in vec3 a_pos;
layout(location=1) in vec3 a_normal;
uniform mat4 u_mvp;
uniform mat4 u_model;
out vec3 v_normal;
out vec3 v_world;
void main(){
    v_world = (u_model * vec4(a_pos, 1.0)).xyz;
    v_normal = mat3(u_model) * a_normal;
    gl_Position = u_mvp * vec4(a_pos, 1.0);
}
"#;

const FRAGMENT_SRC: &str = r#"#version 300 es
precision highp float;
in vec3 v_normal;
in vec3 v_world;
uniform vec3 u_color;
uniform float u_alpha;
uniform vec3 u_light_pos;
uniform vec3 u_light_color;
uniform float u_ambient;
out vec4 outColor;
void main(){
    vec3 n = normalize(v_normal);
    vec3 l = normalize(u_light_pos - v_world);
    float diff = max(dot(n, l), 0.0);
    vec3 col = u_color * (u_ambient + (1.0 - u_ambient) * diff * u_light_color);
    outColor = vec4(col, u_alpha);
}
"#;

const VERTEX_LINE_SRC: &str = r#"#version 300 es
layout(location=0) in vec3 a_pos;
uniform mat4 u_mvp;
void main(){
    gl_Position = u_mvp * vec4(a_pos, 1.0);
}
"#;

const FRAGMENT_LINE_SRC: &str = r#"#version 300 es
precision highp float;
uniform vec3 u_color;
uniform float u_alpha;
out vec4 outColor;
void main(){
    outColor = vec4(u_color, u_alpha);
}
"#;

struct SolidProgram {
    program: glow::Program,
    loc_mvp: Option<glow::UniformLocation>,
    loc_model: Option<glow::UniformLocation>,
    loc_color: Option<glow::UniformLocation>,
    loc_alpha: Option<glow::UniformLocation>,
    loc_light_pos: Option<glow::UniformLocation>,
    loc_light_color: Option<glow::UniformLocation>,
    loc_ambient: Option<glow::UniformLocation>,
}

struct LineProgram {
    program: glow::Program,
    loc_mvp: Option<glow::UniformLocation>,
    loc_color: Option<glow::UniformLocation>,
    loc_alpha: Option<glow::UniformLocation>,
}

struct GpuMesh {
    vao: glow::VertexArray,
    tri_ebo: glow::Buffer,
    line_ebo: glow::Buffer,
    tri_count: i32,
    line_count: i32,
}

pub struct Engine {
    gl: glow::Context,
    solid: SolidProgram,
    line: LineProgram,
    pub interp: Interpreter,
    canvas: web_sys::HtmlCanvasElement,
    mesh_map: HashMap<String, GpuMesh>,
    view: Mat4,
    proj: Mat4,
}

impl Engine {
    pub fn new(canvas: web_sys::HtmlCanvasElement) -> Result<Self, String> {
        let ctx = canvas
            .get_context("webgl2")
            .map_err(|e| format!("Gagal dapat context WebGL2: {e:?}"))?
            .ok_or("Browser tidak mendukung WebGL2")?
            .dyn_into::<web_sys::WebGl2RenderingContext>()
            .map_err(|_| "Context bukan WebGL2")?;
        let gl = glow::Context::from_webgl2_context(ctx);

        unsafe {
            gl.enable(glow::DEPTH_TEST);
            gl.enable(glow::BLEND);
            gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            gl.clear_color(0.02, 0.03, 0.05, 1.0);
        }

        let solid = compile_solid(&gl)?;
        let line = compile_line(&gl)?;
        let interp = Interpreter::new("ADI World".into());

        Ok(Self {
            gl,
            solid,
            line,
            interp,
            canvas,
            mesh_map: HashMap::new(),
            view: math3d::identity(),
            proj: math3d::identity(),
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.canvas.set_width(width);
        self.canvas.set_height(height);
        unsafe {
            self.gl.viewport(0, 0, width as i32, height as i32);
        }
        let aspect = width as f32 / height as f32;
        let fov = self.interp.world.camera.fov as f32;
        self.proj = math3d::perspective(fov, aspect, 0.1, 200.0);
    }

    /// Bangun ulang GPU mesh untuk semua entity.
    pub fn build_meshes(&mut self) -> Result<(), String> {
        self.mesh_map.clear();
        let snapshot: Vec<EntityState> = self.interp.world.entities.clone();
        for e in &snapshot {
            let geom = generate_mesh(e.mesh, &e.mesh_params);
            self.upload_mesh(&e.id, &geom)?;
        }
        Ok(())
    }

    fn upload_mesh(&mut self, id: &str, geom: &Geometry) -> Result<(), String> {
        let gl = &self.gl;
        let mut verts = Vec::with_capacity(geom.verts.len() * 6);
        for (i, v) in geom.verts.iter().enumerate() {
            verts.extend_from_slice(&[v[0], v[1], v[2]]);
            let n = geom.normals.get(i).copied().unwrap_or([0.0, 1.0, 0.0]);
            verts.extend_from_slice(&[n[0], n[1], n[2]]);
        }
        unsafe {
            let vao = gl.create_vertex_array().map_err(|e| e.to_string())?;
            let vbo = gl.create_buffer().map_err(|e| e.to_string())?;
            gl.bind_vertex_array(Some(vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, as_byte_slice(&verts), glow::STATIC_DRAW);
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, 24, 0);
            gl.enable_vertex_attrib_array(1);
            gl.vertex_attrib_pointer_f32(1, 3, glow::FLOAT, false, 24, 12);

            let tri_ebo = gl.create_buffer().map_err(|e| e.to_string())?;
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(tri_ebo));
            gl.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, as_byte_slice(&geom.tris), glow::STATIC_DRAW);

            let line_ebo = gl.create_buffer().map_err(|e| e.to_string())?;
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(line_ebo));
            gl.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, as_byte_slice(&geom.lines), glow::STATIC_DRAW);

            gl.bind_vertex_array(None);
            self.mesh_map.insert(
                id.to_string(),
                GpuMesh {
                    vao,
                    tri_ebo,
                    line_ebo,
                    tri_count: (geom.tris.len() / 3) as i32,
                    line_count: (geom.lines.len() / 2) as i32,
                },
            );
        }
        Ok(())
    }

    pub fn render(&mut self) {
        let cam = self.interp.world.camera.clone();
        let eye: Vec3 = [cam.pos[0] as f32, cam.pos[1] as f32, cam.pos[2] as f32];
        let target: Vec3 = [cam.look[0] as f32, cam.look[1] as f32, cam.look[2] as f32];
        let up: Vec3 = [0.0, 1.0, 0.0];
        self.view = math3d::look_at(&eye, &target, &up);

        let mut light_pos = [5.0f32, 6.0, 4.0];
        let mut light_color = [1.0f32, 0.95, 0.9];
        let mut ambient = 0.35f32;
        for l in &self.interp.world.lights {
            match l.kind {
                LightKind::Point => {
                    light_pos = [l.pos[0] as f32, l.pos[1] as f32, l.pos[2] as f32];
                    light_color = [l.color[0] as f32, l.color[1] as f32, l.color[2] as f32];
                }
                LightKind::Ambient => {
                    ambient = (l.intensity as f32).clamp(0.0, 1.0);
                }
            }
        }

        let gl = &self.gl;
        unsafe {
            gl.clear_color(0.02, 0.03, 0.05, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
        }

        let entities: Vec<EntityState> = self.interp.world.entities.clone();
        for e in &entities {
            let Some(mesh) = self.mesh_map.get(&e.id) else { continue };
            let model = entity_model(e);
            let mvp = math3d::mul(&self.proj, &math3d::mul(&self.view, &model));
            match e.material {
                MaterialKind::Solid => {
                    unsafe {
                        self.gl.use_program(Some(self.solid.program));
                        self.gl.uniform_matrix_4_f32_slice(self.solid.loc_mvp.as_ref(), false, &mvp);
                        self.gl.uniform_matrix_4_f32_slice(self.solid.loc_model.as_ref(), false, &model);
                        self.gl.uniform_3_f32(self.solid.loc_color.as_ref(), e.color[0] as f32, e.color[1] as f32, e.color[2] as f32);
                        self.gl.uniform_1_f32(self.solid.loc_alpha.as_ref(), e.color[3] as f32);
                        self.gl.uniform_3_f32(self.solid.loc_light_pos.as_ref(), light_pos[0], light_pos[1], light_pos[2]);
                        self.gl.uniform_3_f32(self.solid.loc_light_color.as_ref(), light_color[0], light_color[1], light_color[2]);
                        self.gl.uniform_1_f32(self.solid.loc_ambient.as_ref(), ambient);
                        self.gl.bind_vertex_array(Some(mesh.vao));
                        self.gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(mesh.tri_ebo));
                        self.gl.draw_elements(glow::TRIANGLES, mesh.tri_count * 3, glow::UNSIGNED_INT, 0);
                    }
                }
                MaterialKind::Wire | MaterialKind::Glow => {
                    if e.material == MaterialKind::Glow {
                        unsafe {
                            self.gl.enable(glow::BLEND);
                            self.gl.blend_func(glow::SRC_ALPHA, glow::ONE);
                        }
                    }
                    unsafe {
                        self.gl.use_program(Some(self.line.program));
                        self.gl.uniform_matrix_4_f32_slice(self.line.loc_mvp.as_ref(), false, &mvp);
                        self.gl.uniform_3_f32(self.line.loc_color.as_ref(), e.color[0] as f32, e.color[1] as f32, e.color[2] as f32);
                        self.gl.uniform_1_f32(self.line.loc_alpha.as_ref(), e.color[3] as f32);
                        self.gl.bind_vertex_array(Some(mesh.vao));
                        self.gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(mesh.line_ebo));
                        self.gl.draw_elements(glow::LINES, mesh.line_count * 2, glow::UNSIGNED_INT, 0);
                    }
                    if e.material == MaterialKind::Glow {
                        unsafe {
                            self.gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
                        }
                    }
                }
            }
        }
    }

    /// Jalankan handler frame untuk semua entity + world.
    pub fn run_frame_handlers(&mut self) -> Result<(), String> {
        use crate::ast::{EventKind, Stmt};
        let world_handlers = self.interp.world.frame_handlers.clone();
        for h in &world_handlers {
            let body: Vec<Stmt> = h.body.clone();
            self.interp.run_handler(None, &body)?;
        }
        let ids: Vec<String> = self.interp.world.entities.iter().map(|e| e.id.clone()).collect();
        for id in ids {
            let handlers = self.interp.world.handlers_for(EventKind::Frame, &id);
            let body: Vec<Stmt> = handlers.iter().flat_map(|h| h.body.clone()).collect();
            if !body.is_empty() {
                self.interp.run_handler(Some(id), &body)?;
            }
        }
        Ok(())
    }

    pub fn fire_event(&mut self, event: crate::ast::EventKind) -> Result<(), String> {
        use crate::ast::Stmt;
        let ids: Vec<String> = self.interp.world.entities.iter().map(|e| e.id.clone()).collect();
        for id in ids {
            let handlers = self.interp.world.handlers_for(event.clone(), &id);
            let body: Vec<Stmt> = handlers.iter().flat_map(|h| h.body.clone()).collect();
            if !body.is_empty() {
                self.interp.run_handler(Some(id), &body)?;
            }
        }
        Ok(())
    }
}

fn entity_model(e: &EntityState) -> Mat4 {
    let t = math3d::translate(e.transform.pos[0] as f32, e.transform.pos[1] as f32, e.transform.pos[2] as f32);
    let rx = math3d::rot_x(e.transform.rot[0] as f32);
    let ry = math3d::rot_y(e.transform.rot[1] as f32);
    let rz = math3d::rot_z(e.transform.rot[2] as f32);
    let s = math3d::scale(e.transform.scale as f32);
    math3d::mul(&t, &math3d::mul(&math3d::mul(&rx, &math3d::mul(&ry, &rz)), &s))
}

fn compile_solid(gl: &glow::Context) -> Result<SolidProgram, String> {
    let program = link_program(gl, VERTEX_SRC, FRAGMENT_SRC)?;
    unsafe {
        Ok(SolidProgram {
            loc_mvp: gl.get_uniform_location(program, "u_mvp"),
            loc_model: gl.get_uniform_location(program, "u_model"),
            loc_color: gl.get_uniform_location(program, "u_color"),
            loc_alpha: gl.get_uniform_location(program, "u_alpha"),
            loc_light_pos: gl.get_uniform_location(program, "u_light_pos"),
            loc_light_color: gl.get_uniform_location(program, "u_light_color"),
            loc_ambient: gl.get_uniform_location(program, "u_ambient"),
            program,
        })
    }
}

fn compile_line(gl: &glow::Context) -> Result<LineProgram, String> {
    let program = link_program(gl, VERTEX_LINE_SRC, FRAGMENT_LINE_SRC)?;
    unsafe {
        Ok(LineProgram {
            loc_mvp: gl.get_uniform_location(program, "u_mvp"),
            loc_color: gl.get_uniform_location(program, "u_color"),
            loc_alpha: gl.get_uniform_location(program, "u_alpha"),
            program,
        })
    }
}

fn link_program(gl: &glow::Context, vs: &str, fs: &str) -> Result<glow::Program, String> {
    unsafe {
        let v = gl.create_shader(glow::VERTEX_SHADER).map_err(|e| e.to_string())?;
        gl.shader_source(v, vs);
        gl.compile_shader(v);
        if !gl.get_shader_compile_status(v) {
            return Err(format!("Vertex shader: {}", gl.get_shader_info_log(v)));
        }
        let f = gl.create_shader(glow::FRAGMENT_SHADER).map_err(|e| e.to_string())?;
        gl.shader_source(f, fs);
        gl.compile_shader(f);
        if !gl.get_shader_compile_status(f) {
            return Err(format!("Fragment shader: {}", gl.get_shader_info_log(f)));
        }
        let p = gl.create_program().map_err(|e| e.to_string())?;
        gl.attach_shader(p, v);
        gl.attach_shader(p, f);
        gl.link_program(p);
        if !gl.get_program_link_status(p) {
            return Err(format!("Link: {}", gl.get_program_info_log(p)));
        }
        gl.delete_shader(v);
        gl.delete_shader(f);
        Ok(p)
    }
}

// ─── Geometry generation ───

fn as_byte_slice<T: Copy>(data: &[T]) -> &[u8] {
    unsafe {
        std::slice::from_raw_parts(
            data.as_ptr() as *const u8,
            data.len() * std::mem::size_of::<T>(),
        )
    }
}

#[derive(Default)]
pub struct Geometry {
    pub verts: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub tris: Vec<u32>,
    pub lines: Vec<u32>,
}

pub fn generate_mesh(kind: MeshKind, p: &MeshParams) -> Geometry {
    match kind {
        MeshKind::Sphere => sphere(p.radius as f32, p.segments as usize),
        MeshKind::Box => box_mesh(),
        MeshKind::Torus => torus(p.radius as f32, p.tube as f32),
        MeshKind::Icosa => icosa(p.radius as f32, p.inner as f32),
        MeshKind::Ring => ring(p.radius as f32, p.tube as f32),
        MeshKind::Plane => plane(p.size as f32),
        MeshKind::Grid => grid(p.size as f32, p.count as usize),
    }
}

fn push_vert(out: &mut Geometry, v: Vec3, n: Vec3) {
    out.verts.push(v);
    out.normals.push(n);
}

fn sphere(radius: f32, segs: usize) -> Geometry {
    let mut g = Geometry::default();
    let segs = segs.max(2).min(64);
    let stride = segs + 1;
    for lat in 0..=segs {
        let theta = std::f32::consts::PI * lat as f32 / segs as f32;
        let sy = theta.cos();
        let sxy = theta.sin();
        for lon in 0..=segs {
            let phi = 2.0 * std::f32::consts::PI * lon as f32 / segs as f32;
            let n = [sxy * phi.cos(), sy, sxy * phi.sin()];
            push_vert(&mut g, [n[0] * radius, n[1] * radius, n[2] * radius], n);
        }
    }
    for lat in 0..segs {
        for lon in 0..segs {
            let a = lat * stride + lon;
            let b = a + 1;
            let c = a + stride;
            let d = c + 1;
            g.tris.extend_from_slice(&[a as u32, c as u32, b as u32, b as u32, c as u32, d as u32]);
        }
    }
    for lat in 0..=segs {
        for lon in 0..segs {
            let a = lat * stride + lon;
            g.lines.extend_from_slice(&[a as u32, (a + 1) as u32]);
        }
    }
    for lon in 0..=segs {
        for lat in 0..segs {
            let a = lat * stride + lon;
            g.lines.extend_from_slice(&[a as u32, (a + stride) as u32]);
        }
    }
    g
}

fn box_mesh() -> Geometry {
    let mut g = Geometry::default();
    let faces: [([f32; 3], [f32; 3]); 6] = [
        ([1.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
        ([-1.0, 0.0, 0.0], [-1.0, 0.0, 0.0]),
        ([0.0, 1.0, 0.0], [0.0, 1.0, 0.0]),
        ([0.0, -1.0, 0.0], [0.0, -1.0, 0.0]),
        ([0.0, 0.0, 1.0], [0.0, 0.0, 1.0]),
        ([0.0, 0.0, -1.0], [0.0, 0.0, -1.0]),
    ];
    for (axis, n) in faces {
        let (u1, v1) = perpendicular_axes(axis);
        let base = g.verts.len() as u32;
        for s in [0.0f32, 1.0] {
            for t in [0.0f32, 1.0] {
                let cu = u1[0] * (2.0 * s - 1.0) + u1[1] * (2.0 * t - 1.0);
                let cv = v1[0] * (2.0 * s - 1.0) + v1[1] * (2.0 * t - 1.0);
                let cuv = [cu, 0.0, cv];
                let pos = [
                    axis[0] * 0.5 + cuv[0],
                    axis[1] * 0.5 + cuv[2],
                    axis[2] * 0.5 + cuv[1],
                ];
                push_vert(&mut g, pos, n);
            }
        }
        g.tris.extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 1, base + 3]);
        g.lines.extend_from_slice(&[
            base, base + 1, base + 1, base + 3, base + 3, base + 2, base + 2, base,
        ]);
    }
    g
}

fn perpendicular_axes(axis: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    if axis[0].abs() > 0.5 {
        ([0.0, 0.0, 1.0], [0.0, 1.0, 0.0])
    } else if axis[1].abs() > 0.5 {
        ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0])
    } else {
        ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0])
    }
}

fn torus(r: f32, tube: f32) -> Geometry {
    let mut g = Geometry::default();
    let ms = 24;
    let ns = 12;
    let stride = ns + 1;
    for i in 0..=ms {
        let u = 2.0 * std::f32::consts::PI * i as f32 / ms as f32;
        for j in 0..=ns {
            let v = 2.0 * std::f32::consts::PI * j as f32 / ns as f32;
            let n = [v.cos() * u.cos(), v.sin(), v.cos() * u.sin()];
            let cx = r + tube * v.cos();
            let pos = [cx * u.cos(), tube * v.sin(), cx * u.sin()];
            push_vert(&mut g, pos, n);
        }
    }
    for i in 0..ms {
        for j in 0..ns {
            let a = i * stride + j;
            let b = a + 1;
            let c = a + stride;
            let d = c + 1;
            g.tris.extend_from_slice(&[a as u32, c as u32, b as u32, b as u32, c as u32, d as u32]);
        }
    }
    for i in 0..=ms {
        for j in 0..ns {
            let a = i * stride + j;
            g.lines.extend_from_slice(&[a as u32, (a + 1) as u32]);
        }
    }
    g
}

fn icosa(r: f32, inner: f32) -> Geometry {
    let t = (1.0 + 5.0f32.sqrt()) / 2.0;
    let base_verts: [Vec3; 12] = [
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
    // edges icosahedron (termasuk duplikat yang di-dedup)
    let edge_src: &[[usize; 2]] = &[
        [0, 11], [11, 5], [5, 0], [0, 5], [5, 1], [1, 0], [0, 1], [1, 7], [7, 0],
        [0, 7], [7, 10], [10, 0], [0, 10], [10, 11], [11, 0],
        [1, 5], [5, 9], [9, 1], [1, 9], [9, 8], [8, 1], [1, 8], [8, 7], [7, 1],
        [11, 4], [4, 2], [2, 11], [10, 2], [2, 6], [6, 10], [7, 6], [6, 2], [2, 7],
        [10, 6], [6, 3], [3, 2], [2, 3], [3, 4], [4, 9], [9, 3], [3, 8], [8, 6], [6, 3],
    ];
    let mut edge_set: Vec<[usize; 2]> = Vec::new();
    for e in edge_src {
        let (a, b) = (e[0].min(e[1]), e[0].max(e[1]));
        if !edge_set.contains(&[a, b]) {
            edge_set.push([a, b]);
        }
    }

    let norm = |v: Vec3| -> Vec3 {
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        [v[0] / l, v[1] / l, v[2] / l]
    };

    let mut g = Geometry::default();
    for p in base_verts {
        let n = norm(p);
        push_vert(&mut g, [n[0] * r, n[1] * r, n[2] * r], n);
    }
    for f in faces {
        g.tris.extend_from_slice(&[f[0] as u32, f[1] as u32, f[2] as u32]);
    }
    for e in &edge_set {
        g.lines.extend_from_slice(&[e[0] as u32, e[1] as u32]);
    }
    if inner > 0.05 && inner < 1.0 {
        let base = g.verts.len() as u32;
        for p in base_verts {
            let n = norm(p);
            push_vert(&mut g, [n[0] * r * inner, n[1] * r * inner, n[2] * r * inner], n);
        }
        for e in &edge_set {
            g.lines.extend_from_slice(&[base + e[0] as u32, base + e[1] as u32]);
        }
    }
    g
}

fn ring(r: f32, tube: f32) -> Geometry {
    let mut g = Geometry::default();
    let n = 64usize;
    for i in 0..=n {
        let a = 2.0 * std::f32::consts::PI * i as f32 / n as f32;
        let idx = g.verts.len() as u32;
        push_vert(&mut g, [r * a.cos(), 0.0, r * a.sin()], [0.0, 1.0, 0.0]);
        push_vert(&mut g, [(r - tube) * a.cos(), 0.0, (r - tube) * a.sin()], [0.0, 1.0, 0.0]);
        if i < n {
            g.tris.extend_from_slice(&[idx, idx + 1, idx + 2, idx + 1, idx + 3, idx + 2]);
        }
    }
    for i in 0..=n {
        g.lines.extend_from_slice(&[(i as u32) * 2, (((i + 1) % n) as u32) * 2]);
    }
    g
}

fn plane(size: f32) -> Geometry {
    let mut g = Geometry::default();
    let h = size / 2.0;
    let base = 0u32;
    push_vert(&mut g, [-h, 0.0, -h], [0.0, 1.0, 0.0]);
    push_vert(&mut g, [h, 0.0, -h], [0.0, 1.0, 0.0]);
    push_vert(&mut g, [h, 0.0, h], [0.0, 1.0, 0.0]);
    push_vert(&mut g, [-h, 0.0, h], [0.0, 1.0, 0.0]);
    g.tris.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    g.lines.extend_from_slice(&[base, base + 1, base + 1, base + 2, base + 2, base + 3, base + 3, base]);
    g
}

fn grid(size: f32, count: usize) -> Geometry {
    let mut g = Geometry::default();
    let count = count.max(2).min(128);
    let half = size / 2.0;
    let step = size / count as f32;
    for i in 0..=count {
        let x = -half + step * i as f32;
        let z = -half + step * i as f32;
        push_vert(&mut g, [x, 0.0, -half], [0.0, 1.0, 0.0]);
        push_vert(&mut g, [x, 0.0, half], [0.0, 1.0, 0.0]);
        push_vert(&mut g, [-half, 0.0, z], [0.0, 1.0, 0.0]);
        push_vert(&mut g, [half, 0.0, z], [0.0, 1.0, 0.0]);
    }
    for i in 0..=count {
        let base = i as u32 * 4;
        g.lines.extend_from_slice(&[base, base + 1, base + 2, base + 3]);
    }
    g
}
