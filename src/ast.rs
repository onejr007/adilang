// ADILang AST — pohon sintaks abstrak (v2.0.0 — multi-domain).
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).
//
// Blok utama:
//   @payload   — header komunikasi inter-AI
//   ui_layout  — komponen Web 2D deklaratif
//   spatial_3d — adegan 3D (alias: world untuk backward compat)
//   Camera/Light/Entity/Let/Func/Handler — item di dalam spatial_3d/world

// ═══════════════════════════════════════════════════════════════════════════
// EKSPRESI & OPERATOR
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Num(f64),
    Str(String),
    Bool(bool),
    Tuple(Vec<Expr>),
    List(Vec<Expr>),
    Map(Vec<(String, Expr)>),
    Ident(String),
    Call {
        name: String,
        args: Vec<Expr>,
        props: Option<Vec<Prop>>,
    },
    UnaryMinus(Box<Expr>),
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Prop {
    pub name: String,
    pub value: Expr,
}

// ═══════════════════════════════════════════════════════════════════════════
// STATEMENT & CONTROL FLOW
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
pub enum MatchPattern {
    Str(String),
    Num(f64),
    Wildcard,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: MatchPattern,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Let { name: String, value: Expr },
    LetDestructure { names: Vec<String>, value: Expr },
    Assign { name: String, value: Expr },
    ExprStmt(Expr),
    Return(Expr),
    Block(Vec<Stmt>),
    If {
        cond: Expr,
        then_branch: Vec<Stmt>,
        else_branch: Vec<Stmt>,
    },
    While { cond: Expr, body: Vec<Stmt> },
    For { var: String, start: Expr, end: Expr, body: Vec<Stmt> },
    Match { subject: Expr, arms: Vec<MatchArm> },
    // Directive statements (v1.12.0): @navigate("..."), @set_locale("...")
    Navigate { path: String },
    SetLocale { locale: String },
    // Directive generik (v1.13.0): @fetch_data(), @log_change(), ...
    // Statement deklaratif yang dikirim ke runtime (JS/host) untuk diproses —
    // dipakai oleh lifecycle hooks `component` (on_mount/on_update/on_unmount).
    Directive { name: String, args: Vec<Expr> },
}

// ═══════════════════════════════════════════════════════════════════════════
// EVENT & HANDLER
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
pub enum EventKind {
    Frame,
    Speak,
    Silent,
    Click,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Handler {
    pub event: EventKind,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FuncDef {
    pub name: String,
    pub params: Vec<String>,
    pub body: Vec<Stmt>,
}

// ═══════════════════════════════════════════════════════════════════════════
// SPATIAL 3D — adegan 3D (world)
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
pub struct CameraDef {
    pub id: String,
    pub props: Vec<Prop>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LightDef {
    pub id: String,
    pub props: Vec<Prop>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EntityDef {
    pub id: String,
    pub props: Vec<Prop>,
    pub handlers: Vec<Handler>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SpatialItem {
    Camera(CameraDef),
    Light(LightDef),
    Entity(EntityDef),
    Let { name: String, value: Expr },
    Func(FuncDef),
    Handler(Handler),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Spatial3DDef {
    pub name: String,
    pub items: Vec<SpatialItem>,
}

// ═══════════════════════════════════════════════════════════════════════════
// UI LAYOUT — komponen Web 2D deklaratif
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
pub enum FlexDirection {
    Row,
    Column,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UIComponent {
    Container {
        flex: Option<FlexDirection>,
        children: Vec<UIComponent>,
    },
    Text {
        content: String,
    },
    Button {
        label: String,
        onClick: Option<String>,
    },
    Input {
        name: String,
        placeholder: Option<String>,
        /// Path state yang di-bind dua arah (mis. "state.username") — v1.12.0.
        bind: Option<String>,
        /// Aturan validasi dipisah '|' (mis. "required|email") — v1.12.0.
        validate: Option<String>,
    },
    // UI Standard Library (v1.12.0) — komponen 2D deklaratif.
    Card {
        title: Option<String>,
        children: Vec<UIComponent>,
    },
    Modal {
        title: Option<String>,
        children: Vec<UIComponent>,
    },
    Navbar {
        title: Option<String>,
    },
    Footer {
        content: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct UILayoutDef {
    pub name: String,
    pub root: UIComponent,
}

// ═══════════════════════════════════════════════════════════════════════════
// PAYLOAD — header komunikasi inter-AI
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
pub struct PayloadDef {
    pub sender: String,
    pub target_agent: String,
    pub intent: String,
    pub state_data: Option<Expr>,
}

// ═══════════════════════════════════════════════════════════════════════════
// MODUL BARU (v1.12.0) — @use_js / routes / @i18n
// ═══════════════════════════════════════════════════════════════════════════

/// `@use_js { url "https://cdn.example/lib.js" }` — muat skrip CDN eksternal.
#[derive(Debug, Clone, PartialEq)]
pub struct UseJsDef {
    pub url: String,
}

/// Satu rute SPA: path → ui_layout + transisi.
#[derive(Debug, Clone, PartialEq)]
pub struct RouteDef {
    pub path: String,
    pub layout: String,
    pub transition: Option<String>,
}

/// `routes { route "/" layout "home" transition "fade" ... }` — tabel rute SPA.
#[derive(Debug, Clone, PartialEq)]
pub struct RoutesDef {
    pub routes: Vec<RouteDef>,
}

/// Satu bahasa lokal dengan tabel kunci→teks.
#[derive(Debug, Clone, PartialEq)]
pub struct I18nLocale {
    pub name: String,
    pub entries: Vec<(String, String)>,
}

/// `@i18n { locale "en" { welcome "Hello" } ... }` — kamus terjemahan.
#[derive(Debug, Clone, PartialEq)]
pub struct I18nDef {
    pub locales: Vec<I18nLocale>,
}

// ═══════════════════════════════════════════════════════════════════════════
// LIFECYCLE HOOKS (v1.13.0) — Enterprise Application Lifecycle
// ═══════════════════════════════════════════════════════════════════════════

/// Tiga fase lifecycle aplikasi/komponen: mount (dibuat), update (berubah),
/// unmount (dihapus). `component MyCard { on_mount: @fetch_data(), ... }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleHookKind {
    Mount,
    Update,
    Unmount,
}

impl LifecycleHookKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            LifecycleHookKind::Mount => "on_mount",
            LifecycleHookKind::Update => "on_update",
            LifecycleHookKind::Unmount => "on_unmount",
        }
    }
}

/// Satu hook lifecycle: fase + badan statement (biasanya directive `@name()`).
#[derive(Debug, Clone, PartialEq)]
pub struct LifecycleHook {
    pub kind: LifecycleHookKind,
    pub body: Vec<Stmt>,
}

/// `component MyCard { on_mount: @fetch_data() ... }` — blok komponen dengan
/// hook lifecycle yang terintegrasi dengan WASM State Machine (`state.rs`).
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentDef {
    pub name: String,
    pub hooks: Vec<LifecycleHook>,
}

// ═══════════════════════════════════════════════════════════════════════════
// TOP-LEVEL — gabungan semua blok dalam 1 berkas ADILang
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
pub enum TopLevel {
    Payload(PayloadDef),
    UILayout(UILayoutDef),
    Spatial3D(Spatial3DDef),
    World(Spatial3DDef), // alias backward-compat untuk spatial_3d
    Camera(CameraDef),
    Light(LightDef),
    Entity(EntityDef),
    Let { name: String, value: Expr },
    Func(FuncDef),
    Handler(Handler),
    UseJs(UseJsDef),
    Routes(RoutesDef),
    I18n(I18nDef),
    Component(ComponentDef),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub name: String,
    pub items: Vec<TopLevel>,
}
