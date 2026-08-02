// ADILang — entry WASM (wasm-bindgen).
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).

#[cfg(target_arch = "wasm32")]
mod engine;
pub mod ai_guard;
mod ast;
mod bytecode;
pub mod analytics;
pub mod checker;
pub mod compactor;
pub mod crdt;
pub mod dense;
pub mod diagnostics;
pub mod diff;
mod eval;
pub mod exporter;
mod lexer;
pub mod machine_runner;
mod math3d;
pub mod parser;
pub mod pkg;
pub mod protocol;
mod registry;
pub mod scaffolder;
pub mod schema;
pub mod self_heal;
mod scene;
pub mod tester;pub mod spatial;
pub mod state;
pub mod target;

#[cfg(target_arch = "wasm32")]
mod wasm_api;

// DevServer + Build Optimizer hanya untuk target native (std::net/std::thread)
// sehingga target wasm32 tetap bersih (tanpa tokio/axum/notify).
#[cfg(not(target_arch = "wasm32"))]
pub mod build;
#[cfg(not(target_arch = "wasm32"))]
pub mod devserver;

#[cfg(test)]
mod tests {
    use crate::ast::{FlexDirection, SpatialItem, TopLevel, UIComponent};
    use crate::lexer::tokenize;
    use crate::parser::parse;

    #[test]
    fn lexer_handles_numbers_and_ops() {
        let toks = tokenize("x = 3.5 + 2").expect("lex ok");
        assert!(toks.len() >= 5);
    }

    #[test]
    fn parser_parses_simple_world() {
        let src = r#"
            world "Test" {
                camera "c" { pos (0 1 2) }
                entity "e" {
                    pos (0 0 0)
                    mesh sphere { radius 1 segments 3 }
                    material solid (1 0 0) 1
                }
            }
        "#;
        let prog = parse(src).expect("parse ok");
        assert_eq!(prog.items.len(), 1);
        match &prog.items[0] {
            TopLevel::World(w) => {
                assert_eq!(w.items.len(), 2);
                assert!(matches!(w.items[0], SpatialItem::Camera(_)));
                assert!(matches!(w.items[1], SpatialItem::Entity(_)));
            }
            _ => panic!("bukan world"),
        }
    }

    #[test]
    fn parser_parses_tuples_and_math() {
        let src = r#"
            world "W" {
                entity "e" {
                    on frame {
                        rotate(0.5 * t, (0 1 0))
                        setPos(cos(t) * 2, sin(t), 0)
                    }
                }
            }
        "#;
        let prog = parse(src).expect("parse ok");
        let entity = prog.items.iter().find_map(|i| match i {
            TopLevel::World(w) => w.items.iter().find_map(|item| match item {
                SpatialItem::Entity(e) => Some(e),
                _ => None,
            }),
            _ => None,
        });
        assert_eq!(entity.expect("entity").handlers.len(), 1);
    }

    #[test]
    fn default_world_is_valid_adilang() {
        let src = include_str!("../worlds/default.adi");
        let prog = parse(src).expect("default.adi harus valid ADILang");
        assert_eq!(prog.name, "ADI Hologram");
    }

    #[test]
    fn adi_character_world_is_valid_adilang() {
        let src = include_str!("../worlds/adi-character.adi");
        let prog = parse(src).expect("adi-character.adi harus valid ADILang");
        assert!(prog.name.contains("ADI Character"));
    }

    // ═══════════════════════════════════════════════════════════════════════
    // v2.0.0 — multi-domain tests
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn parse_payload_block() {
        let src = r#"
            @payload {
                sender "agent-a"
                target_agent "agent-b"
                intent "query"
                state_data { status: "active" }
            }
            world "T" {
                entity "e" { on frame { rotate(0.1, (0 1 0)) } }
            }
        "#;
        let prog = parse(src).expect("parse ok");
        assert_eq!(prog.items.len(), 2);
        match &prog.items[0] {
            TopLevel::Payload(p) => {
                assert_eq!(p.sender, "agent-a");
                assert_eq!(p.target_agent, "agent-b");
                assert_eq!(p.intent, "query");
                assert!(p.state_data.is_some());
            }
            _ => panic!("bukan payload"),
        }
        match &prog.items[1] {
            TopLevel::World(w) => assert_eq!(w.items.len(), 1),
            _ => panic!("bukan world"),
        }
    }

    #[test]
    fn parse_ui_layout_block() {
        let src = r#"
            ui_layout "main" {
                container {
                    flex column
                    text "Hello ADILang"
                    button "Submit" onClick submitHandler
                    input "username" placeholder "Enter name"
                }
            }
            world "T" {
                entity "e" { on frame { rotate(0.1, (0 1 0)) } }
            }
        "#;
        let prog = parse(src).expect("parse ok");
        assert_eq!(prog.items.len(), 2);
        match &prog.items[0] {
            TopLevel::UILayout(layout) => {
                assert_eq!(layout.name, "main");
        match &layout.root {
            UIComponent::Container { flex, children } => {
                assert!(matches!(flex, Some(FlexDirection::Column)));
                assert_eq!(children.len(), 3);
                match &children[0] {
                    UIComponent::Text { content } => assert_eq!(content, "Hello ADILang"),
                    _ => panic!("bukan text"),
                }
                match &children[1] {
                    UIComponent::Button { label, onClick } => {
                        assert_eq!(label, "Submit");
                        assert_eq!(onClick.as_ref().unwrap(), "submitHandler");
                    }
                    _ => panic!("bukan button"),
                }
                match &children[2] {
                    UIComponent::Input { name, placeholder, .. } => {
                        assert_eq!(name, "username");
                        assert_eq!(placeholder.as_ref().unwrap(), "Enter name");
                    }
                    _ => panic!("bukan input"),
                }
            }
            _ => panic!("bukan container"),
        }
            }
            _ => panic!("bukan ui_layout"),
        }
    }

    #[test]
    fn parse_spatial_3d_block() {
        let src = r#"
            spatial_3d "MyScene" {
                camera "cam" { pos (0 1.6 7) look (0 0 0) fov 55 }
                light "key" { type point pos (5 6 4) color (1 0.95 0.9) intensity 1.5 }
                entity "core" {
                    pos (0 0 0)
                    mesh sphere { radius 0.8 segments 3 }
                    material wire (0.15 0.8 1) 0.9
                    on frame { rotate(0.35 * t, (0.15 1 0.1)) }
                }
            }
        "#;
        let prog = parse(src).expect("parse ok");
        assert_eq!(prog.items.len(), 1);
        match &prog.items[0] {
            TopLevel::Spatial3D(s) => {
                assert_eq!(s.name, "MyScene");
                assert_eq!(s.items.len(), 3);
                assert!(matches!(s.items[0], SpatialItem::Camera(_)));
                assert!(matches!(s.items[1], SpatialItem::Light(_)));
                assert!(matches!(s.items[2], SpatialItem::Entity(_)));
            }
            _ => panic!("bukan spatial_3d"),
        }
    }

    #[test]
    fn parse_mixed_multi_block_file() {
        let src = r#"
            @payload {
                sender "ai-1"
                target_agent "ai-2"
                intent "collaborate"
            }
            ui_layout "hud" {
                container {
                    flex row
                    text "Status: Active"
                    button "Send" onClick send
                }
            }
            spatial_3d "scene" {
                camera "cam" { pos (0 1.6 7) look (0 0 0) fov 55 }
                entity "e" { on frame { rotate(0.1, (0 1 0)) } }
            }
        "#;
        let prog = parse(src).expect("parse ok");
        assert_eq!(prog.items.len(), 3);
        assert!(matches!(prog.items[0], TopLevel::Payload(_)));
        assert!(matches!(prog.items[1], TopLevel::UILayout(_)));
        assert!(matches!(prog.items[2], TopLevel::Spatial3D(_)));
    }

    #[test]
    fn parse_world_alias_still_works() {
        let src = r#"
            world "T" {
                camera "cam" { pos (0 1.6 7) look (0 0 0) fov 55 }
                entity "e" { on frame { rotate(0.1, (0 1 0)) } }
            }
        "#;
        let prog = parse(src).expect("parse ok");
        assert_eq!(prog.items.len(), 1);
        match &prog.items[0] {
            TopLevel::World(s) => {
                assert_eq!(s.name, "T");
                assert_eq!(s.items.len(), 2);
            }
            _ => panic!("bukan world alias"),
        }
    }

    #[test]
    fn parse_ui_button_without_onclick() {
        let src = r#"
            ui_layout "main" {
                button "OK"
            }
        "#;
        let prog = parse(src).expect("parse ok");
        match &prog.items[0] {
            TopLevel::UILayout(layout) => {
                match &layout.root {
                    UIComponent::Button { label, onClick } => {
                        assert_eq!(label, "OK");
                        assert!(onClick.is_none());
                    }
                    _ => panic!("bukan button"),
                }
            }
            _ => panic!("bukan ui_layout"),
        }
    }

    #[test]
    fn parse_ui_input_with_placeholder() {
        let src = r#"
            ui_layout "form" {
                input "email" placeholder "user@example.com"
            }
        "#;
        let prog = parse(src).expect("parse ok");
        match &prog.items[0] {
            TopLevel::UILayout(layout) => {
                match &layout.root {
                    UIComponent::Input { name, placeholder, .. } => {
                        assert_eq!(name, "email");
                        assert_eq!(placeholder.as_ref().unwrap(), "user@example.com");
                    }
                    _ => panic!("bukan input"),
                }
            }
            _ => panic!("bukan ui_layout"),
        }
    }

    // ═══════════════════════════════════════════════════════════════════════
    // v1.12.0 — @use_js / routes / @i18n / directive statements / ui_std
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn parse_use_js_block() {
        let src = r#"
            @use_js { url "https://cdn.example/lib.js" }
            world "T" { entity "e" { on frame { rotate(0.1, (0 1 0)) } } }
        "#;
        let prog = parse(src).expect("parse ok");
        assert_eq!(prog.items.len(), 2);
        match &prog.items[0] {
            TopLevel::UseJs(u) => assert_eq!(u.url, "https://cdn.example/lib.js"),
            _ => panic!("bukan use_js"),
        }
    }

    #[test]
    fn parse_routes_block() {
        let src = r#"
            routes {
                route "/" layout "home" transition "fade"
                route "/about" layout "about" transition "slide"
            }
        "#;
        let prog = parse(src).expect("parse ok");
        match &prog.items[0] {
            TopLevel::Routes(r) => {
                assert_eq!(r.routes.len(), 2);
                assert_eq!(r.routes[0].path, "/");
                assert_eq!(r.routes[0].layout, "home");
                assert_eq!(r.routes[0].transition.as_deref(), Some("fade"));
                assert_eq!(r.routes[1].path, "/about");
                assert_eq!(r.routes[1].transition.as_deref(), Some("slide"));
            }
            _ => panic!("bukan routes"),
        }
    }

    #[test]
    fn parse_i18n_block() {
        let src = r#"
            @i18n {
                locale "en" { welcome "Hello" bye "Goodbye" }
                locale "id" { welcome "Halo" bye "Sampai jumpa" }
            }
        "#;
        let prog = parse(src).expect("parse ok");
        match &prog.items[0] {
            TopLevel::I18n(i) => {
                assert_eq!(i.locales.len(), 2);
                assert_eq!(i.locales[0].name, "en");
                assert_eq!(i.locales[0].entries[0], ("welcome".to_string(), "Hello".to_string()));
                assert_eq!(i.locales[1].name, "id");
                assert_eq!(i.locales[1].entries[1], ("bye".to_string(), "Sampai jumpa".to_string()));
            }
            _ => panic!("bukan i18n"),
        }
    }

    #[test]
    fn parse_directive_statements() {
        let src = r#"
            world "T" {
                entity "e" {
                    on click {
                        @navigate("/about")
                        @set_locale("id")
                    }
                }
            }
        "#;
        let prog = parse(src).expect("parse ok");
        let entity = prog.items.iter().find_map(|i| match i {
            TopLevel::World(w) => w.items.iter().find_map(|item| match item {
                SpatialItem::Entity(e) => Some(e),
                _ => None,
            }),
            _ => None,
        });
        let body = &entity.expect("entity").handlers[0].body;
        assert_eq!(body.len(), 2);
        assert!(matches!(&body[0], crate::ast::Stmt::Navigate { path } if path == "/about"));
        assert!(matches!(&body[1], crate::ast::Stmt::SetLocale { locale } if locale == "id"));
    }

    #[test]
    fn parse_ui_std_components_dan_binding() {
        let src = r#"
            ui_layout "app" {
                navbar "ADI"
                card "Panel" {
                    text "Konten"
                    input "user" placeholder "Nama" bind: @state.username validate "required"
                    button "Kirim" onClick send
                }
                modal "Konfirmasi" {
                    text "Yakin?"
                }
                footer "© 2026 ADI"
            }
        "#;
        let prog = parse(src).expect("parse ok");
        match &prog.items[0] {
            TopLevel::UILayout(layout) => {
                let UIComponent::Container { children, .. } = &layout.root else {
                    panic!("bukan container");
                };
                assert!(matches!(children[0], UIComponent::Navbar { .. }));
                match &children[1] {
                    UIComponent::Card { title, children } => {
                        assert_eq!(title.as_deref(), Some("Panel"));
                        assert!(matches!(children[0], UIComponent::Text { .. }));
                        match &children[1] {
                            UIComponent::Input { bind, validate, .. } => {
                                assert_eq!(bind.as_deref(), Some("state.username"));
                                assert_eq!(validate.as_deref(), Some("required"));
                            }
                            _ => panic!("bukan input"),
                        }
                    }
                    _ => panic!("bukan card"),
                }
                assert!(matches!(children[2], UIComponent::Modal { .. }));
                assert!(matches!(children[3], UIComponent::Footer { .. }));
            }
            _ => panic!("bukan ui_layout"),
        }
    }

    #[test]
    fn bytecode_roundtrip_modul_baru() {
        let src = r#"
            @use_js { url "https://cdn.example/lib.js" }
            routes {
                route "/" layout "home" transition "fade"
            }
            @i18n {
                locale "en" { welcome "Hello" }
            }
            ui_layout "home" {
                button "Go" onClick go
            }
            world "T" {
                entity "e" {
                    on click {
                        @navigate("/")
                        @set_locale("en")
                    }
                }
            }
        "#;
        let prog = parse(src).expect("parse ok");
        let bin = crate::bytecode::encode_program(&prog).expect("encode ok");
        let dec = crate::bytecode::decode_program(bin).expect("decode ok");
        assert_eq!(prog, dec, "bytecode roundtrip harus identik");
    }

    #[test]
    fn optimize_roundtrip_modul_baru() {
        let src = r#"
            @use_js { url "https://cdn.example/lib.js" }
            routes { route "/" layout "home" transition "fade" }
            @i18n { locale "en" { welcome "Hello" } }
            ui_layout "home" {
                card "Panel" {
                    input "user" bind: @state.username validate "required|email"
                    button "Send" onClick send
                }
            }
            world "T" { entity "e" { on click { @navigate("/about") @set_locale("id") } } }
        "#;
        let opt = crate::compactor::optimize_src(src).expect("optimize ok");
        let prog = parse(src).expect("parse ok");
        let prog2 = parse(&opt).expect("hasil optimize harus valid");
        assert_eq!(prog, prog2, "optimize mengubah AST modul baru:\n{opt}");
    }

    // ── v1.13.0: Lifecycle Hooks (component + directive generik) ────────────

    #[test]
    fn parse_component_lifecycle_hooks() {
        let src = r#"
            component MyCard {
                on_mount: @fetch_data()
                on_update: @log_change("card")
                on_unmount: @cleanup_state()
            }
        "#;
        let prog = parse(src).expect("parse ok");
        match &prog.items[0] {
            TopLevel::Component(c) => {
                assert_eq!(c.name, "MyCard");
                assert_eq!(c.hooks.len(), 3);
                assert!(matches!(c.hooks[0].kind, crate::ast::LifecycleHookKind::Mount));
                assert!(matches!(c.hooks[1].kind, crate::ast::LifecycleHookKind::Update));
                assert!(matches!(c.hooks[2].kind, crate::ast::LifecycleHookKind::Unmount));
                assert!(matches!(
                    &c.hooks[0].body[0],
                    crate::ast::Stmt::Directive { name, args }
                        if name == "fetch_data" && args.is_empty()
                ));
                assert!(matches!(
                    &c.hooks[1].body[0],
                    crate::ast::Stmt::Directive { name, args }
                        if name == "log_change" && args.len() == 1
                ));
            }
            _ => panic!("bukan component"),
        }
    }

    #[test]
    fn component_bytecode_roundtrip_dan_optimize() {
        let src = r#"
            component MyCard {
                on_mount: @fetch_data()
                on_update: @log_change("card")
                on_unmount: @cleanup_state()
            }
            @i18n { locale "en" { welcome "Hello" } }
        "#;
        let prog = parse(src).expect("parse ok");
        let bin = crate::bytecode::encode_program(&prog).expect("encode ok");
        let dec = crate::bytecode::decode_program(bin).expect("decode ok");
        assert_eq!(prog, dec, "bytecode roundtrip component harus identik");

        let opt = crate::compactor::optimize_src(src).expect("optimize ok");
        let prog2 = parse(&opt).expect("hasil optimize harus valid");
        assert_eq!(prog, prog2, "optimize mengubah AST component:\n{opt}");
    }

    #[test]
    fn eval_run_lifecycle_menghasilkan_directive_calls() {
        use crate::ast::LifecycleHookKind;
        let src = r#"
            component MyCard {
                on_mount: @fetch_data()
                on_update: @log_change("card")
                on_unmount: @cleanup_state()
            }
        "#;
        let prog = parse(src).expect("parse ok");
        let mut interp = crate::eval::Interpreter::new("Test".into());
        interp.load(prog).expect("load ok");

        let mount = interp
            .run_lifecycle("MyCard", LifecycleHookKind::Mount)
            .expect("mount ok");
        assert_eq!(mount.len(), 1);
        assert_eq!(mount[0].name, "fetch_data");
        assert!(mount[0].args.is_empty());

        let update = interp
            .run_lifecycle("MyCard", LifecycleHookKind::Update)
            .expect("update ok");
        assert_eq!(update.len(), 1);
        assert_eq!(update[0].name, "log_change");
        assert!(matches!(&update[0].args[0], crate::eval::Value::Str(s) if s == "card"));

        let unmount = interp
            .run_lifecycle("MyCard", LifecycleHookKind::Unmount)
            .expect("unmount ok");
        assert_eq!(unmount.len(), 1);
        assert_eq!(unmount[0].name, "cleanup_state");

        assert!(interp
            .run_lifecycle("TidakAda", LifecycleHookKind::Mount)
            .is_err());
    }

    #[test]
    fn lifecycle_parser_menolak_hook_tidak_kenal() {
        let src = r#"
            component MyCard {
                on_click: @do_something()
            }
        "#;
        assert!(parse(src).is_err(), "hook tak dikenal harus ditolak parser");
    }

    #[test]
    fn checker_component_directive_tanpa_warning() {
        let src = r#"
            component MyCard {
                on_mount: @fetch_data()
                on_unmount: @cleanup_state()
            }
        "#;
        let diags = crate::checker::check_src(src).expect("parse ok");
        assert!(
            diags.is_empty(),
            "component sehat harus tanpa diagnosa: {diags:?}"
        );
    }

    #[test]
    fn bytecode_roundtrip_func_if_while_for_match() {
        // Regresi: blok di posisi "blok" (fungsi/if/while/for/match) wajib
        // roundtrip identik — penulisan STMT_BLOCK selalu dilakukan encoder.
        let src = r#"
            func compute(a, b) {
                if a > b {
                    let m = a
                    while m > 0 { m = m - 1 }
                } else {
                    let m = b
                }
                for i in 0 5 {
                    let j = i * 2
                }
                match a {
                    1 => { let x = 10 }
                    _ => { let x = 20 }
                }
                return a + b
            }
            world "T" { entity "e" { on frame { let k = compute(1, 2) } } }
        "#;
        let prog = parse(src).expect("parse ok");
        let bin = crate::bytecode::encode_program(&prog).expect("encode ok");
        let dec = crate::bytecode::decode_program(bin).expect("decode ok");
        assert_eq!(prog, dec, "roundtrip func/if/while/for/match harus identik");
    }
}
