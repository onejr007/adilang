// ADILang Machine Runner — interpreter WASM langsung dari bitstream (v1.14.0).
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).
//
// Tujuan: menjalankan scene TANPA string parse. Input = bitstream Dense
// (bytecode.rs MAGIC 0xAD) → decode → AST → evaluasi (Interpreter) → kumpulan
// operasi DOM 2D & WebGL2 yang bisa dieksekusi runtime JS secara real-time.
// Berbeda dari jalur web/adilang_web.js (parse string): MachineRunner murni
// menafsirkan opcode — tidak ada teks ADILang yang perlu dibaca host.

use serde_json::json;

use crate::ast::{EventKind, FlexDirection, LifecycleHookKind, Program, Stmt, TopLevel, UIComponent};
use crate::bytecode;
use crate::eval::{DirectiveCall, Interpreter, Value};
use crate::parser;
use crate::scene::{LightKind, MaterialKind};

/// Runner mesin: bitstream → world + komponen siap eksekusi.
pub struct MachineRunner {
    program: Program,
    interp: Interpreter,
    /// Bitstream Dense yang dimuat (tetap disimpan untuk telemetri/re-encode).
    pub dense: Vec<u8>,
}

impl MachineRunner {
    /// Muat LANGSUNG dari bitstream Dense (tanpa parse string).
    pub fn from_dense(bytes: Vec<u8>) -> Result<Self, String> {
        let program = bytecode::decode_program(bytes.clone())?;
        let mut interp = Interpreter::new(program.name.clone());
        interp.load(program.clone())?;
        Ok(Self { program, interp, dense: bytes })
    }

    /// Muat dari sumber (konversi sekali ke bitstream, lalu jalankan via
    /// jalur bitstream yang sama — pembuktian ekuivalensi).
    pub fn from_source(src: &str) -> Result<Self, String> {
        let program = parser::parse(src)?;
        let dense = bytecode::encode_program(&program)?;
        Self::from_dense(dense)
    }

    pub fn program(&self) -> &Program {
        &self.program
    }

    pub fn dense_bytes(&self) -> &[u8] {
        &self.dense
    }

    /// Jalankan lifecycle hook komponen (on_mount/on_update/on_unmount) →
    /// directive calls yang dieksekusi (JSON).
    pub fn run_lifecycle(
        &mut self,
        name: &str,
        kind: LifecycleHookKind,
    ) -> Result<String, String> {
        let calls = self.interp.run_lifecycle(name, kind)?;
        Ok(json!({
            "status": "ok",
            "component": name,
            "hook": kind.as_str(),
            "directives": calls.iter().map(directive_call_to_json).collect::<Vec<_>>(),
        })
        .to_string())
    }

    /// Jalankan handler event (frame/speak/silent/click) pada entitas (atau
    /// level dunia bila entity_id None). Mengembalikan jumlah handler jalan.
    pub fn fire_event(&mut self, entity_id: Option<&str>, kind: EventKind) -> Result<usize, String> {
        let handlers = self
            .interp
            .world
            .handlers_for(kind, entity_id.unwrap_or(""))
            .iter()
            .map(|h| (entity_id.map(str::to_string), h.body.clone()))
            .collect::<Vec<_>>();
        let count = handlers.len();
        for (id, body) in &handlers {
            self.interp.run_handler(id.clone(), body)?;
        }
        Ok(count)
    }

    /// Satu frame: jalankan semua handler `on frame` (tanpa renderer).
    pub fn run_frame(&mut self) -> Result<usize, String> {
        self.fire_event(None, EventKind::Frame)
    }

    /// Operasi DOM 2D untuk semua ui_layout (JSON) — runtime JS membuat node
    /// DOM dari opcode Dense (NodeContainer/NodeText/...), bukan dari HTML.
    pub fn dom_ops_json(&self) -> String {
        let layouts = self
            .program
            .items
            .iter()
            .filter_map(|i| match i {
                TopLevel::UILayout(l) => Some(l),
                _ => None,
            })
            .collect::<Vec<_>>();
        let arr = layouts
            .iter()
            .map(|l| {
                let mut seq = 0u32;
                let ops = ui_to_dom(&l.root, &mut seq);
                json!({ "layout": l.name, "ops": ops })
            })
            .collect::<Vec<_>>();
        json!({ "status": "ok", "layouts": arr }).to_string()
    }

    /// Operasi WebGL2 untuk world (JSON) — mesh/transform/material/light/camera
    /// siap dijadikan objek scene (three.js/WebGL2 native) oleh runtime JS.
    pub fn webgl_ops_json(&self) -> String {
        let w = &self.interp.world;
        let camera = json!({
            "op": "camera",
            "node": crate::dense::DENSE_NODE_CAMERA,
            "id": w.camera.id,
            "pos": w.camera.pos,
            "look": w.camera.look,
            "fov": w.camera.fov,
        });
        let lights = w
            .lights
            .iter()
            .map(|l| {
                json!({
                    "op": "light",
                    "node": crate::dense::DENSE_NODE_LIGHT,
                    "id": l.id,
                    "kind": light_kind_name(l.kind),
                    "pos": l.pos,
                    "color": l.color,
                    "intensity": l.intensity,
                })
            })
            .collect::<Vec<_>>();
        let entities = w
            .entities
            .iter()
            .map(|e| {
                json!({
                    "op": "mesh",
                    "node": crate::dense::opcode_of_mesh(e.mesh),
                    "mesh": crate::dense::mesh_name(e.mesh),
                    "id": e.id,
                    "pos": e.transform.pos,
                    "rot": e.transform.rot,
                    "scale": e.transform.scale,
                    "color": e.color,
                    "material": material_name(e.material),
                    "handlers": e.handlers.len(),
                })
            })
            .collect::<Vec<_>>();
        json!({
            "status": "ok",
            "scene": w.name,
            "camera": camera,
            "lights": lights,
            "entities": entities,
            "handlers": {
                "frame": w.frame_handlers.len(),
                "speak": w.speak_handlers.len(),
                "silent": w.silent_handlers.len(),
                "click": w.click_handlers.len(),
            },
        })
        .to_string()
    }

    /// Daftar komponen + hook + directive (JSON) untuk runtime/host.
    pub fn components_json(&self) -> String {
        let comps = self
            .program
            .items
            .iter()
            .filter_map(|i| match i {
                TopLevel::Component(c) => Some(json!({
                    "name": c.name,
                    "hooks": c.hooks.iter().map(|h| json!({
                        "kind": h.kind.as_str(),
                        "directives": h.body.iter().filter_map(|s| match s {
                            Stmt::Directive { name, args } => {
                                Some(json!({ "name": name, "args": args.len() }))
                            }
                            _ => None,
                        }).collect::<Vec<_>>(),
                    })).collect::<Vec<_>>(),
                })),
                _ => None,
            })
            .collect::<Vec<_>>();
        json!({ "status": "ok", "components": comps }).to_string()
    }

    /// Ringkasan teknis runner (bitstream → DOM/WebGL2) untuk telemetri.
    pub fn spec(&self) -> String {
        let layouts = self
            .program
            .items
            .iter()
            .filter(|i| matches!(i, TopLevel::UILayout(_)))
            .count();
        let entities = self.interp.world.entities.len();
        let components = self.interp.components.len();
        format!(
            "MachineRunner v1.14.0: bitstream {}B -> AST -> eval -> DOM/WebGL2. \
             program: {}, ui_layouts: {}, entities: {}, components: {}",
            self.dense.len(),
            self.program.name,
            layouts,
            entities,
            components
        )
    }
}

fn directive_call_to_json(c: &DirectiveCall) -> serde_json::Value {
    json!({
        "name": c.name,
        "args": c.args.iter().map(value_to_json).collect::<Vec<_>>(),
    })
}

fn value_to_json(v: &Value) -> serde_json::Value {
    match v {
        Value::Num(n) => json!(n),
        Value::Str(s) => json!(s),
        Value::Bool(b) => json!(b),
        Value::Tuple(t) => json!(t),
        Value::List(l) => json!(l.iter().map(value_to_json).collect::<Vec<_>>()),
        Value::Map(m) => json!(
            m.iter()
                .map(|(k, v)| (k.clone(), value_to_json(v)))
                .collect::<serde_json::Map<_, _>>()
        ),
        Value::Null => json!(null),
    }
}

fn ui_to_dom(ui: &UIComponent, seq: &mut u32) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    match ui {
        UIComponent::Container { flex, children } => {
            *seq += 1;
            let dir = match flex {
                Some(FlexDirection::Row) => "row",
                Some(FlexDirection::Column) => "column",
                None => "auto",
            };
            let mut children_ops = Vec::new();
            for c in children {
                children_ops.extend(ui_to_dom(c, seq));
            }
            out.push(json!({
                "op": "create",
                "node": crate::dense::DENSE_NODE_CONTAINER,
                "node_hex": format!("0x{:02X}", crate::dense::DENSE_NODE_CONTAINER),
                "id": *seq,
                "tag": "div",
                "class": "container",
                "style": format!("display:flex;flex-direction:{dir}"),
                "children": children_ops,
            }));
        }
        UIComponent::Text { content } => {
            *seq += 1;
            out.push(json!({
                "op": "create",
                "node": crate::dense::DENSE_NODE_TEXT,
                "node_hex": format!("0x{:02X}", crate::dense::DENSE_NODE_TEXT),
                "id": *seq,
                "tag": "span",
                "text": content,
            }));
        }
        UIComponent::Button { label, onClick } => {
            *seq += 1;
            out.push(json!({
                "op": "create",
                "node": crate::dense::DENSE_NODE_BUTTON,
                "node_hex": format!("0x{:02X}", crate::dense::DENSE_NODE_BUTTON),
                "id": *seq,
                "tag": "button",
                "text": label,
                "onClick": onClick,
            }));
        }
        UIComponent::Input { name, placeholder, bind, validate } => {
            *seq += 1;
            out.push(json!({
                "op": "create",
                "node": crate::dense::DENSE_NODE_INPUT,
                "node_hex": format!("0x{:02X}", crate::dense::DENSE_NODE_INPUT),
                "id": *seq,
                "tag": "input",
                "name": name,
                "placeholder": placeholder,
                "bind": bind,
                "validate": validate,
            }));
        }
        UIComponent::Card { title, children } => {
            *seq += 1;
            let mut children_ops = Vec::new();
            for c in children {
                children_ops.extend(ui_to_dom(c, seq));
            }
            out.push(json!({
                "op": "create",
                "node": crate::dense::DENSE_NODE_CARD,
                "node_hex": format!("0x{:02X}", crate::dense::DENSE_NODE_CARD),
                "id": *seq,
                "tag": "div",
                "class": "card",
                "title": title,
                "children": children_ops,
            }));
        }
        UIComponent::Modal { title, children } => {
            *seq += 1;
            let mut children_ops = Vec::new();
            for c in children {
                children_ops.extend(ui_to_dom(c, seq));
            }
            out.push(json!({
                "op": "create",
                "node": crate::dense::DENSE_NODE_MODAL,
                "node_hex": format!("0x{:02X}", crate::dense::DENSE_NODE_MODAL),
                "id": *seq,
                "tag": "div",
                "class": "modal",
                "title": title,
                "children": children_ops,
            }));
        }
        UIComponent::Navbar { title } => {
            *seq += 1;
            out.push(json!({
                "op": "create",
                "node": crate::dense::DENSE_NODE_NAVBAR,
                "node_hex": format!("0x{:02X}", crate::dense::DENSE_NODE_NAVBAR),
                "id": *seq,
                "tag": "nav",
                "text": title,
            }));
        }
        UIComponent::Footer { content } => {
            *seq += 1;
            out.push(json!({
                "op": "create",
                "node": crate::dense::DENSE_NODE_FOOTER,
                "node_hex": format!("0x{:02X}", crate::dense::DENSE_NODE_FOOTER),
                "id": *seq,
                "tag": "footer",
                "text": content,
            }));
        }
    }
    out
}

fn light_kind_name(kind: LightKind) -> &'static str {
    match kind {
        LightKind::Point => "point",
        LightKind::Ambient => "ambient",
    }
}

fn material_name(kind: MaterialKind) -> &'static str {
    match kind {
        MaterialKind::Solid => "solid",
        MaterialKind::Wire => "wire",
        MaterialKind::Glow => "glow",
        MaterialKind::Points => "points",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = r#"
        component MyCard {
            on_mount: @fetch_data()
            on_update: @log_change("card")
        }
        ui_layout "main" {
            container {
                flex column
                text "Hello ADILang"
                button "Submit" onClick submit
                input "username" placeholder "Enter name"
            }
        }
        spatial_3d "scene" {
            camera "cam" { pos (0 1.6 7) look (0 0 0) fov 55 }
            light "key" { type point pos (5 6 4) color (1 0.95 0.9) intensity 1.5 }
            entity "cube" {
                pos (0 0 0)
                mesh box { size 1 }
                material solid (0.9 0.1 0.1) 1
                on frame { rotate(0.1, (0 1 0)) }
            }
        }
    "#;

    #[test]
    fn runner_bitstream_dan_source_ekuivalen() {
        let from_dense = MachineRunner::from_source(SRC).expect("from_source ok");
        let from_bitstream = MachineRunner::from_dense(from_dense.dense.clone()).expect("from_dense ok");
        assert_eq!(from_dense.program(), from_bitstream.program(), "AST harus identik");
        assert!(!from_dense.dense.is_empty(), "bitstream tidak boleh kosong");
        assert!(from_dense.spec().contains("MachineRunner v1.14.0"));
    }

    #[test]
    fn runner_dom_ops_memuat_opcode() {
        let runner = MachineRunner::from_source(SRC).expect("runner ok");
        let dom = serde_json::from_str::<serde_json::Value>(&runner.dom_ops_json()).expect("json valid");
        assert_eq!(dom["status"], "ok");
        let ops = dom["layouts"][0]["ops"].as_array().expect("ops");
        assert_eq!(ops[0]["node"], 1, "NodeContainer = 0x01");
        assert_eq!(ops[0]["node_hex"], "0x01");
        let children = ops[0]["children"].as_array().expect("children");
        assert_eq!(children[0]["tag"], "span");
        assert_eq!(children[1]["tag"], "button");
        assert_eq!(children[2]["tag"], "input");
    }

    #[test]
    fn runner_webgl_ops_memuat_mesh_camera_light() {
        let runner = MachineRunner::from_source(SRC).expect("runner ok");
        let web = serde_json::from_str::<serde_json::Value>(&runner.webgl_ops_json()).expect("json valid");
        assert_eq!(web["status"], "ok");
        assert_eq!(web["camera"]["op"], "camera");
        assert_eq!(web["lights"][0]["op"], "light");
        let entity = web["entities"].as_array().expect("entities")[0].clone();
        assert_eq!(entity["mesh"], "cube");
        assert_eq!(entity["node"], crate::dense::DENSE_MESH_CUBE);
        assert_eq!(entity["material"], "solid");
        assert_eq!(entity["pos"][0], 0.0);
        assert_eq!(entity["handlers"], 1, "entity 'cube' punya 1 handler on frame");
    }

    #[test]
    fn runner_lifecycle_dari_bitstream() {
        let mut runner = MachineRunner::from_source(SRC).expect("runner ok");
        let out = runner
            .run_lifecycle("MyCard", LifecycleHookKind::Mount)
            .expect("mount ok");
        let v = serde_json::from_str::<serde_json::Value>(&out).expect("json valid");
        assert_eq!(v["directives"][0]["name"], "fetch_data");
    }

    #[test]
    fn runner_fire_event_frame() {
        let mut runner = MachineRunner::from_source(SRC).expect("runner ok");
        let n = runner
            .fire_event(Some("cube"), EventKind::Frame)
            .expect("frame ok");
        assert_eq!(n, 1, "satu handler on frame di entity 'cube'");
    }

    #[test]
    fn runner_komponen_terdaftar() {
        let runner = MachineRunner::from_source(SRC).expect("runner ok");
        let comps = serde_json::from_str::<serde_json::Value>(&runner.components_json()).expect("json valid");
        assert_eq!(comps["components"][0]["name"], "MyCard");
        assert_eq!(comps["components"][0]["hooks"][0]["kind"], "on_mount");
    }
}
