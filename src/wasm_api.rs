// ADILang — API WASM (wasm-bindgen) + wiring WebGL2.
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).

use std::sync::Mutex;
use wasm_bindgen::prelude::*;

use crate::ast;
use crate::engine::Engine;
use crate::parser;

static ENGINE: Mutex<Option<Engine>> = Mutex::new(None);

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
        *g.borrow_mut() = Some(Closure::wrap(Box::new(move |_now: f64| {
            let result = with_engine(|e| {
                e.interp.t = timestamp_seconds();
                let _ = e.run_frame_handlers();
                e.render();
                Ok::<(), String>(())
            });
            if let Err(err) = result {
                warn(&format!("frame error: {err}"));
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
        Ok::<(), String>(())
    })
}

/// Trigger event speak (ADI mulai bicara).
#[wasm_bindgen]
pub fn adilang_speak() -> Result<(), String> {
    with_engine(|e| e.fire_event(ast::EventKind::Speak))
}

/// Trigger event silent (ADI selesai bicara).
#[wasm_bindgen]
pub fn adilang_silent() -> Result<(), String> {
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

fn timestamp_seconds() -> f64 {
    use js_sys::Date;
    Date::now() / 1000.0
}

fn get_default_world() -> &'static str {
    include_str!("../worlds/default.adi")
}
