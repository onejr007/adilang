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
                    let name = self.expect_ident("nama variabel")?;
                    self.expect(&TokKind::Assign, "'='")?;
                    let value = self.parse_expr()?;
                    Ok(Stmt::Let { name, value })
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
}
