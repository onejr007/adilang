// ADILang AST — pohon sintaks abstrak.
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).

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
    /// Tuple: (x y z) atau (r g b a)
    Tuple(Vec<Expr>),
    /// List literal (v1.6.0): [ 1 2 3 ] atau ["a", "b"] — elemen heterogen
    /// diperbolehkan (nilai didorong ke Value::List pada evaluasi).
    List(Vec<Expr>),
    /// Map literal (v1.6.0): { key: expr, key2: expr } — pasangan (String, Expr)
    /// urutan sumber DI-PERTAHANKAN (deterministik, P1).
    Map(Vec<(String, Expr)>),
    Ident(String),
    Call {
        name: String,
        args: Vec<Expr>,
        /// Blok properti untuk builder mesh/material: sphere { radius 0.9 segments 3 }
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

/// Pola arm `match` (v1.6.0): literal string/angka, atau wildcard `_`.
#[derive(Debug, Clone, PartialEq)]
pub enum MatchPattern {
    Str(String),
    Num(f64),
    /// `_` — catch-all (wajib terakhir bila ada).
    Wildcard,
}

/// Satu arm `match`: pattern => body.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: MatchPattern,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Let { name: String, value: Expr },
    /// Tuple destructuring (v1.6.0): let (a, b) = f() — bind elemen tuple ke
    /// beberapa nama sekaligus. Panjang names HARUS cocok dgn tuple hasil.
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
    /// `while cond { ... }` — loop bertahan selama kondisi truthy.
    /// Dibatasi evaluator (MAX_LOOP_ITERATIONS) agar deterministik (P1).
    While { cond: Expr, body: Vec<Stmt> },
    /// `for x in start end { ... }` — iterasi numerik [start, end), step 1.
    /// Dibatasi evaluator (MAX_LOOP_ITERATIONS) agar deterministik (P1).
    For { var: String, start: Expr, end: Expr, body: Vec<Stmt> },
    /// `match subject { pat => { ... } pat2 => { ... } _ => { ... } }` (v1.6.0).
    Match {
        subject: Expr,
        arms: Vec<MatchArm>,
    },
}

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
pub enum TopLevel {
    Camera(CameraDef),
    Light(LightDef),
    Entity(EntityDef),
    Let { name: String, value: Expr },
    Func(FuncDef),
    Handler(Handler),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub name: String,
    pub items: Vec<TopLevel>,
}
