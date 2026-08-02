// ADILang — API WASM (wasm-bindgen) + wiring WebGL2.
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).

use std::cell::RefCell;
use std::sync::Mutex;
use wasm_bindgen::prelude::*;

use crate::ast;
use crate::engine::Engine;
use crate::parser;
use crate::state::StateStore;

static ENGINE: Mutex<Option<Engine>> = Mutex::new(None);

// Snapshot source ADILang terakhir yang di-load — dipakai lifecycle hooks
// (adilang_run_lifecycle) untuk menjalankan interpreter tanpa renderer.
static SOURCE: Mutex<Option<String>> = Mutex::new(None);

// State reaktif (adilang_state) — thread_local agar kompatibel dengan
// wasm single-thread & callback non-Send.
thread_local! {
    static STORE: RefCell<StateStore> = RefCell::new(StateStore::new());
}

// Telemetry (adilang_analytics) — thread_local singleton per halaman.
thread_local! {
    static ANALYTICS: RefCell<crate::analytics::Analytics> =
        RefCell::new(crate::analytics::Analytics::new());
}

fn performance_now_ms() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now())
        .unwrap_or(0.0)
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
    #[wasm_bindgen(js_namespace = console)]
    fn warn(s: &str);
}

fn with_engine<R>(f: impl FnOnce(&mut Engine) -> Result<R, String>) -> Result<R, String> {
    let mut guard = ENGINE
        .lock()
        .map_err(|_| "Engine mutex poisoned".to_string())?;
    match guard.as_mut() {
        Some(e) => f(e),
        None => Err("Engine belum di-init (panggil adilang_start).".to_string()),
    }
}

/// Inisialisasi engine + kanvas WebGL2, lalu mulai loop render.
#[wasm_bindgen]
pub fn adilang_start(canvas_id: &str) -> Result<(), String> {
    let document = web_sys::window()
        .ok_or("No window")?
        .document()
        .ok_or("No document")?;
    let canvas: web_sys::HtmlCanvasElement = document
        .get_element_by_id(canvas_id)
        .ok_or("Canvas tidak ditemukan")?
        .dyn_into()
        .map_err(|_| "Bukan HtmlCanvasElement")?;

    let mut engine = Engine::new(canvas.clone())?;

    // Resize awal
    let (w, h) = canvas_dimensions(&canvas);
    engine.resize(w, h);

    // Suntik world default jika belum ada script
    let world_src = get_default_world();
    let program = parser::parse(&world_src)?;
    engine.interp.load(program)?;
    engine.build_meshes()?;

    let mut guard = ENGINE.lock().map_err(|_| "Engine mutex poisoned")?;
    *guard = Some(engine);
    drop(guard);

    start_loop(canvas_id)
}

fn canvas_dimensions(canvas: &web_sys::HtmlCanvasElement) -> (u32, u32) {
    let w = canvas.client_width().max(1) as u32;
    let h = canvas.client_height().max(1) as u32;
    (w, h)
}

fn start_loop(canvas_id: &str) -> Result<(), String> {
    let window = web_sys::window().ok_or("No window")?;
    let document = window.document().ok_or("No document")?;
    let canvas: web_sys::HtmlCanvasElement = document
        .get_element_by_id(canvas_id)
        .ok_or("Canvas tidak ditemukan")?
        .dyn_into()
        .map_err(|_| "Bukan HtmlCanvasElement")?;

    // RAF loop — pola standar wasm-bindgen: Rc + RefCell + forget untuk lifetime statis
    let f = std::rc::Rc::new(std::cell::RefCell::new(None::<Closure<dyn FnMut(f64)>>));
    let g = f.clone();
    {
        let g2 = g.clone();
        *g.borrow_mut() = Some(Closure::wrap(Box::new(move |now: f64| {
            let result = with_engine(|e| {
                e.interp.t = timestamp_seconds();
                let _ = e.run_frame_handlers();
                let t_render = performance_now_ms();
                e.render();
                let render_ms = performance_now_ms() - t_render;
                ANALYTICS.with(|a| a.borrow_mut().record_frame(now, render_ms));
                Ok::<(), String>(())
            });
            if let Err(err) = result {
                warn(&format!("frame error: {err}"));
                ANALYTICS.with(|a| a.borrow_mut().record_error());
            }
            if let Some(window) = web_sys::window() {
                let cb = g2.borrow();
                if let Some(c) = cb.as_ref() {
                    let _ = window.request_animation_frame(c.as_ref().unchecked_ref());
                }
            }
        }) as Box<dyn FnMut(f64)>));
    }

    // request frame pertama; keepalive Rc (forget) agar closure tak pernah dibebaskan
    if let Some(cb) = &*f.borrow() {
        window
            .request_animation_frame(cb.as_ref().unchecked_ref())
            .map_err(|e| format!("request_animation_frame: {e:?}"))?;
    }
    std::mem::forget(f);

    // resize listener
    {
        let canvas_for_resize = canvas.clone();
        let resize_cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(
            move |_: web_sys::Event| {
                let (w, h) = canvas_dimensions(&canvas_for_resize);
                let _ = with_engine(|e| {
                    e.resize(w, h);
                    Ok::<(), String>(())
                });
            },
        ));
        window.set_onresize(Some(resize_cb.as_ref().unchecked_ref()));
        resize_cb.forget();
    }

    // pointer move
    {
        let canvas_for_move = canvas.clone();
        let pointer_cb = Closure::<dyn FnMut(web_sys::PointerEvent)>::wrap(Box::new(
            move |ev: web_sys::PointerEvent| {
                let w = canvas_for_move.client_width().max(1) as f64;
                let h = canvas_for_move.client_height().max(1) as f64;
                let mx = (ev.client_x() as f64 / w) * 2.0 - 1.0;
                let my = -((ev.client_y() as f64 / h) * 2.0 - 1.0);
                let _ = with_engine(|e| {
                    e.interp.mouse_x = mx;
                    e.interp.mouse_y = my;
                    Ok::<(), String>(())
                });
            },
        ));
        canvas.set_onpointermove(Some(pointer_cb.as_ref().unchecked_ref()));
        pointer_cb.forget();
    }

    // click
    {
        let click_cb = Closure::<dyn FnMut(web_sys::PointerEvent)>::wrap(Box::new(
            move |_: web_sys::PointerEvent| {
                ANALYTICS.with(|a| a.borrow_mut().record_action());
                let _ = with_engine(|e| e.fire_event(ast::EventKind::Click));
            },
        ));
        canvas.set_onpointerdown(Some(click_cb.as_ref().unchecked_ref()));
        click_cb.forget();
    }

    Ok(())
}

/// Muat ulang world dari source ADILang (hot reload).
#[wasm_bindgen]
pub fn adilang_load(source: &str) -> Result<(), String> {
    with_engine(|e| {
        let program = parser::parse(source)?;
        e.interp.load(program)?;
        e.build_meshes()?;
        ANALYTICS.with(|a| a.borrow_mut().record_load());
        Ok::<(), String>(())
    })?;
    if let Ok(mut s) = SOURCE.lock() {
        *s = Some(source.to_string());
    }
    Ok(())
}

/// Trigger event speak (ADI mulai bicara).
#[wasm_bindgen]
pub fn adilang_speak() -> Result<(), String> {
    ANALYTICS.with(|a| a.borrow_mut().record_speak());
    with_engine(|e| e.fire_event(ast::EventKind::Speak))
}

/// Trigger event silent (ADI selesai bicara).
#[wasm_bindgen]
pub fn adilang_silent() -> Result<(), String> {
    ANALYTICS.with(|a| a.borrow_mut().record_silent());
    with_engine(|e| e.fire_event(ast::EventKind::Silent))
}

/// Verifikasi sintaks ADILang tanpa menjalankan.
#[wasm_bindgen]
pub fn adilang_check(source: &str) -> Result<(), String> {
    parser::parse(source)?;
    Ok(())
}

/// Static analyzer ADILang (v1.7.0, roadmap §3 "adilang-check").
/// Mengembalikan teks diagnostik baris-per-temuan berformat
/// `severity|line|message|hint` (Error hanya bila syntax error → Err).
/// Frontend/AI bisa memakainya untuk validasi sebelum eksekusi & sebagai
/// sumber event "syntax_error".
#[wasm_bindgen]
pub fn adilang_check_diagnostics(source: &str) -> Result<String, String> {
    let diags = crate::checker::check_src(source)?;
    if diags.is_empty() {
        return Ok("OK|0|bersih|tidak ada temuan".to_string());
    }
    let lines: Vec<String> = diags
        .iter()
        .map(|d| {
            format!(
                "{}|{}|{}|{}",
                d.severity.as_str(),
                d.line,
                d.message,
                d.hint
            )
        })
        .collect();
    Ok(lines.join("\n"))
}

/// Token compactor ADILang (v1.7.0, roadmap §3 "adilang-opt"): rename nama
/// user ke 1–2 karakter + render ulang kompak (hemat token antar-agen LLM).
/// Semantik dipertahankan (AST identik — lihat unit test compactor).
#[wasm_bindgen]
pub fn adilang_optimize(source: &str) -> Result<String, String> {
    crate::compactor::optimize_src(source)
}

/// Debug: hitung jumlah entity setelah load.
#[wasm_bindgen]
pub fn adilang_debug_count() -> usize {
    with_engine(|e| Ok(e.interp.world.entities.len())).unwrap_or(0)
}

#[wasm_bindgen]
pub fn adilang_version() -> String {
    format!("ADILang v{} (Rust → WASM → WebGL2)", crate::registry::VERSION)
}

/// Enumerasi kosakata tertutup ADILang (P6 self-describing) — AI bisa
/// membaca seluruh registry bahasa tanpa membuka docs.
#[wasm_bindgen]
pub fn adilang_registry() -> String {
    crate::registry::registry_text()
}

/// Encode snapshot entity world saat ini → bytecode ADILang FULL (v1.4.0).
/// Untuk transport real-time multiplayer antar-client via WebSocket.
#[wasm_bindgen]
pub fn adilang_binary_encode_full() -> Result<Vec<u8>, String> {
    with_engine(|e| {
        let entities: Vec<crate::scene::EntityState> = e.interp.world.entities.clone();
        crate::bytecode::encode_full(&entities)
    })
}

/// Encode perubahan frame terakhir → bytecode DELTA (mask-based).
/// Baseline dikirim sebagai `prev_full` (bytecode FULL yang sudah dimiliki
/// client). Mengembalikan None (error) bila struktur berubah → kirim FULL.
#[wasm_bindgen]
pub fn adilang_binary_encode_delta(prev_full: &[u8]) -> Result<Vec<u8>, String> {
    let prev = crate::bytecode::decode_full(prev_full)?;
    with_engine(|e| {
        let current: Vec<crate::scene::EntityState> = e.interp.world.entities.clone();
        crate::bytecode::encode_delta(&prev, &current)
            .ok_or_else(|| "Struktur world berubah — kirim FULL snapshot".to_string())
    })
}

/// Decode bytecode FULL → teks deskriptif (debug/verifikasi dari JS).
#[wasm_bindgen]
pub fn adilang_binary_decode_full(bytes: &[u8]) -> Result<String, String> {
    let ents = crate::bytecode::decode_full(bytes)?;
    let mut lines = Vec::new();
    for e in &ents {
        lines.push(format!(
            "{} mesh={:?} mat={:?} pos=({:.2} {:.2} {:.2})",
            e.id,
            e.mesh,
            e.material,
            e.transform.pos[0],
            e.transform.pos[1],
            e.transform.pos[2]
        ));
    }
    Ok(format!(
        "ADILangBinary FULL v{} — {} entity\n{}",
        crate::bytecode::packet_version(bytes),
        ents.len(),
        lines.join("\n")
    ))
}

/// Spesifikasi format bytecode ADILang (untuk registry/docs/AI).
#[wasm_bindgen]
pub fn adilang_binary_spec() -> String {
    crate::bytecode::binary_spec()
}

// ═══════════════════════════════════════════════════════════════════════════
// PARSE & RENDER (v2.0.0 — multi-domain)
// ═══════════════════════════════════════════════════════════════════════════

/// Parse ADILang code dan render ke target element (2D UI + 3D canvas).
/// 
/// Args:
///   code: ADILang source code (bisa berisi @payload, ui_layout, spatial_3d/world)
///   target_element_id: ID elemen DOM yang akan di-render
/// 
/// Returns:
///   JSON string dengan status dan metadata render
#[wasm_bindgen]
pub fn adilang_parse_and_render(code: &str, target_element_id: &str) -> Result<String, String> {
    use crate::ast::*;
    use crate::parser::parse;
    
    let program = parse(code)?;
    
    let document = web_sys::window()
        .ok_or("No window")?
        .document()
        .ok_or("No document")?;
    
    let target = document
        .get_element_by_id(target_element_id)
        .ok_or_else(|| format!("Target element '{}' tidak ditemukan", target_element_id))?;
    
    // Bersihkan target
    target.set_inner_html("");
    
    let mut rendered_2d = false;
    let mut rendered_3d = false;
    let mut payload = None;
    
    for item in &program.items {
        match item {
            TopLevel::Payload(p) => {
                payload = Some(serde_json::json!({
                    "sender": p.sender,
                    "target_agent": p.target_agent,
                    "intent": p.intent,
                    "state_data": p.state_data.as_ref().map(render_expr_to_json),
                }));
            }
            TopLevel::UILayout(layout) => {
                render_ui_layout(&document, &target, &layout.root)?;
                rendered_2d = true;
            }
            TopLevel::Spatial3D(_) | TopLevel::World(_) => {
                // 3D rendering requires WebGL2 canvas — delegate to engine
                // For now, mark as available
                rendered_3d = true;
            }
            _ => {}
        }
    }
    
    let result = serde_json::json!({
        "status": "ok",
        "rendered_2d": rendered_2d,
        "rendered_3d": rendered_3d,
        "payload": payload,
        "blocks_parsed": program.items.len(),
    });
    
    Ok(result.to_string())
}

/// Export agent payload dari ADILang code sebagai JSON string.
/// 
/// Args:
///   code: ADILang source code yang mengandung @payload block
/// 
/// Returns:
///   JSON string dengan payload inter-AI
#[wasm_bindgen]
pub fn adilang_export_agent_payload(code: &str) -> Result<String, String> {
    use crate::ast::*;
    use crate::parser::parse;
    
    let program = parse(code)?;
    
    for item in &program.items {
        if let TopLevel::Payload(p) = item {
            let payload = serde_json::json!({
                "sender": p.sender,
                "target_agent": p.target_agent,
                "intent": p.intent,
                "state_data": p.state_data.as_ref().map(render_expr_to_json),
            });
            return Ok(payload.to_string());
        }
    }
    
    Err("Tidak ada @payload block dalam kode ADILang".to_string())
}

/// Update state internal dari JSON string.
/// 
/// Args:
///   json_state: JSON string dengan state baru
/// 
/// Returns:
///   "ok" jika berhasil
#[wasm_bindgen]
pub fn adilang_update_state(json_state: &str) -> Result<String, String> {
    let value = serde_json::from_str(json_state).map_err(|e| format!("JSON parse error: {}", e))?;
    STORE.with(|s| s.borrow_mut().load_json(&value));
    Ok("ok".to_string())
}

/// Ambil state internal sebagai JSON string.
/// 
/// Returns:
///   JSON string snapshot StateStore (adilang_state)
#[wasm_bindgen]
pub fn adilang_get_state() -> String {
    STORE.with(|s| s.borrow().snapshot_json().to_string())
}

// ═══════════════════════════════════════════════════════════════════════════
// 2D DOM RENDERING BRIDGE
// ═══════════════════════════════════════════════════════════════════════════

fn render_ui_layout(
    document: &web_sys::Document,
    target: &web_sys::Element,
    root: &crate::ast::UIComponent,
) -> Result<(), String> {
    let container = document
        .create_element("div")
        .map_err(|e| format!("create_element: {:?}", e))?;
    container.set_class_name("adilang-ui-layout");
    render_ui_component(document, &container, root)?;
    target.append_child(&container).map_err(|e| format!("append_child: {:?}", e))?;
    Ok(())
}

fn render_ui_component(
    document: &web_sys::Document,
    parent: &web_sys::Element,
    comp: &crate::ast::UIComponent,
) -> Result<(), String> {
    match comp {
        crate::ast::UIComponent::Container { flex, children } => {
            let el = document.create_element("div").map_err(|e| format!("{:?}", e))?;
            el.set_class_name("adilang-container");
            if let Some(f) = flex {
                let style_str = match f {
                    crate::ast::FlexDirection::Row => "display: flex; flex-direction: row;",
                    crate::ast::FlexDirection::Column => "display: flex; flex-direction: column;",
                };
                el.set_attribute("style", style_str).map_err(|e| format!("{:?}", e))?;
            }
            for child in children {
                render_ui_component(document, &el, child)?;
            }
            parent.append_child(&el).map_err(|e| format!("{:?}", e))?;
            Ok(())
        }
        crate::ast::UIComponent::Text { content } => {
            let el = document.create_element("span").map_err(|e| format!("{:?}", e))?;
            el.set_class_name("adilang-text");
            el.set_text_content(Some(content));
            parent.append_child(&el).map_err(|e| format!("{:?}", e))?;
            Ok(())
        }
        crate::ast::UIComponent::Button { label, onClick } => {
            let el = document.create_element("button").map_err(|e| format!("{:?}", e))?;
            el.set_class_name("adilang-button");
            el.set_text_content(Some(label));
            if let Some(handler) = onClick {
                let handler_name = handler.clone();
                let cb = Closure::wrap(Box::new(move |_ev: web_sys::MouseEvent| {
                    let _ = with_engine(|e| e.fire_event(crate::ast::EventKind::Click));
                    log(&format!("Button '{}' clicked", handler_name));
                }) as Box<dyn FnMut(web_sys::MouseEvent)>);
                let btn_el = el.clone().dyn_into::<web_sys::HtmlButtonElement>().map_err(|e| format!("{:?}", e))?;
                btn_el.set_onclick(Some(cb.as_ref().unchecked_ref()));
                cb.forget();
            }
            parent.append_child(&el).map_err(|e| format!("{:?}", e))?;
            Ok(())
        }
        crate::ast::UIComponent::Input { name, placeholder, bind, validate } => {
            let el = document.create_element("input").map_err(|e| format!("{:?}", e))?;
            el.set_class_name("adilang-input");
            el.set_attribute("name", name).map_err(|e| format!("{:?}", e))?;
            if let Some(p) = placeholder {
                el.set_attribute("placeholder", p).map_err(|e| format!("{:?}", e))?;
            }
            if let Some(b) = bind {
                el.set_attribute("data-bind", b).map_err(|e| format!("{:?}", e))?;
            }
            if let Some(v) = validate {
                el.set_attribute("data-validate", v).map_err(|e| format!("{:?}", e))?;
            }
            parent.append_child(&el).map_err(|e| format!("{:?}", e))?;
            Ok(())
        }
        crate::ast::UIComponent::Card { title, children } => {
            let el = document.create_element("div").map_err(|e| format!("{:?}", e))?;
            el.set_class_name("adilang-card");
            if let Some(t) = title {
                let h = document.create_element("h3").map_err(|e| format!("{:?}", e))?;
                h.set_class_name("adilang-card-title");
                h.set_text_content(Some(t));
                el.append_child(&h).map_err(|e| format!("{:?}", e))?;
            }
            for child in children {
                render_ui_component(document, &el, child)?;
            }
            parent.append_child(&el).map_err(|e| format!("{:?}", e))?;
            Ok(())
        }
        crate::ast::UIComponent::Modal { title, children } => {
            let el = document.create_element("div").map_err(|e| format!("{:?}", e))?;
            el.set_class_name("adilang-modal");
            if let Some(t) = title {
                let h = document.create_element("h3").map_err(|e| format!("{:?}", e))?;
                h.set_class_name("adilang-modal-title");
                h.set_text_content(Some(t));
                el.append_child(&h).map_err(|e| format!("{:?}", e))?;
            }
            for child in children {
                render_ui_component(document, &el, child)?;
            }
            parent.append_child(&el).map_err(|e| format!("{:?}", e))?;
            Ok(())
        }
        crate::ast::UIComponent::Navbar { title } => {
            let el = document.create_element("nav").map_err(|e| format!("{:?}", e))?;
            el.set_class_name("adilang-navbar");
            if let Some(t) = title {
                el.set_text_content(Some(t));
            }
            parent.append_child(&el).map_err(|e| format!("{:?}", e))?;
            Ok(())
        }
        crate::ast::UIComponent::Footer { content } => {
            let el = document.create_element("footer").map_err(|e| format!("{:?}", e))?;
            el.set_class_name("adilang-footer");
            el.set_text_content(Some(content));
            parent.append_child(&el).map_err(|e| format!("{:?}", e))?;
            Ok(())
        }
    }
}

fn render_expr_to_json(expr: &crate::ast::Expr) -> serde_json::Value {
    use crate::ast::*;
    match expr {
        Expr::Num(n) => serde_json::json!(n),
        Expr::Str(s) => serde_json::json!(s),
        Expr::Bool(b) => serde_json::json!(b),
        Expr::Tuple(items) => serde_json::json!(items.iter().map(render_expr_to_json).collect::<Vec<_>>()),
        Expr::List(items) => serde_json::json!(items.iter().map(render_expr_to_json).collect::<Vec<_>>()),
        Expr::Map(pairs) => {
            let mut map = serde_json::Map::new();
            for (k, v) in pairs {
                map.insert(k.clone(), render_expr_to_json(v));
            }
            serde_json::Value::Object(map)
        }
        Expr::Ident(s) => serde_json::json!(s),
        Expr::Call { name, args, .. } => {
            serde_json::json!({
                "call": name,
                "args": args.iter().map(render_expr_to_json).collect::<Vec<_>>(),
            })
        }
        Expr::UnaryMinus(inner) => serde_json::json!({"neg": render_expr_to_json(inner)}),
        Expr::Binary { op, lhs, rhs } => {
            let op_str = match op {
                BinOp::Add => "+",
                BinOp::Sub => "-",
                BinOp::Mul => "*",
                BinOp::Div => "/",
                BinOp::Mod => "%",
                BinOp::Eq => "==",
                BinOp::Ne => "!=",
                BinOp::Lt => "<",
                BinOp::Gt => ">",
                BinOp::Le => "<=",
                BinOp::Ge => ">=",
            };
            serde_json::json!({
                "op": op_str,
                "lhs": render_expr_to_json(lhs),
                "rhs": render_expr_to_json(rhs),
            })
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// HELPERS
// ═══════════════════════════════════════════════════════════════════════════

fn timestamp_seconds() -> f64 {
    use js_sys::Date;
    Date::now() / 1000.0
}

fn get_default_world() -> &'static str {
    include_str!("../worlds/default.adi")
}

// ═══════════════════════════════════════════════════════════════════════════
// ADILANG PROTOCOL — text transport Base64 (adilang_protocol)
// ═══════════════════════════════════════════════════════════════════════════

/// Parse source ADILang → bytecode biner → Base64 text transport.
/// Untuk kirim program antar-agen AI lewat channel teks (WS/HTTP/prompt).
#[wasm_bindgen]
pub fn adilang_protocol_encode_source_to_b64(code: &str) -> Result<String, String> {
    crate::protocol::encode_source_to_b64(code)
}

/// Decode Base64 text transport → AST → ringkasan JSON.
#[wasm_bindgen]
pub fn adilang_protocol_decode_b64_to_ast(transport: &str) -> Result<String, String> {
    use crate::ast::TopLevel;
    let ast = crate::protocol::decode_b64_to_ast(transport)?;
    let kind = ast.items.first().map(|i| match i {
        TopLevel::Payload(_) => "payload",
        TopLevel::UILayout(_) => "ui_layout",
        TopLevel::Spatial3D(_) => "spatial_3d",
        TopLevel::World(_) => "world",
        TopLevel::Camera(_) => "camera",
        TopLevel::Light(_) => "light",
        TopLevel::Entity(_) => "entity",
        TopLevel::Let { .. } => "let",
        TopLevel::Func(_) => "func",
        TopLevel::Handler(_) => "handler",
        TopLevel::UseJs(_) => "use_js",
        TopLevel::Routes(_) => "routes",
        TopLevel::I18n(_) => "i18n",
        TopLevel::Component(_) => "component",
    });
    let summary = serde_json::json!({
        "name": ast.name,
        "blocks": ast.items.len(),
        "kind": kind,
    });
    Ok(summary.to_string())
}

/// Laporan Zero-Token-Waste (source vs binary vs base64).
#[wasm_bindgen]
pub fn adilang_protocol_size_report(code: &str) -> Result<String, String> {
    crate::protocol::size_report(code)
}

// ═══════════════════════════════════════════════════════════════════════════
// ADILANG SCHEMA — JSON Schema & System Prompt (adilang_schema)
// ═══════════════════════════════════════════════════════════════════════════

/// JSON Schema resmi IR ADILang (draft-07) sebagai string.
#[wasm_bindgen]
pub fn adilang_schema_json() -> Result<String, String> {
    crate::schema::json_schema_string()
}

/// System Prompt lengkap untuk LLM (kosakata tertutup + aturan).
#[wasm_bindgen]
pub fn adilang_schema_prompt() -> String {
    crate::schema::system_prompt()
}

/// System Prompt ringkas untuk konteks terbatas.
#[wasm_bindgen]
pub fn adilang_schema_prompt_compact() -> String {
    crate::schema::system_prompt_compact()
}

// ═══════════════════════════════════════════════════════════════════════════
// ADILANG STATE — reactive engine (adilang_state)
// ═══════════════════════════════════════════════════════════════════════════

/// Set nilai state pada dot-path (JSON value sebagai string).
#[wasm_bindgen]
pub fn adilang_state_set_json(path: &str, json: &str) -> Result<String, String> {
    let value = serde_json::from_str(json).map_err(|e| format!("JSON parse error: {}", e))?;
    let changed = STORE.with(|s| s.borrow_mut().set_json(path, &value))?;
    ANALYTICS.with(|a| a.borrow_mut().record_state_set());
    Ok(if changed { "changed" } else { "unchanged" }.to_string())
}

/// Get nilai state pada dot-path sebagai JSON string ("" bila tidak ada).
#[wasm_bindgen]
pub fn adilang_state_get(path: &str) -> String {
    STORE.with(|s| {
        match s.borrow().get(path) {
            Ok(v) => v.to_json().to_string(),
            Err(_) => String::new(),
        }
    })
}

/// Snapshot seluruh state sebagai JSON string.
#[wasm_bindgen]
pub fn adilang_state_snapshot() -> String {
    STORE.with(|s| s.borrow().snapshot_json().to_string())
}

/// Revisi state terakhir (naik saat ada mutasi nyata).
#[wasm_bindgen]
pub fn adilang_state_revision() -> u64 {
    STORE.with(|s| s.borrow().revision())
}

/// Increment numerik pada dot-path (path dibuat 0.0 bila belum ada).
#[wasm_bindgen]
pub fn adilang_state_incr(path: &str, by: f64) -> Result<f64, String> {
    STORE.with(|s| s.borrow_mut().incr(path, by))
}

/// Apakah perubahan path ini relevan untuk auto re-render (@payload/ui).
#[wasm_bindgen]
pub fn adilang_state_is_render_relevant(path: &str) -> bool {
    crate::state::is_render_relevant(path)
}

// ═══════════════════════════════════════════════════════════════════════════
// ADILANG SPATIAL — procedural 3D & spatial UI (adilang_spatial)
// ═══════════════════════════════════════════════════════════════════════════

/// Generate mesh prosedural tanpa aset eksternal → JSON lengkap
/// (positions/normals/uvs/interleaved/indices) siap upload ke WebGL2.
/// Param bentuk: radius, tube, inner, segments, size, count (0 = default).
#[wasm_bindgen]
pub fn adilang_spatial_generate(
    name: &str,
    radius: f64,
    tube: f64,
    inner: f64,
    segments: f64,
    size: f64,
    count: f64,
) -> Result<String, String> {
    let kind = crate::spatial::ShapeKind::from_str(name)
        .ok_or_else(|| format!("Bentuk '{name}' tidak dikenal (sphere|box|torus|icosa|ring|plane|grid)"))?;
    let params = crate::spatial::ShapeParams {
        radius: radius as f32,
        tube: tube as f32,
        inner: inner as f32,
        segments: segments as f32,
        size: size as f32,
        count: count as f32,
    };
    let m = crate::spatial::generate_shape(kind, &params);
    Ok(serde_json::json!({
        "shape": name,
        "vertexCount": m.vertex_count(),
        "triangleCount": m.triangle_count(),
        "positions": m.positions,
        "normals": m.normals,
        "uvs": m.uvs,
        "interleaved": m.interleaved(),
        "indices": m.indices,
    })
    .to_string())
}

/// Daftar semua bentuk prosedural + statistik (untuk preview/palette).
#[wasm_bindgen]
pub fn adilang_spatial_shapes() -> Result<String, String> {
    let list: Vec<serde_json::Value> = crate::spatial::generate_all()
        .iter()
        .map(|(name, m)| {
            serde_json::json!({
                "shape": name,
                "vertexCount": m.vertex_count(),
                "triangleCount": m.triangle_count(),
            })
        })
        .collect();
    Ok(serde_json::Value::Array(list).to_string())
}

/// Rasterisasi ui_layout dari source ADILang → tekstur RGBA (spatial UI).
/// Hasil: JSON { width, height, data: base64(RGBA) } siap gl.texImage2D
/// dan dipetakan ke permukaan objek 3D.
#[wasm_bindgen]
pub fn adilang_spatial_ui_texture(
    source: &str,
    layout_name: &str,
    width: usize,
    height: usize,
) -> Result<String, String> {
    use crate::ast::TopLevel;
    let program = crate::parser::parse(source)?;
    let layout = program
        .items
        .iter()
        .find_map(|i| match i {
            TopLevel::UILayout(l) if l.name == layout_name => Some(l),
            _ => None,
        })
        .ok_or_else(|| format!("ui_layout '{layout_name}' tidak ditemukan dalam source"))?;
    let tex = crate::spatial::render_layout_to_texture(layout, width, height);
    let data = crate::protocol::b64_encode(&tex.rgba);
    Ok(serde_json::json!({
        "width": tex.width,
        "height": tex.height,
        "data": data,
    })
    .to_string())
}

// ═══════════════════════════════════════════════════════════════════════════
// ADILANG CRDT — multi-agent collaborative state (adilang_crdt)
// ═══════════════════════════════════════════════════════════════════════════
// Register CRDT per path (sel AST). Beberapa agen AI mengedit dokumen yang
// sama bersamaan (AI 'A' → ui_layout, AI 'B' → spatial_3d) tanpa menimpa satu
// sama lain: merge konvergen (LWW), op bisa ditukar antar-agen via snapshot
// atau delta (missing_ops). Semua hasil JSON ringkas untuk hemat token.

thread_local! {
    static CRDT: RefCell<crate::crdt::CrdtState> = RefCell::new(crate::crdt::CrdtState::new());
    static CRDT_CLOCK: RefCell<u64> = const { RefCell::new(0) };
}

fn crdt_next_lamport() -> u64 {
    CRDT_CLOCK.with(|c| {
        let mut clock = c.borrow_mut();
        *clock += 1;
        let current = CRDT.with(|s| s.borrow().max_lamport());
        let n = current.max(*clock);
        *clock = n;
        n
    })
}

/// Set nilai register pada path (lamport dihitung otomatis dari jam lokal).
/// Mengembalikan "applied" bila menang / "rejected" bila kalah (LWW).
#[wasm_bindgen]
pub fn adilang_crdt_set(path: &str, value: &str, agent: &str) -> String {
    let lamport = crdt_next_lamport();
    let op = CRDT.with(|s| s.borrow().make_set(path, value.to_string(), lamport, agent));
    let applied = CRDT.with(|s| s.borrow_mut().apply(&op));
    if applied { "applied".to_string() } else { "rejected".to_string() }
}

/// Hapus register pada path (tombstone) — lamport otomatis.
#[wasm_bindgen]
pub fn adilang_crdt_delete(path: &str, agent: &str) -> String {
    let lamport = crdt_next_lamport();
    let op = CRDT.with(|s| s.borrow().make_delete(path, lamport, agent));
    let applied = CRDT.with(|s| s.borrow_mut().apply(&op));
    if applied { "deleted".to_string() } else { "rejected".to_string() }
}

/// Ambil nilai hidup pada path ("" bila tombstone/absen).
#[wasm_bindgen]
pub fn adilang_crdt_get(path: &str) -> String {
    CRDT.with(|s| {
        s.borrow()
            .get_value(path)
            .unwrap_or("")
            .to_string()
    })
}

/// Snapshot seluruh register → JSON (untuk sinkronisasi antar-agen).
#[wasm_bindgen]
pub fn adilang_crdt_snapshot() -> String {
    CRDT.with(|s| s.borrow().snapshot_string())
}

/// Muat snapshot JSON (replace penuh replica lokal).
#[wasm_bindgen]
pub fn adilang_crdt_load_snapshot(json: &str) -> Result<(), String> {
    CRDT.with(|s| s.borrow_mut().load_snapshot_string(json))
}

/// Merge snapshot replica lain (komutatif & idempotent).
#[wasm_bindgen]
pub fn adilang_crdt_merge(json: &str) -> Result<String, String> {
    let mut other = crate::crdt::CrdtState::new();
    other.load_snapshot_string(json)?;
    let before = CRDT.with(|s| s.borrow().live_count());
    CRDT.with(|s| s.borrow_mut().merge(&other));
    let after = CRDT.with(|s| s.borrow().live_count());
    Ok(serde_json::json!({
        "merged": after >= before,
        "live": after,
    })
    .to_string())
}

/// Statistik live/total register.
#[wasm_bindgen]
pub fn adilang_crdt_count() -> String {
    let (live, total) = CRDT.with(|s| {
        let s = s.borrow();
        (s.live_count(), s.total_count())
    });
    serde_json::json!({ "live": live, "total": total }).to_string()
}

/// Delta: op yang dimiliki replica lain (dari snapshot-nya) tapi belum ada
/// di replica lokal → JSON array op, siap dikirim balik ke si pengirim.
#[wasm_bindgen]
pub fn adilang_crdt_missing_ops(json: &str) -> Result<String, String> {
    let mut other = crate::crdt::CrdtState::new();
    other.load_snapshot_string(json)?;
    let ops = CRDT.with(|s| s.borrow().missing_ops(&other));
    let list: Vec<serde_json::Value> = ops
        .iter()
        .map(|o| {
            serde_json::json!({
                "path": o.path,
                "value": o.value,
                "lamport": o.lamport,
                "agent": o.agent,
            })
        })
        .collect();
    Ok(serde_json::Value::Array(list).to_string())
}

// ═══════════════════════════════════════════════════════════════════════════
// DIFF & PATCH (adilang_diff)
// ═══════════════════════════════════════════════════════════════════════════

/// Identitas format ADILang Patch Script.
#[wasm_bindgen]
pub fn adilang_patch_info() -> String {
    serde_json::json!({
        "kind": crate::diff::PATCH_KIND,
        "version": crate::diff::PATCH_VERSION,
    })
    .to_string()
}

/// Diff dua dokumen ADILang → JSON array op (add/remove/replace) level-blok.
#[wasm_bindgen]
pub fn adilang_diff(src_a: &str, src_b: &str) -> Result<String, String> {
    crate::diff::diff_docs_json(src_a, src_b)
}

/// Terapkan JSON array op ke dokumen → dokumen baru (hanya blok yg di-patch).
#[wasm_bindgen]
pub fn adilang_apply_patch(src: &str, ops_json: &str) -> Result<String, String> {
    crate::diff::apply_doc_json(src, ops_json)
}

/// Parse teks ADILang Patch Script → JSON array op (utk preview/validasi LLM).
#[wasm_bindgen]
pub fn adilang_parse_patch_script(script: &str) -> Result<String, String> {
    crate::diff::parse_patch_script_json(script)
}

/// Terapkan ADILang Patch Script langsung ke dokumen sumber.
#[wasm_bindgen]
pub fn adilang_apply_patch_script(src: &str, script: &str) -> Result<String, String> {
    crate::diff::apply_patch_script(src, script)
}

// ═══════════════════════════════════════════════════════════════════════════
// TELEMETRY & ANALYTICS (adilang_analytics)
// ═══════════════════════════════════════════════════════════════════════════

/// Rekam satu frame (dipanggil RAF loop): frame_ms antar-frame, render_ms
/// durasi render — menggerakkan FPS & statistik render.
#[wasm_bindgen]
pub fn adilang_analytics_record_frame(frame_ms: f64, render_ms: f64) {
    ANALYTICS.with(|a| a.borrow_mut().record_frame(frame_ms, render_ms));
}

/// Rekam event bernama: load / speak / silent / action / state_set / error.
#[wasm_bindgen]
pub fn adilang_analytics_record_event(kind: &str) -> Result<(), String> {
    ANALYTICS.with(|a| {
        let mut a = a.borrow_mut();
        match kind {
            "load" => a.record_load(),
            "speak" => a.record_speak(),
            "silent" => a.record_silent(),
            "action" => a.record_action(),
            "state_set" => a.record_state_set(),
            "error" => a.record_error(),
            other => {
                return Err(format!("analytics: event tidak dikenal '{other}'"));
            }
        }
        Ok(())
    })
}

/// Snapshot telemetry JSON (schema `analytics.snapshot_json`).
#[wasm_bindgen]
pub fn adilang_analytics_snapshot() -> String {
    ANALYTICS.with(|a| a.borrow().snapshot_json())
}

/// Reset semua counter telemetry.
#[wasm_bindgen]
pub fn adilang_analytics_reset() {
    ANALYTICS.with(|a| a.borrow_mut().reset());
}

// ═══════════════════════════════════════════════════════════════════════════
// VISION SNAPSHOT (adilang_capture_viewport_snapshot)
// ═══════════════════════════════════════════════════════════════════════════

/// Snapshot viewport (vision) — data URL PNG dari frame terakhir canvas.
/// AI dapat "melihat" adegan sisi-klien; fallback JSON error bila kanvas
/// tidak ada / belum di-start.
#[wasm_bindgen]
pub fn adilang_capture_viewport_snapshot() -> Result<String, String> {
    with_engine(|e| e.capture_viewport_snapshot())
}

// ═══════════════════════════════════════════════════════════════════════════
// RENDER TARGET / BACKEND (adilang_target)
// ═══════════════════════════════════════════════════════════════════════════

/// Info backend render yang tersedia di runtime + yang terseleksi (auto).
/// Di WASM, backend yang benar-benar tersedia = WebGL2 (context wajib ada);
/// WebGPU/wgpu-native tercantum sebagai backend target.
#[wasm_bindgen]
pub fn adilang_target_info() -> String {
    use crate::target::{default_caps, Backend};
    let available = vec![Backend::WebGl2];
    let selected = crate::target::select_backend(&available, None, &[])
        .unwrap_or(Backend::WebGl2);
    serde_json::json!({
        "available": available.iter().map(|b| b.as_str()).collect::<Vec<_>>(),
        "selected": selected.as_str(),
        "capabilities": {
            "webgl2": caps_json(&default_caps(Backend::WebGl2)),
            "webgpu": caps_json(&default_caps(Backend::WebGpu)),
            "wgpu_native": caps_json(&default_caps(Backend::WgpuNative)),
        },
    })
    .to_string()
}

fn caps_json(c: &crate::target::RenderCaps) -> serde_json::Value {
    serde_json::json!({
        "backend": c.backend.as_str(),
        "max_texture_size": c.max_texture_size,
        "compute": c.compute,
        "float_textures": c.float_textures,
        "instancing": c.instancing,
    })
}

/// Seleksi backend render secara deterministik dari preferensi AI/klien.
/// `prefs` JSON: `{"preference":"webgpu","require":["compute"]}`.
/// `require` opsional; `preference` opsional (auto → prioritas tertinggi).
#[wasm_bindgen]
pub fn adilang_target_select(prefs: &str) -> Result<String, String> {
    use crate::target::{select_backend, Backend, Cap};
    let v: serde_json::Value =
        serde_json::from_str(prefs).map_err(|e| format!("target: JSON prefs tidak valid — {e}"))?;

    let preference = v
        .get("preference")
        .and_then(|p| p.as_str())
        .and_then(Backend::from_str);
    let require: Vec<Cap> = v
        .get("require")
        .and_then(|r| r.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|c| match c.as_str()? {
                    "compute" => Some(Cap::Compute),
                    "float_textures" => Some(Cap::FloatTextures),
                    "instancing" => Some(Cap::Instancing),
                    "texture_4096" => Some(Cap::Texture4096),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();

    let available = vec![Backend::WebGl2];
    let selected = select_backend(&available, preference, &require)?;
    Ok(serde_json::json!({
        "selected": selected.as_str(),
        "preference": preference.map(|b| b.as_str()),
        "require": require.iter().map(|c| cap_name(*c)).collect::<Vec<_>>(),
    })
    .to_string())
}

fn cap_name(c: crate::target::Cap) -> &'static str {
    match c {
        crate::target::Cap::Compute => "compute",
        crate::target::Cap::FloatTextures => "float_textures",
        crate::target::Cap::Instancing => "instancing",
        crate::target::Cap::Texture4096 => "texture_4096",
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// HEADLESS TESTER (adilang_test_report)
// ═══════════════════════════════════════════════════════════════════════════

/// Jalankan headless test (parse/check/struktur/simulasi) terhadap satu sumber
/// ADILang dan kembalikan laporan JSON (sama dengan `adi test`). AI/browser
/// dapat memanggilnya TANPA renderer 3D.
#[wasm_bindgen]
pub fn adilang_test_report(source: &str) -> String {
    crate::tester::test_program(source).to_json_string()
}

// ═══════════════════════════════════════════════════════════════════════════
// LIFECYCLE HOOKS (adilang_components / adilang_run_lifecycle) — v1.13.0
// ═══════════════════════════════════════════════════════════════════════════

fn value_to_json(v: &crate::eval::Value) -> serde_json::Value {
    match v {
        crate::eval::Value::Num(n) => serde_json::json!(n),
        crate::eval::Value::Str(s) => serde_json::json!(s),
        crate::eval::Value::Bool(b) => serde_json::json!(b),
        crate::eval::Value::Tuple(t) => serde_json::json!(t),
        crate::eval::Value::List(items) => {
            serde_json::Value::Array(items.iter().map(value_to_json).collect())
        }
        crate::eval::Value::Map(pairs) => {
            let obj = pairs
                .iter()
                .map(|(k, v)| (k.clone(), value_to_json(v)))
                .collect::<serde_json::Map<_, _>>();
            serde_json::Value::Object(obj)
        }
        crate::eval::Value::Null => serde_json::Value::Null,
    }
}

/// Parse source dan kembalikan daftar component + lifecycle hooks (JSON):
/// `{"components":[{"name":"MyCard","hooks":["on_mount","on_update"]}]}`
#[wasm_bindgen]
pub fn adilang_parse_components(source: &str) -> Result<String, String> {
    use crate::ast::{LifecycleHookKind, TopLevel};
    let program = crate::parser::parse(source)?;
    let components = program
        .items
        .iter()
        .filter_map(|i| match i {
            TopLevel::Component(c) => Some(serde_json::json!({
                "name": c.name,
                "hooks": c.hooks.iter().map(|h| h.kind.as_str()).collect::<Vec<_>>(),
            })),
            _ => None,
        })
        .collect::<Vec<_>>();
    Ok(serde_json::json!({
        "status": "ok",
        "count": components.len(),
        "components": components,
    })
    .to_string())
}

/// Jalankan lifecycle hook (on_mount/on_update/on_unmount) dari component
/// tertentu dan kembalikan directive calls yang dieksekusi (JSON).
/// Integration point: WASM State Machine — directive `@set_state(...)`/akses
/// `bind @state.*` dapat dipakai di dalam hook.
#[wasm_bindgen]
pub fn adilang_run_lifecycle(component: &str, kind: &str) -> Result<String, String> {
    use crate::ast::LifecycleHookKind;
    let hook = match kind {
        "on_mount" => LifecycleHookKind::Mount,
        "on_update" => LifecycleHookKind::Update,
        "on_unmount" => LifecycleHookKind::Unmount,
        other => return Err(format!("Lifecycle hook tidak dikenal '{other}' (yang sah: on_mount, on_update, on_unmount)")),
    };
    let source = SOURCE
        .lock()
        .map_err(|_| "SOURCE mutex poisoned".to_string())?
        .clone()
        .ok_or_else(|| "Belum ada source di-load (panggil adilang_load terlebih dahulu)".to_string())?;
    let program = crate::parser::parse(&source)?;
    let mut interp = crate::eval::Interpreter::new(String::new());
    interp.load(program)?;
    let calls = interp.run_lifecycle(component, hook)?;
    Ok(serde_json::json!({
        "status": "ok",
        "component": component,
        "hook": hook.as_str(),
        "directives": calls.iter().map(|c| serde_json::json!({
            "name": c.name,
            "args": c.args.iter().map(value_to_json).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
    .to_string())
}

// ═══════════════════════════════════════════════════════════════════════════
// v1.14.0 — Dense Compact AST / AI Guard / Diagnostics / Machine Runner
// ═══════════════════════════════════════════════════════════════════════════

/// Dense opcode map (P6 self-describing) — UI 2D / Layout / mesh WebGL 3D.
#[wasm_bindgen]
pub fn adilang_dense_spec() -> String {
    crate::dense::dense_spec()
}

/// Laporan penghematan token bitstream Dense vs ekuivalen HTML/JS/JSON.
#[wasm_bindgen]
pub fn adilang_dense_size_report(source: &str) -> Result<String, String> {
    crate::dense::size_report(source)
}

/// Tandatangani dokumen sebagai output mesin (marker `ADILANG-SIG`).
#[wasm_bindgen]
pub fn adilang_ai_guard_sign(source: &str) -> Result<String, String> {
    crate::ai_guard::attach_signature(source)
}

/// Verifikasi apakah dokumen ditandatangani & dihasilkan mesin (JSON report).
#[wasm_bindgen]
pub fn adilang_ai_guard_verify(source: &str) -> Result<String, String> {
    let report = crate::ai_guard::is_machine_generated(source);
    Ok(serde_json::json!({
        "status": "ok",
        "valid": report.valid,
        "signature": report.signature,
        "entropy": report.entropy,
        "reason": report.reason,
    })
    .to_string())
}

/// Payload diagnostik mesin (pasangan err-code + node) dari check_src —
/// bukan string error manusia.
#[wasm_bindgen]
pub fn adilang_diag_payload(source: &str) -> String {
    crate::diagnostics::diagnostics_report(source)
}

/// Muat bitstream Dense langsung (tanpa parse string) dan jalankan lifecycle
/// hook komponen. Input `bytes` = bytecode v0x04 (MAGIC 0xAD).
#[wasm_bindgen]
pub fn adilang_machine_run_lifecycle(
    bytes: &[u8],
    component: &str,
    kind: &str,
) -> Result<String, String> {
    use crate::ast::LifecycleHookKind;
    let hook = match kind {
        "on_mount" => LifecycleHookKind::Mount,
        "on_update" => LifecycleHookKind::Update,
        "on_unmount" => LifecycleHookKind::Unmount,
        other => {
            return Err(format!(
                "Lifecycle hook tidak dikenal '{other}' (yang sah: on_mount, on_update, on_unmount)"
            ))
        }
    };
    let mut runner = crate::machine_runner::MachineRunner::from_dense(bytes.to_vec())?;
    runner.run_lifecycle(component, hook)
}

/// Operasi DOM 2D dari bitstream Dense (JSON) — runtime JS membuat node DOM
/// dari opcode (NodeContainer 0x01 / NodeText 0x02 / ...), bukan dari HTML.
#[wasm_bindgen]
pub fn adilang_machine_dom_ops(bytes: &[u8]) -> Result<String, String> {
    Ok(crate::machine_runner::MachineRunner::from_dense(bytes.to_vec())?.dom_ops_json())
}

/// Operasi WebGL2 dari bitstream Dense (JSON) — mesh/camera/light/transform
/// siap menjadi objek scene (three.js/WebGL2) oleh runtime JS.
#[wasm_bindgen]
pub fn adilang_machine_webgl_ops(bytes: &[u8]) -> Result<String, String> {
    Ok(crate::machine_runner::MachineRunner::from_dense(bytes.to_vec())?.webgl_ops_json())
}

/// Jalankan handler event (frame/speak/silent/click) dari bitstream Dense.
/// `entity_id` kosong = handler level dunia. Mengembalikan jumlah handler.
#[wasm_bindgen]
pub fn adilang_machine_fire_event(
    bytes: &[u8],
    entity_id: &str,
    event: &str,
) -> Result<String, String> {
    use crate::ast::EventKind;
    let kind = match event {
        "frame" => EventKind::Frame,
        "speak" => EventKind::Speak,
        "silent" => EventKind::Silent,
        "click" => EventKind::Click,
        other => {
            return Err(format!(
                "Event tidak dikenal '{other}' (yang sah: frame, speak, silent, click)"
            ))
        }
    };
    let mut runner = crate::machine_runner::MachineRunner::from_dense(bytes.to_vec())?;
    let id = if entity_id.is_empty() { None } else { Some(entity_id) };
    let count = runner.fire_event(id, kind)?;
    Ok(serde_json::json!({ "status": "ok", "handlers_run": count }).to_string())
}

/// Daftar komponen + hook + directive dari bitstream Dense (JSON).
#[wasm_bindgen]
pub fn adilang_machine_components(bytes: &[u8]) -> Result<String, String> {
    Ok(crate::machine_runner::MachineRunner::from_dense(bytes.to_vec())?.components_json())
}
