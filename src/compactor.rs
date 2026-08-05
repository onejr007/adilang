// ADILang optimizer / token compactor (v1.7.0, roadmap §3 "adilang-opt").
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).
//
// Compiler-pass yang memicu kompresi sintaks otomatis: nama variabel panjang
// di-rename ke nama 1–2 karakter (`let x_position = 1` → `let a = 1`) dan
// source di-render ulang TANPA whitespace berlebih, sehingga menghemat token
// input/output saat skrip dikirim antar-agen LLM.
//
// Semantik DIJAMIN dipertahankan (deterministik P1):
//   - Rename hanya nama yang ter-BIND (let/let-destructure/func/param/for/assign)
//     dan PEMAKAIANNYA; kosakata reserved (registry P6) tidak pernah disentuh.
//   - Pemetaan bijektif & deterministik (urutan penemuan, nama pendek a..z, aa..).
//   - Render ulang memakai struktur AST yang sama → parse ulang menghasilkan
//     AST identik (diuji `render_roundtrip_identity`).

use std::collections::HashMap;

use crate::ast::*;
use crate::checker::Vocabulary;
use crate::parser::parse;

/// Nama pendek yang dilarang (reserved vocabulary P6 + keyword parser).
fn is_short_allowed(vocab: &Vocabulary, short: &str) -> bool {
    !vocab.is_reserved(short) && short != "_"
}

struct Renamer {
    vocab: Vocabulary,
    /// Nama user yang ter-bind, urutan penemuan (deterministik).
    names: Vec<String>,
    map: HashMap<String, String>,
}

impl Renamer {
    fn new() -> Self {
        Self {
            vocab: Vocabulary::from_registry(),
            names: Vec::new(),
            map: HashMap::new(),
        }
    }

    fn note(&mut self, name: &str) {
        if !self.vocab.is_reserved(name) && !self.names.iter().any(|n| n == name) {
            self.names.push(name.to_string());
        }
    }

    fn build_map(&mut self) {
        let mut used: HashMap<String, ()> = HashMap::new();
        let mut next = |used: &mut HashMap<String, ()>| -> String {
            let mut i = 0usize;
            loop {
                // Bijective base-26 (urutan Excel): 0→'a' .. 25→'z', 26→"aa",
                // 27→"ab", 51→"az", 52→"ba" — tanpa nama yang bentrok reserved.
                let short = {
                    let mut s = String::new();
                    let mut v = i + 1;
                    while v > 0 {
                        v -= 1;
                        s.insert(0, ((b'a' + (v % 26) as u8) as char));
                        v /= 26;
                    }
                    s
                };
                i += 1;
                if !used.contains_key(&short) && is_short_allowed(&self.vocab, &short) {
                    used.insert(short.clone(), ());
                    return short;
                }
            }
        };
        for name in self.names.iter() {
            let short = next(&mut used);
            self.map.insert(name.clone(), short);
        }
    }

    fn get(&self, name: &str) -> Option<&str> {
        self.map.get(name).map(|s| s.as_str())
    }
}

fn collect_bindings(item: &TopLevel, r: &mut Renamer) {
    match item {
        TopLevel::Let { name, .. } => r.note(name),
        TopLevel::Func(f) => {
            r.note(&f.name);
            for p in &f.params {
                r.note(p);
            }
            for s in &f.body {
                collect_stmt_bindings(s, r);
            }
        }
        TopLevel::Entity(e) => {
            for h in &e.handlers {
                for s in &h.body {
                    collect_stmt_bindings(s, r);
                }
            }
        }
        TopLevel::Handler(h) => {
            for s in &h.body {
                collect_stmt_bindings(s, r);
            }
        }
        TopLevel::World(s) | TopLevel::Spatial3D(s) => {
            for si in &s.items {
                match si {
                    crate::ast::SpatialItem::Func(f) => {
                        r.note(&f.name);
                        for p in &f.params {
                            r.note(p);
                        }
                        for s in &f.body {
                            collect_stmt_bindings(s, r);
                        }
                    }
                    crate::ast::SpatialItem::Let { name, .. } => r.note(name),
                    crate::ast::SpatialItem::Entity(e) => {
                        for h in &e.handlers {
                            for s in &h.body {
                                collect_stmt_bindings(s, r);
                            }
                        }
                    }
                    crate::ast::SpatialItem::Handler(h) => {
                        for s in &h.body {
                            collect_stmt_bindings(s, r);
                        }
                    }
                    _ => {}
                }
            }
        }
        TopLevel::Payload(_) | TopLevel::UILayout(_) => {}
        TopLevel::Component(c) => {
            for h in &c.hooks {
                for s in &h.body {
                    collect_stmt_bindings(s, r);
                }
            }
        }
        _ => {}
    }
}

fn collect_stmt_bindings(s: &Stmt, r: &mut Renamer) {
    match s {
        Stmt::Let { name, .. } => r.note(name),
        Stmt::LetDestructure { names, .. } => {
            for n in names {
                r.note(n);
            }
        }
        Stmt::Assign { name, .. } => r.note(name),
        Stmt::Block(inner) => {
            for x in inner {
                collect_stmt_bindings(x, r);
            }
        }
        Stmt::If { then_branch, else_branch, .. } => {
            for x in then_branch {
                collect_stmt_bindings(x, r);
            }
            for x in else_branch {
                collect_stmt_bindings(x, r);
            }
        }
        Stmt::While { body, .. } => {
            for x in body {
                collect_stmt_bindings(x, r);
            }
        }
        Stmt::For { var, body, .. } => {
            r.note(var);
            for x in body {
                collect_stmt_bindings(x, r);
            }
        }
        Stmt::Match { arms, .. } => {
            for arm in arms {
                for x in &arm.body {
                    collect_stmt_bindings(x, r);
                }
            }
        }
        _ => {}
    }
}

fn rewrite_expr(e: &mut Expr, r: &Renamer) {
    match e {
        Expr::Ident(name) => {
            if let Some(short) = r.get(name) {
                *name = short.to_string();
            }
        }
        Expr::Call { name, args, props } => {
            // Hanya fungsi user yang di-rename — builtin/builder reserved.
            if r.get(name).is_some() && !r.vocab.is_builder(name) {
                if let Some(short) = r.get(name) {
                    *name = short.to_string();
                }
            }
            for a in args {
                rewrite_expr(a, r);
            }
            if let Some(ps) = props {
                for p in ps {
                    rewrite_expr(&mut p.value, r);
                }
            }
        }
        Expr::Tuple(items) | Expr::List(items) => {
            for it in items {
                rewrite_expr(it, r);
            }
        }
        Expr::Map(pairs) => {
            for (_, v) in pairs {
                rewrite_expr(v, r);
            }
        }
        Expr::UnaryMinus(inner) => rewrite_expr(inner, r),
        Expr::Binary { lhs, rhs, .. } => {
            rewrite_expr(lhs, r);
            rewrite_expr(rhs, r);
        }
        _ => {}
    }
}

fn rewrite_stmt(s: &mut Stmt, r: &Renamer) {
    match s {
        Stmt::Let { name, value } => {
            if let Some(short) = r.get(name) {
                *name = short.to_string();
            }
            rewrite_expr(value, r);
        }
        Stmt::LetDestructure { names, value } => {
            for n in names.iter_mut() {
                if let Some(short) = r.get(n) {
                    *n = short.to_string();
                }
            }
            rewrite_expr(value, r);
        }
        Stmt::Assign { name, value } => {
            if let Some(short) = r.get(name) {
                *name = short.to_string();
            }
            rewrite_expr(value, r);
        }
        Stmt::ExprStmt(e) => rewrite_expr(e, r),
        Stmt::Return(e) => rewrite_expr(e, r),
        Stmt::Block(inner) => {
            for x in inner {
                rewrite_stmt(x, r);
            }
        }
        Stmt::If { cond, then_branch, else_branch } => {
            rewrite_expr(cond, r);
            for x in then_branch {
                rewrite_stmt(x, r);
            }
            for x in else_branch {
                rewrite_stmt(x, r);
            }
        }
        Stmt::While { cond, body } => {
            rewrite_expr(cond, r);
            for x in body {
                rewrite_stmt(x, r);
            }
        }
        Stmt::For { var, start, end, body } => {
            if let Some(short) = r.get(var) {
                *var = short.to_string();
            }
            rewrite_expr(start, r);
            rewrite_expr(end, r);
            for x in body {
                rewrite_stmt(x, r);
            }
        }
        Stmt::Match { subject, arms } => {
            rewrite_expr(subject, r);
            for arm in arms {
                for x in &mut arm.body {
                    rewrite_stmt(x, r);
                }
            }
        }
        Stmt::Navigate { .. } | Stmt::SetLocale { .. } => {}
        Stmt::Directive { args, .. } => {
            for a in args {
                rewrite_expr(a, r);
            }
        }
    }
}

fn rewrite_top_level(item: &mut TopLevel, r: &Renamer) {
    match item {
        TopLevel::Camera(c) => {
            for p in &mut c.props {
                rewrite_expr(&mut p.value, r);
            }
        }
        TopLevel::Light(l) => {
            for p in &mut l.props {
                rewrite_expr(&mut p.value, r);
            }
        }
        TopLevel::Entity(e) => {
            for p in &mut e.props {
                rewrite_expr(&mut p.value, r);
            }
            for h in &mut e.handlers {
                for s in &mut h.body {
                    rewrite_stmt(s, r);
                }
            }
        }
        TopLevel::Let { name, value } => {
            if let Some(short) = r.get(name) {
                *name = short.to_string();
            }
            rewrite_expr(value, r);
        }
        TopLevel::Func(f) => {
            if let Some(short) = r.get(&f.name) {
                f.name = short.to_string();
            }
            for p in f.params.iter_mut() {
                if let Some(short) = r.get(p) {
                    *p = short.to_string();
                }
            }
            for s in &mut f.body {
                rewrite_stmt(s, r);
            }
        }
        TopLevel::Handler(h) => {
            for s in &mut h.body {
                rewrite_stmt(s, r);
            }
        }
        TopLevel::World(s) | TopLevel::Spatial3D(s) => {
            for si in &mut s.items {
                match si {
                    SpatialItem::Camera(c) => {
                        for p in &mut c.props {
                            rewrite_expr(&mut p.value, r);
                        }
                    }
                    SpatialItem::Light(l) => {
                        for p in &mut l.props {
                            rewrite_expr(&mut p.value, r);
                        }
                    }
                    SpatialItem::Entity(e) => {
                        for p in &mut e.props {
                            rewrite_expr(&mut p.value, r);
                        }
                        for h in &mut e.handlers {
                            for s in &mut h.body {
                                rewrite_stmt(s, r);
                            }
                        }
                    }
                    SpatialItem::Let { name, value } => {
                        if let Some(short) = r.get(name) {
                            *name = short.to_string();
                        }
                        rewrite_expr(value, r);
                    }
                    SpatialItem::Func(f) => {
                        if let Some(short) = r.get(&f.name) {
                            f.name = short.to_string();
                        }
                        for p in f.params.iter_mut() {
                            if let Some(short) = r.get(p) {
                                *p = short.to_string();
                            }
                        }
                        for s in &mut f.body {
                            rewrite_stmt(s, r);
                        }
                    }
                    SpatialItem::Handler(h) => {
                        for s in &mut h.body {
                            rewrite_stmt(s, r);
                        }
                    }
                }
            }
        }
        TopLevel::Payload(_) | TopLevel::UILayout(_) => {}
        TopLevel::Component(c) => {
            for h in &mut c.hooks {
                for s in &mut h.body {
                    rewrite_stmt(s, r);
                }
            }
        }
        _ => {}
    }
}

/// Optimasi utama: rename + render ulang kompak. `Err` = source tidak valid.
pub fn optimize_src(src: &str) -> Result<String, String> {
    let mut program = parse(src)?;
    let mut renamer = Renamer::new();
    for item in &program.items {
        collect_bindings(item, &mut renamer);
    }
    renamer.build_map();
    for item in &mut program.items {
        rewrite_top_level(item, &renamer);
    }
    Ok(render_program(&program))
}

// ── Renderer kompak (v1.8.1, T-123 — token-minimum, tanpa spasi opsional) ─
// Aturan kompresi (aman leksikal, roundtrip AST diuji di mod tests):
//   - operator biner TANPA spasi   (`a+b`, `a==b`) — lexer memisah token;
//   - list/map/call rapat          (`[1,2]`, `{k:v}`, `f(1,2)`);
//   - braces rapat                 (`{stmts}`, `entity "e"{...}`);
//   - statement dipisah SATU spasi (parser whitespace-agnostic; spasi wajib
//     agar dua identifier berdekatan tidak menyatu, mis. `let a = 1 b`);
//   - top-level tetap dipisah baris baru (biaya token sama dengan spasi,
//     lebih terbaca saat inspect; render_pretty tetap untuk presentasi).
fn q(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn num(n: f64) -> String {
    // Rust "{}" mencetak 1.0 → "1", 0.35 → "0.35" — round-trip via parse.
    if n == n.trunc() && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{}", n)
    }
}

fn op_sym(op: &BinOp) -> &'static str {
    match op {
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
    }
}

/// Render ekspresi — TANPA spasi di sekitar operator & pemisah (hemat token).
/// Binary di-render FLAT (tanpa paren) — aman karena ADILang tidak memiliki
/// paren-gruping: paren = tuple, sehingga AST valid dari precedence-climbing
/// selalu berisi anak ber-precedence ≥ parent (roundtrip diuji).
pub fn render_expr(e: &Expr) -> String {
    match e {
        Expr::Num(n) => num(*n),
        Expr::Str(s) => q(s),
        Expr::Bool(b) => {
            if *b {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        Expr::Tuple(items) => {
            format!("({})", items.iter().map(render_expr).collect::<Vec<_>>().join(" "))
        }
        Expr::List(items) => {
            format!("[{}]", items.iter().map(render_expr).collect::<Vec<_>>().join(","))
        }
        Expr::Map(pairs) => {
            format!(
                "{{{}}}",
                pairs
                    .iter()
                    .map(|(k, v)| format!("{}:{}", k, render_expr(v)))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
        Expr::Ident(name) => name.clone(),
        Expr::Call { name, args, props } => {
            let builder = crate::registry::is_builder(name);
            let mut out = String::new();
            if builder {
                // bentuk kanonik builder: `sphere 1{...}` (posisi = arg)
                out.push_str(name);
                for a in args {
                    out.push(' ');
                    out.push_str(&render_expr(a));
                }
            } else {
                out.push_str(name);
                out.push('(');
                out.push_str(&args.iter().map(render_expr).collect::<Vec<_>>().join(","));
                out.push(')');
            }
            if let Some(ps) = props {
                out.push('{');
                out.push_str(
                    &ps.iter()
                        .map(|p| format!("{} {}", p.name, render_expr(&p.value)))
                        .collect::<Vec<_>>()
                        .join(" "),
                );
                out.push('}');
            }
            out
        }
        Expr::UnaryMinus(inner) => format!("-{}", render_expr(inner)),
        Expr::Binary { op, lhs, rhs } => {
            format!("{}{}{}", render_expr(lhs), op_sym(op), render_expr(rhs))
        }
    }
}

fn render_pattern(p: &MatchPattern) -> String {
    match p {
        MatchPattern::Str(s) => q(s),
        MatchPattern::Num(n) => num(*n),
        MatchPattern::Wildcard => "_".to_string(),
    }
}

fn render_stmt(s: &Stmt) -> String {
    match s {
        Stmt::Let { name, value } => format!("let {} = {}", name, render_expr(value)),
        Stmt::LetDestructure { names, value } => {
            format!("let ({}) = {}", names.join(","), render_expr(value))
        }
        Stmt::Assign { name, value } => format!("{} = {}", name, render_expr(value)),
        Stmt::ExprStmt(e) => render_expr(e),
        Stmt::Return(e) => format!("return {}", render_expr(e)),
        Stmt::Block(inner) => format!("{{{}}}", render_stmts(inner)),
        Stmt::If { cond, then_branch, else_branch } => {
            let mut out = format!("if {} {{{}}}", render_expr(cond), render_stmts(then_branch));
            if !else_branch.is_empty() {
                out.push_str(&format!("else {{{}}}", render_stmts(else_branch)));
            }
            out
        }
        Stmt::While { cond, body } => {
            format!("while {} {{{}}}", render_expr(cond), render_stmts(body))
        }
        Stmt::For { var, start, end, body } => {
            format!(
                "for {} in {} {} {{{}}}",
                var,
                render_expr(start),
                render_expr(end),
                render_stmts(body)
            )
        }
        Stmt::Match { subject, arms } => {
            let mut out = format!("match {} {{", render_expr(subject));
            for arm in arms {
                out.push_str(&format!(
                    "{}=>{{{}}}",
                    render_pattern(&arm.pattern),
                    render_stmts(&arm.body)
                ));
            }
            out.push('}');
            out
        }
        Stmt::Navigate { path } => format!("@navigate({})", q(path)),
        Stmt::SetLocale { locale } => format!("@set_locale({})", q(locale)),
        Stmt::Directive { name, args } => {
            format!(
                "@{}({})",
                name,
                args.iter().map(render_expr).collect::<Vec<_>>().join(",")
            )
        }
    }
}

fn render_stmts(stmts: &[Stmt]) -> String {
    stmts.iter().map(render_stmt).collect::<Vec<_>>().join(" ")
}

fn render_props(props: &[Prop]) -> String {
    if props.is_empty() {
        return "{}".to_string();
    }
    format!(
        "{{{}}}",
        props
            .iter()
            .map(|p| format!("{} {}", p.name, render_expr(&p.value)))
            .collect::<Vec<_>>()
            .join(" ")
    )
}

fn render_prop(p: &Prop) -> String {
    format!("{} {}", p.name, render_expr(&p.value))
}

pub(crate) fn render_top_level(item: &TopLevel) -> String {
    match item {
        TopLevel::Camera(c) => format!("camera {} {}", q(&c.id), render_props(&c.props)),
        TopLevel::Light(l) => format!("light {} {}", q(&l.id), render_props(&l.props)),
        TopLevel::Entity(e) => {
            let mut out = format!("entity {} {{", q(&e.id));
            for p in &e.props {
                out.push(' ');
                out.push_str(&render_prop(p));
            }
            for h in &e.handlers {
                out.push(' ');
                out.push_str(&render_handler(h));
            }
            out.push('}');
            out
        }
        TopLevel::Let { name, value } => format!("let {} = {}", name, render_expr(value)),
        TopLevel::Func(f) => {
            format!(
                "func {}({}){{{}}}",
                f.name,
                f.params.join(" "),
                render_stmts(&f.body)
            )
        }
        TopLevel::Handler(h) => render_handler(h),
        TopLevel::Payload(p) => render_payload(p),
        TopLevel::UILayout(u) => render_ui_layout(u),
        TopLevel::Spatial3D(s) => render_spatial_3d(s, false),
        TopLevel::World(s) => render_spatial_3d(s, true),
        TopLevel::UseJs(u) => format!("@use_js{{url {}}}", q(&u.url)),
        TopLevel::Routes(r) => {
            let body = r
                .routes
                .iter()
                .map(|route| {
                    let mut s = format!("route {} layout {}", q(&route.path), q(&route.layout));
                    if let Some(t) = &route.transition {
                        s.push_str(&format!(" transition {}", q(t)));
                    }
                    s
                })
                .collect::<Vec<_>>()
                .join(" ");
            format!("routes{{{body}}}")
        }
        TopLevel::I18n(i) => {
            let body = i
                .locales
                .iter()
                .map(|loc| {
                    let entries = loc
                        .entries
                        .iter()
                        .map(|(k, v)| format!("{} {}", k, q(v)))
                        .collect::<Vec<_>>()
                        .join(" ");
                    format!("locale {} {{{}}}", q(&loc.name), entries)
                })
                .collect::<Vec<_>>()
                .join(" ");
            format!("@i18n{{{body}}}")
        }
        TopLevel::Component(c) => {
            let body = c
                .hooks
                .iter()
                .map(|h| format!("{}:{}", h.kind.as_str(), render_stmts(&h.body)))
                .collect::<Vec<_>>()
                .join(" ");
            format!("component {} {{{body}}}", c.name)
        }
    }
}

fn render_payload(p: &PayloadDef) -> String {
    let mut parts = vec![
        format!("sender {}", q(&p.sender)),
        format!("target_agent {}", q(&p.target_agent)),
        format!("intent {}", q(&p.intent)),
    ];
    if let Some(state) = &p.state_data {
        parts.push(format!("state_data {}", render_expr(state)));
    }
    format!("@payload{{{}}}", parts.join(" "))
}

fn render_ui_layout(u: &UILayoutDef) -> String {
    let mut out = format!("ui_layout {} {{", q(&u.name));
    out.push_str(&render_ui_component(&u.root));
    out.push('}');
    out
}

fn render_ui_component(comp: &UIComponent) -> String {
    match comp {
        UIComponent::Container { flex, children } => {
            let mut out = String::from("container{");
            if let Some(f) = flex {
                out.push_str(&format!(
                    "flex {} ",
                    match f {
                        FlexDirection::Row => "row",
                        FlexDirection::Column => "column",
                    }
                ));
            }
            for child in children {
                out.push_str(&render_ui_component(child));
                out.push(' ');
            }
            out.push('}');
            out
        }
        UIComponent::Text { content } => format!("text {}", q(content)),
        UIComponent::Button { label, onClick } => {
            let mut out = format!("button {}", q(label));
            if let Some(h) = onClick {
                out.push_str(&format!(" onClick {}", h));
            }
            out
        }
        UIComponent::Input { name, placeholder, bind, validate } => {
            let mut out = format!("input {}", q(name));
            if let Some(p) = placeholder {
                out.push_str(&format!(" placeholder {}", q(p)));
            }
            if let Some(b) = bind {
                out.push_str(&format!(" bind @{}", b));
            }
            if let Some(v) = validate {
                out.push_str(&format!(" validate {}", q(v)));
            }
            out
        }
        UIComponent::Card { title, children } => {
            let mut out = String::from("card");
            if let Some(t) = title {
                out.push(' ');
                out.push_str(&q(t));
            }
            out.push('{');
            for child in children {
                out.push_str(&render_ui_component(child));
                out.push(' ');
            }
            out.push('}');
            out
        }
        UIComponent::Modal { title, children } => {
            let mut out = String::from("modal");
            if let Some(t) = title {
                out.push(' ');
                out.push_str(&q(t));
            }
            out.push('{');
            for child in children {
                out.push_str(&render_ui_component(child));
                out.push(' ');
            }
            out.push('}');
            out
        }
        UIComponent::Navbar { title } => {
            let mut out = String::from("navbar");
            if let Some(t) = title {
                out.push(' ');
                out.push_str(&q(t));
            }
            out
        }
        UIComponent::Footer { content } => format!("footer {}", q(content)),
    }
}

fn render_spatial_3d(s: &Spatial3DDef, is_world: bool) -> String {
    let mut out = String::new();
    if is_world {
        out.push_str(&format!("world {} {{", q(&s.name)));
    } else {
        out.push_str(&format!("spatial_3d {} {{", q(&s.name)));
    }
    let items = s
        .items
        .iter()
        .map(render_spatial_item)
        .collect::<Vec<_>>()
        .join(" ");
    if !items.is_empty() {
        out.push(' ');
        out.push_str(&items);
    }
    out.push('}');
    out
}

fn render_spatial_item(item: &SpatialItem) -> String {
    match item {
        SpatialItem::Camera(c) => format!("camera {} {}", q(&c.id), render_props(&c.props)),
        SpatialItem::Light(l) => format!("light {} {}", q(&l.id), render_props(&l.props)),
        SpatialItem::Entity(e) => {
            let mut out = format!("entity {} {{", q(&e.id));
            for p in &e.props {
                out.push(' ');
                out.push_str(&render_prop(p));
            }
            for h in &e.handlers {
                out.push(' ');
                out.push_str(&render_handler(h));
            }
            out.push('}');
            out
        }
        SpatialItem::Let { name, value } => format!("let {} = {}", name, render_expr(value)),
        SpatialItem::Func(f) => {
            format!(
                "func {}({}){{{}}}",
                f.name,
                f.params.join(" "),
                render_stmts(&f.body)
            )
        }
        SpatialItem::Handler(h) => render_handler(h),
    }
}

fn render_handler(h: &Handler) -> String {
    let ev = match h.event {
        EventKind::Frame => "frame",
        EventKind::Speak => "speak",
        EventKind::Silent => "silent",
        EventKind::Click => "click",
    };
    format!("on {ev}{{{}}}", render_stmts(&h.body))
}

pub fn render_program(p: &Program) -> String {
    let mut out = String::new();
    for item in &p.items {
        out.push_str(&render_top_level(item));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Roundtrip AST — hanya sah untuk source yang TIDAK memicu rename
    /// (nama ter-bind sudah pendek a/b/c → pemetaan identitas). Untuk
    /// pembuktian semantik dengan rename sungguhan lihat test
    /// `optimize_pertahankan_semantik_*` (membandingkan hasil eval).
    fn roundtrip_ok(src: &str) {
        let prog = parse(src).expect("source asli harus valid");
        let opt = optimize_src(src).expect("optimize harus berhasil");
        let prog2 = parse(&opt).expect("hasil optimize harus valid");
        assert_eq!(prog, prog2, "optimize mengubah AST (semantik tidak terjaga):\n{opt}");
        // idempotent: optimize kedua tidak mengubah apa pun
        assert_eq!(
            opt,
            optimize_src(&opt).expect("optimize ulang harus berhasil"),
            "optimize tidak idempotent:\n{opt}"
        );
    }

    /// Snapshot state evaluasi setelah memuat program & menjalankan semua
    /// handler frame (entity context per-entity) — pembanding semantik.
    type Snap = (
        String,      // id
        [f64; 3],    // pos
        [f64; 3],    // rot
        [f64; 3],    // scale
        [f64; 4],    // color
        crate::scene::MeshKind,
        crate::scene::MaterialKind,
        (f64, f64, f64, f64, f64, f64), // mesh_params
    );

    fn eval_snapshot(src: &str) -> Vec<Snap> {
        use crate::ast::EventKind;
        let prog = parse(src).expect("source harus valid");
        let mut interp = crate::eval::Interpreter::new("w".to_string());
        interp.load(prog).expect("load harus sukses");
        interp.t = 1.2345;
        interp.mouse_x = 0.25;
        interp.mouse_y = -0.5;
        let entities = interp.world.entities.clone();
        for ent in &entities {
            for h in &ent.handlers {
                if h.event == EventKind::Frame {
                    interp
                        .run_handler(Some(ent.id.clone()), &h.body)
                        .expect("handler frame harus jalan");
                }
            }
        }
        interp
            .world
            .entities
            .iter()
            .map(|e| {
                let mp = &e.mesh_params;
                (
                    e.id.clone(),
                    e.transform.pos,
                    e.transform.rot,
                    e.transform.scale,
                    e.color,
                    e.mesh,
                    e.material,
                    (mp.radius, mp.tube, mp.inner, mp.segments, mp.size, mp.count),
                )
            })
            .collect()
    }

    #[test]
    fn optimize_memperpendek_nama_variabel() {
        let src = r#"
            world "T" {
                func get_status() { return [200, "OK"] }
                entity "e" {
                    on frame {
                        let x_position = t * 1.5
                        let y_speed = 0.35 * sin(x_position)
                        setPos(x_position, y_speed, 0)
                        let (code, msg) = get_status()
                    }
                }
            }
        "#;
        let opt = optimize_src(src).unwrap();
        // Urutan penemuan: get_status, x_position, y_speed, code, msg →
        // a=get_status, b=x_position, c=y_speed, d=code, e=msg.
        assert!(opt.contains("func a()"), "fungsi di-rename:\n{opt}");
        assert!(opt.contains("let b = t*1.5"), "variabel panjang di-rename:\n{opt}");
        assert!(opt.contains("setPos(b,c,0)"), "call arg rapat tanpa spasi:\n{opt}");
        assert!(!opt.contains("x_position"), "nama lama tidak boleh tersisa:\n{opt}");
        // Semantik: state evaluasi sebelum vs sesudah optimize harus identik.
        assert_eq!(eval_snapshot(src), eval_snapshot(&opt), "semantik berubah:\n{opt}");
    }

    #[test]
    fn optimize_pertahankan_semantik_for_match_destructure() {
        // while + for + match + destructuring setelah rename tetap semantik sama.
        let src = r#"
            world "T" {
                entity "e" {
                    on frame {
                        let total_rotasi = 0
                        for i in 0 3 {
                            rotate(0.1, (0 1 0))
                        }
                        match total_rotasi {
                            0 => setScale(2.0)
                            _ => setScale(1.0)
                        }
                    }
                }
            }
        "#;
        let opt = optimize_src(src).unwrap();
        assert!(!opt.contains("total_rotasi"), "nama lama tersisa:\n{opt}");
        assert_eq!(eval_snapshot(src), eval_snapshot(&opt), "semantik berubah:\n{opt}");
    }

    #[test]
    fn optimize_mengurangi_ukuran_source() {
        let src = include_str!("../worlds/default.adi");
        let opt = optimize_src(src).unwrap();
        assert!(
            opt.len() < src.len(),
            "hasil optimize harus lebih kecil: {} vs {}",
            opt.len(),
            src.len()
        );
        roundtrip_ok(src);
    }

    #[test]
    fn render_roundtrip_identity_untuk_korpus_ekspresi() {
        // Ekspresi biner flat (tanpa paren-grouping — ADILang tidak punya)
        // harus round-trip identik.
        roundtrip_ok(r#"world "T" { entity "e" { on frame { let a = 2 + 3 * 4 } } }"#);
        roundtrip_ok(r#"world "T" { entity "e" { on frame { let a = (2 + 3) * 4 } } }"#);
        roundtrip_ok(r#"world "T" { entity "e" { on frame { let a = 2 + 3 * 4 == 14 } } }"#);
        roundtrip_ok(r#"world "T" { entity "e" { on frame { let a = -5 + 2 } } }"#);
        roundtrip_ok(r#"world "T" { entity "e" { on frame { let a = (1 2 3) } } }"#);
        roundtrip_ok(r#"world "T" { entity "e" { on frame { let a = { timeout: 30, retry: 3 } } } }"#);
        roundtrip_ok(r#"world "T" { entity "e" { on frame { let a = [1, 2, 3] } } }"#);
        roundtrip_ok(
            r#"world "T" { entity "e" { on frame { match verb { "ask" => rotate(0.1, (0 1 0)) _ => rotate(0.2, (0 1 0)) } } } }"#,
        );
        roundtrip_ok(
            r#"world "T" { entity "e" { mesh sphere { radius 0.8 segments 3 } material wire (0.15 0.8 1) 0.9 } }"#,
        );
        // Nama bound sudah pendek & urutan penemuan a→a,b→b,c→d → rename identitas.
        roundtrip_ok(r#"world "T" { func a(b c) { let d = b * c } entity "e" { on frame { a(2, 3) } } }"#);
    }

    #[test]
    fn optimize_deterministik() {
        let src = include_str!("../worlds/default.adi");
        assert_eq!(optimize_src(src).unwrap(), optimize_src(src).unwrap());
    }

    #[test]
    fn optimize_menolak_source_invalid() {
        assert!(optimize_src("world \"w\" { entity \"e\" { mesh sphre { } } }").is_err());
    }

    #[test]
    fn optimize_kompak_tanpa_spasi_opsional() {
        // Biner tanpa spasi, list/map rapat, braces rapat — roundtrip & semantik.
        let src = r#"
            world "T" {
                entity "e" {
                    on frame {
                        let a = 2 + 3 * 4
                        let b = [1, 2, 3]
                        let c = { timeout: 30, retry: 3 }
                    }
                }
            }
        "#;
        let opt = optimize_src(src).unwrap();
        assert!(opt.contains("2+3*4"), "biner tanpa spasi:\\n{opt}");
        assert!(opt.contains("[1,2,3]"), "list rapat:\\n{opt}");
        assert!(opt.contains("{timeout:30,retry:3}"), "map rapat:\\n{opt}");
        roundtrip_ok(src);
        assert_eq!(eval_snapshot(src), eval_snapshot(&opt), "semantik berubah:\\n{opt}");
    }

    #[test]
    fn optimize_roundtrip_korpus_fullstack() {
        // Template fullstack: @use_js, @payload, routes, @i18n, ui_layout,
        // component (lifecycle hooks), func + directive — semua varian
        // top-level harus roundtrip identik + idempotent + tidak membengkak.
        let src = include_str!("../templates/fullstack-agent.adi");
        let before = parse(src).expect("source asli harus valid");
        let opt = optimize_src(src).unwrap();
        let after = parse(&opt).expect("hasil optimize harus valid");
        // Rename bijektif mengubah NAMA bound, tapi struktur top-level identik.
        assert_eq!(
            before.items.len(),
            after.items.len(),
            "struktur top-level berubah:\n{opt}"
        );
        // idempotent + tidak membengkak
        assert_eq!(
            opt,
            optimize_src(&opt).expect("optimize ulang harus berhasil")
        );
        assert!(
            opt.len() <= src.len(),
            "compact tidak boleh membengkak: {} vs {}",
            opt.len(),
            src.len()
        );
    }

}
