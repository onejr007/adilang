// ADILang checker — static analyzer / linter (v1.7.0, roadmap §3 "adilang-check").
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).
//
// Memeriksa keabsahan file .adi ATAU string ADILang terhadap grammar EBNF
// secara instan, OFFLINE (tanpa evaluasi / tanpa WebGL) — cocok untuk
// pre-commit hook, editor diagnostics, dan gate LLM sebelum eksekusi.
//
// Dua lapis:
//   1. `check_src` mengembalikan Err(String) bila PARSE gagal (syntax error)
//      — pesan parser sudah memuat baris & token yang salah (sumber untuk
//      event "syntax_error").
//   2. Bila parse sukses, mengembalikan Vec<Diagnostic> temuan SEMANTIK:
//      variabel/fungsi/property tidak dikenal, arity salah, dll.
//
// Seluruh kosakata dibangun dari `registry::registry_text()` (P6 sumber
// tunggal) — checker TIDAK menduplikasi daftar literal sehingga anti-drift.

use std::collections::HashSet;

use crate::ast::*;
use crate::lexer::tokenize;
use crate::parser::parse;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Error => "ERROR",
            Severity::Warning => "WARN",
            Severity::Info => "INFO",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub line: usize,
    pub message: String,
    pub hint: String,
}

/// Kosakata tertutup — dibangun DARI registry_text() (sumber tunggal P6).
/// Menambah kosakata baru di registry.rs otomatis dikenali checker.
#[derive(Debug, Default)]
pub struct Vocabulary {
    pub mesh: HashSet<String>,
    pub material: HashSet<String>,
    pub transform: HashSet<String>,
    pub math1: HashSet<String>,
    pub math2: HashSet<String>,
    pub math3: HashSet<String>,
    pub ident: HashSet<String>,
    pub meshparam: HashSet<String>,
    pub cameraprop: HashSet<String>,
    pub lightprop: HashSet<String>,
    /// Enum nilai prop light `type` (point/ambient) — kategori registry
    /// `lightprop.type` (P6: satu sumber, bukan literal hardcode).
    pub lightprop_type: HashSet<String>,
    pub entityprop: HashSet<String>,
    pub event: HashSet<String>,
    pub declaration: HashSet<String>,
    pub statement: HashSet<String>,
    pub keyword: HashSet<String>,
}

impl Vocabulary {
    pub fn from_registry() -> Self {
        let mut v = Vocabulary::default();
        let text = crate::registry::registry_text();
        for line in text.lines() {
            let Some((cat, vals)) = line.split_once(':') else { continue };
            let cat = cat.trim();
            let mut target: Option<&mut HashSet<String>> = None;
            macro_rules! pick {
                ($($name:expr => $field:ident),* $(,)?) => {
                    $( if cat == $name { target = Some(&mut v.$field); } )*
                };
            }
            pick!(
                "mesh" => mesh,
                "material" => material,
                "transform" => transform,
                "math1" => math1,
                "math2" => math2,
                "math3" => math3,
                "ident" => ident,
                "meshparam" => meshparam,
                "cameraprop" => cameraprop,
                "lightprop" => lightprop,
                "lightprop.type" => lightprop_type,
                "entityprop" => entityprop,
                "event" => event,
                "declaration" => declaration,
                "statement" => statement,
                "keyword" => keyword,
            );
            if let Some(set) = target {
                for w in vals.split_whitespace() {
                    set.insert(w.to_string());
                }
            }
        }
        v
    }

    /// Apakah nama termasuk kata RESERVED (tidak boleh di-rename optimizer)?
    /// Gabungan seluruh kategori + keyword parser tambahan (in/else).
    pub fn is_reserved(&self, name: &str) -> bool {
        self.mesh.contains(name)
            || self.material.contains(name)
            || self.transform.contains(name)
            || self.math1.contains(name)
            || self.math2.contains(name)
            || self.math3.contains(name)
            || self.ident.contains(name)
            || self.meshparam.contains(name)
            || self.cameraprop.contains(name)
            || self.lightprop.contains(name)
            || self.lightprop_type.contains(name)
            || self.entityprop.contains(name)
            || self.event.contains(name)
            || self.declaration.contains(name)
            || self.statement.contains(name)
            || self.keyword.contains(name)
            || matches!(name, "in" | "else" | "_" | "world" | "on")
    }

    pub fn is_builder(&self, name: &str) -> bool {
        self.mesh.contains(name) || self.material.contains(name)
    }

    pub fn is_builtin_call(&self, name: &str) -> bool {
        name == "t" // i18n marker (v1.12.0)
            || self.transform.contains(name)
            || self.math1.contains(name)
            || self.math2.contains(name)
            || self.math3.contains(name)
    }

    /// Rentang arity yang diterima untuk builtin — (min, max); None max = tak hingga.
    pub fn arity(&self, name: &str) -> Option<(usize, Option<usize>)> {
        match name {
            "move" | "setPos" | "setColor" => Some((3, None)),
            "setAlpha" => Some((1, None)),
            "rotate" => Some((1, Some(2))),
            "setScale" | "scaleBy" => Some((1, Some(3))),
            "t" => Some((1, Some(1))),
            _ => {
                if self.math1.contains(name) {
                    Some((1, Some(1)))
                } else if self.math2.contains(name) {
                    Some((2, Some(2)))
                } else if self.math3.contains(name) {
                    Some((3, Some(3)))
                } else {
                    None
                }
            }
        }
    }

    /// Arity maksimal argumen POSITIONAL untuk builder mesh (0 = tanpa arg).
    pub fn builder_max_args(&self, name: &str) -> Option<usize> {
        match name {
            "box" | "plane" => Some(1),
            "sphere" | "torus" | "icosa" | "ring" | "grid" => Some(2),
            _ => None,
        }
    }
}

/// Jalankan seluruh pemeriksaan. `Err` = syntax error (parse gagal).
/// `Ok(diags)` = temuan semantik (bisa kosong = bersih).
pub fn check_src(src: &str) -> Result<Vec<Diagnostic>, String> {
    // Tokenisasi dulu agar error lexer (mis. karakter tidak dikenal) juga
    // dibalas lengkap dengan baris/kolom.
    tokenize(src)?;
    let program = parse(src)?;
    let vocab = Vocabulary::from_registry();
    Ok(check_program(src, &vocab, &program))
}

/// Perkiraan baris kemunculan `needle` di source (linter — bukan posisi presisi
/// token; deterministik & cukup untuk pesan diagnosa).
fn line_of(src: &str, needle: &str) -> usize {
    src.lines()
        .position(|l| l.contains(needle))
        .map(|i| i + 1)
        .unwrap_or(1)
}

fn diag(severity: Severity, src: &str, needle: &str, message: &str, hint: &str) -> Diagnostic {
    Diagnostic {
        severity,
        line: line_of(src, needle),
        message: message.to_string(),
        hint: hint.to_string(),
    }
}

// ── Pass A: kumpulkan nama yang ter-bind ────────────────────────────────────
// `declared` = nama yang DIDEKLARASIKAN (let/func/param/for/destructure) —
// assignment hanya sah ke nama declared (KB §6). `bound` = declared + nama
// yang hanya di-assign (agar penggunaan berikutnya tidak di-flag variabel
// tidak dikenal).
#[derive(Default)]
struct Names {
    declared: HashSet<String>,
    bound: HashSet<String>,
}

impl Names {
    fn declare(&mut self, name: &str) {
        self.declared.insert(name.to_string());
        self.bound.insert(name.to_string());
    }
    fn assign(&mut self, name: &str) {
        self.bound.insert(name.to_string());
    }
}

fn collect_stmt_bindings(stmts: &[Stmt], names: &mut Names) {
    for s in stmts {
        match s {
            Stmt::Let { name, .. } => names.declare(name),
            Stmt::LetDestructure { names: ns, .. } => {
                for n in ns {
                    names.declare(n);
                }
            }
            Stmt::Assign { name, .. } => names.assign(name),
            Stmt::Block(inner) => collect_stmt_bindings(inner, names),
            Stmt::If { then_branch, else_branch, .. } => {
                collect_stmt_bindings(then_branch, names);
                collect_stmt_bindings(else_branch, names);
            }
            Stmt::While { body, .. } => collect_stmt_bindings(body, names),
            Stmt::For { var, body, .. } => {
                names.declare(var);
                collect_stmt_bindings(body, names);
            }
            Stmt::Match { arms, .. } => {
                for arm in arms {
                    collect_stmt_bindings(&arm.body, names);
                }
            }
            _ => {}
        }
    }
}

fn collect_program_bindings(program: &Program, names: &mut Names) {
    for item in &program.items {
        match item {
            TopLevel::Let { name, .. } => names.declare(name),
            TopLevel::Func(f) => {
                names.declare(&f.name);
                for p in &f.params {
                    names.declare(p);
                }
                collect_stmt_bindings(&f.body, names);
            }
            TopLevel::Entity(e) => {
                for h in &e.handlers {
                    collect_stmt_bindings(&h.body, names);
                }
            }
            TopLevel::Handler(h) => collect_stmt_bindings(&h.body, names),
            TopLevel::Camera(_) | TopLevel::Light(_) => {}
            TopLevel::World(s) | TopLevel::Spatial3D(s) => {
                for si in &s.items {
                    match si {
                        crate::ast::SpatialItem::Func(f) => {
                            names.declare(&f.name);
                            for p in &f.params {
                                names.declare(p);
                            }
                            collect_stmt_bindings(&f.body, names);
                        }
                        crate::ast::SpatialItem::Let { name, .. } => names.declare(name),
                        crate::ast::SpatialItem::Entity(e) => {
                            for h in &e.handlers {
                                collect_stmt_bindings(&h.body, names);
                            }
                        }
                        crate::ast::SpatialItem::Handler(h) => collect_stmt_bindings(&h.body, names),
                        _ => {}
                    }
                }
            }
            TopLevel::Payload(_) | TopLevel::UILayout(_) => {}
            TopLevel::UseJs(_) | TopLevel::Routes(_) | TopLevel::I18n(_) => {}
            TopLevel::Component(c) => {
                for h in &c.hooks {
                    collect_stmt_bindings(&h.body, names);
                }
            }
        }
    }
}

// ── Pass B: periksa penggunaan ──────────────────────────────────────────────
struct Ctx<'a> {
    src: &'a str,
    vocab: &'a Vocabulary,
    names: &'a Names,
    diags: Vec<Diagnostic>,
}

impl<'a> Ctx<'a> {
    fn warn(&mut self, needle: &str, msg: &str, hint: &str) {
        self.diags.push(diag(Severity::Warning, self.src, needle, msg, hint));
    }
    fn info(&mut self, needle: &str, msg: &str, hint: &str) {
        self.diags.push(diag(Severity::Info, self.src, needle, msg, hint));
    }

    fn check_expr(&mut self, e: &Expr) {
        match e {
            Expr::Ident(name) => {
                // builtin idents (t/mouseX/mouseY/PI) & keyword (true/false) sah
                if self.vocab.ident.contains(name) || self.vocab.keyword.contains(name) {
                    return;
                }
                // nama yang di-bind di program → sah
                if self.names.bound.contains(name) {
                    return;
                }
                // Nama reserved lain (mis. prop, builder) dipakai sebagai
                // nilai ident hanya di konteks prop (dicek terpisah).
                if self.vocab.is_reserved(name) {
                    return;
                }
                self.warn(
                    name,
                    &format!("Variabel tidak dikenal '{name}'"),
                    "Deklarasikan dengan let, atau gunakan builtin (t/mouseX/mouseY/PI).",
                );
            }
            Expr::Call { name, args, props } => {
                self.check_call(name, args, props.as_ref());
                for a in args {
                    self.check_expr(a);
                }
                if let Some(ps) = props {
                    for p in ps {
                        self.check_builder_prop(p);
                    }
                }
            }
            Expr::Tuple(items) | Expr::List(items) => {
                for it in items {
                    self.check_expr(it);
                }
            }
            Expr::Map(pairs) => {
                for (_, v) in pairs {
                    self.check_expr(v);
                }
            }
            Expr::UnaryMinus(inner) => self.check_expr(inner),
            Expr::Binary { lhs, rhs, .. } => {
                self.check_expr(lhs);
                self.check_expr(rhs);
            }
            _ => {}
        }
    }

    fn check_call(&mut self, name: &str, args: &[Expr], props: Option<&Vec<Prop>>) {
        // Builder mesh/material — property harus ∈ meshparam
        if self.vocab.is_builder(name) {
            if let Some(ps) = props {
                for p in ps {
                    if !self.vocab.meshparam.contains(&p.name) {
                        self.warn(
                            &p.name,
                            &format!("Property builder '{name}' tidak dikenal: '{}'", p.name),
                            "Property mesh/material: radius tube inner segments size count.",
                        );
                    }
                }
            }
            // Arity positional builder
            if let Some(max) = self.vocab.builder_max_args(name) {
                if args.len() > max {
                    self.info(
                        name,
                        &format!(
                            "Builder '{name}' menerima maksimal {max} argumen positional (dapat {})",
                            args.len()
                        ),
                        "Gunakan property block { ... } untuk parameter tambahan.",
                    );
                }
            }
            return;
        }
        // Builtin atau fungsi user?
        let known = self.vocab.is_builtin_call(name) || self.names.bound.contains(name);
        if !known {
            self.warn(
                name,
                &format!("Fungsi tidak dikenal '{name}'"),
                "Gunakan builtin (move setPos rotate sin cos clamp ...) atau definisikan func.",
            );
            return;
        }
        // Arity builtin
        if let Some((min, max)) = self.vocab.arity(name) {
            if args.len() < min {
                self.warn(
                    name,
                    &format!("'{name}' butuh minimal {min} argumen (dapat {})", args.len()),
                    "Periksa jumlah argumen pemanggilan.",
                );
            } else if let Some(max) = max {
                if args.len() > max {
                    self.info(
                        name,
                        &format!("'{name}' menerima maksimal {max} argumen (dapat {})", args.len()),
                        "Argumen berlebih diabaikan evaluator.",
                    );
                }
            }
        }
    }

    fn check_builder_prop(&mut self, p: &Prop) {
        // Nilai property builder adalah angka/tuple — ident di sini tidak
        // dianggap penggunaan variabel (eval: as_num/as_tuple).
        if let Expr::Ident(name) = &p.value {
            if !self.vocab.meshparam.contains(name)
                && !self.vocab.ident.contains(name)
                && !self.vocab.keyword.contains(name)
            {
                self.warn(
                    name,
                    &format!("Property '{}' menerima angka/tuple, dapat ident '{name}'", p.name),
                    "Gunakan angka atau tuple: radius 0.9 / segments 3.",
                );
            }
        } else {
            self.check_expr(&p.value);
        }
    }

    fn check_props(&mut self, what: &str, allowed: &HashSet<String>, props: &[Prop]) {
        for p in props {
            if !allowed.contains(&p.name) {
                self.warn(
                    &p.name,
                    &format!("Property {what} tidak dikenal: '{}'", p.name),
                    &format!(
                        "Property sah: {}",
                        allowed.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" ")
                    ),
                );
            }
            // Nilai property prop (kecuali builder) berupa expr biasa
            self.check_expr(&p.value);
        }
    }

    fn check_stmt(&mut self, s: &Stmt) {
        match s {
            Stmt::Let { value, .. } => self.check_expr(value),
            Stmt::LetDestructure { value, .. } => self.check_expr(value),
            Stmt::Assign { name, value } => {
                if !self.names.declared.contains(name) && !self.vocab.ident.contains(name) {
                    self.warn(
                        name,
                        &format!("Assign ke variabel yang belum di-deklarasi '{name}'"),
                        "Deklarasikan dulu dengan let (assignment hanya ke nama yang ada).",
                    );
                }
                self.check_expr(value);
            }
            Stmt::ExprStmt(e) => self.check_expr(e),
            Stmt::Return(e) => self.check_expr(e),
            Stmt::Block(inner) => {
                for x in inner {
                    self.check_stmt(x);
                }
            }
            Stmt::If { cond, then_branch, else_branch } => {
                self.check_expr(cond);
                for x in then_branch {
                    self.check_stmt(x);
                }
                for x in else_branch {
                    self.check_stmt(x);
                }
            }
            Stmt::While { cond, body } => {
                self.check_expr(cond);
                for x in body {
                    self.check_stmt(x);
                }
            }
            Stmt::For { start, end, body, .. } => {
                self.check_expr(start);
                self.check_expr(end);
                for x in body {
                    self.check_stmt(x);
                }
            }
            Stmt::Match { subject, arms } => {
                self.check_expr(subject);
                for arm in arms {
                    for x in &arm.body {
                        self.check_stmt(x);
                    }
                }
            }
            Stmt::Navigate { path } => {
                if path.is_empty() {
                    self.warn("@navigate", "@navigate path kosong", "Berikan path rute, mis. @navigate(\"/home\")");
                }
            }
            Stmt::SetLocale { locale } => {
                if locale.is_empty() {
                    self.warn("@set_locale", "@set_locale locale kosong", "Berikan kode locale, mis. @set_locale(\"en\")");
                }
            }
            Stmt::Directive { name, args } => {
                for a in args {
                    self.check_expr(a);
                }
                if name.is_empty() {
                    self.warn("directive", "directive tanpa nama", "Berikan nama directive, mis. @fetch_data()");
                }
            }
        }
    }
}

fn check_program(src: &str, vocab: &Vocabulary, program: &Program) -> Vec<Diagnostic> {
    let mut names = Names::default();
    collect_program_bindings(program, &mut names);
    let mut ctx = Ctx { src, vocab, names: &names, diags: Vec::new() };

    // Nama ui_layout yang tersedia — referensi routes (v1.12.0) diverifikasi.
    let layout_names: std::collections::HashSet<&str> = program
        .items
        .iter()
        .filter_map(|i| match i {
            TopLevel::UILayout(l) => Some(l.name.as_str()),
            _ => None,
        })
        .collect();

    for item in &program.items {
        match item {
            TopLevel::Camera(c) => ctx.check_props("camera", &vocab.cameraprop, &c.props),
            TopLevel::Light(l) => {
                ctx.check_props("light", &vocab.lightprop, &l.props);
                // prop `type` → enum lightprop.type (dari registry, anti-drift)
                for p in &l.props {
                    if p.name == "type" {
                        if let Expr::Ident(t) = &p.value {
                            if !vocab.lightprop_type.contains(t) {
                                ctx.warn(
                                    t,
                                    &format!("Tipe lampu tidak dikenal '{t}'"),
                                    &format!(
                                        "Gunakan: {}",
                                        vocab.lightprop_type
                                            .iter()
                                            .map(|s| s.as_str())
                                            .collect::<Vec<_>>()
                                            .join(" ")
                                    ),
                                );
                            }
                        }
                    }
                }
            }
            TopLevel::Entity(e) => {
                ctx.check_props("entity", &vocab.entityprop, &e.props);
                for h in &e.handlers {
                    for s in &h.body {
                        ctx.check_stmt(s);
                    }
                }
            }
            TopLevel::Let { value, .. } => ctx.check_expr(value),
            TopLevel::Func(f) => {
                for s in &f.body {
                    ctx.check_stmt(s);
                }
            }
            TopLevel::Handler(h) => {
                for s in &h.body {
                    ctx.check_stmt(s);
                }
            }
            TopLevel::World(s) | TopLevel::Spatial3D(s) => {
                for si in &s.items {
                    match si {
                        crate::ast::SpatialItem::Camera(c) => ctx.check_props("camera", &vocab.cameraprop, &c.props),
                        crate::ast::SpatialItem::Light(l) => {
                            ctx.check_props("light", &vocab.lightprop, &l.props);
                            for p in &l.props {
                                if p.name == "type" {
                                    if let Expr::Ident(t) = &p.value {
                                        if !vocab.lightprop_type.contains(t) {
                                            ctx.warn(
                                                t,
                                                &format!("Tipe lampu tidak dikenal '{t}'"),
                                                &format!("Gunakan: {}", vocab.lightprop_type.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" ")),
                                            );
                                        }
                                    }
                                }
                            }
                        }
                        crate::ast::SpatialItem::Entity(e) => {
                            ctx.check_props("entity", &vocab.entityprop, &e.props);
                            for h in &e.handlers {
                                for s in &h.body {
                                    ctx.check_stmt(s);
                                }
                            }
                        }
                        crate::ast::SpatialItem::Let { value, .. } => ctx.check_expr(value),
                        crate::ast::SpatialItem::Func(f) => {
                            for s in &f.body {
                                ctx.check_stmt(s);
                            }
                        }
                        crate::ast::SpatialItem::Handler(h) => {
                            for s in &h.body {
                                ctx.check_stmt(s);
                            }
                        }
                    }
                }
            }
            TopLevel::Payload(_) | TopLevel::UILayout(_) => {}
            TopLevel::UseJs(u) => {
                if !u.url.starts_with("https://") && !u.url.starts_with("http://") {
                    ctx.warn(
                        &u.url,
                        "@use_js url tanpa skema http(s)",
                        "Gunakan URL absolut, mis. @use_js { url \"https://cdn.example/lib.js\" }",
                    );
                }
            }
            TopLevel::Routes(r) => {
                for route in &r.routes {
                    if !layout_names.contains(route.layout.as_str()) {
                        ctx.warn(
                            &route.layout,
                            &format!("Route '{}' menunjuk ui_layout '{}' yang tidak ada", route.path, route.layout),
                            "Tambahkan ui_layout dengan nama tersebut atau perbaiki route.layout",
                        );
                    }
                }
            }
            TopLevel::I18n(_) => {}
            TopLevel::Component(c) => {
                for h in &c.hooks {
                    for s in &h.body {
                        ctx.check_stmt(s);
                    }
                }
            }
        }
    }
    ctx.diags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_bersih_tidak_ada_diagnosa() {
        let src = r#"
            world "ADI Hologram" {
                camera "cam" { pos (0 1.6 7) look (0 0 0) fov 55 }
                light "key" { type point pos (5 6 4) color (1 0.95 0.9) intensity 1.5 }
                entity "core" {
                    pos (0 0 0)
                    mesh sphere { radius 0.8 segments 3 }
                    material wire (0.15 0.8 1) 0.9
                    on frame {
                        let s = sin(t) * 0.5
                        rotate(0.35 * t, (0 1 0))
                        setPos(cos(t) * 2, s, 0)
                    }
                }
            }
        "#;
        let diags = check_src(src).expect("parse");
        assert!(diags.is_empty(), "world sehat harus tanpa diagnosa: {diags:?}");
    }

    #[test]
    fn variabel_dan_fungsi_tidak_dikenal_dilaporkan() {
        let src = r#"
            world "T" {
                entity "e" {
                    on frame {
                        let speed = 1
                        setPos(unknownVar, speed, 0)
                        mysteryFunc(2)
                    }
                }
            }
        "#;
        let diags = check_src(src).expect("parse");
        let msgs: Vec<&str> = diags.iter().map(|d| d.message.as_str()).collect();
        assert!(msgs.iter().any(|m| m.contains("unknownVar")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("mysteryFunc")), "{msgs:?}");
        // speed ter-deklarasi → tidak boleh dilaporkan
        assert!(!msgs.iter().any(|m| m.contains("'speed'")), "{msgs:?}");
    }

    #[test]
    fn assign_ke_nama_yang_belum_dideklarasi_dilaporkan() {
        let src = r#"
            world "T" {
                entity "e" {
                    on frame {
                        counter = 5
                    }
                }
            }
        "#;
        let diags = check_src(src).expect("parse");
        assert!(diags.iter().any(|d| d.message.contains("counter")), "{diags:?}");
    }

    #[test]
    fn prop_entity_dan_mesh_tidak_dikenal_dilaporkan() {
        let src = r#"
            world "T" {
                entity "e" {
                    pos (0 0 0)
                    texture gold
                    mesh sphere { radius 1 foo 2 }
                }
            }
        "#;
        let diags = check_src(src).expect("parse");
        let msgs: Vec<&str> = diags.iter().map(|d| d.message.as_str()).collect();
        assert!(msgs.iter().any(|m| m.contains("texture")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("'foo'")), "{msgs:?}");
    }

    #[test]
    fn arity_builtin_salah_dilaporkan() {
        // sin butuh 1 argumen; move butuh minimal 3
        let src = r#"
            world "T" {
                entity "e" {
                    on frame {
                        let a = sin(1, 2)
                        move(1, 2)
                    }
                }
            }
        "#;
        let diags = check_src(src).expect("parse");
        let msgs: Vec<&str> = diags.iter().map(|d| d.message.as_str()).collect();
        assert!(msgs.iter().any(|m| m.contains("'sin'") && m.contains("1 argumen")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("'move'") && m.contains("minimal 3")), "{msgs:?}");
    }

    #[test]
    fn tipe_lampu_tidak_dikenal_dilaporkan() {
        let src = r#"
            world "T" {
                light "key" { type strobe pos (0 0 0) }
            }
        "#;
        let diags = check_src(src).expect("parse");
        assert!(diags.iter().any(|d| d.message.contains("strobe")), "{diags:?}");
    }

    #[test]
    fn parse_error_dikembalikan_sebagai_err_dengan_baris() {
        let src = "world \"w\" { entity \"e\" { mesh sphre { radius 1 } } }";
        let res = check_src(src);
        assert!(res.is_err(), "builder typo harus syntax error");
        let err = res.unwrap_err();
        assert!(err.contains("sphre") || err.contains("baris"), "{err}");
    }

    #[test]
    fn kosakata_identity_dari_registry() {
        let v = Vocabulary::from_registry();
        assert!(v.mesh.contains("sphere"));
        assert!(v.material.contains("glow"));
        assert!(v.transform.contains("rotate"));
        assert!(v.math1.contains("sin"));
        assert!(v.ident.contains("t"));
        assert!(v.entityprop.contains("mesh"));
        assert!(v.is_reserved("in"));
        assert!(v.is_reserved("else"));
        assert!(!v.is_reserved("kecepatanUser"));
    }

    #[test]
    fn default_world_adi_bersih() {
        let src = include_str!("../worlds/default.adi");
        let diags = check_src(src).expect("default.adi harus parse");
        assert!(diags.is_empty(), "default.adi sehat tanpa diagnosa: {diags:?}");
    }
}
