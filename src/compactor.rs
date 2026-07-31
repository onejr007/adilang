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

// ── Renderer kompak (tanpa indentasi, tanpa spasi berlebih) ─────────────────
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

/// Render ekspresi. Binary di-render FLAT (tanpa paren) — aman karena ADILang
/// tidak memiliki paren-gruping: paren = tuple, sehingga AST valid dari
/// precedence-climbing selalu berisi anak ber-precedence ≥ parent (roundtrip
/// diuji). Argumen builder di-render tanpa paren (bentuk kanonik `sphere 1 {...}`).
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
            let body = items
                .iter()
                .map(render_expr)
                .collect::<Vec<_>>()
                .join(" ");
            format!("({body})")
        }
        Expr::List(items) => {
            let body = items
                .iter()
                .map(render_expr)
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{body}]")
        }
        Expr::Map(pairs) => {
            let body = pairs
                .iter()
                .map(|(k, v)| format!("{}: {}", k, render_expr(v)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{{body}}}")
        }
        Expr::Ident(name) => name.clone(),
        Expr::Call { name, args, props } => {
            let builder = crate::registry::is_builder(name);
            let mut out = String::new();
            if builder {
                // bentuk kanonik builder: `sphere 1 { ... }` (posisi = arg)
                out.push_str(name);
                for a in args {
                    out.push(' ');
                    out.push_str(&render_expr(a));
                }
            } else {
                out.push_str(name);
                out.push('(');
                let body = args
                    .iter()
                    .map(render_expr)
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push_str(&body);
                out.push(')');
            }
            if let Some(ps) = props {
                out.push_str(" {");
                for p in ps {
                    out.push(' ');
                    out.push_str(&p.name);
                    out.push(' ');
                    out.push_str(&render_expr(&p.value));
                }
                out.push('}');
            }
            out
        }
        Expr::UnaryMinus(inner) => format!("-{}", render_expr(inner)),
        Expr::Binary { op, lhs, rhs } => {
            format!("{} {} {}", render_expr(lhs), op_sym(op), render_expr(rhs))
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
            format!(
                "let ({}) = {}",
                names.join(", "),
                render_expr(value)
            )
        }
        Stmt::Assign { name, value } => format!("{} = {}", name, render_expr(value)),
        Stmt::ExprStmt(e) => render_expr(e),
        Stmt::Return(e) => format!("return {}", render_expr(e)),
        Stmt::Block(inner) => format!("{{\n{}}}", render_stmts(inner)),
        Stmt::If { cond, then_branch, else_branch } => {
            let mut out = format!("if {} {{\n{}}}", render_expr(cond), render_stmts(then_branch));
            if !else_branch.is_empty() {
                out.push_str(&format!(" else {{\n{}}}", render_stmts(else_branch)));
            }
            out
        }
        Stmt::While { cond, body } => {
            format!("while {} {{\n{}}}", render_expr(cond), render_stmts(body))
        }
        Stmt::For { var, start, end, body } => {
            format!(
                "for {} in {} {} {{\n{}}}",
                var,
                render_expr(start),
                render_expr(end),
                render_stmts(body)
            )
        }
        Stmt::Match { subject, arms } => {
            let mut out = format!("match {} {{\n", render_expr(subject));
            for arm in arms {
                out.push_str(&format!(
                    "{} => {{\n{}}}",
                    render_pattern(&arm.pattern),
                    render_stmts(&arm.body)
                ));
            }
            out.push('}');
            out
        }
    }
}

fn render_stmts(stmts: &[Stmt]) -> String {
    stmts
        .iter()
        .map(render_stmt)
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_props(props: &[Prop]) -> String {
    let body = props
        .iter()
        .map(|p| format!("{} {}", p.name, render_expr(&p.value)))
        .collect::<Vec<_>>()
        .join(" ");
    if body.is_empty() {
        "{}".to_string()
    } else {
        format!("{{ {body} }}")
    }
}

fn render_top_level(item: &TopLevel) -> String {
    match item {
        TopLevel::Camera(c) => format!("camera {} {}", q(&c.id), render_props(&c.props)),
        TopLevel::Light(l) => format!("light {} {}", q(&l.id), render_props(&l.props)),
        TopLevel::Entity(e) => {
            let mut out = format!("entity {} {{", q(&e.id));
            for p in &e.props {
                out.push('\n');
                out.push_str(&format!("{} {}", p.name, render_expr(&p.value)));
            }
            for h in &e.handlers {
                out.push('\n');
                out.push_str(&render_handler(h));
            }
            out.push('}');
            out
        }
        TopLevel::Let { name, value } => format!("let {} = {}", name, render_expr(value)),
        TopLevel::Func(f) => {
            format!(
                "func {}({}) {{\n{}}}",
                f.name,
                f.params.join(" "),
                render_stmts(&f.body)
            )
        }
        TopLevel::Handler(h) => render_handler(h),
    }
}

fn render_handler(h: &Handler) -> String {
    let ev = match h.event {
        EventKind::Frame => "frame",
        EventKind::Speak => "speak",
        EventKind::Silent => "silent",
        EventKind::Click => "click",
    };
    format!("on {ev} {{\n{}}}", render_stmts(&h.body))
}

pub fn render_program(p: &Program) -> String {
    let mut out = format!("world {} {{", q(&p.name));
    for item in &p.items {
        out.push('\n');
        out.push_str(&render_top_level(item));
    }
    out.push('}');
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
        assert!(opt.contains("let b = t * 1.5"), "variabel panjang di-rename:\n{opt}");
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
}
