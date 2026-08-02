// ADILang headless tester — `adi test` (v1.12.0).
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).
//
// Menguji program ADILang TANPA browser/GPU:
//   1. Parse    — AST harus valid.
//   2. Check    — dijalankan checker (error = GAGAL, warning = catatan).
//   3. Struktur — semua ui_layout terpenuhi: onClick merujuk fungsi yang
//                 benar-benar ada; komponen UI (input) memakai aturan
//                 validasi yang dikenal; bind path diawali `state.`.
//   4. Simulasi — setiap handler event (frame/speak/silent/click) dieksekusi
//                 oleh interpreter tree-walking; error runtime = GAGAL.
// Laporan JSON (P1 deterministik: urutan checks selalu sama).

use crate::ast::{SpatialItem, TopLevel, UIComponent};
use crate::checker::Severity;
use crate::eval::Interpreter;
use crate::parser::parse;

/// Aturan validasi input yang dikenal oleh runtime (v1.12.0).
pub const KNOWN_VALIDATORS: &[&str] = &["required", "email"];

/// Satu baris hasil pengujian.
#[derive(Debug, Clone, PartialEq)]
pub struct Check {
    pub name: String,
    pub ok: bool,
    pub message: String,
}

/// Laporan headless `adi test`.
#[derive(Debug, Clone, PartialEq)]
pub struct TestReport {
    pub program: String,
    pub passed: usize,
    pub failed: usize,
    pub checks: Vec<Check>,
}

impl TestReport {
    fn new(program: String) -> Self {
        Self {
            program,
            passed: 0,
            failed: 0,
            checks: Vec::new(),
        }
    }
    fn push(&mut self, c: Check) {
        if c.ok {
            self.passed += 1;
        } else {
            self.failed += 1;
        }
        self.checks.push(c);
    }
    pub fn to_json(&self) -> serde_json::Value {
        let checks: Vec<serde_json::Value> = self
            .checks
            .iter()
            .map(|c| {
                serde_json::json!({
                    "name": c.name,
                    "ok": c.ok,
                    "message": c.message,
                })
            })
            .collect();
        serde_json::json!({
            "program": self.program,
            "passed": self.passed,
            "failed": self.failed,
            "checks": checks,
        })
    }
    pub fn to_json_string(&self) -> String {
        self.to_json().to_string()
    }
}

/// Jalankan headless test terhadap satu sumber ADILang. Selalu mengembalikan
/// laporan (tidak panic) — semua kegagalan tercatat sebagai check `ok:false`.
pub fn test_program(src: &str) -> TestReport {
    let mut rep = TestReport::new(match parse(src) {
        Ok(p) => p.name.clone(),
        Err(_) => "<tanpa-nama>".to_string(),
    });

    // ── 1. Parse ──────────────────────────────────────────────────────────
    let prog = match parse(src) {
        Ok(p) => p,
        Err(e) => {
            rep.push(Check {
                name: "parse".to_string(),
                ok: false,
                message: format!("AST tidak valid: {e}"),
            });
            return rep;
        }
    };
    rep.push(Check {
        name: "parse".to_string(),
        ok: true,
        message: format!("AST valid ({} item top-level)", prog.items.len()),
    });

    // ── 2. Checker ────────────────────────────────────────────────────────
    match crate::checker::check_src(src) {
        Err(e) => {
            rep.push(Check {
                name: "check".to_string(),
                ok: false,
                message: format!("checker tidak berjalan: {e}"),
            });
            return rep;
        }
        Ok(diags) => {
            let errors: Vec<_> = diags.iter().filter(|d| d.severity == Severity::Error).collect();
            let warnings: usize = diags.iter().filter(|d| d.severity == Severity::Warning).count();
            if errors.is_empty() {
                rep.push(Check {
                    name: "check".to_string(),
                    ok: true,
                    message: if warnings == 0 {
                        "checker bersih".to_string()
                    } else {
                        format!("checker bersih, {warnings} warning")
                    },
                });
            } else {
                let mut msgs: Vec<String> = errors.iter().map(|d| d.message.clone()).collect();
                msgs.truncate(3);
                rep.push(Check {
                    name: "check".to_string(),
                    ok: false,
                    message: format!("{} error: {}", errors.len(), msgs.join("; ")),
                });
                return rep;
            }
        }
    }

    // ── 3. Struktur ──────────────────────────────────────────────────────
    let funcs = collect_func_names(&prog);
    let mut ui_ok = true;
    let mut ui_msgs = Vec::new();
    for item in &prog.items {
        if let TopLevel::UILayout(layout) = item {
            let mut buf = Vec::new();
            validate_ui_component(&layout.root, &funcs, &layout.name, &mut buf);
            for (name, ok, msg) in buf {
                ui_ok &= ok;
                if !ok {
                    ui_msgs.push(format!("{name}: {msg}"));
                }
            }
        }
    }
    if ui_ok {
        rep.push(Check {
            name: "ui_struktur".to_string(),
            ok: true,
            message: "seluruh komponen UI valid (onClick/bind/validate)".to_string(),
        });
    } else {
        ui_msgs.truncate(3);
        rep.push(Check {
            name: "ui_struktur".to_string(),
            ok: false,
            message: ui_msgs.join("; "),
        });
    }

    // ── 4. Simulasi event (interpreter headless) ─────────────────────────
    let mut it = Interpreter::new(prog.name.clone());
    if let Err(e) = it.load(prog.clone()) {
        rep.push(Check {
            name: "simulasi".to_string(),
            ok: false,
            message: format!("load world gagal: {e}"),
        });
        return rep;
    }
    let mut sim_ok = true;
    let mut sim_msgs = Vec::new();
    let mut event_count = 0usize;

    let mut run_body = |name: &str, entity: Option<String>, body: &[crate::ast::Stmt]| {
        event_count += 1;
        if let Err(e) = it.run_handler(entity, body) {
            sim_ok = false;
            sim_msgs.push(format!("{name}: {e}"));
        }
    };

    for item in &prog.items {
        match item {
            TopLevel::Handler(h) => {
                run_body(&format!("event {:?}", h.event), None, &h.body);
            }
            TopLevel::Entity(e) => {
                for h in &e.handlers {
                    run_body(&format!("{}.{:?}", e.id, h.event), Some(e.id.clone()), &h.body);
                }
            }
            TopLevel::World(w) | TopLevel::Spatial3D(w) => {
                for si in &w.items {
                    if let SpatialItem::Entity(e) = si {
                        for h in &e.handlers {
                            run_body(
                                &format!("{}.{:?}", e.id, h.event),
                                Some(e.id.clone()),
                                &h.body,
                            );
                        }
                    } else if let SpatialItem::Handler(h) = si {
                        run_body(&format!("event {:?}", h.event), None, &h.body);
                    }
                }
            }
            _ => {}
        }
    }

    if sim_ok {
        rep.push(Check {
            name: "simulasi".to_string(),
            ok: true,
            message: format!("{event_count} handler dieksekusi tanpa error"),
        });
    } else {
        sim_msgs.truncate(3);
        rep.push(Check {
            name: "simulasi".to_string(),
            ok: false,
            message: format!("{} dari {event_count} handler error: {}", sim_msgs.len(), sim_msgs.join("; ")),
        });
    }

    rep
}

/// Kumpulkan semua nama fungsi (top-level + di dalam world/spatial_3d).
fn collect_func_names(prog: &crate::ast::Program) -> Vec<String> {
    let mut out = Vec::new();
    for item in &prog.items {
        match item {
            TopLevel::Func(f) => out.push(f.name.clone()),
            TopLevel::World(w) | TopLevel::Spatial3D(w) => {
                for si in &w.items {
                    if let SpatialItem::Func(f) = si {
                        out.push(f.name.clone());
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// Validasi pohon komponen UI secara rekursif.
fn validate_ui_component(
    c: &UIComponent,
    funcs: &[String],
    layout: &str,
    out: &mut Vec<(String, bool, String)>,
) {
    match c {
        UIComponent::Container { children, .. } => {
            for child in children {
                validate_ui_component(child, funcs, layout, out);
            }
        }
        UIComponent::Card { children, .. } | UIComponent::Modal { children, .. } => {
            for child in children {
                validate_ui_component(child, funcs, layout, out);
            }
        }
        UIComponent::Button { onClick, .. } => {
            if let Some(name) = onClick {
                if !funcs.contains(name) {
                    out.push((
                        format!("onClick '{name}'"),
                        false,
                        format!("layout '{layout}': onClick merujuk fungsi yang tidak ada"),
                    ));
                }
            }
        }
        UIComponent::Input { bind, validate, .. } => {
            if let Some(bind_path) = bind {
                if !bind_path.starts_with("state.") {
                    out.push((
                        "bind path".to_string(),
                        false,
                        format!("layout '{layout}': bind '{bind_path}' harus diawali 'state.'"),
                    ));
                }
            }
            if let Some(rules) = validate {
                for rule in rules.split('|') {
                    if !KNOWN_VALIDATORS.contains(&rule) {
                        out.push((
                            format!("validator '{rule}'"),
                            false,
                            format!(
                                "layout '{layout}': validator tak dikenal (didukung: {})",
                                KNOWN_VALIDATORS.join("|")
                            ),
                        ));
                    }
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn program_sehat_lulus() {
        let src = r#"
            world "App" {
                func spin() { rotate(0.1, (0 1 0)) }
                entity "box" {
                    on click { spin() }
                }
            }
            ui_layout "home" {
                button "Mulai" onClick spin
            }
        "#;
        let rep = test_program(src);
        assert_eq!(rep.failed, 0, "laporan: {}", rep.to_json_string());
        assert!(rep.passed >= 4);
    }

    #[test]
    fn parse_gagal_dicatat() {
        let rep = test_program("world {{ ini salah");
        assert!(!rep.checks.is_empty());
        assert_eq!(rep.failed, 1);
        assert_eq!(rep.checks[0].name, "parse");
    }

    #[test]
    fn onClick_hilang_gagal() {
        let src = r#"
            world "App" { entity "box" { on click { rotate(0.1, (0 1 0)) } } }
            ui_layout "home" {
                button "Mulai" onClick tidak_ada
            }
        "#;
        let rep = test_program(src);
        let ui = rep.checks.iter().find(|c| c.name == "ui_struktur").expect("ada");
        assert!(!ui.ok, "onClick harus dirujuk ke fungsi yang ada");
    }

    #[test]
    fn validasi_eksekusi_handler_error() {
        let src = r#"
            world "App" {
                entity "box" {
                    on click { rotate(0.1, (0 1 0)) x = x + 1 }
                }
            }
        "#;
        let rep = test_program(src);
        let sim = rep.checks.iter().find(|c| c.name == "simulasi").expect("ada");
        assert!(!sim.ok, "akses variabel tak dikenal harus error runtime");
    }

    #[test]
    fn json_report_deterministik() {
        let a = test_program("world \"A\" { entity \"e\" { on frame { rotate(0.1,(0 1 0)) } } }");
        let b = test_program("world \"A\" { entity \"e\" { on frame { rotate(0.1,(0 1 0)) } } }");
        assert_eq!(a.to_json_string(), b.to_json_string());
    }
}
