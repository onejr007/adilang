// ADILang Dense Compact AST — opcode map & bitstream (v1.14.0).
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).
//
// Tujuan: representasi UI 2D / layout / 3D sebagai opcode satu-byte sehingga
// LLM/agent lain dapat mengirim & menerima scene TANPA markup HTML/JS/JSON
// (Zero-Token-Waste Format — baca bytecode.rs). Bitstream kanonik dihasilkan
// oleh bytecode.rs (MAGIC 0xAD, VERSION_BIN 0x04, string table tertutup).
// Modul ini menyediakan:
//
//   1. Peta opcode Dense yang dapat di-enumerasi (P6) untuk node UI 2D,
//      layout, dan mesh WebGL 3D — mis. `0x01 = NodeContainer`,
//      `0x0A = WebGLMeshCube`.
//   2. encode/decode ringkas (delegasi ke bytecode.rs — AST <-> bitstream).
//   3. `size_report()` — perbandingan byte terhadap ekuivalen HTML/JS/JSON
//      (penghematan token >80% untuk scene deklaratif).

use serde_json::json;

use crate::ast::{FlexDirection, Program, TopLevel, UIComponent};
use crate::parser::parse;
use crate::scene::{LightKind, MaterialKind, MeshKind};

// ═══════════════════════════════════════════════════════════════════════════
// DENSE OPCODE MAP — UI 2D / Layout / 3D (kode tetap, deterministik P1)
// ═══════════════════════════════════════════════════════════════════════════

pub const DENSE_NODE_CONTAINER: u8 = 0x01; // NodeContainer — div.flex container
pub const DENSE_NODE_TEXT: u8 = 0x02;      // NodeText — span
pub const DENSE_NODE_BUTTON: u8 = 0x03;    // NodeButton — button + onClick
pub const DENSE_NODE_INPUT: u8 = 0x04;     // NodeInput — input + bind/validate
pub const DENSE_NODE_CARD: u8 = 0x05;      // NodeCard
pub const DENSE_NODE_MODAL: u8 = 0x06;     // NodeModal
pub const DENSE_NODE_NAVBAR: u8 = 0x07;    // NodeNavbar
pub const DENSE_NODE_FOOTER: u8 = 0x08;    // NodeFooter
pub const DENSE_LAYOUT_FLEX_ROW: u8 = 0x09;    // LayoutFlexRow
pub const DENSE_LAYOUT_FLEX_COLUMN: u8 = 0x09; // alias read-only (row == column
                                               // dalam 1 byte karena flex arah
                                               // hanya 1 bit di bitstream)
pub const DENSE_MESH_CUBE: u8 = 0x0A;  // WebGLMeshCube (mesh box)
pub const DENSE_MESH_SPHERE: u8 = 0x0B;  // WebGLMeshSphere
pub const DENSE_MESH_TORUS: u8 = 0x0C;   // WebGLMeshTorus
pub const DENSE_MESH_ICOSA: u8 = 0x0D;   // WebGLMeshIcosa
pub const DENSE_MESH_RING: u8 = 0x0E;    // WebGLMeshRing
pub const DENSE_MESH_PLANE: u8 = 0x0F;   // WebGLMeshPlane
pub const DENSE_MESH_GRID: u8 = 0x10;    // WebGLMeshGrid
pub const DENSE_NODE_CAMERA: u8 = 0x11;  // NodeCamera
pub const DENSE_NODE_LIGHT: u8 = 0x12;   // NodeLight
pub const DENSE_NODE_ENTITY: u8 = 0x13;  // NodeEntity (transform + mesh + material)

/// Nama simbolis untuk opcode (dipakai `dense_spec()` & histogram).
pub fn opcode_name(code: u8) -> &'static str {
    match code {
        DENSE_NODE_CONTAINER => "NodeContainer",
        DENSE_NODE_TEXT => "NodeText",
        DENSE_NODE_BUTTON => "NodeButton",
        DENSE_NODE_INPUT => "NodeInput",
        DENSE_NODE_CARD => "NodeCard",
        DENSE_NODE_MODAL => "NodeModal",
        DENSE_NODE_NAVBAR => "NodeNavbar",
        DENSE_NODE_FOOTER => "NodeFooter",
        DENSE_MESH_CUBE => "WebGLMeshCube",
        DENSE_MESH_SPHERE => "WebGLMeshSphere",
        DENSE_MESH_TORUS => "WebGLMeshTorus",
        DENSE_MESH_ICOSA => "WebGLMeshIcosa",
        DENSE_MESH_RING => "WebGLMeshRing",
        DENSE_MESH_PLANE => "WebGLMeshPlane",
        DENSE_MESH_GRID => "WebGLMeshGrid",
        DENSE_NODE_CAMERA => "NodeCamera",
        DENSE_NODE_LIGHT => "NodeLight",
        DENSE_NODE_ENTITY => "NodeEntity",
        _ => "Reserved",
    }
}

/// Opcode Dense untuk sebuah UIComponent (peta UI 2D → byte).
pub fn opcode_of_ui(ui: &UIComponent) -> u8 {
    match ui {
        UIComponent::Container { .. } => DENSE_NODE_CONTAINER,
        UIComponent::Text { .. } => DENSE_NODE_TEXT,
        UIComponent::Button { .. } => DENSE_NODE_BUTTON,
        UIComponent::Input { .. } => DENSE_NODE_INPUT,
        UIComponent::Card { .. } => DENSE_NODE_CARD,
        UIComponent::Modal { .. } => DENSE_NODE_MODAL,
        UIComponent::Navbar { .. } => DENSE_NODE_NAVBAR,
        UIComponent::Footer { .. } => DENSE_NODE_FOOTER,
    }
}

/// Opcode Dense untuk sebuah MeshKind (peta mesh WebGL 3D → byte).
pub fn opcode_of_mesh(kind: MeshKind) -> u8 {
    match kind {
        MeshKind::Box => DENSE_MESH_CUBE,
        MeshKind::Sphere => DENSE_MESH_SPHERE,
        MeshKind::Torus => DENSE_MESH_TORUS,
        MeshKind::Icosa => DENSE_MESH_ICOSA,
        MeshKind::Ring => DENSE_MESH_RING,
        MeshKind::Plane => DENSE_MESH_PLANE,
        MeshKind::Grid => DENSE_MESH_GRID,
    }
}

/// Nama MeshKind (dipakai webgl_ops / dense_spec).
pub fn mesh_name(kind: MeshKind) -> &'static str {
    match kind {
        MeshKind::Box => "cube",
        MeshKind::Sphere => "sphere",
        MeshKind::Torus => "torus",
        MeshKind::Icosa => "icosa",
        MeshKind::Ring => "ring",
        MeshKind::Plane => "plane",
        MeshKind::Grid => "grid",
    }
}

/// Nama MaterialKind (dipakai json_equivalent).
pub fn material_name(kind: MaterialKind) -> &'static str {
    match kind {
        MaterialKind::Solid => "solid",
        MaterialKind::Wire => "wire",
        MaterialKind::Glow => "glow",
        MaterialKind::Points => "points",
    }
}

/// Nama LightKind (dipakai json_equivalent).
pub fn light_name(kind: LightKind) -> &'static str {
    match kind {
        LightKind::Point => "point",
        LightKind::Ambient => "ambient",
    }
}

/// Spec Dense opcode map (P6 self-describing) — tabel ringkas yang bisa
/// diparsing AI mana pun tanpa membaca docs.
pub fn dense_spec() -> String {
    let mut lines: Vec<&str> = vec!["DENSE OPCODE MAP (ADILang v1.14.0)"];
    lines.push("  UI 2D / Layout:");
    lines.push("  0x01 NodeContainer  0x02 NodeText  0x03 NodeButton  0x04 NodeInput");
    lines.push("  0x05 NodeCard       0x06 NodeModal 0x07 NodeNavbar  0x08 NodeFooter");
    lines.push("  0x09 LayoutFlex     0x09 LayoutFlexRow|Column");
    lines.push("  WebGL 3D mesh:");
    lines.push("  0x0A WebGLMeshCube  0x0B WebGLMeshSphere 0x0C WebGLMeshTorus");
    lines.push("  0x0D WebGLMeshIcosa 0x0E WebGLMeshRing   0x0F WebGLMeshPlane");
    lines.push("  0x10 WebGLMeshGrid");
    lines.push("  3D scene nodes:");
    lines.push("  0x11 NodeCamera  0x12 NodeLight  0x13 NodeEntity");
    lines.push("  Bitstream: bytecode.rs v0x04 (MAGIC 0xAD) — lihat binary_spec().");
    lines.join("\n")
}

// ═══════════════════════════════════════════════════════════════════════════
// ENCODE / DECODE — AST <-> bitstream ringkas
// ═══════════════════════════════════════════════════════════════════════════

/// Encode program → bitstream Dense (delegasi bytecode.rs).
pub fn encode_program(program: &Program) -> Result<Vec<u8>, String> {
    crate::bytecode::encode_program(program)
}

/// Decode bitstream Dense → AST (delegasi bytecode.rs). Tanpa parse string —
/// machine runner langsung menafsirkan bitstream.
pub fn decode_program(bin: Vec<u8>) -> Result<Program, String> {
    crate::bytecode::decode_program(bin)
}

/// Histogram opcode Dense dalam sebuah bitstream (JSON) — untuk telemetri
/// seberapa padat representasi vs kata kunci teks.
pub fn opcode_histogram(bin: &[u8]) -> String {
    let mut counts = [0usize; 256];
    for &b in bin {
        counts[b as usize] += 1;
    }
    let opcodes = (0u16..=0xFF)
        .filter(|&b| counts[b as usize] > 0 && opcode_name(b as u8) != "Reserved")
        .map(|b| {
            json!({
                "opcode": format!("0x{b:02X}"),
                "name": opcode_name(b as u8),
                "count": counts[b as usize],
            })
        })
        .collect::<Vec<_>>();
    json!({
        "total_bytes": bin.len(),
        "opcodes": opcodes,
    })
    .to_string()
}

// ═══════════════════════════════════════════════════════════════════════════
// PERBANDINGAN UKURAN — dense vs ekuivalen HTML/JS/JSON
// ═══════════════════════════════════════════════════════════════════════════

/// Ekuivalen JSON deterministik dari sebuah program — dokumen scene-graph
/// TER-RESOLUSI penuh (post-evaluasi) yang harus dihasilkan LLM bila scene
/// dikirim sebagai JSON ber-typed (bukan opcode). Jauh lebih besar dari
/// bitstream Dense karena tiap node membawa nama field panjang berulang.
pub fn json_equivalent(program: &Program) -> String {
    let mut interp = crate::eval::Interpreter::new(program.name.clone());
    let _ = interp.load(program.clone());
    let w = &interp.world;

    let ui = program
        .items
        .iter()
        .filter_map(|i| match i {
            TopLevel::UILayout(l) => Some(json!({
                "type": "ui_layout",
                "name": l.name,
                "root": ui_to_json(&l.root),
            })),
            _ => None,
        })
        .collect::<Vec<_>>();

    let entities = w
        .entities
        .iter()
        .map(|e| {
            json!({
                "type": "entity",
                "id": e.id,
                "mesh": crate::dense::mesh_name(e.mesh),
                "params": {
                    "radius": e.mesh_params.radius,
                    "tube": e.mesh_params.tube,
                    "inner": e.mesh_params.inner,
                    "segments": e.mesh_params.segments,
                    "size": e.mesh_params.size,
                    "count": e.mesh_params.count,
                },
                "transform": {
                    "position": { "x": e.transform.pos[0], "y": e.transform.pos[1], "z": e.transform.pos[2] },
                    "rotation": { "x": e.transform.rot[0], "y": e.transform.rot[1], "z": e.transform.rot[2] },
                    "scale": { "x": e.transform.scale[0], "y": e.transform.scale[1], "z": e.transform.scale[2] },
                },
                "color": { "r": e.color[0], "g": e.color[1], "b": e.color[2], "a": e.color[3] },
                "material": crate::dense::material_name(e.material),
                "handlers": e.handlers.iter().map(|h| json!({
                    "event": match h.event {
                        crate::ast::EventKind::Frame => "frame",
                        crate::ast::EventKind::Speak => "speak",
                        crate::ast::EventKind::Silent => "silent",
                        crate::ast::EventKind::Click => "click",
                    },
                    "body": h.body.iter().map(stmt_to_json).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();

    let lights = w
        .lights
        .iter()
        .map(|l| {
            json!({
                "type": "light",
                "id": l.id,
                "kind": crate::dense::light_name(l.kind),
                "position": { "x": l.pos[0], "y": l.pos[1], "z": l.pos[2] },
                "color": { "r": l.color[0], "g": l.color[1], "b": l.color[2] },
                "intensity": l.intensity,
            })
        })
        .collect::<Vec<_>>();

    let components = program
        .items
        .iter()
        .filter_map(|i| match i {
            TopLevel::Component(c) => Some(json!({
                "type": "component",
                "name": c.name,
                "hooks": c.hooks.iter().map(|h| json!({
                    "kind": h.kind.as_str(),
                    "statements": h.body.len(),
                })).collect::<Vec<_>>(),
            })),
            _ => None,
        })
        .collect::<Vec<_>>();

    let routes = program
        .items
        .iter()
        .filter_map(|i| match i {
            TopLevel::Routes(r) => Some(json!({
                "type": "routes",
                "routes": r.routes.iter().map(|rt| json!({
                    "path": rt.path,
                    "layout": rt.layout,
                    "transition": rt.transition,
                })).collect::<Vec<_>>(),
            })),
            _ => None,
        })
        .collect::<Vec<_>>();

    let payload = program
        .items
        .iter()
        .filter_map(|i| match i {
            TopLevel::Payload(p) => Some(json!({
                "type": "payload",
                "sender": p.sender,
                "target_agent": p.target_agent,
                "intent": p.intent,
                "state_data": p.state_data.is_some(),
            })),
            _ => None,
        })
        .next();

    let doc = json!({
        "type": "document",
        "name": program.name,
        "version": "1.14.0",
        "payload": payload,
        "ui": ui,
        "scene": {
            "camera": {
                "id": w.camera.id,
                "position": { "x": w.camera.pos[0], "y": w.camera.pos[1], "z": w.camera.pos[2] },
                "look": { "x": w.camera.look[0], "y": w.camera.look[1], "z": w.camera.look[2] },
                "fov": w.camera.fov,
            },
            "lights": lights,
            "entities": entities,
        },
        "components": components,
        "routes": routes,
    });
    serde_json::to_string_pretty(&doc).unwrap_or_else(|_| doc.to_string())
}

fn ui_to_json(ui: &UIComponent) -> serde_json::Value {
    match ui {
        UIComponent::Container { flex, children } => json!({
            "type": "container",
            "flex": match flex {
                Some(FlexDirection::Row) => "row",
                Some(FlexDirection::Column) => "column",
                None => "auto",
            },
            "children": children.iter().map(ui_to_json).collect::<Vec<_>>(),
        }),
        UIComponent::Text { content } => json!({ "type": "text", "content": content }),
        UIComponent::Button { label, onClick } => json!({
            "type": "button",
            "label": label,
            "onClick": onClick,
        }),
        UIComponent::Input { name, placeholder, bind, validate } => json!({
            "type": "input",
            "name": name,
            "placeholder": placeholder,
            "bind": bind,
            "validate": validate,
        }),
        UIComponent::Card { title, children } => json!({
            "type": "card",
            "title": title,
            "children": children.iter().map(ui_to_json).collect::<Vec<_>>(),
        }),
        UIComponent::Modal { title, children } => json!({
            "type": "modal",
            "title": title,
            "children": children.iter().map(ui_to_json).collect::<Vec<_>>(),
        }),
        UIComponent::Navbar { title } => json!({ "type": "navbar", "title": title }),
        UIComponent::Footer { content } => json!({ "type": "footer", "content": content }),
    }
}

/// Ekspresi AST → JSON ber-typed (bagian dari representasi dokumen penuh).
fn expr_to_json(e: &crate::ast::Expr) -> serde_json::Value {
    use crate::ast::Expr;
    match e {
        Expr::Num(n) => json!({ "num": n }),
        Expr::Str(s) => json!({ "str": s }),
        Expr::Bool(b) => json!({ "bool": b }),
        Expr::Tuple(items) => {
            json!({ "tuple": items.iter().map(expr_to_json).collect::<Vec<_>>() })
        }
        Expr::List(items) => {
            json!({ "list": items.iter().map(expr_to_json).collect::<Vec<_>>() })
        }
        Expr::Map(pairs) => json!({
            "map": pairs.iter().map(|(k, v)| json!({
                "key": k,
                "value": expr_to_json(v),
            })).collect::<Vec<_>>(),
        }),
        Expr::Ident(name) => json!({ "ident": name }),
        Expr::Call { name, args, props } => json!({
            "call": name,
            "args": args.iter().map(expr_to_json).collect::<Vec<_>>(),
            "props": props.as_ref().map(|ps| ps.iter().map(|p| json!({
                "name": p.name,
                "value": expr_to_json(&p.value),
            })).collect::<Vec<_>>()),
        }),
        Expr::UnaryMinus(inner) => json!({ "unary_minus": expr_to_json(inner) }),
        Expr::Binary { op, lhs, rhs } => json!({
            "binary": format!("{op:?}"),
            "lhs": expr_to_json(lhs),
            "rhs": expr_to_json(rhs),
        }),
    }
}

/// Statement AST → JSON ber-typed (representasi dokumen penuh).
fn stmt_to_json(s: &crate::ast::Stmt) -> serde_json::Value {
    use crate::ast::Stmt;
    match s {
        Stmt::Let { name, value } => json!({ "let": name, "value": expr_to_json(value) }),
        Stmt::LetDestructure { names, value } => {
            json!({ "let_destructure": names, "value": expr_to_json(value) })
        }
        Stmt::Assign { name, value } => json!({ "assign": name, "value": expr_to_json(value) }),
        Stmt::ExprStmt(e) => json!({ "expr": expr_to_json(e) }),
        Stmt::Return(e) => json!({ "return": expr_to_json(e) }),
        Stmt::Block(body) => {
            json!({ "block": body.iter().map(stmt_to_json).collect::<Vec<_>>() })
        }
        Stmt::If { cond, then_branch, else_branch } => json!({
            "if": expr_to_json(cond),
            "then": then_branch.iter().map(stmt_to_json).collect::<Vec<_>>(),
            "else": else_branch.iter().map(stmt_to_json).collect::<Vec<_>>(),
        }),
        Stmt::While { cond, body } => json!({
            "while": expr_to_json(cond),
            "body": body.iter().map(stmt_to_json).collect::<Vec<_>>(),
        }),
        Stmt::For { var, start, end, body } => json!({
            "for": var,
            "start": expr_to_json(start),
            "end": expr_to_json(end),
            "body": body.iter().map(stmt_to_json).collect::<Vec<_>>(),
        }),
        Stmt::Match { subject, arms } => json!({
            "match": expr_to_json(subject),
            "arms": arms.iter().map(|a| json!({
                "pattern": format!("{:?}", a.pattern),
                "body": a.body.iter().map(stmt_to_json).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        }),
        Stmt::Navigate { path } => json!({ "navigate": path }),
        Stmt::SetLocale { locale } => json!({ "set_locale": locale }),
        Stmt::Directive { name, args } => json!({
            "directive": name,
            "args": args.iter().map(expr_to_json).collect::<Vec<_>>(),
        }),
    }
}

/// Ekuivalen HTML+JS deterministik (naive namun realistis) — perkiraan byte
/// situs web penuh (CSS + createElement JS + scene three.js) yang harus
/// dihasilkan LLM bila scene ditulis sebagai HTML/JS manual.
pub fn html_equivalent(program: &Program) -> String {
    let css = "<style>body{margin:0;font-family:system-ui,sans-serif}.container{display:flex;gap:8px;padding:8px}.card{background:#fff;border:1px solid #ddd;border-radius:8px;padding:16px}.modal{position:fixed;inset:0;display:flex;align-items:center;justify-content:center;background:rgba(0,0,0,.5)}nav{background:#1e293b;color:#fff;padding:12px 16px}footer{background:#0f172a;color:#94a3b8;padding:12px;text-align:center}button{padding:8px 16px;border-radius:6px;border:0;background:#2563eb;color:#fff;cursor:pointer}input{padding:8px;border:1px solid #ccc;border-radius:6px}</style>";

    let mut body = String::from("<body><div id=\"app\">");
    for item in &program.items {
        if let TopLevel::UILayout(l) = item {
            body.push_str(&format!("<section id=\"layout-{}\">", l.name));
            ui_to_html(&l.root, &mut body);
            body.push_str("</section>");
        }
    }
    body.push_str("</div>");

    // JS DOM builder verbose (createElement per node) — ekuivalen realistis
    // yang dihasilkan LLM bila tidak memakai ADILang.
    let mut js = String::from("<script>const app=document.getElementById('app');");
    for item in &program.items {
        match item {
            TopLevel::UILayout(l) => {
                js.push_str(&format!(
                    "const s{}=document.createElement('section');s{}.id='layout-{}';",
                    l.name.replace(' ', ""),
                    l.name.replace(' ', ""),
                    l.name
                ));
                ui_to_js(&l.root, &mut js);
                js.push_str(&format!("app.appendChild(s{});", l.name.replace(' ', "")));
            }
            TopLevel::Spatial3D(s) | TopLevel::World(s) => {
                js.push_str(&format!(
                    "const scene=this.scene||new THREE.Scene();const camera=new THREE.PerspectiveCamera(55,innerWidth/innerHeight,0.1,100);camera.position.set(0,1.6,7);camera.lookAt(0,0,0);const light=new THREE.PointLight(0xfff2e6,1.5,100);light.position.set(5,6,4);scene.add(light);"
                ));
                for it in &s.items {
                    if let crate::ast::SpatialItem::Entity(e) = it {
                        js.push_str(&format!(
                            "const m{}_g=new THREE.BoxGeometry(1,1,1);const m{}_m=new THREE.MeshStandardMaterial({{color:0xff3333}});const m{}=new THREE.Mesh(m{}_g,m{}_m);m{}.position.set(0,0,0);scene.add(m{});",
                            e.id, e.id, e.id, e.id, e.id, e.id, e.id
                        ));
                    }
                }
            }
            _ => {}
        }
    }
    js.push_str("</script>");

    format!("<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>{}</title>{css}</head>{body}{js}</body></html>", program.name)
}

fn ui_to_html(ui: &UIComponent, out: &mut String) {
    match ui {
        UIComponent::Container { flex, children } => {
            let dir = match flex {
                Some(FlexDirection::Row) => "row",
                Some(FlexDirection::Column) => "column",
                None => "auto",
            };
            out.push_str(&format!(
                "<div class=\"container\" style=\"flex-direction:{dir}\">"
            ));
            for c in children {
                ui_to_html(c, out);
            }
            out.push_str("</div>");
        }
        UIComponent::Text { content } => out.push_str(&format!("<span>{content}</span>")),
        UIComponent::Button { label, onClick } => {
            out.push_str(&format!(
                "<button onclick=\"{}\">{}</button>",
                onClick.as_deref().unwrap_or(""),
                label
            ));
        }
        UIComponent::Input { name, placeholder, .. } => {
            out.push_str(&format!(
                "<input name=\"{name}\" placeholder=\"{}\">",
                placeholder.as_deref().unwrap_or("")
            ));
        }
        UIComponent::Card { title, children } => {
            out.push_str("<div class=\"card\">");
            if let Some(t) = title {
                out.push_str(&format!("<h3>{t}</h3>"));
            }
            for c in children {
                ui_to_html(c, out);
            }
            out.push_str("</div>");
        }
        UIComponent::Modal { title, children } => {
            out.push_str("<div class=\"modal\">");
            if let Some(t) = title {
                out.push_str(&format!("<h3>{t}</h3>"));
            }
            for c in children {
                ui_to_html(c, out);
            }
            out.push_str("</div>");
        }
        UIComponent::Navbar { title } => {
            out.push_str(&format!("<nav><span>{}</span></nav>", title.as_deref().unwrap_or("")));
        }
        UIComponent::Footer { content } => out.push_str(&format!("<footer>{content}</footer>")),
    }
}

/// JS verbose setara ui_to_html (createElement per node) — ekuivalen realistis.
fn ui_to_js(ui: &UIComponent, out: &mut String) {
    match ui {
        UIComponent::Container { flex, children } => {
            let dir = match flex {
                Some(FlexDirection::Row) => "row",
                Some(FlexDirection::Column) => "column",
                None => "auto",
            };
            out.push_str(&format!(
                "const c=document.createElement('div');c.className='container';c.style.display='flex';c.style.flexDirection='{dir}';"
            ));
            for child in children {
                ui_to_js(child, out);
            }
            out.push_str("const parent=c.lastElementChild||c;app.appendChild(c);");
        }
        UIComponent::Text { content } => {
            out.push_str(&format!(
                "const t=document.createElement('span');t.textContent='{content}';c.appendChild(t);"
            ));
        }
        UIComponent::Button { label, onClick } => {
            let handler = onClick.as_deref().unwrap_or("noop");
            out.push_str(&format!(
                "const b=document.createElement('button');b.textContent='{label}';b.onclick=()=>{{{handler}()}};c.appendChild(b);"
            ));
        }
        UIComponent::Input { name, placeholder, .. } => {
            out.push_str(&format!(
                "const i=document.createElement('input');i.name='{name}';i.placeholder='{}';c.appendChild(i);",
                placeholder.as_deref().unwrap_or("")
            ));
        }
        UIComponent::Card { title, children } => {
            out.push_str("<div class=\"card\">");
            if let Some(t) = title {
                out.push_str(&format!("<h3>{t}</h3>"));
            }
            for child in children {
                ui_to_js(child, out);
            }
            out.push_str("</div>");
        }
        UIComponent::Modal { title, children } => {
            out.push_str("<div class=\"modal\">");
            if let Some(t) = title {
                out.push_str(&format!("<h3>{t}</h3>"));
            }
            for child in children {
                ui_to_js(child, out);
            }
            out.push_str("</div>");
        }
        UIComponent::Navbar { title } => {
            out.push_str(&format!(
                "const n=document.createElement('nav');n.textContent='{}';app.appendChild(n);",
                title.as_deref().unwrap_or("")
            ));
        }
        UIComponent::Footer { content } => {
            out.push_str(&format!(
                "const f=document.createElement('footer');f.textContent='{content}';app.appendChild(f);"
            ));
        }
    }
}

/// Persentase penghematan byte (0..=100). 0 bila tidak ada penghematan.
pub fn savings_percent(original: usize, dense: usize) -> usize {
    if original <= dense {
        return 0;
    }
    (original - dense) * 100 / original
}

/// Laporan ukuran Dense vs ekuivalen HTML/JS/JSON (JSON). Menunjukkan
/// penghematan token yang dicapai kompresi AST ke bitstream.
pub fn size_report(src: &str) -> Result<String, String> {
    let program = parse(src)?;
    let dense = encode_program(&program)?;
    let json_str = json_equivalent(&program);
    let html_str = html_equivalent(&program);
    let src_bytes = src.len();
    let dense_bytes = dense.len();
    let json_bytes = json_str.len();
    let html_bytes = html_str.len();
    Ok(json!({
        "status": "ok",
        "program": program.name,
        "source_bytes": src_bytes,
        "dense_bytes": dense_bytes,
        "json_equivalent_bytes": json_bytes,
        "html_equivalent_bytes": html_bytes,
        "savings_vs_json_percent": savings_percent(json_bytes, dense_bytes),
        "savings_vs_html_percent": savings_percent(html_bytes, dense_bytes),
    })
    .to_string())
}

// ═══════════════════════════════════════════════════════════════════════════
// TES
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
        ui_layout "main" {
            navbar "ADI App"
            container {
                flex column
                card "Panel" {
                    text "Hello ADILang"
                    button "Submit" onClick submit
                    input "username" placeholder "Enter name"
                }
                text "Status: Active"
            }
            footer "© 2026 ADI"
        }
        spatial_3d "scene" {
            camera "cam" { pos (0 1.6 7) look (0 0 0) fov 55 }
            light "key" { type point pos (5 6 4) color (1 0.95 0.9) intensity 1.5 }
            entity "cube" {
                pos (0 0 0)
                mesh box { size 1 }
                material solid (0.9 0.1 0.1) 1
            }
            entity "halo" {
                pos (1.5 0.8 0)
                mesh torus { radius 0.6 tube 0.08 segments 3 }
                material wire (0.15 0.8 1) 0.9
            }
        }
    "#;

    #[test]
    fn dense_roundtrip_ast_identik() {
        let program = parse(SAMPLE).expect("parse ok");
        let bin = encode_program(&program).expect("encode ok");
        let dec = decode_program(bin).expect("decode ok");
        assert_eq!(program, dec, "dense roundtrip harus identik");
    }

    #[test]
    fn dense_opcode_map_lengkap() {
        let spec = dense_spec();
        assert!(spec.contains("0x01 NodeContainer"), "harus memuat NodeContainer 0x01");
        assert!(spec.contains("0x0A WebGLMeshCube"), "harus memuat WebGLMeshCube 0x0A");
        assert!(spec.contains("0x11 NodeCamera"), "harus memuat NodeCamera");
        assert!(spec.contains("0x13 NodeEntity"), "harus memuat NodeEntity");
    }

    #[test]
    fn dense_opcode_mesh_benar() {
        assert_eq!(opcode_of_mesh(MeshKind::Box), DENSE_MESH_CUBE);
        assert_eq!(opcode_of_mesh(MeshKind::Sphere), DENSE_MESH_SPHERE);
        assert_eq!(opcode_of_ui(&UIComponent::Button { label: "x".into(), onClick: None }), DENSE_NODE_BUTTON);
    }

    #[test]
    fn dense_size_report_penghematan_lebih_dari_80_persen() {
        let report = serde_json::from_str::<serde_json::Value>(&size_report(SAMPLE).expect("report ok"))
            .expect("json valid");
        assert!(report["savings_vs_html_percent"].as_u64().unwrap() >= 80);
        assert!(report["savings_vs_json_percent"].as_u64().unwrap() >= 80);
        assert!(report["dense_bytes"].as_u64().unwrap() < report["source_bytes"].as_u64().unwrap());
    }

    #[test]
    fn dense_histogram_berisi_opcode() {
        let program = parse(SAMPLE).expect("parse ok");
        let bin = encode_program(&program).expect("encode ok");
        let hist = serde_json::from_str::<serde_json::Value>(&opcode_histogram(&bin)).expect("json valid");
        assert!(hist["total_bytes"].as_u64().unwrap() > 0);
        let names = hist["opcodes"]
            .as_array()
            .map(|a| a.iter().map(|o| o["name"].as_str().unwrap_or("")).collect::<Vec<_>>())
            .unwrap_or_default();
        assert!(names.iter().any(|n| *n == "NodeContainer"), "histogram harus memuat NodeContainer: {names:?}");
    }
}
