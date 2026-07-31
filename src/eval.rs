// ADILang interpreter — tree-walking evaluator.
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).

use std::collections::HashMap;

use crate::ast::{BinOp, EventKind, Expr, FuncDef, Prop, Stmt, TopLevel};
use crate::scene::{EntityState, LightKind, LightState, MaterialKind, MeshKind, MeshParams, World};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Num(f64),
    Str(String),
    Bool(bool),
    Tuple(Vec<f64>),
    Null,
}

impl Value {
    fn as_num(&self) -> Result<f64, String> {
        match self {
            Value::Num(n) => Ok(*n),
            other => Err(format!("Bukan angka: {other:?}")),
        }
    }
    fn as_tuple(&self) -> Result<Vec<f64>, String> {
        match self {
            Value::Tuple(v) => Ok(v.clone()),
            Value::Num(n) => Ok(vec![*n]),
            other => Err(format!("Bukan tuple: {other:?}")),
        }
    }
}

pub struct Interpreter {
    pub world: World,
    globals: HashMap<String, Value>,
    /// Stack scope lokal (handler / block / frame fungsi).
    /// `let` di dalam scope bind ke scope terdalam; assignment menarget
    /// scope TERDEKAT yang memiliki nama (lokal dulu, lalu global).
    /// Lihat spec LANGUAGE.md §7 dan KB §6.
    scopes: Vec<HashMap<String, Value>>,
    functions: HashMap<String, FuncDef>,
    /// Nilai ekspresi terakhir yang dievaluasi (untuk implicit return fungsi).
    last_expr: Option<Value>,
    pub t: f64,
    pub mouse_x: f64,
    pub mouse_y: f64,
    current_entity: Option<String>,
}

impl Interpreter {
    pub fn new(name: String) -> Self {
        Self {
            world: World::new(name),
            globals: HashMap::new(),
            scopes: Vec::new(),
            functions: HashMap::new(),
            last_expr: None,
            t: 0.0,
            mouse_x: 0.0,
            mouse_y: 0.0,
            current_entity: None,
        }
    }

    /// Evaluasi seluruh program → bangun world + daftarkan fungsi.
    pub fn load(&mut self, program: crate::ast::Program) -> Result<(), String> {
        self.world = World::new(program.name.clone());
        self.globals.clear();
        self.scopes.clear();
        self.functions.clear();

        // Pass 1: kumpulkan fungsi & let global
        for item in &program.items {
            match item {
                TopLevel::Func(f) => {
                    self.functions.insert(f.name.clone(), f.clone());
                }
                TopLevel::Let { name, value } => {
                    let v = self.eval_expr(value)?;
                    self.globals.insert(name.clone(), v);
                }
                _ => {}
            }
        }

        // Pass 2: bangun camera / lights / entities
        for item in &program.items {
            match item {
                TopLevel::Handler(h) => match h.event {
                    EventKind::Frame => self.world.frame_handlers.push(h.clone()),
                    EventKind::Speak => self.world.speak_handlers.push(h.clone()),
                    EventKind::Silent => self.world.silent_handlers.push(h.clone()),
                    EventKind::Click => self.world.click_handlers.push(h.clone()),
                },
                TopLevel::Camera(c) => {
                    let mut cam = self.world.camera.clone();
                    cam.id = c.id.clone();
                    for p in &c.props {
                        self.apply_camera_prop(&mut cam, p)?;
                    }
                    self.world.camera = cam;
                }
                TopLevel::Light(l) => {
                    let mut light = LightState {
                        id: l.id.clone(),
                        kind: LightKind::Point,
                        pos: [0.0, 0.0, 0.0],
                        color: [1.0, 1.0, 1.0],
                        intensity: 1.0,
                    };
                    for p in &l.props {
                        self.apply_light_prop(&mut light, p)?;
                    }
                    // replace bila id sama
                    if let Some(existing) = self.world.lights.iter_mut().find(|x| x.id == light.id) {
                        *existing = light;
                    } else {
                        self.world.lights.push(light);
                    }
                }
                TopLevel::Entity(e) => {
                    let mut entity = EntityState {
                        id: e.id.clone(),
                        transform: Default::default(),
                        color: [1.0, 1.0, 1.0, 1.0],
                        material: MaterialKind::Wire,
                        mesh: MeshKind::Sphere,
                        mesh_params: MeshParams::default(),
                        handlers: e.handlers.clone(),
                    };
                    for p in &e.props {
                        self.apply_entity_prop(&mut entity, p)?;
                    }
                    self.world.entities.push(entity);
                }
                _ => {}
            }
        }
        Ok(())
    }

    // ── Property applis ──
    fn apply_camera_prop(&mut self, cam: &mut crate::scene::CameraState, p: &Prop) -> Result<(), String> {
        match p.name.as_str() {
            "pos" => cam.pos = self.eval_vec3(&p.value)?,
            "look" => cam.look = self.eval_vec3(&p.value)?,
            "fov" => cam.fov = self.eval_expr(&p.value)?.as_num()?,
            _ => {}
        }
        Ok(())
    }

    fn apply_light_prop(&mut self, l: &mut LightState, p: &Prop) -> Result<(), String> {
        match p.name.as_str() {
            "type" => {
                if let Expr::Ident(t) = &p.value {
                    l.kind = if t == "ambient" { LightKind::Ambient } else { LightKind::Point };
                }
            }
            "pos" => l.pos = self.eval_vec3(&p.value)?,
            "color" => l.color = self.eval_vec3(&p.value)?,
            "intensity" => l.intensity = self.eval_expr(&p.value)?.as_num()?,
            _ => {}
        }
        Ok(())
    }

    fn apply_entity_prop(&mut self, e: &mut EntityState, p: &Prop) -> Result<(), String> {
        match p.name.as_str() {
            "pos" => e.transform.pos = self.eval_vec3(&p.value)?,
            "rot" => e.transform.rot = self.eval_vec3(&p.value)?,
            "scale" => {
                let n = self.eval_expr(&p.value)?.as_num()?;
                e.transform.scale = [n, n, n]
            }
            "mesh" => self.apply_mesh(e, &p.value)?,
            "material" => self.apply_material(e, &p.value)?,
            _ => {}
        }
        Ok(())
    }

    fn apply_mesh(&mut self, e: &mut EntityState, v: &Expr) -> Result<(), String> {
        let (name, args, props) = match v {
            Expr::Call { name, args, props } => (name.clone(), args.clone(), props.clone()),
            other => return Err(format!("mesh harus berupa builder, dapat {other:?}")),
        };
        // SUMBER TUNGGAL KEBENARAN kosakata mesh = registry.rs (tabel
        // MESH_BUILDERS). Tidak ada match literal di sini — tambah builder baru
        // cukup di registry.rs, evaluator otomatis ikut (P6, anti-drift).
        let kind = match crate::registry::mesh_kind(&name) {
            Some(k) => k,
            None => return Err(format!("Mesh builder tidak dikenal '{name}'")),
        };
        // Default per builder (spec §5.2 / KB §4.3) — sisanya dari MeshParams::default().
        let mut mp = match kind {
            MeshKind::Sphere => MeshParams { radius: 1.0, segments: 3.0, ..MeshParams::default() },
            MeshKind::Box => MeshParams { size: 1.0, ..MeshParams::default() },
            MeshKind::Torus => MeshParams { radius: 1.0, tube: 0.1, ..MeshParams::default() },
            MeshKind::Icosa => MeshParams { radius: 1.0, inner: 1.0, ..MeshParams::default() },
            MeshKind::Ring => MeshParams { radius: 1.0, tube: 0.02, ..MeshParams::default() },
            MeshKind::Plane => MeshParams { size: 10.0, ..MeshParams::default() },
            MeshKind::Grid => MeshParams { size: 20.0, count: 20.0, ..MeshParams::default() },
        };
        if let Some(props) = props {
            for p in props {
                match p.name.as_str() {
                    "radius" => mp.radius = self.eval_expr(&p.value)?.as_num()?,
                    "tube" => mp.tube = self.eval_expr(&p.value)?.as_num()?,
                    "inner" => mp.inner = self.eval_expr(&p.value)?.as_num()?,
                    "segments" => mp.segments = self.eval_expr(&p.value)?.as_num()?,
                    "size" => mp.size = self.eval_expr(&p.value)?.as_num()?,
                    "count" => mp.count = self.eval_expr(&p.value)?.as_num()?,
                    _ => {}
                }
            }
        }
        // Argumen positional per builder (KB §4.3):
        // box/plane/grid → size; sphere → radius(,segments);
        // torus/ring → radius, tube; icosa → radius, inner; grid → size, count.
        if let Some(a0) = args.first() {
            let v0 = self.eval_expr(a0)?.as_num()?;
            match kind {
                MeshKind::Box | MeshKind::Plane | MeshKind::Grid => mp.size = v0,
                _ => mp.radius = v0,
            }
        }
        if let Some(a1) = args.get(1) {
            let v1 = self.eval_expr(a1)?.as_num()?;
            match kind {
                MeshKind::Icosa => mp.inner = v1,
                MeshKind::Grid => mp.count = v1,
                MeshKind::Sphere => mp.segments = v1,
                _ => mp.tube = v1,
            }
        }
        e.mesh = kind;
        e.mesh_params = mp;
        Ok(())
    }

    fn apply_material(&mut self, e: &mut EntityState, v: &Expr) -> Result<(), String> {
        let (name, args) = match v {
            Expr::Call { name, args, .. } => (name.clone(), args.clone()),
            other => return Err(format!("material harus berupa builder, dapat {other:?}")),
        };
        // SUMBER TUNGGAL KEBENARAN kosakata material = registry.rs (tabel
        // MATERIAL_BUILDERS). Tidak ada match literal di sini — tambah builder
        // baru cukup di registry.rs, evaluator otomatis ikut (P6, anti-drift).
        let kind = match crate::registry::material_kind(&name) {
            Some(k) => k,
            None => return Err(format!("Material builder tidak dikenal '{name}'")),
        };
        e.material = kind;
        if let Some(c) = args.first() {
            let col = self.eval_vec3(c)?;
            e.color = [col[0], col[1], col[2], 1.0];
        }
        if let Some(a) = args.get(1) {
            e.color[3] = self.eval_expr(a)?.as_num()?;
        }
        Ok(())
    }

    // ── Evaluasi ekspresi ──
    pub fn eval_expr(&mut self, e: &Expr) -> Result<Value, String> {
        match e {
            Expr::Num(n) => Ok(Value::Num(*n)),
            Expr::Str(s) => Ok(Value::Str(s.clone())),
            Expr::Bool(b) => Ok(Value::Bool(*b)),
            Expr::Tuple(items) => {
                let mut out = Vec::new();
                for it in items {
                    out.push(self.eval_expr(it)?.as_num()?);
                }
                Ok(Value::Tuple(out))
            }
            Expr::Ident(name) => {
                match name.as_str() {
                    "t" => Ok(Value::Num(self.t)),
                    "mouseX" => Ok(Value::Num(self.mouse_x)),
                    "mouseY" => Ok(Value::Num(self.mouse_y)),
                    "PI" => Ok(Value::Num(std::f64::consts::PI)),
                    _ => {
                        // Builtins tetap prioritas (tidak bisa di-shadow oleh local
                        // `let t = ...` — perilaku sengaja dipertahankan).
                        // Lalu scope lokal terdalam → global.
                        for scope in self.scopes.iter().rev() {
                            if let Some(v) = scope.get(name) {
                                return Ok(v.clone());
                            }
                        }
                        self.globals
                            .get(name)
                            .cloned()
                            .ok_or_else(|| format!("Variabel tidak dikenal '{name}'"))
                    }
                }
            }
            Expr::UnaryMinus(inner) => Ok(Value::Num(-self.eval_expr(inner)?.as_num()?)),
            Expr::Binary { op, lhs, rhs } => {
                let a = self.eval_expr(lhs)?;
                let b = self.eval_expr(rhs)?;
                match op {
                    BinOp::Add => Ok(Value::Num(a.as_num()? + b.as_num()?)),
                    BinOp::Sub => Ok(Value::Num(a.as_num()? - b.as_num()?)),
                    BinOp::Mul => Ok(Value::Num(a.as_num()? * b.as_num()?)),
                    BinOp::Div => Ok(Value::Num(a.as_num()? / b.as_num()?)),
                    BinOp::Mod => Ok(Value::Num(a.as_num()? % b.as_num()?)),
                    BinOp::Eq => Ok(Value::Bool(a == b)),
                    BinOp::Ne => Ok(Value::Bool(a != b)),
                    BinOp::Lt => Ok(Value::Bool(a.as_num()? < b.as_num()?)),
                    BinOp::Gt => Ok(Value::Bool(a.as_num()? > b.as_num()?)),
                    BinOp::Le => Ok(Value::Bool(a.as_num()? <= b.as_num()?)),
                    BinOp::Ge => Ok(Value::Bool(a.as_num()? >= b.as_num()?)),
                }
            }
            Expr::Call { name, args, props } => {
                let mut arg_vals = Vec::new();
                for a in args {
                    arg_vals.push(self.eval_expr(a)?);
                }
                self.call_builtin(name, &arg_vals, props.as_ref())
            }
        }
    }

    fn eval_vec3(&mut self, e: &Expr) -> Result<[f64; 3], String> {
        let v = self.eval_expr(e)?.as_tuple()?;
        if v.is_empty() {
            return Err("Tuple kosong".into());
        }
        Ok([v[0], v.get(1).copied().unwrap_or(0.0), v.get(2).copied().unwrap_or(0.0)])
    }

    // ── Statement runner ──
    pub fn run_block(&mut self, stmts: &[Stmt]) -> Result<Option<Value>, String> {
        // Setiap blok (body handler, body fungsi, branch if/else, blok bersarang)
        // membuka scope lokal BARU — `let` di dalamnya bersifat lokal per blok
        // (spec LANGUAGE.md §7; KB §6 "handler/block let = local").
        self.scopes.push(HashMap::new());
        let result = self.run_block_inner(stmts);
        self.scopes.pop();
        result
    }

    fn run_block_inner(&mut self, stmts: &[Stmt]) -> Result<Option<Value>, String> {
        for s in stmts {
            if let Some(ret) = self.run_stmt(s)? {
                return Ok(Some(ret));
            }
        }
        Ok(None)
    }

    pub fn run_stmt(&mut self, s: &Stmt) -> Result<Option<Value>, String> {
        match s {
            Stmt::Let { name, value } => {
                let v = self.eval_expr(value)?;
                // `let` bind ke scope terdalam (lokal); shadowing global.
                // Fallback defensif ke globals bila tidak ada scope aktif.
                if let Some(scope) = self.scopes.last_mut() {
                    scope.insert(name.clone(), v);
                } else {
                    self.globals.insert(name.clone(), v);
                }
                Ok(None)
            }
            Stmt::Assign { name, value } => {
                let v = self.eval_expr(value)?;
                // Assignment menarget scope TERDEKAT yang memiliki nama tsb
                // (lokal dulu, lalu global). Target tidak dikenal = error (KB §6).
                match self.find_assignable_scope(name) {
                    Some(scope) => {
                        scope.insert(name.clone(), v);
                        Ok(None)
                    }
                    None => Err(format!("Variabel tidak dikenal '{name}'")),
                }
            }
            Stmt::ExprStmt(e) => {
                let v = self.eval_expr(e)?;
                // Rekam nilai ekspresi terakhir → dipakai untuk implicit return
                // fungsi bila body berakhir tanpa `return` eksplisit (KB §4.1).
                self.last_expr = Some(v);
                Ok(None)
            }
            Stmt::Return(e) => Ok(Some(self.eval_expr(e)?)),
            Stmt::Block(inner) => self.run_block(inner),
            Stmt::If { cond, then_branch, else_branch } => {
                let c = self.eval_expr(cond)?;
                let truthy = match c {
                    Value::Bool(b) => b,
                    Value::Num(n) => n != 0.0,
                    _ => false,
                };
                if truthy {
                    self.run_block(then_branch)
                } else {
                    self.run_block(else_branch)
                }
            }
        }
    }

    pub fn run_handler(&mut self, entity_id: Option<String>, body: &[Stmt]) -> Result<(), String> {
        let prev = self.current_entity.clone();
        self.current_entity = entity_id;
        let _ = self.run_block(body)?;
        self.current_entity = prev;
        Ok(())
    }

    // ── Builtins ──
    fn call_builtin(&mut self, name: &str, args: &[Value], _props: Option<&Vec<Prop>>) -> Result<Value, String> {
        // Fungsi yang diizinkan hanya sebagai statement (transform) bila punya entity konteks
        match name {
            "move" => {
                let e = self.current_entity_mut()?;
                if args.len() >= 3 {
                    e.transform.pos[0] += args[0].as_num()?;
                    e.transform.pos[1] += args[1].as_num()?;
                    e.transform.pos[2] += args[2].as_num()?;
                }
                Ok(Value::Null)
            }
            "setPos" => {
                let e = self.current_entity_mut()?;
                if args.len() >= 3 {
                    e.transform.pos = [args[0].as_num()?, args[1].as_num()?, args[2].as_num()?];
                }
                Ok(Value::Null)
            }
            "scaleBy" => {
                let e = self.current_entity_mut()?;
                // 1 arg = uniform (backward compat); 3 arg = per-axis; hilang = 1.0
                if args.len() < 2 {
                    let a = args.get(0).map(|a| a.as_num()).transpose()?.unwrap_or(1.0);
                    e.transform.scale = [
                        e.transform.scale[0] * a,
                        e.transform.scale[1] * a,
                        e.transform.scale[2] * a,
                    ];
                } else {
                    let a = args.get(0).map(|a| a.as_num()).transpose()?.unwrap_or(1.0);
                    let b = args.get(1).map(|a| a.as_num()).transpose()?.unwrap_or(1.0);
                    let c = args.get(2).map(|a| a.as_num()).transpose()?.unwrap_or(1.0);
                    e.transform.scale = [
                        e.transform.scale[0] * a,
                        e.transform.scale[1] * b,
                        e.transform.scale[2] * c,
                    ];
                }
                Ok(Value::Null)
            }
            "setScale" => {
                let e = self.current_entity_mut()?;
                // 1 arg = uniform (backward compat); 2 arg = x,y (z=1); 3 arg = per-axis
                if args.len() < 2 {
                    let a = args.get(0).map(|a| a.as_num()).transpose()?.unwrap_or(1.0);
                    e.transform.scale = [a, a, a];
                } else {
                    let a = args.get(0).map(|a| a.as_num()).transpose()?.unwrap_or(1.0);
                    let b = args.get(1).map(|a| a.as_num()).transpose()?.unwrap_or(1.0);
                    let c = args.get(2).map(|a| a.as_num()).transpose()?.unwrap_or(1.0);
                    e.transform.scale = [a, b, c];
                }
                Ok(Value::Null)
            }
            "rotate" => {
                let e = self.current_entity_mut()?;
                let angle = args.first().map(|a| a.as_num()).transpose()?.unwrap_or(0.0);
                let axis = if args.len() >= 2 {
                    args[1].as_tuple().ok().unwrap_or_else(|| vec![0.0, 1.0, 0.0])
                } else {
                    vec![0.0, 1.0, 0.0]
                };
                let ax = axis.get(0).copied().unwrap_or(0.0);
                let ay = axis.get(1).copied().unwrap_or(1.0);
                let az = axis.get(2).copied().unwrap_or(0.0);
                // Accumulate euler-ish rotation
                e.transform.rot[0] += angle * ax;
                e.transform.rot[1] += angle * ay;
                e.transform.rot[2] += angle * az;
                Ok(Value::Null)
            }
            "setColor" => {
                let e = self.current_entity_mut()?;
                if args.len() >= 3 {
                    e.color[0] = args[0].as_num()?;
                    e.color[1] = args[1].as_num()?;
                    e.color[2] = args[2].as_num()?;
                }
                Ok(Value::Null)
            }
            "setAlpha" => {
                let e = self.current_entity_mut()?;
                if let Some(a) = args.first() {
                    e.color[3] = a.as_num()?;
                }
                Ok(Value::Null)
            }
            // Matematika
            "sin" => Ok(Value::Num(args.first().map(|a| a.as_num()).transpose()?.unwrap_or(0.0).sin())),
            "cos" => Ok(Value::Num(args.first().map(|a| a.as_num()).transpose()?.unwrap_or(0.0).cos())),
            "tan" => Ok(Value::Num(args.first().map(|a| a.as_num()).transpose()?.unwrap_or(0.0).tan())),
            "asin" => Ok(Value::Num(args.first().map(|a| a.as_num()).transpose()?.unwrap_or(0.0).asin())),
            "acos" => Ok(Value::Num(args.first().map(|a| a.as_num()).transpose()?.unwrap_or(0.0).acos())),
            "atan" => Ok(Value::Num(args.first().map(|a| a.as_num()).transpose()?.unwrap_or(0.0).atan())),
            "sqrt" => Ok(Value::Num(args.first().map(|a| a.as_num()).transpose()?.unwrap_or(0.0).sqrt())),
            "abs" => Ok(Value::Num(args.first().map(|a| a.as_num()).transpose()?.unwrap_or(0.0).abs())),
            "floor" => Ok(Value::Num(args.first().map(|a| a.as_num()).transpose()?.unwrap_or(0.0).floor())),
            "ceil" => Ok(Value::Num(args.first().map(|a| a.as_num()).transpose()?.unwrap_or(0.0).ceil())),
            "round" => Ok(Value::Num(args.first().map(|a| a.as_num()).transpose()?.unwrap_or(0.0).round())),
            "pow" => {
                let a = args.first().map(|a| a.as_num()).transpose()?.unwrap_or(1.0);
                let b = args.get(1).map(|a| a.as_num()).transpose()?.unwrap_or(1.0);
                Ok(Value::Num(a.powf(b)))
            }
            "min" => {
                let a = args.first().map(|a| a.as_num()).transpose()?.unwrap_or(0.0);
                let b = args.get(1).map(|a| a.as_num()).transpose()?.unwrap_or(0.0);
                Ok(Value::Num(a.min(b)))
            }
            "max" => {
                let a = args.first().map(|a| a.as_num()).transpose()?.unwrap_or(0.0);
                let b = args.get(1).map(|a| a.as_num()).transpose()?.unwrap_or(0.0);
                Ok(Value::Num(a.max(b)))
            }
            "clamp" => {
                let a = args.first().map(|a| a.as_num()).transpose()?.unwrap_or(0.0);
                let lo = args.get(1).map(|a| a.as_num()).transpose()?.unwrap_or(0.0);
                let hi = args.get(2).map(|a| a.as_num()).transpose()?.unwrap_or(1.0);
                Ok(Value::Num(a.clamp(lo, hi)))
            }
            "lerp" => {
                let a = args.first().map(|a| a.as_num()).transpose()?.unwrap_or(0.0);
                let b = args.get(1).map(|a| a.as_num()).transpose()?.unwrap_or(0.0);
                let k = args.get(2).map(|a| a.as_num()).transpose()?.unwrap_or(0.0);
                Ok(Value::Num(a + (b - a) * k))
            }
            _ => {
                if let Some(f) = self.functions.get(name).cloned() {
                    // call user function
                    // Reentrancy (spec §8.3): globals di-snapshot & di-restore
                    // setelah pemanggilan agar panggilan fungsi deterministik.
                    let saved = self.globals.clone();
                    // Simpan last_expr caller, reset untuk body fungsi.
                    let saved_last = self.last_expr.take();
                    // Frame scope baru untuk params — body fungsi membuka scope
                    // lokal sendiri via run_block, jadi `let` di body tetap lokal.
                    self.scopes.push(HashMap::new());
                    for (i, p) in f.params.iter().enumerate() {
                        let v = args.get(i).cloned().unwrap_or(Value::Null);
                        if let Some(frame) = self.scopes.last_mut() {
                            frame.insert(p.clone(), v);
                        }
                    }
                    let result = self.run_block(&f.body);
                    self.scopes.pop();
                    self.globals = saved;
                    let ret = match result {
                        Ok(Some(v)) => v, // return eksplisit
                        Ok(None) => self.last_expr.take().unwrap_or(Value::Null), // implicit return
                        Err(e) => {
                            self.last_expr = saved_last;
                            return Err(e);
                        }
                    };
                    self.last_expr = saved_last;
                    Ok(ret)
                } else {
                    Err(format!("Fungsi tidak dikenal '{name}'"))
                }
            }
        }
    }

    /// Cari scope TERDEKAT (lokal terdalam → global) yang memiliki nama tsb.
    /// Assignment menarget scope ini; `None` = nama tidak dikenal → error.
    fn find_assignable_scope(&mut self, name: &str) -> Option<&mut HashMap<String, Value>> {
        for scope in self.scopes.iter_mut().rev() {
            if scope.contains_key(name) {
                return Some(scope);
            }
        }
        if self.globals.contains_key(name) {
            return Some(&mut self.globals);
        }
        None
    }

    fn current_entity_mut(&mut self) -> Result<&mut EntityState, String> {
        let id = self
            .current_entity
            .clone()
            .ok_or_else(|| "Fungsi transform hanya valid di dalam handler entity".to_string())?;
        self.world
            .entity_mut(&id)
            .ok_or_else(|| format!("Entity '{id}' tidak ditemukan"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    #[test]
    fn eval_world_builds_entities() {
        let src = r#"
            world "T" {
                camera "cam" { pos (0 1 5) look (0 0 0) fov 60 }
                light "k" { type point pos (1 2 3) color (1 1 1) intensity 2 }
                entity "a" { pos (1 2 3) mesh sphere { radius 0.5 } material solid (0.1 0.8 0.2) 0.9 }
                entity "b" { pos (-1 0 0) mesh torus { radius 1 tube 0.05 } material wire (0.9 0.2 0.2) 0.5 }
            }
        "#;
        let prog = parse(src).unwrap();
        let mut interp = Interpreter::new("T".into());
        interp.load(prog).unwrap();
        assert_eq!(interp.world.entities.len(), 2);
        assert_eq!(interp.world.camera.fov, 60.0);
        assert_eq!(interp.world.entities[0].transform.pos, [1.0, 2.0, 3.0]);
        assert_eq!(interp.world.entities[0].mesh, MeshKind::Sphere);
        assert_eq!(interp.world.entities[0].mesh_params.radius, 0.5);
        assert_eq!(interp.world.entities[0].color[3], 0.9);
        assert_eq!(interp.world.lights.len(), 2);
        assert_eq!(interp.world.lights[1].intensity, 2.0);
    }

    #[test]
    fn eval_frame_handler_mutates() {
        let src = r#"
            world "T" {
                entity "core" {
                    pos (0 0 0)
                    on frame {
                        rotate(0.35 * t, (0 1 0))
                        scaleBy(1 + 0.05 * sin(2.0 * t))
                    }
                }
            }
        "#;
        let prog = parse(src).unwrap();
        let mut interp = Interpreter::new("T".into());
        interp.load(prog).unwrap();
        interp.t = 1.0;
        let handlers = interp.world.handlers_for(crate::ast::EventKind::Frame, "core");
        let body: Vec<Stmt> = handlers.iter().flat_map(|h| h.body.clone()).collect();
        interp.run_handler(Some("core".into()), &body).unwrap();
        let e = interp.world.entity("core").unwrap();
        assert_ne!(e.transform.rot, [0.0, 0.0, 0.0]);
        assert!(e.transform.scale[0] > 1.0);
    }

    #[test]
    fn eval_math() {
        let src = r#"world "T" { let x = 2 + 3 * 4 }"#;
        let prog = parse(src).unwrap();
        let mut interp = Interpreter::new("T".into());
        interp.load(prog).unwrap();
        assert_eq!(interp.globals.get("x"), Some(&Value::Num(14.0)));
    }

    // ─── Local scoping (spec LANGUAGE.md §7 / KB §6) ───────────────────────

    #[test]
    fn eval_handler_let_is_local_and_shadows_global() {
        let src = r#"
            world "T" {
                let x = 100
                entity "e" {
                    on frame {
                        let x = 5
                        scaleBy(x)
                    }
                }
            }
        "#;
        let prog = parse(src).unwrap();
        let mut interp = Interpreter::new("T".into());
        interp.load(prog).unwrap();
        let handlers = interp.world.handlers_for(crate::ast::EventKind::Frame, "e");
        let body: Vec<Stmt> = handlers.iter().flat_map(|h| h.body.clone()).collect();
        interp.run_handler(Some("e".into()), &body).unwrap();
        let e = interp.world.entity("e").unwrap();
        // Handler memakai local x=5, bukan global x=100.
        assert_eq!(e.transform.scale, [5.0, 5.0, 5.0]);
        // Global x TIDAK boleh berubah (shadowing, bukan overwrite).
        assert_eq!(interp.globals.get("x"), Some(&Value::Num(100.0)));
    }

    #[test]
    fn eval_block_let_is_scoped() {
        let src = r#"
            world "T" {
                entity "e" {
                    on frame {
                        let x = 2
                        {
                            let x = 99
                            scaleBy(x)   # inner block: x = 99
                        }
                        scaleBy(x)       # outer handler scope: x masih 2
                    }
                }
            }
        "#;
        let prog = parse(src).unwrap();
        let mut interp = Interpreter::new("T".into());
        interp.load(prog).unwrap();
        let handlers = interp.world.handlers_for(crate::ast::EventKind::Frame, "e");
        let body: Vec<Stmt> = handlers.iter().flat_map(|h| h.body.clone()).collect();
        interp.run_handler(Some("e".into()), &body).unwrap();
        let e = interp.world.entity("e").unwrap();
        // 99 * 2 = 198 — let di blok dalam tidak bocor keluar blok.
        assert_eq!(e.transform.scale, [198.0, 198.0, 198.0]);
    }

    #[test]
    fn eval_assign_targets_nearest_scope() {
        let src = r#"
            world "T" {
                let g = 1
                entity "e" {
                    on frame {
                        g = 7                # assign ke GLOBAL (terdekat: global)
                        let l = 1
                        l = l + 2            # assign ke LOCAL (terdekat: local)
                        scaleBy(g * l)       # 7 * 3 = 21
                    }
                }
            }
        "#;
        let prog = parse(src).unwrap();
        let mut interp = Interpreter::new("T".into());
        interp.load(prog).unwrap();
        let handlers = interp.world.handlers_for(crate::ast::EventKind::Frame, "e");
        let body: Vec<Stmt> = handlers.iter().flat_map(|h| h.body.clone()).collect();
        interp.run_handler(Some("e".into()), &body).unwrap();
        let e = interp.world.entity("e").unwrap();
        assert_eq!(e.transform.scale, [21.0, 21.0, 21.0]);
        // g ter-update di globals (assignment eksplisit ke global).
        assert_eq!(interp.globals.get("g"), Some(&Value::Num(7.0)));
        // l TIDAK boleh bocor ke globals — local murni.
        assert!(!interp.globals.contains_key("l"));
    }

    #[test]
    fn eval_assign_unknown_variable_errors() {
        let src = r#"
            world "T" {
                entity "e" {
                    on frame {
                        ghost = 42
                    }
                }
            }
        "#;
        let prog = parse(src).unwrap();
        let mut interp = Interpreter::new("T".into());
        interp.load(prog).unwrap();
        let handlers = interp.world.handlers_for(crate::ast::EventKind::Frame, "e");
        let body: Vec<Stmt> = handlers.iter().flat_map(|h| h.body.clone()).collect();
        let res = interp.run_handler(Some("e".into()), &body);
        // Assignment ke variabel yang tidak pernah dideklarasikan = error (KB §6).
        assert!(res.is_err());
        let msg = res.unwrap_err();
        assert!(msg.contains("ghost"), "error harus menyebut nama variabel: {msg}");
    }

    #[test]
    fn eval_handler_locals_isolated_between_entities() {
        let src = r#"
            world "T" {
                entity "a" {
                    on frame {
                        let k = 3
                        scaleBy(k)
                    }
                }
                entity "b" {
                    on frame {
                        let k = 7
                        scaleBy(k)
                    }
                }
            }
        "#;
        let prog = parse(src).unwrap();
        let mut interp = Interpreter::new("T".into());
        interp.load(prog).unwrap();

        // Jalankan handler entity "a" → scale jadi 3
        let ha = interp.world.handlers_for(crate::ast::EventKind::Frame, "a");
        let ba: Vec<Stmt> = ha.iter().flat_map(|h| h.body.clone()).collect();
        interp.run_handler(Some("a".into()), &ba).unwrap();
        assert_eq!(interp.world.entity("a").unwrap().transform.scale, [3.0, 3.0, 3.0]);

        // Jalankan handler entity "b" → scale jadi 7 (bukan 3!)
        let hb = interp.world.handlers_for(crate::ast::EventKind::Frame, "b");
        let bb: Vec<Stmt> = hb.iter().flat_map(|h| h.body.clone()).collect();
        interp.run_handler(Some("b".into()), &bb).unwrap();
        assert_eq!(interp.world.entity("b").unwrap().transform.scale, [7.0, 7.0, 7.0]);

        // k tidak pernah bocor ke globals.
        assert!(!interp.globals.contains_key("k"));
    }

    #[test]
    fn eval_function_scope_and_reentrancy() {
        let src = r#"
            world "T" {
                let g = 5
                func f(x) {
                    let y = x * 2
                    g = 99        # assign ke global — harus di-restore setelah call
                    return y
                }
                entity "e" {
                    on frame {
                        scaleBy(f(4))
                    }
                }
            }
        "#;
        let prog = parse(src).unwrap();
        let mut interp = Interpreter::new("T".into());
        interp.load(prog).unwrap();
        let handlers = interp.world.handlers_for(crate::ast::EventKind::Frame, "e");
        let body: Vec<Stmt> = handlers.iter().flat_map(|h| h.body.clone()).collect();
        interp.run_handler(Some("e".into()), &body).unwrap();
        let e = interp.world.entity("e").unwrap();
        // f(4) = 4 * 2 = 8
        assert_eq!(e.transform.scale, [8.0, 8.0, 8.0]);
        // Reentrancy spec §8.3: globals di-restore — g kembali ke 5.
        assert_eq!(interp.globals.get("g"), Some(&Value::Num(5.0)));
        // Param x dan local y tidak bocor ke globals.
        assert!(!interp.globals.contains_key("x"));
        assert!(!interp.globals.contains_key("y"));
    }

    #[test]
    fn eval_assign_from_nested_block_targets_enclosing_local() {
        // Nearest-scope walk: assignment di blok dalam harus menemukan local
        // yang dideklarasikan di scope handler (bukan global).
        let src = r#"
            world "T" {
                let a = 100
                entity "e" {
                    on frame {
                        let a = 1
                        {
                            a = 5      # target: local handler `a`, bukan global 100
                        }
                        scaleBy(a)     # 5
                    }
                }
            }
        "#;
        let prog = parse(src).unwrap();
        let mut interp = Interpreter::new("T".into());
        interp.load(prog).unwrap();
        let handlers = interp.world.handlers_for(crate::ast::EventKind::Frame, "e");
        let body: Vec<Stmt> = handlers.iter().flat_map(|h| h.body.clone()).collect();
        interp.run_handler(Some("e".into()), &body).unwrap();
        let e = interp.world.entity("e").unwrap();
        assert_eq!(e.transform.scale, [5.0, 5.0, 5.0]);
        // Global a tidak tersentuh.
        assert_eq!(interp.globals.get("a"), Some(&Value::Num(100.0)));
    }

    #[test]
    fn eval_handler_locals_fresh_per_invocation() {
        // Stateless per frame: local let di-inisialisasi ulang setiap eksekusi
        // handler (bukan di-cache antar frame).
        let src = r#"
            world "T" {
                entity "e" {
                    on frame {
                        let n = 3
                        scaleBy(n)
                    }
                }
            }
        "#;
        let prog = parse(src).unwrap();
        let mut interp = Interpreter::new("T".into());
        interp.load(prog).unwrap();
        let handlers = interp.world.handlers_for(crate::ast::EventKind::Frame, "e");
        let body: Vec<Stmt> = handlers.iter().flat_map(|h| h.body.clone()).collect();

        // Frame 1: 1 * 3 = 3
        interp.run_handler(Some("e".into()), &body).unwrap();
        // Frame 2: tetap 3 * 3 = 9 (bukan 3^2 yang salah karena local n
        // tidak dibawa antar frame)
        interp.run_handler(Some("e".into()), &body).unwrap();
        assert_eq!(interp.world.entity("e").unwrap().transform.scale, [9.0, 9.0, 9.0]);
        assert!(!interp.globals.contains_key("n"));
    }

    // ─── Peningkatan spec↔impl (mesh positional, points, implicit return) ──

    #[test]
    fn eval_mesh_positional_box_sets_size() {
        let src = r#"
            world "T" {
                entity "e" { mesh box 2.0 }
            }
        "#;
        let prog = parse(src).unwrap();
        let mut interp = Interpreter::new("T".into());
        interp.load(prog).unwrap();
        let e = interp.world.entity("e").unwrap();
        // `mesh box 2.0` → ukuran box 2.0 (spec §5.2 / KB §4.3), bukan radius.
        assert_eq!(e.mesh, MeshKind::Box);
        assert_eq!(e.mesh_params.size, 2.0);
    }

    #[test]
    fn eval_mesh_positional_grid_sets_size_and_count() {
        let src = r#"
            world "T" {
                entity "e" { mesh grid 26 20 }
            }
        "#;
        let prog = parse(src).unwrap();
        let mut interp = Interpreter::new("T".into());
        interp.load(prog).unwrap();
        let e = interp.world.entity("e").unwrap();
        assert_eq!(e.mesh, MeshKind::Grid);
        assert_eq!(e.mesh_params.size, 26.0);
        assert_eq!(e.mesh_params.count, 20.0);
    }

    #[test]
    fn eval_mesh_positional_icosa_sets_inner() {
        let src = r#"
            world "T" {
                entity "e" { mesh icosa 1.5 0.65 }
            }
        "#;
        let prog = parse(src).unwrap();
        let mut interp = Interpreter::new("T".into());
        interp.load(prog).unwrap();
        let e = interp.world.entity("e").unwrap();
        assert_eq!(e.mesh, MeshKind::Icosa);
        assert_eq!(e.mesh_params.radius, 1.5);
        assert_eq!(e.mesh_params.inner, 0.65);
    }

    #[test]
    fn eval_points_material_maps_to_points() {
        let src = r#"
            world "T" {
                entity "e" { material points (1 1 1) 0.5 }
            }
        "#;
        let prog = parse(src).unwrap();
        let mut interp = Interpreter::new("T".into());
        interp.load(prog).unwrap();
        let e = interp.world.entity("e").unwrap();
        assert_eq!(e.material, MaterialKind::Points);
        assert_eq!(e.color, [1.0, 1.0, 1.0, 0.5]);
    }

    #[test]
    fn eval_function_implicit_return_last_expression() {
        let src = r#"
            world "T" {
                func spin_speed() { 0.4 }
                func triple(x) { x * 3 }
                entity "e" {
                    on frame {
                        scaleBy(spin_speed() + triple(2))   # 0.4 + 6 = 6.4
                    }
                }
            }
        "#;
        let prog = parse(src).unwrap();
        let mut interp = Interpreter::new("T".into());
        interp.load(prog).unwrap();
        let handlers = interp.world.handlers_for(crate::ast::EventKind::Frame, "e");
        let body: Vec<Stmt> = handlers.iter().flat_map(|h| h.body.clone()).collect();
        interp.run_handler(Some("e".into()), &body).unwrap();
        let e = interp.world.entity("e").unwrap();
        // Implicit return: nilai ekspresi terakhir body fungsi dikembalikan (KB §4.1).
        assert!((e.transform.scale[0] - 6.4).abs() < 1e-9);
    }

    #[test]
    fn eval_function_implicit_return_keeps_explicit_return_priority() {
        let src = r#"
            world "T" {
                func f() { 10 return 5 }   # whitespace-separated; ADILang tanpa semicolon
                entity "e" {
                    on frame {
                        scaleBy(f())
                    }
                }
            }
        "#;
        let prog = parse(src).unwrap();
        let mut interp = Interpreter::new("T".into());
        interp.load(prog).unwrap();
        let handlers = interp.world.handlers_for(crate::ast::EventKind::Frame, "e");
        let body: Vec<Stmt> = handlers.iter().flat_map(|h| h.body.clone()).collect();
        interp.run_handler(Some("e".into()), &body).unwrap();
        let e = interp.world.entity("e").unwrap();
        assert_eq!(e.transform.scale[0], 5.0);
    }

    #[test]
    fn eval_function_recursion_scoped_params() {
        let src = r#"
            world "T" {
                func fib(n) {
                    if n < 2 {
                        return n
                    }
                    return fib(n - 1) + fib(n - 2)
                }
                entity "e" {
                    on frame {
                        scaleBy(fib(6))
                    }
                }
            }
        "#;
        let prog = parse(src).unwrap();
        let mut interp = Interpreter::new("T".into());
        interp.load(prog).unwrap();
        let handlers = interp.world.handlers_for(crate::ast::EventKind::Frame, "e");
        let body: Vec<Stmt> = handlers.iter().flat_map(|h| h.body.clone()).collect();
        interp.run_handler(Some("e".into()), &body).unwrap();
        let e = interp.world.entity("e").unwrap();
        // fib(6) = 8 — param n di-frame terpisah per call, rekursi aman.
        assert_eq!(e.transform.scale, [8.0, 8.0, 8.0]);
    }

    // ─── Per-axis scale (v1.2.0, Extension Protocol) ─────────────────────

    #[test]
    fn eval_set_scale_per_axis() {
        let src = r#"
            world "T" {
                entity "e" {
                    on frame {
                        setScale(2, 3, 4)
                    }
                }
            }
        "#;
        let prog = parse(src).unwrap();
        let mut interp = Interpreter::new("T".into());
        interp.load(prog).unwrap();
        let handlers = interp.world.handlers_for(crate::ast::EventKind::Frame, "e");
        let body: Vec<Stmt> = handlers.iter().flat_map(|h| h.body.clone()).collect();
        interp.run_handler(Some("e".into()), &body).unwrap();
        let e = interp.world.entity("e").unwrap();
        assert_eq!(e.transform.scale, [2.0, 3.0, 4.0]);
    }

    #[test]
    fn eval_set_scale_uniform_backward_compat() {
        let src = r#"
            world "T" {
                entity "e" {
                    on frame {
                        setScale(5)
                    }
                }
            }
        "#;
        let prog = parse(src).unwrap();
        let mut interp = Interpreter::new("T".into());
        interp.load(prog).unwrap();
        let handlers = interp.world.handlers_for(crate::ast::EventKind::Frame, "e");
        let body: Vec<Stmt> = handlers.iter().flat_map(|h| h.body.clone()).collect();
        interp.run_handler(Some("e".into()), &body).unwrap();
        assert_eq!(interp.world.entity("e").unwrap().transform.scale, [5.0, 5.0, 5.0]);
    }

    #[test]
    fn eval_scale_by_per_axis() {
        let src = r#"
            world "T" {
                entity "e" {
                    on frame {
                        scaleBy(2, 3, 4)
                    }
                }
            }
        "#;
        let prog = parse(src).unwrap();
        let mut interp = Interpreter::new("T".into());
        interp.load(prog).unwrap();
        let handlers = interp.world.handlers_for(crate::ast::EventKind::Frame, "e");
        let body: Vec<Stmt> = handlers.iter().flat_map(|h| h.body.clone()).collect();
        interp.run_handler(Some("e".into()), &body).unwrap();
        assert_eq!(interp.world.entity("e").unwrap().transform.scale, [2.0, 3.0, 4.0]);
    }
}
