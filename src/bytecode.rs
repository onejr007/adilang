// ADILang binary protocol — compact AST serializer/deserializer (v2.0.0 → v1.10.0).
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).
//
// Tujuan: AI-to-AI communication dengan konsumsi token seminimal mungkin
// (Zero-Token-Waste Format). Memanfaatkan closed-vocabulary ADILang untuk
// mengencode identifier sebagai small integers.
//
// Format (v0x04 — compact string table):
//   Header: [0xAD, 0x04, flags]
//   Blocks: [type, ...payload]
//   String: kata registry tertutup di-encode 2 byte (0xFE + index 0..255);
//           string lain memakai prefix-panjang raw (lihat write_string).

#![allow(dead_code)]

use std::sync::OnceLock;

use crate::ast::*;
use crate::registry;

// ═══════════════════════════════════════════════════════════════════════════
// CONSTANTS
// ═══════════════════════════════════════════════════════════════════════════

const MAGIC: u8 = 0xAD;
const VERSION_BIN: u8 = 0x04;

// Tag string (bukan flag struktur) — lihat write_string/read_string.
const STR_EMPTY: u8 = 0x00;      // string kosong (1 byte)
const STR_RAW_MAX: u8 = 0xFD;    // string mentah dengan panjang 1..=253
const STR_REGISTRY: u8 = 0xFE;   // kata registry tertutup: 0xFE + index
const STR_LONG: u8 = 0xFF;       // string panjang: 0xFF + u32 + bytes

// Block types
const BLOCK_PAYLOAD: u8 = 0x01;
const BLOCK_UI_LAYOUT: u8 = 0x02;
const BLOCK_SPATIAL_3D: u8 = 0x03;
const BLOCK_WORLD: u8 = 0x04;
const BLOCK_CAMERA: u8 = 0x05;
const BLOCK_LIGHT: u8 = 0x06;
const BLOCK_ENTITY: u8 = 0x07;
const BLOCK_LET: u8 = 0x08;
const BLOCK_FUNC: u8 = 0x09;
const BLOCK_HANDLER: u8 = 0x0A;
const BLOCK_USE_JS: u8 = 0x0B;
const BLOCK_ROUTES: u8 = 0x0C;
const BLOCK_I18N: u8 = 0x0D;
const BLOCK_COMPONENT: u8 = 0x0E; // v1.13.0 — lifecycle hooks component

// Expression types
const EXPR_NUM: u8 = 0x10;
const EXPR_STR: u8 = 0x11;
const EXPR_BOOL: u8 = 0x12;
const EXPR_TUPLE: u8 = 0x13;
const EXPR_LIST: u8 = 0x14;
const EXPR_MAP: u8 = 0x15;
const EXPR_IDENT: u8 = 0x16;
const EXPR_CALL: u8 = 0x17;
const EXPR_UNARY_MINUS: u8 = 0x18;
const EXPR_BINARY: u8 = 0x19;

// Property types
const PROP: u8 = 0x20;

// Statement types
const STMT_LET: u8 = 0x30;
const STMT_LET_DESTRUCTURE: u8 = 0x31;
const STMT_ASSIGN: u8 = 0x32;
const STMT_EXPR: u8 = 0x33;
const STMT_RETURN: u8 = 0x34;
const STMT_BLOCK: u8 = 0x35;
const STMT_IF: u8 = 0x36;
const STMT_WHILE: u8 = 0x37;
const STMT_FOR: u8 = 0x38;
const STMT_MATCH: u8 = 0x39;
const STMT_NAVIGATE: u8 = 0x3A;
const STMT_SET_LOCALE: u8 = 0x3B;
const STMT_DIRECTIVE: u8 = 0x3C; // v1.13.0 — directive generik @name(args)

// Match pattern types
const PATTERN_STR: u8 = 0x40;
const PATTERN_NUM: u8 = 0x41;
const PATTERN_WILDCARD: u8 = 0x42;

// Event kinds
const EVENT_FRAME: u8 = 0x50;
const EVENT_SPEAK: u8 = 0x51;
const EVENT_SILENT: u8 = 0x52;
const EVENT_CLICK: u8 = 0x53;

// UI Component types
const UI_CONTAINER: u8 = 0x60;
const UI_TEXT: u8 = 0x61;
const UI_BUTTON: u8 = 0x62;
const UI_INPUT: u8 = 0x63;
const UI_CARD: u8 = 0x64;
const UI_MODAL: u8 = 0x65;
const UI_NAVBAR: u8 = 0x66;
const UI_FOOTER: u8 = 0x67;

// Lifecycle hook kinds (v1.13.0) — component on_mount/on_update/on_unmount
const HOOK_MOUNT: u8 = 0x70;
const HOOK_UPDATE: u8 = 0x71;
const HOOK_UNMOUNT: u8 = 0x72;

// Flex direction
const FLEX_ROW: u8 = 0x00;
const FLEX_COLUMN: u8 = 0x01;
const FLEX_NONE: u8 = 0xFF;

// Binary operators
const BINOP_ADD: u8 = 0x00;
const BINOP_SUB: u8 = 0x01;
const BINOP_MUL: u8 = 0x02;
const BINOP_DIV: u8 = 0x03;
const BINOP_MOD: u8 = 0x04;
const BINOP_EQ: u8 = 0x05;
const BINOP_NE: u8 = 0x06;
const BINOP_LT: u8 = 0x07;
const BINOP_GT: u8 = 0x08;
const BINOP_LE: u8 = 0x09;
const BINOP_GE: u8 = 0x0A;

// ═══════════════════════════════════════════════════════════════════════════
// STRING TABLE (closed vocabulary encoding)
// ═══════════════════════════════════════════════════════════════════════════

/// Daftar kata registry tertutup (P6) — dibangun sekali dari
/// `registry::registry_text()` dan di-cache (OnceLock). Encoder & decoder
/// memakai daftar IDENTIK yang sama → deterministik, tanpa tabel serial.
fn registry_words() -> &'static Vec<String> {
    static WORDS: OnceLock<Vec<String>> = OnceLock::new();
    WORDS.get_or_init(|| {
        let text = registry::registry_text();
        let mut words = Vec::new();
        for line in text.lines() {
            if let Some(colon_pos) = line.find(':') {
                let values = &line[colon_pos + 1..];
                for word in values.split_whitespace() {
                    if !word.is_empty() {
                        words.push(word.to_string());
                    }
                }
            }
        }
        words
    })
}

/// Indeks kata registry (≤255) — kata yang tidak ada di registry → None
/// (di-encode sebagai string raw biasa).
fn registry_word_index(s: &str) -> Option<u8> {
    registry_words()
        .iter()
        .position(|w| w == s)
        .and_then(|i| u8::try_from(i).ok())
}

/// Kata registry pada indeks tertentu (kebalikan `registry_word_index`).
fn registry_word_at(idx: u8) -> Option<String> {
    registry_words().get(idx as usize).cloned()
}

// ═══════════════════════════════════════════════════════════════════════════
// ENCODER
// ═══════════════════════════════════════════════════════════════════════════

pub fn encode_program(program: &Program) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    
    out.push(MAGIC);
    out.push(VERSION_BIN);
    out.push(0x00); // flags (reserved)
    
    write_string(program.name.as_str(), &mut out);
    
    for item in &program.items {
        encode_top_level(item, &mut out)?;
    }
    
    Ok(out)
}

fn encode_top_level(item: &TopLevel, out: &mut Vec<u8>) -> Result<(), String> {
    match item {
        TopLevel::Payload(p) => {
            out.push(BLOCK_PAYLOAD);
            write_string(&p.sender, out);
            write_string(&p.target_agent, out);
            write_string(&p.intent, out);
            match &p.state_data {
                Some(e) => {
                    out.push(1);
                    encode_expr(e, out)?;
                }
                None => out.push(0),
            }
        }
        TopLevel::UILayout(layout) => {
            out.push(BLOCK_UI_LAYOUT);
            write_string(&layout.name, out);
            encode_ui_component(&layout.root, out)?;
        }
        TopLevel::Spatial3D(def) => {
            out.push(BLOCK_SPATIAL_3D);
            write_string(&def.name, out);
            out.push(def.items.len() as u8);
            for item in &def.items {
                encode_spatial_item(item, out)?;
            }
        }
        TopLevel::World(def) => {
            out.push(BLOCK_WORLD);
            write_string(&def.name, out);
            out.push(def.items.len() as u8);
            for item in &def.items {
                encode_spatial_item(item, out)?;
            }
        }
        TopLevel::Camera(c) => {
            out.push(BLOCK_CAMERA);
            write_string(&c.id, out);
            for p in &c.props {
                encode_prop(p, out)?;
            }
        }
        TopLevel::Light(l) => {
            out.push(BLOCK_LIGHT);
            write_string(&l.id, out);
            for p in &l.props {
                encode_prop(p, out)?;
            }
        }
        TopLevel::Entity(e) => {
            out.push(BLOCK_ENTITY);
            write_string(&e.id, out);
            for p in &e.props {
                encode_prop(p, out)?;
            }
            out.push(e.handlers.len() as u8);
            for h in &e.handlers {
                encode_handler(h, out)?;
            }
        }
        TopLevel::Let { name, value } => {
            out.push(BLOCK_LET);
            write_string(name, out);
            encode_expr(value, out)?;
        }
        TopLevel::Func(f) => {
            out.push(BLOCK_FUNC);
            write_string(&f.name, out);
            out.push(f.params.len() as u8);
            for p in &f.params {
                write_string(p, out);
            }
            encode_block(&f.body, out)?;
        }
        TopLevel::Handler(h) => {
            out.push(BLOCK_HANDLER);
            encode_handler(h, out)?;
        }
        TopLevel::UseJs(u) => {
            out.push(BLOCK_USE_JS);
            write_string(&u.url, out);
        }
        TopLevel::Routes(r) => {
            out.push(BLOCK_ROUTES);
            out.push(r.routes.len() as u8);
            for route in &r.routes {
                write_string(&route.path, out);
                write_string(&route.layout, out);
                match &route.transition {
                    Some(t) => {
                        out.push(1);
                        write_string(t, out);
                    }
                    None => out.push(0),
                }
            }
        }
        TopLevel::I18n(i) => {
            out.push(BLOCK_I18N);
            out.push(i.locales.len() as u8);
            for loc in &i.locales {
                write_string(&loc.name, out);
                out.push(loc.entries.len() as u8);
                for (k, v) in &loc.entries {
                    write_string(k, out);
                    write_string(v, out);
                }
            }
        }
        TopLevel::Component(c) => {
            out.push(BLOCK_COMPONENT);
            write_string(&c.name, out);
            out.push(c.hooks.len() as u8);
            for h in &c.hooks {
                match h.kind {
                    LifecycleHookKind::Mount => out.push(HOOK_MOUNT),
                    LifecycleHookKind::Update => out.push(HOOK_UPDATE),
                    LifecycleHookKind::Unmount => out.push(HOOK_UNMOUNT),
                }
                encode_block(&h.body, out)?;
            }
        }
    }
    Ok(())
}

fn encode_spatial_item(item: &SpatialItem, out: &mut Vec<u8>) -> Result<(), String> {
    match item {
        SpatialItem::Camera(c) => {
            out.push(BLOCK_CAMERA);
            write_string(&c.id, out);
            for p in &c.props {
                encode_prop(p, out)?;
            }
        }
        SpatialItem::Light(l) => {
            out.push(BLOCK_LIGHT);
            write_string(&l.id, out);
            for p in &l.props {
                encode_prop(p, out)?;
            }
        }
        SpatialItem::Entity(e) => {
            out.push(BLOCK_ENTITY);
            write_string(&e.id, out);
            for p in &e.props {
                encode_prop(p, out)?;
            }
            out.push(e.handlers.len() as u8);
            for h in &e.handlers {
                encode_handler(h, out)?;
            }
        }
        SpatialItem::Let { name, value } => {
            out.push(BLOCK_LET);
            write_string(name, out);
            encode_expr(value, out)?;
        }
        SpatialItem::Func(f) => {
            out.push(BLOCK_FUNC);
            write_string(&f.name, out);
            out.push(f.params.len() as u8);
            for p in &f.params {
                write_string(p, out);
            }
            encode_block(&f.body, out)?;
        }
        SpatialItem::Handler(h) => {
            out.push(BLOCK_HANDLER);
            encode_handler(h, out)?;
        }
    }
    Ok(())
}

fn encode_prop(p: &Prop, out: &mut Vec<u8>) -> Result<(), String> {
    out.push(PROP);
    write_string(&p.name, out);
    encode_expr(&p.value, out)?;
    Ok(())
}

fn encode_handler(h: &Handler, out: &mut Vec<u8>) -> Result<(), String> {
    match h.event {
        EventKind::Frame => out.push(EVENT_FRAME),
        EventKind::Speak => out.push(EVENT_SPEAK),
        EventKind::Silent => out.push(EVENT_SILENT),
        EventKind::Click => out.push(EVENT_CLICK),
    }
    encode_block(&h.body, out)?;
    Ok(())
}

fn encode_expr(e: &Expr, out: &mut Vec<u8>) -> Result<(), String> {
    match e {
        Expr::Num(n) => {
            out.push(EXPR_NUM);
            out.extend_from_slice(&n.to_le_bytes());
        }
        Expr::Str(s) => {
            out.push(EXPR_STR);
            write_string(s, out);
        }
        Expr::Bool(b) => {
            out.push(EXPR_BOOL);
            out.push(if *b { 1 } else { 0 });
        }
        Expr::Tuple(items) => {
            out.push(EXPR_TUPLE);
            out.push(items.len() as u8);
            for item in items {
                encode_expr(item, out)?;
            }
        }
        Expr::List(items) => {
            out.push(EXPR_LIST);
            out.push(items.len() as u8);
            for item in items {
                encode_expr(item, out)?;
            }
        }
        Expr::Map(pairs) => {
            out.push(EXPR_MAP);
            out.push(pairs.len() as u8);
            for (k, v) in pairs {
                write_string(k, out);
                encode_expr(v, out)?;
            }
        }
        Expr::Ident(s) => {
            out.push(EXPR_IDENT);
            write_string(s, out);
        }
        Expr::Call { name, args, props } => {
            out.push(EXPR_CALL);
            write_string(name, out);
            out.push(args.len() as u8);
            for arg in args {
                encode_expr(arg, out)?;
            }
            match props {
                Some(ps) => {
                    out.push(1);
                    out.push(ps.len() as u8);
                    for p in ps {
                        encode_prop(p, out)?;
                    }
                }
                None => out.push(0),
            }
        }
        Expr::UnaryMinus(inner) => {
            out.push(EXPR_UNARY_MINUS);
            encode_expr(inner, out)?;
        }
        Expr::Binary { op, lhs, rhs } => {
            out.push(EXPR_BINARY);
            out.push(match op {
                BinOp::Add => BINOP_ADD,
                BinOp::Sub => BINOP_SUB,
                BinOp::Mul => BINOP_MUL,
                BinOp::Div => BINOP_DIV,
                BinOp::Mod => BINOP_MOD,
                BinOp::Eq => BINOP_EQ,
                BinOp::Ne => BINOP_NE,
                BinOp::Lt => BINOP_LT,
                BinOp::Gt => BINOP_GT,
                BinOp::Le => BINOP_LE,
                BinOp::Ge => BINOP_GE,
            });
            encode_expr(lhs, out)?;
            encode_expr(rhs, out)?;
        }
    }
    Ok(())
}

fn encode_block(stmts: &[Stmt], out: &mut Vec<u8>) -> Result<(), String> {
    out.push(STMT_BLOCK);
    out.push(stmts.len() as u8);
    for s in stmts {
        encode_stmt(s, out)?;
    }
    Ok(())
}

fn encode_stmt(s: &Stmt, out: &mut Vec<u8>) -> Result<(), String> {
    match s {
        Stmt::Let { name, value } => {
            out.push(STMT_LET);
            write_string(name, out);
            encode_expr(value, out)?;
        }
        Stmt::LetDestructure { names, value } => {
            out.push(STMT_LET_DESTRUCTURE);
            out.push(names.len() as u8);
            for n in names {
                write_string(n, out);
            }
            encode_expr(value, out)?;
        }
        Stmt::Assign { name, value } => {
            out.push(STMT_ASSIGN);
            write_string(name, out);
            encode_expr(value, out)?;
        }
        Stmt::ExprStmt(e) => {
            out.push(STMT_EXPR);
            encode_expr(e, out)?;
        }
        Stmt::Return(e) => {
            out.push(STMT_RETURN);
            encode_expr(e, out)?;
        }
        Stmt::Block(inner) => {
            encode_block(inner, out)?;
        }
        Stmt::If { cond, then_branch, else_branch } => {
            out.push(STMT_IF);
            encode_expr(cond, out)?;
            encode_block(then_branch, out)?;
            encode_block(else_branch, out)?;
        }
        Stmt::While { cond, body } => {
            out.push(STMT_WHILE);
            encode_expr(cond, out)?;
            encode_block(body, out)?;
        }
        Stmt::For { var, start, end, body } => {
            out.push(STMT_FOR);
            write_string(var, out);
            encode_expr(start, out)?;
            encode_expr(end, out)?;
            encode_block(body, out)?;
        }
        Stmt::Match { subject, arms } => {
            out.push(STMT_MATCH);
            encode_expr(subject, out)?;
            out.push(arms.len() as u8);
            for arm in arms {
                encode_match_arm(arm, out)?;
            }
        }
        Stmt::Navigate { path } => {
            out.push(STMT_NAVIGATE);
            write_string(path, out);
        }
        Stmt::SetLocale { locale } => {
            out.push(STMT_SET_LOCALE);
            write_string(locale, out);
        }
        Stmt::Directive { name, args } => {
            out.push(STMT_DIRECTIVE);
            write_string(name, out);
            out.push(args.len() as u8);
            for a in args {
                encode_expr(a, out)?;
            }
        }
    }
    Ok(())
}

fn encode_match_arm(arm: &MatchArm, out: &mut Vec<u8>) -> Result<(), String> {
    match &arm.pattern {
        MatchPattern::Str(s) => {
            out.push(PATTERN_STR);
            write_string(s, out);
        }
        MatchPattern::Num(n) => {
            out.push(PATTERN_NUM);
            out.extend_from_slice(&n.to_le_bytes());
        }
        MatchPattern::Wildcard => {
            out.push(PATTERN_WILDCARD);
        }
    }
    encode_block(&arm.body, out)?;
    Ok(())
}

fn encode_ui_component(comp: &UIComponent, out: &mut Vec<u8>) -> Result<(), String> {
    match comp {
        UIComponent::Container { flex, children } => {
            out.push(UI_CONTAINER);
            match flex {
                Some(FlexDirection::Row) => out.push(FLEX_ROW),
                Some(FlexDirection::Column) => out.push(FLEX_COLUMN),
                None => out.push(FLEX_NONE),
            }
            out.push(children.len() as u8);
            for child in children {
                encode_ui_component(child, out)?;
            }
        }
        UIComponent::Text { content } => {
            out.push(UI_TEXT);
            write_string(content, out);
        }
        UIComponent::Button { label, onClick } => {
            out.push(UI_BUTTON);
            write_string(label, out);
            match onClick {
                Some(h) => {
                    out.push(1);
                    write_string(h, out);
                }
                None => out.push(0),
            }
        }
        UIComponent::Input { name, placeholder, bind, validate } => {
            out.push(UI_INPUT);
            write_string(name, out);
            match placeholder {
                Some(p) => {
                    out.push(1);
                    write_string(p, out);
                }
                None => out.push(0),
            }
            match bind {
                Some(b) => {
                    out.push(1);
                    write_string(b, out);
                }
                None => out.push(0),
            }
            match validate {
                Some(v) => {
                    out.push(1);
                    write_string(v, out);
                }
                None => out.push(0),
            }
        }
        UIComponent::Card { title, children } => {
            out.push(UI_CARD);
            match title {
                Some(t) => {
                    out.push(1);
                    write_string(t, out);
                }
                None => out.push(0),
            }
            out.push(children.len() as u8);
            for child in children {
                encode_ui_component(child, out)?;
            }
        }
        UIComponent::Modal { title, children } => {
            out.push(UI_MODAL);
            match title {
                Some(t) => {
                    out.push(1);
                    write_string(t, out);
                }
                None => out.push(0),
            }
            out.push(children.len() as u8);
            for child in children {
                encode_ui_component(child, out)?;
            }
        }
        UIComponent::Navbar { title } => {
            out.push(UI_NAVBAR);
            match title {
                Some(t) => {
                    out.push(1);
                    write_string(t, out);
                }
                None => out.push(0),
            }
        }
        UIComponent::Footer { content } => {
            out.push(UI_FOOTER);
            write_string(content, out);
        }
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// HELPERS
// ═══════════════════════════════════════════════════════════════════════════

fn write_string(s: &str, out: &mut Vec<u8>) {
    // Kata registry tertutup → 2 byte (0xFE + index). Ini sumber penghematan
    // utama "Zero-Token-Waste": kata bahasa yang sering muncul (mode, payload,
    // entity, pos, sphere, ...) tidak membawa byte teksnya lagi.
    if let Some(idx) = registry_word_index(s) {
        out.push(STR_REGISTRY);
        out.push(idx);
        return;
    }
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        out.push(STR_EMPTY as u8);
        return;
    }
    if bytes.len() <= STR_RAW_MAX as usize {
        out.push(bytes.len() as u8);
        out.extend_from_slice(bytes);
    } else {
        out.push(STR_LONG);
        out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(bytes);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// DECODER
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct Decoder {
    data: Vec<u8>,
    pos: usize,
}

impl Decoder {
    fn new(data: Vec<u8>) -> Result<Self, String> {
        if data.len() < 3 {
            return Err("Data terlalu pendek untuk header ADILang binary".into());
        }
        if data[0] != MAGIC {
            return Err(format!("Magic salah: 0x{:02X} (harus 0x{:02X})", data[0], MAGIC));
        }
        if data[1] != VERSION_BIN {
            return Err(format!("Version salah: {} (harus {})", data[1], VERSION_BIN));
        }
        Ok(Self { data, pos: 3 })
    }

    /// Konstruktor tanpa validasi header — untuk packet selain AST (mis. 0xAF).
    fn new_fast(data: Vec<u8>) -> Self {
        Self { data, pos: 0 }
    }
    
    fn peek(&self) -> u8 {
        if self.pos < self.data.len() {
            self.data[self.pos]
        } else {
            0
        }
    }
    
    fn advance(&mut self) -> u8 {
        let b = self.data[self.pos];
        self.pos += 1;
        b
    }
    
    fn read_bytes(&mut self, n: usize) -> Result<Vec<u8>, String> {
        if self.pos + n > self.data.len() {
            return Err(format!("Unexpected EOF di posisi {}", self.pos));
        }
        let bytes = self.data[self.pos..self.pos + n].to_vec();
        self.pos += n;
        Ok(bytes)
    }
    
    fn read_string(&mut self) -> Result<String, String> {
        let len_pos = self.pos;
        let tag = self.advance();
        match tag {
            STR_EMPTY => Ok(String::new()),
            STR_REGISTRY => {
                let idx = self.advance();
                registry_word_at(idx).ok_or_else(|| {
                    format!("Indeks registry invalid: {idx} (di posisi {len_pos})")
                })
            }
            STR_LONG => {
                let len = u32::from_le_bytes(self.read_bytes(4)?.try_into().unwrap()) as usize;
                let raw = self.read_bytes(len)?;
                return String::from_utf8(raw)
                    .map_err(|e| format!("String UTF-8 invalid @lenpos{} len{} idx{}: {}", len_pos, len, e.utf8_error().valid_up_to(), e));
            }
            len => {
                let raw = self.read_bytes(len as usize)?;
                let preview = raw[..raw.len().min(12)].to_vec();
                String::from_utf8(raw)
                    .map_err(|e| format!("String UTF-8 invalid @lenpos{} len{} idx{} raw{:02X?}: {}", len_pos, len, e.utf8_error().valid_up_to(), preview, e))
            }
        }
    }
    
    fn read_f64(&mut self) -> Result<f64, String> {
        let bytes = self.read_bytes(8)?;
        Ok(f64::from_le_bytes(bytes.try_into().unwrap()))
    }
    
    fn read_u8(&mut self) -> Result<u8, String> {
        Ok(self.advance())
    }
}

pub fn decode_program(data: Vec<u8>) -> Result<Program, String> {
    let mut dec = Decoder::new(data)?;
    let name = dec.read_string()?;
    let mut items = Vec::new();
    while dec.pos < dec.data.len() {
        let tag = dec.read_u8()?;
        let item = decode_top_level(tag, &mut dec)?;
        items.push(item);
    }
    Ok(Program { name, items })
}

fn decode_top_level(tag: u8, dec: &mut Decoder) -> Result<TopLevel, String> {
    match tag {
        BLOCK_PAYLOAD => {
            let sender = dec.read_string()?;
            let target_agent = dec.read_string()?;
            let intent = dec.read_string()?;
            let has_state = dec.read_u8()?;
            let state_data = if has_state != 0 { Some(decode_expr(dec)?) } else { None };
            Ok(TopLevel::Payload(PayloadDef { sender, target_agent, intent, state_data }))
        }
        BLOCK_UI_LAYOUT => {
            let name = dec.read_string()?;
            let root = decode_ui_component(dec)?;
            Ok(TopLevel::UILayout(UILayoutDef { name, root }))
        }
        BLOCK_SPATIAL_3D => {
            let name = dec.read_string()?;
            let items = decode_spatial_items(dec)?;
            Ok(TopLevel::Spatial3D(Spatial3DDef { name, items }))
        }
        BLOCK_WORLD => {
            let name = dec.read_string()?;
            let items = decode_spatial_items(dec)?;
            Ok(TopLevel::World(Spatial3DDef { name, items }))
        }
        BLOCK_CAMERA => {
            let id = dec.read_string()?;
            let props = decode_props(dec)?;
            Ok(TopLevel::Camera(CameraDef { id, props }))
        }
        BLOCK_LIGHT => {
            let id = dec.read_string()?;
            let props = decode_props(dec)?;
            Ok(TopLevel::Light(LightDef { id, props }))
        }
        BLOCK_ENTITY => {
            let id = dec.read_string()?;
            let props = decode_props(dec)?;
            let handler_count = dec.read_u8()?;
            let mut handlers = Vec::new();
            for _ in 0..handler_count {
                handlers.push(decode_handler(dec)?);
            }
            Ok(TopLevel::Entity(EntityDef { id, props, handlers }))
        }
        BLOCK_LET => {
            let name = dec.read_string()?;
            let value = decode_expr(dec)?;
            Ok(TopLevel::Let { name, value })
        }
        BLOCK_FUNC => {
            let name = dec.read_string()?;
            let param_count = dec.read_u8()?;
            let mut params = Vec::new();
            for _ in 0..param_count {
                params.push(dec.read_string()?);
            }
            let body = decode_block_stmt(dec)?;
            Ok(TopLevel::Func(FuncDef { name, params, body }))
        }
        BLOCK_HANDLER => {
            let handler = decode_handler(dec)?;
            Ok(TopLevel::Handler(handler))
        }
        BLOCK_USE_JS => {
            let url = dec.read_string()?;
            Ok(TopLevel::UseJs(UseJsDef { url }))
        }
        BLOCK_ROUTES => {
            let count = dec.read_u8()?;
            let mut routes = Vec::new();
            for _ in 0..count {
                let path = dec.read_string()?;
                let layout = dec.read_string()?;
                let has_transition = dec.read_u8()? != 0;
                let transition = if has_transition { Some(dec.read_string()?) } else { None };
                routes.push(RouteDef { path, layout, transition });
            }
            Ok(TopLevel::Routes(RoutesDef { routes }))
        }
        BLOCK_I18N => {
            let locale_count = dec.read_u8()?;
            let mut locales = Vec::new();
            for _ in 0..locale_count {
                let name = dec.read_string()?;
                let entry_count = dec.read_u8()?;
                let mut entries = Vec::new();
                for _ in 0..entry_count {
                    let k = dec.read_string()?;
                    let v = dec.read_string()?;
                    entries.push((k, v));
                }
                locales.push(I18nLocale { name, entries });
            }
            Ok(TopLevel::I18n(I18nDef { locales }))
        }
        BLOCK_COMPONENT => {
            let name = dec.read_string()?;
            let hook_count = dec.read_u8()?;
            let mut hooks = Vec::new();
            for _ in 0..hook_count {
                let tag = dec.read_u8()?;
                let kind = match tag {
                    HOOK_MOUNT => LifecycleHookKind::Mount,
                    HOOK_UPDATE => LifecycleHookKind::Update,
                    HOOK_UNMOUNT => LifecycleHookKind::Unmount,
                    other => return Err(format!("Unknown hook kind: 0x{:02X}", other)),
                };
                let body = decode_block_stmt(dec)?;
                hooks.push(LifecycleHook { kind, body });
            }
            Ok(TopLevel::Component(ComponentDef { name, hooks }))
        }
        other => Err(format!("Unknown block type: 0x{:02X}", other)),
    }
}

fn decode_spatial_items(dec: &mut Decoder) -> Result<Vec<SpatialItem>, String> {
    let count = dec.read_u8()?;
    let mut items = Vec::new();
    for _ in 0..count {
        let tag = dec.read_u8()?;
        items.push(decode_spatial_item(tag, dec)?);
    }
    Ok(items)
}

fn decode_spatial_item(tag: u8, dec: &mut Decoder) -> Result<SpatialItem, String> {
    match tag {
        BLOCK_CAMERA => {
            let id = dec.read_string()?;
            let props = decode_props(dec)?;
            Ok(SpatialItem::Camera(CameraDef { id, props }))
        }
        BLOCK_LIGHT => {
            let id = dec.read_string()?;
            let props = decode_props(dec)?;
            Ok(SpatialItem::Light(LightDef { id, props }))
        }
        BLOCK_ENTITY => {
            let id = dec.read_string()?;
            let props = decode_props(dec)?;
            let handler_count = dec.read_u8()?;
            let mut handlers = Vec::new();
            for _ in 0..handler_count {
                handlers.push(decode_handler(dec)?);
            }
            Ok(SpatialItem::Entity(EntityDef { id, props, handlers }))
        }
        BLOCK_LET => {
            let name = dec.read_string()?;
            let value = decode_expr(dec)?;
            Ok(SpatialItem::Let { name, value })
        }
        BLOCK_FUNC => {
            let name = dec.read_string()?;
            let param_count = dec.read_u8()?;
            let mut params = Vec::new();
            for _ in 0..param_count {
                params.push(dec.read_string()?);
            }
            let body = decode_block_stmt(dec)?;
            Ok(SpatialItem::Func(FuncDef { name, params, body }))
        }
        BLOCK_HANDLER => {
            let handler = decode_handler(dec)?;
            Ok(SpatialItem::Handler(handler))
        }
        other => Err(format!("Unknown spatial item type: 0x{:02X}", other)),
    }
}

fn decode_props(dec: &mut Decoder) -> Result<Vec<Prop>, String> {
    let mut props = Vec::new();
    while dec.pos < dec.data.len() && dec.peek() == PROP {
        dec.read_u8()?;
        let name = dec.read_string()?;
        let value = decode_expr(dec)?;
        props.push(Prop { name, value });
    }
    Ok(props)
}

fn decode_handler(dec: &mut Decoder) -> Result<Handler, String> {
    let event = match dec.read_u8()? {
        EVENT_FRAME => EventKind::Frame,
        EVENT_SPEAK => EventKind::Speak,
        EVENT_SILENT => EventKind::Silent,
        EVENT_CLICK => EventKind::Click,
        other => return Err(format!("Unknown event kind: 0x{:02X}", other)),
    };
    let body = decode_block_stmt(dec)?;
    Ok(Handler { event, body })
}

fn decode_expr(dec: &mut Decoder) -> Result<Expr, String> {
    let tag = dec.read_u8()?;
    match tag {
        EXPR_NUM => {
            let n = dec.read_f64()?;
            Ok(Expr::Num(n))
        }
        EXPR_STR => {
            let s = dec.read_string()?;
            Ok(Expr::Str(s))
        }
        EXPR_BOOL => {
            let b = dec.read_u8()? != 0;
            Ok(Expr::Bool(b))
        }
        EXPR_TUPLE => {
            let count = dec.read_u8()?;
            let mut items = Vec::new();
            for _ in 0..count {
                items.push(decode_expr(dec)?);
            }
            Ok(Expr::Tuple(items))
        }
        EXPR_LIST => {
            let count = dec.read_u8()?;
            let mut items = Vec::new();
            for _ in 0..count {
                items.push(decode_expr(dec)?);
            }
            Ok(Expr::List(items))
        }
        EXPR_MAP => {
            let count = dec.read_u8()?;
            let mut pairs = Vec::new();
            for _ in 0..count {
                let key = dec.read_string()?;
                let value = decode_expr(dec)?;
                pairs.push((key, value));
            }
            Ok(Expr::Map(pairs))
        }
        EXPR_IDENT => {
            let s = dec.read_string()?;
            Ok(Expr::Ident(s))
        }
        EXPR_CALL => {
            let name = dec.read_string()?;
            let arg_count = dec.read_u8()?;
            let mut args = Vec::new();
            for _ in 0..arg_count {
                args.push(decode_expr(dec)?);
            }
            let has_props = dec.read_u8()? != 0;
            let props = if has_props {
                let prop_count = dec.read_u8()?;
                let mut props = Vec::new();
                for _ in 0..prop_count {
                    if dec.read_u8()? != PROP {
                        return Err("EXPR_CALL prop harus diawali tag PROP".into());
                    }
                    let pname = dec.read_string()?;
                    let pvalue = decode_expr(dec)?;
                    props.push(Prop { name: pname, value: pvalue });
                }
                Some(props)
            } else {
                None
            };
            Ok(Expr::Call { name, args, props })
        }
        EXPR_UNARY_MINUS => {
            let inner = decode_expr(dec)?;
            Ok(Expr::UnaryMinus(Box::new(inner)))
        }
        EXPR_BINARY => {
            let op = match dec.read_u8()? {
                BINOP_ADD => BinOp::Add,
                BINOP_SUB => BinOp::Sub,
                BINOP_MUL => BinOp::Mul,
                BINOP_DIV => BinOp::Div,
                BINOP_MOD => BinOp::Mod,
                BINOP_EQ => BinOp::Eq,
                BINOP_NE => BinOp::Ne,
                BINOP_LT => BinOp::Lt,
                BINOP_GT => BinOp::Gt,
                BINOP_LE => BinOp::Le,
                BINOP_GE => BinOp::Ge,
                other => return Err(format!("Unknown binary op: 0x{:02X}", other)),
            };
            let lhs = Box::new(decode_expr(dec)?);
            let rhs = Box::new(decode_expr(dec)?);
            Ok(Expr::Binary { op, lhs, rhs })
        }
        other => Err(format!("Unknown expression type: 0x{:02X}", other)),
    }
}

fn decode_block(dec: &mut Decoder) -> Result<Vec<Stmt>, String> {
    let count = dec.read_u8()?;
    let mut stmts = Vec::new();
    for _ in 0..count {
        stmts.push(decode_stmt(dec)?);
    }
    Ok(stmts)
}

/// Baca blok statement di posisi "blok" (bukan statement-stream): encoder
/// selalu menulis tag STMT_BLOCK sebelum count (lihat `encode_block`).
fn decode_block_stmt(dec: &mut Decoder) -> Result<Vec<Stmt>, String> {
    let tag = dec.read_u8()?;
    if tag != STMT_BLOCK {
        return Err(format!(
            "Expected STMT_BLOCK for block body, got 0x{:02X}",
            tag
        ));
    }
    decode_block(dec)
}

fn decode_stmt(dec: &mut Decoder) -> Result<Stmt, String> {
    let tag = dec.read_u8()?;
    match tag {
        STMT_LET => {
            let name = dec.read_string()?;
            let value = decode_expr(dec)?;
            Ok(Stmt::Let { name, value })
        }
        STMT_LET_DESTRUCTURE => {
            let count = dec.read_u8()?;
            let mut names = Vec::new();
            for _ in 0..count {
                names.push(dec.read_string()?);
            }
            let value = decode_expr(dec)?;
            Ok(Stmt::LetDestructure { names, value })
        }
        STMT_ASSIGN => {
            let name = dec.read_string()?;
            let value = decode_expr(dec)?;
            Ok(Stmt::Assign { name, value })
        }
        STMT_EXPR => {
            let e = decode_expr(dec)?;
            Ok(Stmt::ExprStmt(e))
        }
        STMT_RETURN => {
            let e = decode_expr(dec)?;
            Ok(Stmt::Return(e))
        }
        STMT_BLOCK => {
            let body = decode_block(dec)?;
            Ok(Stmt::Block(body))
        }
        STMT_IF => {
            let cond = decode_expr(dec)?;
            let then_branch = decode_block_stmt(dec)?;
            let else_branch = decode_block_stmt(dec)?;
            Ok(Stmt::If { cond, then_branch, else_branch })
        }
        STMT_WHILE => {
            let cond = decode_expr(dec)?;
            let body = decode_block_stmt(dec)?;
            Ok(Stmt::While { cond, body })
        }
        STMT_FOR => {
            let var = dec.read_string()?;
            let start = decode_expr(dec)?;
            let end = decode_expr(dec)?;
            let body = decode_block_stmt(dec)?;
            Ok(Stmt::For { var, start, end, body })
        }
        STMT_MATCH => {
            let subject = decode_expr(dec)?;
            let arm_count = dec.read_u8()?;
            let mut arms = Vec::new();
            for _ in 0..arm_count {
                arms.push(decode_match_arm(dec)?);
            }
            Ok(Stmt::Match { subject, arms })
        }
        STMT_NAVIGATE => {
            let path = dec.read_string()?;
            Ok(Stmt::Navigate { path })
        }
        STMT_SET_LOCALE => {
            let locale = dec.read_string()?;
            Ok(Stmt::SetLocale { locale })
        }
        STMT_DIRECTIVE => {
            let name = dec.read_string()?;
            let arg_count = dec.read_u8()?;
            let mut args = Vec::new();
            for _ in 0..arg_count {
                args.push(decode_expr(dec)?);
            }
            Ok(Stmt::Directive { name, args })
        }
        other => Err(format!("Unknown statement type: 0x{:02X}", other)),
    }
}

fn decode_match_arm(dec: &mut Decoder) -> Result<MatchArm, String> {
    let tag = dec.read_u8()?;
    let pattern = match tag {
        PATTERN_STR => {
            let s = dec.read_string()?;
            MatchPattern::Str(s)
        }
        PATTERN_NUM => {
            let n = dec.read_f64()?;
            MatchPattern::Num(n)
        }
        PATTERN_WILDCARD => MatchPattern::Wildcard,
        other => return Err(format!("Unknown match pattern: 0x{:02X}", other)),
    };
    let body = decode_block_stmt(dec)?;
    Ok(MatchArm { pattern, body })
}

fn decode_ui_component(dec: &mut Decoder) -> Result<UIComponent, String> {
    let tag = dec.read_u8()?;
    match tag {
        UI_CONTAINER => {
            let flex_byte = dec.read_u8()?;
            let flex = match flex_byte {
                FLEX_ROW => Some(FlexDirection::Row),
                FLEX_COLUMN => Some(FlexDirection::Column),
                FLEX_NONE => None,
                _ => return Err(format!("Unknown flex direction: 0x{:02X}", flex_byte)),
            };
            let child_count = dec.read_u8()?;
            let mut children = Vec::new();
            for _ in 0..child_count {
                children.push(decode_ui_component(dec)?);
            }
            Ok(UIComponent::Container { flex, children })
        }
        UI_TEXT => {
            let content = dec.read_string()?;
            Ok(UIComponent::Text { content })
        }
        UI_BUTTON => {
            let label = dec.read_string()?;
            let has_handler = dec.read_u8()? != 0;
            let onClick = if has_handler { Some(dec.read_string()?) } else { None };
            Ok(UIComponent::Button { label, onClick })
        }
        UI_INPUT => {
            let name = dec.read_string()?;
            let has_placeholder = dec.read_u8()? != 0;
            let placeholder = if has_placeholder { Some(dec.read_string()?) } else { None };
            let has_bind = dec.read_u8()? != 0;
            let bind = if has_bind { Some(dec.read_string()?) } else { None };
            let has_validate = dec.read_u8()? != 0;
            let validate = if has_validate { Some(dec.read_string()?) } else { None };
            Ok(UIComponent::Input { name, placeholder, bind, validate })
        }
        UI_CARD => {
            let has_title = dec.read_u8()? != 0;
            let title = if has_title { Some(dec.read_string()?) } else { None };
            let child_count = dec.read_u8()?;
            let mut children = Vec::new();
            for _ in 0..child_count {
                children.push(decode_ui_component(dec)?);
            }
            Ok(UIComponent::Card { title, children })
        }
        UI_MODAL => {
            let has_title = dec.read_u8()? != 0;
            let title = if has_title { Some(dec.read_string()?) } else { None };
            let child_count = dec.read_u8()?;
            let mut children = Vec::new();
            for _ in 0..child_count {
                children.push(decode_ui_component(dec)?);
            }
            Ok(UIComponent::Modal { title, children })
        }
        UI_NAVBAR => {
            let has_title = dec.read_u8()? != 0;
            let title = if has_title { Some(dec.read_string()?) } else { None };
            Ok(UIComponent::Navbar { title })
        }
        UI_FOOTER => {
            let content = dec.read_string()?;
            Ok(UIComponent::Footer { content })
        }
        other => Err(format!("Unknown UI component: 0x{:02X}", other)),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// ENTITY SNAPSHOT PROTOCOL — FULL & DELTA (transport real-time multiplayer)
// ═══════════════════════════════════════════════════════════════════════════

use crate::scene::{EntityState, MaterialKind, MeshKind, MeshParams};

const PACKET_FULL: u8 = 0x01;
const PACKET_DELTA: u8 = 0x02;
// Mask bit untuk field yang berubah (delta).
const DM_POS: u8 = 0x01;
const DM_ROT: u8 = 0x02;
const DM_SCALE: u8 = 0x04;
const DM_COLOR: u8 = 0x08;
const DM_MESH: u8 = 0x10;
const DM_MATERIAL: u8 = 0x20;
const DM_PARAMS: u8 = 0x40;

fn write_f64s(vals: &[f64], out: &mut Vec<u8>) {
    for v in vals {
        out.extend_from_slice(&v.to_le_bytes());
    }
}

fn read_f64s(dec: &mut Decoder, n: usize) -> Result<Vec<f64>, String> {
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        out.push(dec.read_f64()?);
    }
    Ok(out)
}

fn encode_entity_full(e: &EntityState, out: &mut Vec<u8>) {
    write_string(&e.id, out);
    write_f64s(&e.transform.pos, out);
    write_f64s(&e.transform.rot, out);
    write_f64s(&e.transform.scale, out);
    write_f64s(&e.color, out);
    out.push(mesh_bits(e.mesh));
    out.push(material_bits(e.material));
    let mp = &e.mesh_params;
    write_f64s(&[mp.radius, mp.tube, mp.inner, mp.segments, mp.size, mp.count], out);
}

fn decode_entity_full(dec: &mut Decoder) -> Result<EntityState, String> {
    let id = dec.read_string()?;
    let pos: [f64; 3] = read_f64s(dec, 3)?.try_into().unwrap();
    let rot: [f64; 3] = read_f64s(dec, 3)?.try_into().unwrap();
    let scale: [f64; 3] = read_f64s(dec, 3)?.try_into().unwrap();
    let color: [f64; 4] = read_f64s(dec, 4)?.try_into().unwrap();
    let mesh = mesh_from_bits(dec.read_u8()?);
    let material = material_from_bits(dec.read_u8()?);
    let mp = read_f64s(dec, 6)?;
    Ok(EntityState {
        id,
        transform: crate::scene::Transform { pos, rot, scale },
        color,
        material,
        mesh,
        mesh_params: MeshParams {
            radius: mp[0], tube: mp[1], inner: mp[2], segments: mp[3], size: mp[4], count: mp[5],
        },
        handlers: Vec::new(),
    })
}

fn mesh_bits(k: MeshKind) -> u8 {
    match k { MeshKind::Sphere => 0, MeshKind::Box => 1, MeshKind::Torus => 2, MeshKind::Icosa => 3, MeshKind::Ring => 4, MeshKind::Plane => 5, MeshKind::Grid => 6 }
}
fn mesh_from_bits(b: u8) -> MeshKind {
    match b { 1 => MeshKind::Box, 2 => MeshKind::Torus, 3 => MeshKind::Icosa, 4 => MeshKind::Ring, 5 => MeshKind::Plane, 6 => MeshKind::Grid, _ => MeshKind::Sphere }
}
fn material_bits(k: MaterialKind) -> u8 {
    match k { MaterialKind::Solid => 0, MaterialKind::Wire => 1, MaterialKind::Glow => 2, MaterialKind::Points => 3 }
}
fn material_from_bits(b: u8) -> MaterialKind {
    match b { 1 => MaterialKind::Wire, 2 => MaterialKind::Glow, 3 => MaterialKind::Points, _ => MaterialKind::Solid }
}

/// Snapshot penuh seluruh entity → bytecode FULL (transport pertama).
pub fn encode_full(entities: &[EntityState]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    out.push(0xAF);
    out.push(PACKET_FULL);
    out.push(entities.len() as u8);
    for e in entities {
        encode_entity_full(e, &mut out);
    }
    Ok(out)
}

/// Decode bytecode FULL → snapshot entity.
pub fn decode_full(bytes: &[u8]) -> Result<Vec<EntityState>, String> {
    if bytes.len() < 3 || bytes[0] != 0xAF {
        return Err("Bukan packet entity ADILang (0xAF)".into());
    }
    let mut dec = Decoder::new_fast(bytes.to_vec());
    dec.pos = 2; // skip magic + kind
    let count = dec.read_u8()?;
    let mut ents = Vec::new();
    for _ in 0..count {
        ents.push(decode_entity_full(&mut dec)?);
    }
    Ok(ents)
}

/// Delta antar snapshot (mask-based). `None` = struktur berubah → kirim FULL.
pub fn encode_delta(prev: &[EntityState], current: &[EntityState]) -> Option<Vec<u8>> {
    if prev.len() != current.len() {
        return None;
    }
    let mut out = Vec::new();
    out.push(0xAF);
    out.push(PACKET_DELTA);
    out.push(current.len() as u8);
    for (p, c) in prev.iter().zip(current.iter()) {
        if p.id != c.id {
            return None;
        }
        write_string(&c.id, &mut out);
        let mut mask = 0u8;
        if p.transform.pos != c.transform.pos { mask |= DM_POS; }
        if p.transform.rot != c.transform.rot { mask |= DM_ROT; }
        if p.transform.scale != c.transform.scale { mask |= DM_SCALE; }
        if p.color != c.color { mask |= DM_COLOR; }
        if p.mesh != c.mesh { mask |= DM_MESH; }
        if p.material != c.material { mask |= DM_MATERIAL; }
        if p.mesh_params != c.mesh_params { mask |= DM_PARAMS; }
        out.push(mask);
        if mask & DM_POS != 0 { write_f64s(&c.transform.pos, &mut out); }
        if mask & DM_ROT != 0 { write_f64s(&c.transform.rot, &mut out); }
        if mask & DM_SCALE != 0 { write_f64s(&c.transform.scale, &mut out); }
        if mask & DM_COLOR != 0 { write_f64s(&c.color, &mut out); }
        if mask & DM_MESH != 0 { out.push(mesh_bits(c.mesh)); }
        if mask & DM_MATERIAL != 0 { out.push(material_bits(c.material)); }
        if mask & DM_PARAMS != 0 {
            let mp = &c.mesh_params;
            write_f64s(&[mp.radius, mp.tube, mp.inner, mp.segments, mp.size, mp.count], &mut out);
        }
    }
    Some(out)
}

/// Terapkan delta ke snapshot baseline → snapshot baru.
pub fn apply_delta(base: &[EntityState], delta: &[u8]) -> Result<Vec<EntityState>, String> {
    if delta.len() < 3 || delta[0] != 0xAF || delta[1] != PACKET_DELTA {
        return Err("Bukan packet delta ADILang".into());
    }
    let mut dec = Decoder::new_fast(delta.to_vec());
    dec.pos = 2;
    let count = dec.read_u8()?;
    if count != base.len() as u8 {
        return Err("Delta count tidak cocok dengan baseline".into());
    }
    let mut out = base.to_vec();
    for ent in out.iter_mut() {
        let id = dec.read_string()?;
        if id != ent.id {
            return Err(format!("Delta id mismatch: {id} vs {}", ent.id));
        }
        let mask = dec.read_u8()?;
        if mask & DM_POS != 0 { ent.transform.pos = read_f64s(&mut dec, 3)?.try_into().unwrap(); }
        if mask & DM_ROT != 0 { ent.transform.rot = read_f64s(&mut dec, 3)?.try_into().unwrap(); }
        if mask & DM_SCALE != 0 { ent.transform.scale = read_f64s(&mut dec, 3)?.try_into().unwrap(); }
        if mask & DM_COLOR != 0 { ent.color = read_f64s(&mut dec, 4)?.try_into().unwrap(); }
        if mask & DM_MESH != 0 { ent.mesh = mesh_from_bits(dec.read_u8()?); }
        if mask & DM_MATERIAL != 0 { ent.material = material_from_bits(dec.read_u8()?); }
        if mask & DM_PARAMS != 0 {
            let mp = read_f64s(&mut dec, 6)?;
            ent.mesh_params = MeshParams { radius: mp[0], tube: mp[1], inner: mp[2], segments: mp[3], size: mp[4], count: mp[5] };
        }
    }
    Ok(out)
}

/// Tipe packet (FULL/DELTA) dari byte mentah.
pub fn packet_kind(bytes: &[u8]) -> String {
    match bytes.get(1) {
        Some(&PACKET_FULL) => "full".into(),
        Some(&PACKET_DELTA) => "delta".into(),
        _ => "unknown".into(),
    }
}

/// Jumlah entity dalam packet (byte ke-2).
pub fn packet_entity_count(bytes: &[u8]) -> u8 {
    bytes.get(2).copied().unwrap_or(0)
}

/// Versi packet.
pub fn packet_version(bytes: &[u8]) -> String {
    format!("ADILangBinary {}", packet_kind(bytes))
}

/// Spesifikasi format bytecode (untuk registry/docs/AI).
pub fn binary_spec() -> String {
    format!(
        "ADILang Binary Protocol v{}\n\
         AST encode/decode: MAGIC 0x{magic:02X}, version 0x{ver:02X} — compact string table\n\
         \x20 closed-vocabulary registry words → 2 byte (0xFE + index)\n\
         Entity snapshot FULL: [0xAF, 0x01, count, ...entity]\n\
         Entity snapshot DELTA: [0xAF, 0x02, count, ...{{id, mask, field}}]\n\
         Mask bits: pos=0x01 rot=0x02 scale=0x04 color=0x08 mesh=0x10 material=0x20 params=0x40",
        VERSION_BIN,
        magic = MAGIC,
        ver = VERSION_BIN,
    )
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;
    #[test]
    fn roundtrip_payload_ui_spatial() {
        let src = r#"
            @payload {
                sender "agent-a"
                target_agent "agent-b"
                intent "collaborate"
                state_data { status: "active" }
            }
            ui_layout "hud" {
                container {
                    flex column
                    text "Hello"
                    button "Send" onClick send
                }
            }
            spatial_3d "scene" {
                camera "cam" { pos (0 1.6 7) look (0 0 0) fov 55 }
                entity "e" { on frame { rotate(0.1, (0 1 0)) } }
            }
        "#;
        let prog = parse(src).expect("parse ok");
        let bin = encode_program(&prog).expect("encode ok");
        let decoded = decode_program(bin).expect("decode ok");
        assert_eq!(prog.items.len(), decoded.items.len());
        assert_eq!(prog, decoded, "roundtrip harus menghasilkan AST identik");
    }

    #[test]
    fn roundtrip_world_alias() {
        let src = r#"
            world "T" {
                camera "cam" { pos (0 1.6 7) look (0 0 0) fov 55 }
                entity "e" { on frame { rotate(0.1, (0 1 0)) } }
            }
        "#;
        let prog = parse(src).expect("parse ok");
        let bin = encode_program(&prog).expect("encode ok");
        let decoded = decode_program(bin).expect("decode ok");
        assert_eq!(prog, decoded, "roundtrip harus menghasilkan AST identik");
    }

    #[test]
    fn binary_header_magic_version() {
        let src = "world \"T\" { entity \"e\" { on frame { rotate(0.1, (0 1 0)) } } }";
        let prog = parse(src).expect("parse ok");
        let bin = encode_program(&prog).expect("encode ok");
        assert_eq!(bin[0], MAGIC);
        assert_eq!(bin[1], VERSION_BIN);
    }

    #[test]
    fn binary_roundtrip_multi_block() {
        let src = r#"
            @payload {
                sender "ai-1"
                target_agent "ai-2"
                intent "query"
            }
            ui_layout "main" {
                container {
                    flex row
                    text "Status"
                    input "email" placeholder "user@example.com"
                }
            }
            spatial_3d "scene" {
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
        let bin = encode_program(&prog).expect("encode ok");
        let decoded = decode_program(bin).expect("decode ok");
        assert_eq!(prog, decoded, "roundtrip multi-block harus identik (Zero-Token-Waste)");
        match &decoded.items[0] {
            TopLevel::Payload(p) => {
                assert_eq!(p.sender, "ai-1");
                assert_eq!(p.target_agent, "ai-2");
                assert_eq!(p.intent, "query");
            }
            _ => panic!("bukan payload"),
        }
        match &decoded.items[1] {
            TopLevel::UILayout(l) => {
                assert_eq!(l.name, "main");
            }
            _ => panic!("bukan ui_layout"),
        }
        match &decoded.items[2] {
            TopLevel::Spatial3D(s) => {
                assert_eq!(s.name, "scene");
                assert_eq!(s.items.len(), 2);
            }
            _ => panic!("bukan spatial_3d"),
        }
    }

    /// Binary compact harus LEBIH KECIL dari source teks untuk dokumen yang
    /// banyak memakai kata registry tertutup (bukti Zero-Token-Waste).
    #[test]
    fn binary_lebih_kecil_dari_source_teks() {
        let src = include_str!("../worlds/default.adi");
        let prog = parse(src).expect("default.adi harus valid");
        let bin = encode_program(&prog).expect("encode ok");
        let src_bytes = src.len();
        assert!(
            bin.len() < src_bytes,
            "binary ({}) harus lebih kecil dari source teks ({})",
            bin.len(),
            src_bytes
        );
        // sanity: decode kembali identik
        assert_eq!(prog, decode_program(bin).expect("decode ok"));
    }

    /// Kata registry tertutup harus di-encode sebagai 0xFE + index (2 byte),
    /// bukan sebagai string raw.
    #[test]
    fn registry_word_diencode_kompak() {
        let src = "world \"T\" { entity \"e\" { on frame { rotate(0.1, (0 1 0)) } } }";
        let prog = parse(src).expect("parse ok");
        let bin = encode_program(&prog).expect("encode ok");
        // 'world' adalah kata registry → harus ada pasangan 0xFE + index
        let has_registry_tag = bin.windows(2).any(|w| w[0] == STR_REGISTRY && w[1] < 200);
        assert!(has_registry_tag, "harus ada kata registry terkompresi");
        // decode kembali identik (konsistensi tabel dua arah)
        assert_eq!(prog, decode_program(bin).expect("decode ok"));
    }

    fn sample_entities() -> Vec<crate::scene::EntityState> {
        use crate::scene::{EntityState, MaterialKind, MeshKind, MeshParams, Transform};
        vec![
            EntityState {
                id: "core".into(),
                transform: Transform { pos: [0.0, 1.6, 7.0], rot: [0.1, 0.0, 0.0], scale: [1.0, 1.0, 1.0] },
                color: [0.15, 0.8, 1.0, 0.9],
                material: MaterialKind::Wire,
                mesh: MeshKind::Sphere,
                mesh_params: MeshParams { radius: 0.8, tube: 0.05, inner: 1.0, segments: 3.0, size: 10.0, count: 16.0 },
                handlers: Vec::new(),
            },
            EntityState {
                id: "grid".into(),
                transform: Transform { pos: [0.0, -2.6, 0.0], rot: [0.0, 0.0, 0.0], scale: [1.0, 1.0, 1.0] },
                color: [0.15, 0.6, 1.0, 0.18],
                material: MaterialKind::Wire,
                mesh: MeshKind::Grid,
                mesh_params: MeshParams { radius: 1.0, tube: 0.05, inner: 1.0, segments: 2.0, size: 26.0, count: 26.0 },
                handlers: Vec::new(),
            },
        ]
    }

    #[test]
    fn snapshot_full_roundtrip() {
        let ents = sample_entities();
        let bin = encode_full(&ents).expect("encode full");
        assert_eq!(packet_kind(&bin), "full");
        assert_eq!(packet_entity_count(&bin), 2);
        let decoded = decode_full(&bin).expect("decode full");
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].id, "core");
        assert_eq!(decoded[1].transform.pos, [0.0, -2.6, 0.0]);
        assert_eq!(decoded[1].mesh_params.count, 26.0);
    }

    #[test]
    fn snapshot_delta_hemat_bandwidth() {
        let base = sample_entities();
        let mut current = base.clone();
        current[0].transform.pos[2] = 8.5;
        current[1].color[3] = 0.5;
        let delta = encode_delta(&base, &current).expect("delta ok");
        // DELTA harus jauh lebih kecil dari FULL (hanya 2 field yang berubah)
        let full = encode_full(&current).expect("full");
        assert!(delta.len() < full.len() / 2, "delta {} vs full {}", delta.len(), full.len());
        let restored = apply_delta(&base, &delta).expect("apply delta");
        assert_eq!(restored[0].transform.pos, [0.0, 1.6, 8.5]);
        assert_eq!(restored[1].color[3], 0.5);
        // field lain tidak berubah
        assert_eq!(restored[1].transform.pos, [0.0, -2.6, 0.0]);
    }

    #[test]
    fn snapshot_delta_struktur_berubah_kembali_none() {
        let base = sample_entities();
        let mut current = sample_entities();
        current[0].id = "lain".into();
        assert!(encode_delta(&base, &current).is_none());
        let mut short = sample_entities();
        short.pop();
        assert!(encode_delta(&base, &short).is_none());
    }
}
