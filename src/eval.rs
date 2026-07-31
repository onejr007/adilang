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
    functions: HashMap<String, FuncDef>,
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
            functions: HashMap::new(),
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
            "scale" => e.transform.scale = self.eval_expr(&p.value)?.as_num()?,
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
        let mut mp = MeshParams::default();
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
        // argumen positional
        if let Some(a0) = args.first() {
            mp.radius = self.eval_expr(a0)?.as_num()?;
        }
        if let Some(a1) = args.get(1) {
            mp.tube = self.eval_expr(a1)?.as_num()?;
        }
        let kind = match name.as_str() {
            "sphere" => MeshKind::Sphere,
            "box" => MeshKind::Box,
            "torus" => MeshKind::Torus,
            "icosa" => MeshKind::Icosa,
            "ring" => MeshKind::Ring,
            "plane" => MeshKind::Plane,
            "grid" => MeshKind::Grid,
            other => return Err(format!("Mesh builder tidak dikenal '{other}'")),
        };
        e.mesh = kind;
        e.mesh_params = mp;
        Ok(())
    }

    fn apply_material(&mut self, e: &mut EntityState, v: &Expr) -> Result<(), String> {
        let (name, args) = match v {
            Expr::Call { name, args, .. } => (name.clone(), args.clone()),
            other => return Err(format!("material harus berupa builder, dapat {other:?}")),
        };
        let kind = match name.as_str() {
            "solid" => MaterialKind::Solid,
            "wire" => MaterialKind::Wire,
            "glow" => MaterialKind::Glow,
            "points" => MaterialKind::Solid,
            other => return Err(format!("Material builder tidak dikenal '{other}'")),
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
                    _ => self
                        .globals
                        .get(name)
                        .cloned()
                        .ok_or_else(|| format!("Variabel tidak dikenal '{name}'")),
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
                self.globals.insert(name.clone(), v);
                Ok(None)
            }
            Stmt::Assign { name, value } => {
                let v = self.eval_expr(value)?;
                self.globals.insert(name.clone(), v);
                Ok(None)
            }
            Stmt::ExprStmt(e) => {
                self.eval_expr(e)?;
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
                if let Some(a) = args.first() {
                    e.transform.scale *= a.as_num()?;
                }
                Ok(Value::Null)
            }
            "setScale" => {
                let e = self.current_entity_mut()?;
                if let Some(a) = args.first() {
                    e.transform.scale = a.as_num()?;
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
                    let saved = self.globals.clone();
                    for (i, p) in f.params.iter().enumerate() {
                        let v = args.get(i).cloned().unwrap_or(Value::Null);
                        self.globals.insert(p.clone(), v);
                    }
                    let result = self.run_block(&f.body)?;
                    self.globals = saved;
                    Ok(result.unwrap_or(Value::Null))
                } else {
                    Err(format!("Fungsi tidak dikenal '{name}'"))
                }
            }
        }
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
        assert!(e.transform.scale > 1.0);
    }

    #[test]
    fn eval_math() {
        let src = r#"world "T" { let x = 2 + 3 * 4 }"#;
        let prog = parse(src).unwrap();
        let mut interp = Interpreter::new("T".into());
        interp.load(prog).unwrap();
        assert_eq!(interp.globals.get("x"), Some(&Value::Num(14.0)));
    }
}
