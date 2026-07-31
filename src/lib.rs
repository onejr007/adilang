// ADILang — entry WASM (wasm-bindgen).
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).

#[cfg(target_arch = "wasm32")]
mod engine;
mod ast;
mod bytecode;
mod eval;
mod lexer;
mod math3d;
mod parser;
mod registry;
mod scene;

#[cfg(target_arch = "wasm32")]
mod wasm_api;

#[cfg(test)]
mod tests {
    use crate::ast::TopLevel;
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
        assert_eq!(prog.items.len(), 2);
        assert!(matches!(prog.items[0], TopLevel::Camera(_)));
        assert!(matches!(prog.items[1], TopLevel::Entity(_)));
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
            TopLevel::Entity(e) => Some(e),
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
}
