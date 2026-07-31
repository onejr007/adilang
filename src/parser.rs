// ADILang parser — recursive descent.
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).

use crate::ast::*;
use crate::lexer::{tokenize, TokKind, Token};

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

pub fn parse(src: &str) -> Result<Program, String> {
    let tokens = tokenize(src)?;
    let mut p = Parser { tokens, pos: 0 };
    p.parse_program()
}

impl Parser {
    fn peek(&self) -> &TokKind {
        &self.tokens[self.pos].kind
    }
    fn peek2(&self) -> &TokKind {
        if self.pos + 1 < self.tokens.len() {
            &self.tokens[self.pos + 1].kind
        } else {
            &TokKind::Eof
        }
    }
    fn line(&self) -> usize {
        self.tokens[self.pos].line
    }
    fn advance(&mut self) -> TokKind {
        let t = self.tokens[self.pos].kind.clone();
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        t
    }
    fn expect(&mut self, k: &TokKind, what: &str) -> Result<(), String> {
        if self.peek() == k {
            self.advance();
            Ok(())
        } else {
            Err(format!("Ekspektasi {what} di baris {}", self.line()))
        }
    }
    fn expect_ident(&mut self, what: &str) -> Result<String, String> {
        match self.peek().clone() {
            TokKind::Ident(name) => {
                self.advance();
                Ok(name)
            }
            _ => Err(format!("Ekspektasi {what} di baris {}", self.line())),
        }
    }

    // ── Top level ──
    fn parse_program(&mut self) -> Result<Program, String> {
        self.expect(&TokKind::Ident("world".to_string()), "'world'")?;
        let name = self.expect_str()?;
        self.expect(&TokKind::LBrace, "'{'")?;
        let mut items = Vec::new();
        while self.peek() != &TokKind::RBrace {
            if self.peek() == &TokKind::Eof {
                return Err(format!("'world' tidak ditutup, baris {}", self.line()));
            }
            items.push(self.parse_top_level()?);
        }
        self.advance(); // }
        Ok(Program { name, items })
    }

    fn parse_top_level(&mut self) -> Result<TopLevel, String> {
        let kw = match self.peek().clone() {
            TokKind::Ident(id) => id,
            _ => return Err(format!("Ekspektasi deklarasi di baris {}", self.line())),
        };
        match kw.as_str() {
            "camera" => {
                self.advance();
                let id = self.expect_str()?;
                let props = self.parse_prop_block()?;
                Ok(TopLevel::Camera(CameraDef { id, props }))
            }
            "light" => {
                self.advance();
                let id = self.expect_str()?;
                let props = self.parse_prop_block()?;
                Ok(TopLevel::Light(LightDef { id, props }))
            }
            "entity" => {
                self.advance();
                let id = self.expect_str()?;
                let mut props = Vec::new();
                let mut handlers = Vec::new();
                self.expect(&TokKind::LBrace, "'{'")?;
                while self.peek() != &TokKind::RBrace {
                    if self.peek() == &TokKind::Ident("on".to_string()) {
                        handlers.push(self.parse_handler()?);
                    } else {
                        props.push(self.parse_prop()?);
                    }
                }
                self.advance();
                Ok(TopLevel::Entity(EntityDef { id, props, handlers }))
            }
            "let" => {
                self.advance();
                let name = self.expect_ident("nama variabel")?;
                self.expect(&TokKind::Assign, "'='")?;
                let value = self.parse_expr()?;
                Ok(TopLevel::Let { name, value })
            }
            "func" => {
                self.advance();
                let name = self.expect_ident("nama fungsi")?;
                self.expect(&TokKind::LParen, "'('")?;
                let mut params = Vec::new();
                while self.peek() != &TokKind::RParen {
                    if let TokKind::Ident(p) = self.peek().clone() {
                        params.push(p);
                        self.advance();
                    } else {
                        self.advance();
                    }
                }
                self.advance();
                let body = self.parse_block()?;
                Ok(TopLevel::Func(FuncDef { name, params, body }))
            }
            "on" => Ok(TopLevel::Handler(self.parse_handler()?)),
            other => Err(format!("Keyword/deklarasi tidak dikenal '{other}' di baris {}", self.line())),
        }
    }

    fn parse_handler(&mut self) -> Result<Handler, String> {
        // sudah di posisi 'on'
        self.advance();
        let ev = self.expect_ident("event (frame/speak/silent/click)")?;
        let event = match ev.as_str() {
            "frame" => EventKind::Frame,
            "speak" => EventKind::Speak,
            "silent" => EventKind::Silent,
            "click" => EventKind::Click,
            other => return Err(format!("Event tidak dikenal '{other}' di baris {}", self.line())),
        };
        let body = self.parse_block()?;
        Ok(Handler { event, body })
    }

    fn parse_prop_block(&mut self) -> Result<Vec<Prop>, String> {
        self.expect(&TokKind::LBrace, "'{'")?;
        let mut props = Vec::new();
        while self.peek() != &TokKind::RBrace {
            if self.peek() == &TokKind::Eof {
                return Err(format!("blok tidak ditutup, baris {}", self.line()));
            }
            props.push(self.parse_prop()?);
        }
        self.advance();
        Ok(props)
    }

    fn parse_prop(&mut self) -> Result<Prop, String> {
        let name = self.expect_ident("nama property")?;
        let value = self.parse_prop_value()?;
        Ok(Prop { name, value })
    }

    /// Nilai property: bisa builder (mesh/material), enum ident, tuple, atau angka.
    fn parse_prop_value(&mut self) -> Result<Expr, String> {
        match self.peek().clone() {
            TokKind::Ident(id) if is_builder(&id) => self.parse_builder(&id),
            TokKind::Ident(id) => {
                // enum / ident biasa
                self.advance();
                Ok(Expr::Ident(id))
            }
            _ => self.parse_expr(),
        }
    }

    fn parse_builder(&mut self, name: &str) -> Result<Expr, String> {
        self.advance(); // name
        let mut args = Vec::new();
        let mut props = None;
        // argumen positional: angka / tuple / string
        loop {
            match self.peek().clone() {
                TokKind::Num(_) | TokKind::Minus => {
                    args.push(self.parse_expr()?);
                }
                TokKind::LParen => {
                    args.push(self.parse_expr()?);
                }
                TokKind::Str(_) => {
                    args.push(self.parse_expr()?);
                }
                TokKind::LBrace => {
                    props = Some(self.parse_prop_block()?);
                    break;
                }
                _ => break,
            }
        }
        Ok(Expr::Call { name: name.to_string(), args, props })
    }

    fn parse_block(&mut self) -> Result<Vec<Stmt>, String> {
        self.expect(&TokKind::LBrace, "'{'")?;
        let mut stmts = Vec::new();
        while self.peek() != &TokKind::RBrace {
            if self.peek() == &TokKind::Eof {
                return Err(format!("blok tidak ditutup, baris {}", self.line()));
            }
            stmts.push(self.parse_stmt()?);
        }
        self.advance();
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> Result<Stmt, String> {
        match self.peek().clone() {
            TokKind::Ident(kw) => match kw.as_str() {
                "let" => {
                    self.advance();
                    // Tuple destructuring (v1.6.0): let (a, b) = expr
                    if self.peek() == &TokKind::LParen {
                        self.advance();
                        let mut names = Vec::new();
                        while self.peek() != &TokKind::RParen {
                            if let TokKind::Ident(n) = self.peek().clone() {
                                names.push(n);
                                self.advance();
                            } else if self.peek() == &TokKind::Comma {
                                self.advance();
                            } else {
                                return Err(format!(
                                    "Ekspektasi nama variabel di destructuring, baris {}",
                                    self.line()
                                ));
                            }
                        }
                        self.advance(); // )
                        self.expect(&TokKind::Assign, "'='")?;
                        let value = self.parse_expr()?;
                        if names.is_empty() {
                            return Err("Destructuring kosong: butuh minimal satu nama".into());
                        }
                        return Ok(Stmt::LetDestructure { names, value });
                    }
                    let name = self.expect_ident("nama variabel")?;
                    self.expect(&TokKind::Assign, "'='")?;
                    let value = self.parse_expr()?;
                    Ok(Stmt::Let { name, value })
                }
                "match" => {
                    // match subject { "pat" => { ... } _ => { ... } } (v1.6.0)
                    self.advance();
                    let subject = self.parse_expr()?;
                    self.expect(&TokKind::LBrace, "'{'")?;
                    let mut arms = Vec::new();
                    let mut seen_wildcard = false;
                    while self.peek() != &TokKind::RBrace {
                        if self.peek() == &TokKind::Eof {
                            return Err(format!("match tidak ditutup, baris {}", self.line()));
                        }
                        if seen_wildcard {
                            return Err(format!(
                                "Wildcard '_' wajib arm TERAKHIR di match (baris {})",
                                self.line()
                            ));
                        }
                        // pattern: "str" | number | _
                        let pattern = match self.peek().clone() {
                            TokKind::Str(s) => {
                                self.advance();
                                MatchPattern::Str(s)
                            }
                            TokKind::Num(n) => {
                                self.advance();
                                MatchPattern::Num(n)
                            }
                            TokKind::Minus => {
                                self.advance();
                                let n = match self.peek().clone() {
                                    TokKind::Num(n) => n,
                                    _ => return Err(format!(
                                        "Ekspektasi angka setelah '-' di pattern match, baris {}",
                                        self.line()
                                    )),
                                };
                                self.advance();
                                MatchPattern::Num(-n)
                            }
                            TokKind::Ident(id) if id == "_" => {
                                self.advance();
                                seen_wildcard = true;
                                MatchPattern::Wildcard
                            }
                            other => {
                                return Err(format!(
                                    "Pattern match tidak valid: {:?} di baris {}",
                                    other,
                                    self.line()
                                ))
                            }
                        };
                        self.expect(&TokKind::Arrow, "'=>' (arm match)")?;
                        // Body arm: blok `{ ... }` ATAU satu statement/ekspresi
                        // tanpa kurung (roadmap: `"ask" => process_query()`).
                        let body = if self.peek() == &TokKind::LBrace {
                            self.parse_block()?
                        } else {
                            vec![self.parse_stmt()?]
                        };
                        arms.push(MatchArm { pattern, body });
                    }
                    self.advance(); // }
                    if arms.is_empty() {
                        return Err("match tanpa arm".into());
                    }
                    Ok(Stmt::Match { subject, arms })
                }
                "if" => {
                    self.advance();
                    let cond = self.parse_expr()?;
                    let then_branch = self.parse_block()?;
                    let mut else_branch = Vec::new();
                    if self.peek() == &TokKind::Ident("else".to_string()) {
                        self.advance();
                        else_branch = self.parse_block()?;
                    }
                    Ok(Stmt::If { cond, then_branch, else_branch })
                }
                "return" => {
                    self.advance();
                    let value = self.parse_expr()?;
                    Ok(Stmt::Return(value))
                }
                "while" => {
                    // while cond { ... } — loop bounded (deterministik, P1)
                    self.advance();
                    let cond = self.parse_expr()?;
                    let body = self.parse_block()?;
                    Ok(Stmt::While { cond, body })
                }
                "for" => {
                    // for x in start end { ... } — iterasi [start, end), step 1
                    self.advance();
                    let var = self.expect_ident("nama variabel loop")?;
                    if self.peek() == &TokKind::Ident("in".to_string()) {
                        self.advance();
                    } else {
                        return Err(format!(
                            "Ekspektasi 'in' pada for di baris {}",
                            self.line()
                        ));
                    }
                    let start = self.parse_expr()?;
                    let end = self.parse_expr()?;
                    let body = self.parse_block()?;
                    Ok(Stmt::For { var, start, end, body })
                }
                "on" => {
                    // Handler hanya diizinkan di level entity / top-level, BUKAN
                    // di dalam statement (spec §4.5, EBNF handler ::= "on" event_name ...).
                    // Dulu di-parse lalu dibuang diam-diam — sekarang error eksplisit
                    // agar deterministik (KB §5.1: unknown usage = non-conforming).
                    // Namun `on` TETAP identifier bebas (P4 — tanpa reserved words):
                    // error hanya saat `on` diikuti event yang dikenal, sehingga
                    // user masih boleh memakai `on` sebagai nama fungsi/variabel.
                    let is_handler_attempt = matches!(
                        self.peek2(),
                        TokKind::Ident(ev) if matches!(ev.as_str(), "frame" | "speak" | "silent" | "click")
                    );
                    if is_handler_attempt {
                        return Err(format!(
                            "Handler 'on' hanya diizinkan di level entity/top-level, bukan di dalam statement (baris {})",
                            self.line()
                        ));
                    }
                    // jatuh ke penanganan ident biasa (assign / call / expr)
                    let name = kw;
                    if self.peek2() == &TokKind::Assign {
                        self.advance();
                        self.advance();
                        let value = self.parse_expr()?;
                        Ok(Stmt::Assign { name, value })
                    } else {
                        let e = self.parse_expr()?;
                        Ok(Stmt::ExprStmt(e))
                    }
                }
                _ => {
                    // assign `x = expr` atau call `name(...)` atau expr
                    let name = kw;
                    if self.peek2() == &TokKind::Assign {
                        self.advance();
                        self.advance();
                        let value = self.parse_expr()?;
                        Ok(Stmt::Assign { name, value })
                    } else {
                        let e = self.parse_expr()?;
                        Ok(Stmt::ExprStmt(e))
                    }
                }
            },
            TokKind::LBrace => Ok(Stmt::Block(self.parse_block()?)),
            _ => {
                let e = self.parse_expr()?;
                Ok(Stmt::ExprStmt(e))
            }
        }
    }

    fn expect_str(&mut self) -> Result<String, String> {
        match self.peek().clone() {
            TokKind::Str(s) => {
                self.advance();
                Ok(s)
            }
            _ => Err(format!("Ekspektasi string di baris {}", self.line())),
        }
    }

    // ── Expressions (precedence climbing) ──
    fn parse_expr(&mut self) -> Result<Expr, String> {
        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_additive()?;
        loop {
            let op = match self.peek() {
                TokKind::Eq => Some(BinOp::Eq),
                TokKind::Ne => Some(BinOp::Ne),
                TokKind::Lt => Some(BinOp::Lt),
                TokKind::Gt => Some(BinOp::Gt),
                TokKind::Le => Some(BinOp::Le),
                TokKind::Ge => Some(BinOp::Ge),
                _ => None,
            };
            match op {
                Some(op) => {
                    self.advance();
                    let rhs = self.parse_additive()?;
                    lhs = Expr::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs) };
                }
                None => break,
            }
        }
        Ok(lhs)
    }

    fn parse_additive(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_multiplicative()?;
        loop {
            let op = match self.peek() {
                TokKind::Plus => Some(BinOp::Add),
                TokKind::Minus => Some(BinOp::Sub),
                _ => None,
            };
            match op {
                Some(op) => {
                    self.advance();
                    let rhs = self.parse_multiplicative()?;
                    lhs = Expr::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs) };
                }
                None => break,
            }
        }
        Ok(lhs)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                TokKind::Star => Some(BinOp::Mul),
                TokKind::Slash => Some(BinOp::Div),
                TokKind::Percent => Some(BinOp::Mod),
                _ => None,
            };
            match op {
                Some(op) => {
                    self.advance();
                    let rhs = self.parse_unary()?;
                    lhs = Expr::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs) };
                }
                None => break,
            }
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        if self.peek() == &TokKind::Minus {
            self.advance();
            let inner = self.parse_unary()?;
            return Ok(Expr::UnaryMinus(Box::new(inner)));
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        match self.peek().clone() {
            TokKind::Num(n) => {
                self.advance();
                Ok(Expr::Num(n))
            }
            TokKind::Str(s) => {
                self.advance();
                Ok(Expr::Str(s))
            }
            TokKind::Ident(id) => {
                let name = id.clone();
                self.advance();
                if self.peek() == &TokKind::LParen {
                    self.advance();
                    let args = self.parse_paren_list()?;
                    Ok(Expr::Call { name, args, props: None })
                } else {
                    match name.as_str() {
                        "true" => Ok(Expr::Bool(true)),
                        "false" => Ok(Expr::Bool(false)),
                        _ => Ok(Expr::Ident(name)),
                    }
                }
            }
            TokKind::LParen => {
                self.advance();
                let items = self.parse_paren_list()?;
                Ok(Expr::Tuple(items))
            }
            TokKind::LBracket => {
                // List literal (v1.6.0): [ expr, expr ] — koma opsional
                self.advance();
                let mut items = Vec::new();
                while self.peek() != &TokKind::RBracket {
                    if self.peek() == &TokKind::Eof {
                        return Err(format!("']' tidak ditutup, baris {}", self.line()));
                    }
                    if self.peek() == &TokKind::Comma {
                        self.advance();
                        continue;
                    }
                    items.push(self.parse_expr()?);
                }
                self.advance(); // ]
                Ok(Expr::List(items))
            }
            TokKind::LBrace => {
                // Map literal (v1.6.0): { key: expr, key2: expr }
                self.advance();
                let mut pairs = Vec::new();
                while self.peek() != &TokKind::RBrace {
                    if self.peek() == &TokKind::Eof {
                        return Err(format!("map tidak ditutup, baris {}", self.line()));
                    }
                    if self.peek() == &TokKind::Comma {
                        self.advance();
                        continue;
                    }
                    let key = self.expect_ident("kunci map")?;
                    self.expect(&TokKind::Colon, "':' (kunci map)")?;
                    let val = self.parse_expr()?;
                    pairs.push((key, val));
                }
                self.advance(); // }
                Ok(Expr::Map(pairs))
            }
            other => Err(format!("Ekspektasi ekspresi, dapat {:?} di baris {}", other, self.line())),
        }
    }

    /// Daftar ekspresi di dalam ( ) dipisah spasi / koma.
    fn parse_paren_list(&mut self) -> Result<Vec<Expr>, String> {
        let mut items = Vec::new();
        loop {
            if self.peek() == &TokKind::RParen {
                break;
            }
            if self.peek() == &TokKind::Eof {
                return Err(format!("')' tidak ditutup, baris {}", self.line()));
            }
            if self.peek() == &TokKind::Comma {
                self.advance();
                continue;
            }
            items.push(self.parse_expr()?);
        }
        self.expect(&TokKind::RParen, "')'")?;
        Ok(items)
    }
}

fn is_builder(id: &str) -> bool {
    // SUMBER TUNGGAL KEBENARAN: daftar builder hidup di registry.rs
    // (MESH_BUILDERS / MATERIAL_BUILDERS). Parser tidak menduplikasi daftar —
    // tambah builder baru cukup di registry.rs, parser ikut otomatis.
    crate::registry::is_builder(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_world() {
        let src = r#"
            world "ADI Hologram" {
                camera "cam" { pos (0 1.6 7) look (0 0 0) fov 55 }
                light "key" { type point pos (5 6 4) color (1 0.95 0.9) intensity 1.5 }
                entity "core" {
                    pos (0 0 0)
                    mesh sphere { radius 0.8 segments 3 }
                    material wire (0.15 0.8 1) 0.9
                    on frame {
                        rotate(0.35 * t, (0.15 1 0.1))
                        scaleBy(1 + 0.05 * sin(2.1 * t))
                    }
                }
            }
        "#;
        let prog = parse(src).expect("parse");
        assert_eq!(prog.name, "ADI Hologram");
        assert_eq!(prog.items.len(), 3);
        match &prog.items[2] {
            TopLevel::Entity(e) => {
                assert_eq!(e.id, "core");
                assert_eq!(e.handlers.len(), 1);
                assert_eq!(e.props.len(), 3);
            }
            _ => panic!("bukan entity"),
        }
    }

    #[test]
    fn parse_tuples_and_math() {
        let src = "world \"w\" { entity \"e\" { on frame { let a = t * 1.2
setPos(2.1 * cos(a), 0.35 * sin(2.3 * t), 2.1 * sin(a)) } } }";
        let prog = parse(src).unwrap();
        assert_eq!(prog.items.len(), 1);
    }

    #[test]
    fn parse_rejects_handler_inside_statement() {
        // Handler `on` HANYA diizinkan di level entity/top-level (spec §4.5).
        // Dulu di-parse lalu dibuang diam-diam; sekarang harus error eksplisit.
        let src = "world \"w\" { entity \"e\" { on frame { on click { } } } }";
        let res = parse(src);
        assert!(res.is_err(), "handler di dalam statement harus ditolak");
        let msg = res.unwrap_err();
        assert!(msg.contains("on"), "error harus menyebut 'on': {msg}");
    }

    #[test]
    fn parse_while_loop() {
        // v1.3.0 (Extension Protocol §11): while cond { ... } — loop bounded.
        let src = r#"
            world "T" {
                entity "e" {
                    on frame {
                        let i = 0
                        while i < 10 {
                            i = i + 1
                        }
                    }
                }
            }
        "#;
        let prog = parse(src).expect("while loop harus di-parse");
        assert_eq!(prog.items.len(), 1);
    }

    #[test]
    fn parse_for_loop() {
        // v1.3.0 (Extension Protocol §11): for x in start end { ... }
        let src = r#"
            world "T" {
                entity "e" {
                    on frame {
                        for i in 0 10 {
                            rotate(0.1, (0 1 0))
                        }
                    }
                }
            }
        "#;
        let prog = parse(src).expect("for loop harus di-parse");
        assert_eq!(prog.items.len(), 1);
    }

    #[test]
    fn parse_list_and_map_literals() {
        // v1.6.0: List [ ... ] dan Map { key: value } dalam ekspresi.
        let src = r#"
            world "T" {
                entity "e" {
                    on frame {
                        let tags = ["fastapi", "crewai", "adilang"]
                        let cfg = { timeout: 30, retry: 3 }
                    }
                }
            }
        "#;
        let prog = parse(src).expect("list/map harus di-parse");
        assert_eq!(prog.items.len(), 1);
    }

    #[test]
    fn parse_match_statement() {
        // v1.6.0: match subject { "pat" => { ... } _ => { ... } }
        let src = r#"
            world "T" {
                entity "e" {
                    on frame {
                        match verb {
                            "ask" => { rotate(0.1, (0 1 0)) }
                            "command" => { rotate(0.2, (0 1 0)) }
                            _ => { rotate(0.05, (0 1 0)) }
                        }
                    }
                }
            }
        "#;
        let prog = parse(src).expect("match harus di-parse");
        assert_eq!(prog.items.len(), 1);
    }

    #[test]
    fn parse_match_numeric_pattern_and_no_wildcard() {
        // Angka + unary minus sebagai pattern, tanpa wildcard tetap boleh parse
        let src = r#"
            world "T" {
                entity "e" {
                    on frame {
                        match n {
                            1 => { rotate(0.1, (0 1 0)) }
                            -2 => { rotate(0.2, (0 1 0)) }
                        }
                    }
                }
            }
        "#;
        let prog = parse(src).expect("match numerik harus di-parse");
        assert_eq!(prog.items.len(), 1);
    }

    #[test]
    fn parse_match_arm_bare_expression() {
        // Roadmap: arm tanpa kurung `"ask" => process_query()` — body = satu
        // statement/ekspresi (bukan wajib blok `{ ... }`).
        let src = r#"
            world "T" {
                entity "e" {
                    on frame {
                        match verb {
                            "ask" => rotate(0.1, (0 1 0))
                            _ => rotate(0.2, (0 1 0))
                        }
                    }
                }
            }
        "#;
        let prog = parse(src).expect("match arm tanpa kurung harus di-parse");
        assert_eq!(prog.items.len(), 1);
    }

    #[test]
    fn parse_match_wildcard_must_be_last() {
        // Docstring MatchPattern: wildcard wajib arm terakhir — harus di-enforce.
        let src = "world \"w\" { entity \"e\" { on frame { match x { _ => { } \"a\" => { } } } } }";
        let res = parse(src);
        assert!(res.is_err(), "wildcard sebelum arm lain harus ditolak");
        let msg = res.unwrap_err();
        assert!(msg.contains("TERAKHIR"), "error harus menyebut wildcard-last: {msg}");
    }

    #[test]
    fn parse_match_requires_arrow() {
        let src = "world \"w\" { entity \"e\" { on frame { match x { \"a\" { } } } } }";
        let res = parse(src);
        assert!(res.is_err(), "arm tanpa '=>' harus ditolak");
    }

    #[test]
    fn parse_tuple_destructuring() {
        // v1.6.0: let (code, msg) = get_status()
        let src = r#"
            world "T" {
                func get_status() { return (200, "OK") }
                entity "e" {
                    on frame {
                        let (code, msg) = get_status()
                    }
                }
            }
        "#;
        let prog = parse(src).expect("destructuring harus di-parse");
        assert_eq!(prog.items.len(), 2);
    }

    #[test]
    fn parse_for_requires_in() {
        // `for` tanpa 'in' harus error (bukan salah parse diam-diam).
        let src = "world \"w\" { entity \"e\" { on frame { for i 0 10 { } } } }";
        let res = parse(src);
        assert!(res.is_err(), "for tanpa 'in' harus ditolak");
        let msg = res.unwrap_err();
        assert!(msg.contains("in"), "error harus menyebut 'in': {msg}");
    }

    // ═══════════════════════════════════════════════════════════════════════
    // FUZZ / BENCHMARK (P3.10, P1 determinism)
    // ═══════════════════════════════════════════════════════════════════════
    // LCG deterministik (tanpa RNG global, tanpa dependensi) — seed tetap
    // membuat fuzz REPRODUCIBLE: kegagalan yang sama selalu terulang.
    struct Lcg(u64);
    impl Lcg {
        fn new(seed: u64) -> Self {
            Self(seed)
        }
        fn next(&mut self) -> u64 {
            // Numerical Recipes LCG — cukup untuk fuzz, deterministik
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.0 >> 33
        }
        fn range(&mut self, lo: usize, hi: usize) -> usize {
            if hi <= lo {
                lo
            } else {
                lo + (self.next() as usize) % (hi - lo)
            }
        }
        fn num(&mut self, scale: f64) -> f64 {
            let v = (self.next() % 1000) as f64 / 1000.0;
            (v * scale * 100.0).round() / 100.0
        }
    }

    /// Bangun world ADILang acak-yet-valid dari registry tertutup (P6).
    /// Semua builder diambil dari MESH_BUILDERS/MATERIAL_BUILDERS (sumber
    /// tunggal) sehingga hasilnya dijamin berada dalam kosakata bahasa.
    fn random_world(rng: &mut Lcg) -> String {
        use crate::registry;
        let meshes: Vec<&str> = registry::mesh_builder_names().collect();
        let mats: Vec<&str> = registry::material_builder_names().collect();
        let transforms = [
            "move", "setPos", "setScale", "scaleBy", "rotate", "setColor", "setAlpha",
        ];
        let math1 = ["sin", "cos", "tan", "sqrt", "abs", "floor"];

        let mut s = String::from("world \"fuzz\" {\n");
        s.push_str("  camera \"cam\" { pos (0 1.6 6) look (0 0 0) fov 55 }\n");
        s.push_str("  light \"key\" { type point pos (4 5 3) color (1 0.95 0.9) intensity 1.4 }\n");
        let n_entities = rng.range(1, 4);
        for e in 0..n_entities {
            s.push_str(&format!("  entity \"e{e}\" {{\n"));
            s.push_str("    pos (0 0 0)\n");
            let m = meshes[rng.range(0, meshes.len())];
            let radius = rng.range(1, 5);
            let seg = rng.range(3, 8);
            s.push_str(&format!("    mesh {m} {{ radius {radius} segments {seg} }}\n"));
            let mat = mats[rng.range(0, mats.len())];
            let (r, g, b) = (rng.num(1.0), rng.num(1.0), rng.num(1.0));
            let alpha = rng.range(0, 2);
            s.push_str(&format!("    material {mat} ({r} {g} {b}) {alpha}\n"));
            s.push_str("    on frame {\n");
            // transform acak dari registry transform
            let tf = transforms[rng.range(0, transforms.len())];
            let sp = rng.num(1.0);
            let (ax, ay, az) = (rng.num(1.0), rng.num(1.0), rng.num(1.0));
            s.push_str(&format!("      {tf}({sp} * t, ({ax} {ay} {az}))\n"));
            // math 1-arg kadang disertakan
            if rng.range(0, 3) == 0 {
                let fn1 = math1[rng.range(0, math1.len())];
                s.push_str(&format!("      setScale(1 + 0.05 * {fn1}({sp} * t))\n"));
            }
            // loop bounded (v1.3.0) kadang disertakan
            if rng.range(0, 3) == 0 {
                s.push_str("      let i = 0\n");
                s.push_str("      while i < 3 {\n");
                s.push_str("        i = i + 1\n");
                s.push_str("      }\n");
            }
            if rng.range(0, 3) == 0 {
                s.push_str("      for k in 0 2 {\n");
                s.push_str("        rotate(0.1, (0 1 0))\n");
                s.push_str("      }\n");
            }
            s.push_str("    }\n");
            s.push_str("  }\n");
        }
        s.push_str("}\n");
        s
    }

    #[test]
    fn fuzz_random_worlds_selalu_valid_dan_reproducible() {
        // P3.10 — bukti determinisme (P1): semua world acak dari registry harus
        // valid, dan seed yang sama menghasilkan urutan parse yang sama.
        let mut rng = Lcg::new(0xAD1_2026);
        for _ in 0..300 {
            let src = random_world(&mut rng);
            let res = parse(&src);
            assert!(
                res.is_ok(),
                "world acak harus valid (P1 determinism):\n{src}\nerr: {:?}",
                res.err()
            );
        }
        // Reproducibility: seed sama → generator menghasilkan source yang sama
        let mut a = Lcg::new(7);
        let mut b = Lcg::new(7);
        for _ in 0..50 {
            assert_eq!(random_world(&mut a), random_world(&mut b), "fuzz harus reproducible");
        }
    }

    #[test]
    fn fuzz_mutasi_tidak_pernah_panic() {
        // P1 determinism — parser TIDAK boleh panic pada input apa pun:
        // hasil Ok atau Err sama-sama sah, yang penting tidak crash.
        let base = include_str!("../worlds/default.adi");
        let base: Vec<char> = base.chars().collect();
        // v1.6.0: sertakan token baru [ ] : => (=> dibentuk '='+'>') agar mutasi
        // benar-benar melatih cabang lexer baru (list/map/match).
        let alphabet: Vec<char> = "(){}\",.=+-*/%<>_:[]0123456789abcdefghijklmnopqrstuvwxyz "
            .chars()
            .collect();
        let mut rng = Lcg::new(0xC0FFEE);
        for _ in 0..500 {
            let mut chars = base.clone();
            for _ in 0..rng.range(1, 8) {
                let idx = rng.range(0, chars.len().max(1));
                match rng.range(0, 3) {
                    0 => chars[idx] = alphabet[rng.range(0, alphabet.len())],
                    1 => chars.insert(idx, alphabet[rng.range(0, alphabet.len())]),
                    _ => {
                        if chars.len() > 1 {
                            chars.remove(idx);
                        }
                    }
                }
            }
            let mutated: String = chars.into_iter().collect();
            let _ = parse(&mutated); // Ok atau Err — tidak boleh panic
        }
    }
}
