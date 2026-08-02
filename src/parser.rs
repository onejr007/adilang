// ADILang parser — recursive descent (v2.0.0 — multi-domain).
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

    // ── Top level — multi-block file ──────────────────────────────────────
    fn parse_program(&mut self) -> Result<Program, String> {
        let mut items = Vec::new();
        let mut name = String::new();
        while self.peek() != &TokKind::Eof {
            let item = self.parse_top_level()?;
            // Derive program name dari spatial_3d/world pertama
            if name.is_empty() {
                match &item {
                    TopLevel::Spatial3D(def) | TopLevel::World(def) => {
                        name = def.name.clone();
                    }
                    _ => {}
                }
            }
            items.push(item);
        }
        Ok(Program { name, items })
    }

    fn parse_top_level(&mut self) -> Result<TopLevel, String> {
        match self.peek().clone() {
            TokKind::At => self.parse_at_block(),
            TokKind::Ident(id) => match id.as_str() {
                "ui_layout" => self.parse_ui_layout(),
                "spatial_3d" | "world" => self.parse_spatial_3d(id == "world"),
                "routes" => self.parse_routes(),
                "component" => self.parse_component(),
                "camera" | "light" | "entity" | "let" | "func" | "on" => {
                    // Legacy: item di luar world/spatial_3d — bungkus ke Spatial3D implisit
                    let mut implicit_items = Vec::new();
                    loop {
                        if self.peek() == &TokKind::Eof {
                            break;
                        }
                        // Deteksi blok berikutnya (ui_layout / spatial_3d / world / routes / component / @payload)
                        if matches!(self.peek(), TokKind::At)
                            || matches!(self.peek(), TokKind::Ident(ref i)
                                if matches!(i.as_str(), "ui_layout" | "spatial_3d" | "world" | "routes" | "component"))
                        {
                            break;
                        }
                        implicit_items.push(self.parse_spatial_item()?);
                    }
                    if implicit_items.is_empty() {
                        return Err(format!(
                            "Ekspektasi deklarasi spatial/ui/payload, dapat '{}' di baris {}",
                            id,
                            self.line()
                        ));
                    }
                    Ok(TopLevel::Spatial3D(Spatial3DDef {
                        name: "__implicit__".to_string(),
                        items: implicit_items,
                    }))
                }
                other => Err(format!(
                    "Keyword/deklarasi tidak dikenal '{other}' di baris {}",
                    self.line()
                )),
            },
            other => Err(format!(
                "Ekspektasi blok (@payload / ui_layout / spatial_3d / world) atau deklarasi, dapat {:?} di baris {}",
                other,
                self.line()
            )),
        }
    }

    // ── @payload / @use_js / @i18n ─────────────────────────────────────────
    fn parse_at_block(&mut self) -> Result<TopLevel, String> {
        self.advance(); // @
        let directive = self.expect_ident("directive (@payload/@use_js/@i18n)")?;
        match directive.as_str() {
            "payload" => self.parse_payload_body(),
            "use_js" => self.parse_use_js(),
            "i18n" => self.parse_i18n(),
            other => Err(format!(
                "Directive '@{other}' tidak dikenal di baris {} (yang sah: @payload, @use_js, @i18n)",
                self.line()
            )),
        }
    }

    // ── @payload ──────────────────────────────────────────────────────────
    fn parse_payload(&mut self) -> Result<TopLevel, String> {
        self.advance(); // @
        self.expect(&TokKind::Ident("payload".to_string()), "'payload' setelah @")?;
        self.parse_payload_body()
    }

    fn parse_payload_body(&mut self) -> Result<TopLevel, String> {
        self.expect(&TokKind::LBrace, "'{'")?;
        let mut sender = String::new();
        let mut target_agent = String::new();
        let mut intent = String::new();
        let mut state_data = None;
        while self.peek() != &TokKind::RBrace {
            if self.peek() == &TokKind::Eof {
                return Err(format!("'@payload' tidak ditutup, baris {}", self.line()));
            }
            let key = self.expect_ident("kunci payload")?;
            match key.as_str() {
                "sender" => sender = self.expect_str()?,
                "target_agent" => target_agent = self.expect_str()?,
                "intent" => intent = self.expect_str()?,
                "state_data" => {
                    state_data = Some(self.parse_expr()?);
                }
                other => {
                    return Err(format!(
                        "Kunci payload tidak dikenal '{other}' di baris {}",
                        self.line()
                    ));
                }
            }
        }
        self.advance(); // }
        if sender.is_empty() || target_agent.is_empty() || intent.is_empty() {
            return Err(format!(
                "@payload membutuhkan sender, target_agent, intent (baris {})",
                self.line()
            ));
        }
        Ok(TopLevel::Payload(PayloadDef { sender, target_agent, intent, state_data }))
    }

    // ── @use_js ────────────────────────────────────────────────────────────
    fn parse_use_js(&mut self) -> Result<TopLevel, String> {
        self.expect(&TokKind::LBrace, "'{'")?;
        let mut url = String::new();
        while self.peek() != &TokKind::RBrace {
            if self.peek() == &TokKind::Eof {
                return Err(format!("'@use_js' tidak ditutup, baris {}", self.line()));
            }
            let key = self.expect_ident("kunci (@use_js)")?;
            match key.as_str() {
                "url" => url = self.expect_str()?,
                other => {
                    return Err(format!(
                        "Kunci '@use_js' tidak dikenal '{other}' (hanya 'url') di baris {}",
                        self.line()
                    ));
                }
            }
        }
        self.advance(); // }
        if url.is_empty() {
            return Err(format!("'@use_js' wajib punya url, baris {}", self.line()));
        }
        Ok(TopLevel::UseJs(UseJsDef { url }))
    }

    // ── routes ─────────────────────────────────────────────────────────────
    fn parse_routes(&mut self) -> Result<TopLevel, String> {
        self.advance(); // routes
        self.expect(&TokKind::LBrace, "'{'")?;
        let mut routes = Vec::new();
        while self.peek() != &TokKind::RBrace {
            if self.peek() == &TokKind::Eof {
                return Err(format!("'routes' tidak ditutup, baris {}", self.line()));
            }
            let kw = self.expect_ident("'route'")?;
            if kw != "route" {
                return Err(format!(
                    "Ekspektasi 'route' di dalam routes, dapat '{kw}' di baris {}",
                    self.line()
                ));
            }
            let path = self.expect_str()?;
            let mut layout = String::new();
            let mut transition = None;
            while self.peek() != &TokKind::RBrace {
                if self.peek() == &TokKind::Eof {
                    return Err(format!("'routes' tidak ditutup, baris {}", self.line()));
                }
                match self.peek().clone() {
                    TokKind::Ident(id) if id == "route" => break,
                    TokKind::Ident(id) if id == "layout" => {
                        self.advance();
                        layout = self.expect_str()?;
                    }
                    TokKind::Ident(id) if id == "transition" => {
                        self.advance();
                        transition = Some(self.expect_str()?);
                    }
                    _ => break,
                }
            }
            if layout.is_empty() {
                return Err(format!(
                    "Route '{path}' wajib punya layout, baris {}",
                    self.line()
                ));
            }
            routes.push(RouteDef { path, layout, transition });
        }
        self.advance(); // }
        if routes.is_empty() {
            return Err(format!("'routes' tanpa route, baris {}", self.line()));
        }
        Ok(TopLevel::Routes(RoutesDef { routes }))
    }

    // ── @i18n ──────────────────────────────────────────────────────────────
    fn parse_i18n(&mut self) -> Result<TopLevel, String> {
        self.expect(&TokKind::LBrace, "'{'")?;
        let mut locales = Vec::new();
        while self.peek() != &TokKind::RBrace {
            if self.peek() == &TokKind::Eof {
                return Err(format!("'@i18n' tidak ditutup, baris {}", self.line()));
            }
            let kw = self.expect_ident("'locale'")?;
            if kw != "locale" {
                return Err(format!(
                    "Ekspektasi 'locale' di dalam @i18n, dapat '{kw}' di baris {}",
                    self.line()
                ));
            }
            let name = self.expect_str()?;
            self.expect(&TokKind::LBrace, "'{'")?;
            let mut entries = Vec::new();
            while self.peek() != &TokKind::RBrace {
                if self.peek() == &TokKind::Eof {
                    return Err(format!("locale '{name}' tidak ditutup, baris {}", self.line()));
                }
                let key = self.expect_ident("kunci terjemahan")?;
                let value = self.expect_str()?;
                entries.push((key, value));
            }
            self.advance(); // }
            if entries.is_empty() {
                return Err(format!("locale '{name}' tanpa entri, baris {}", self.line()));
            }
            locales.push(I18nLocale { name, entries });
        }
        self.advance(); // }
        if locales.is_empty() {
            return Err(format!("'@i18n' tanpa locale, baris {}", self.line()));
        }
        Ok(TopLevel::I18n(I18nDef { locales }))
    }

    // ── component (lifecycle hooks) ────────────────────────────────────────
    // `component MyCard { on_mount: @fetch_data() on_update: @log_change() on_unmount: @cleanup_state() }`
    fn parse_component(&mut self) -> Result<TopLevel, String> {
        self.advance(); // component
        let name = self.expect_ident("nama komponen")?;
        self.expect(&TokKind::LBrace, "'{'")?;
        let mut hooks = Vec::new();
        while self.peek() != &TokKind::RBrace {
            if self.peek() == &TokKind::Eof {
                return Err(format!("component '{name}' tidak ditutup, baris {}", self.line()));
            }
            let kw = self.expect_ident("lifecycle hook (on_mount/on_update/on_unmount)")?;
            let kind = match kw.as_str() {
                "on_mount" => LifecycleHookKind::Mount,
                "on_update" => LifecycleHookKind::Update,
                "on_unmount" => LifecycleHookKind::Unmount,
                other => {
                    return Err(format!(
                        "Lifecycle hook tidak dikenal '{other}' di baris {} (yang sah: on_mount, on_update, on_unmount)",
                        self.line()
                    ));
                }
            };
            if self.peek() == &TokKind::Colon {
                self.advance();
            }
            let mut body = Vec::new();
            while self.peek() != &TokKind::RBrace
                && !matches!(self.peek(), TokKind::Ident(id)
                    if matches!(id.as_str(), "on_mount" | "on_update" | "on_unmount"))
            {
                if self.peek() == &TokKind::Eof {
                    return Err(format!("component '{name}' tidak ditutup, baris {}", self.line()));
                }
                body.push(self.parse_stmt()?);
            }
            if body.is_empty() {
                return Err(format!(
                    "Hook '{kw}' pada component '{name}' tanpa isi, baris {}",
                    self.line()
                ));
            }
            hooks.push(LifecycleHook { kind, body });
        }
        self.advance(); // }
        if hooks.is_empty() {
            return Err(format!("component '{name}' tanpa hook, baris {}", self.line()));
        }
        Ok(TopLevel::Component(ComponentDef { name, hooks }))
    }

    // ── ui_layout ─────────────────────────────────────────────────────────
    fn parse_ui_layout(&mut self) -> Result<TopLevel, String> {
        self.advance(); // ui_layout
        let name = self.expect_str()?;
        self.expect(&TokKind::LBrace, "'{'")?;
        let mut children = Vec::new();
        while self.peek() != &TokKind::RBrace {
            if self.peek() == &TokKind::Eof {
                return Err(format!("'ui_layout' tidak ditutup, baris {}", self.line()));
            }
            children.push(self.parse_ui_component()?);
        }
        self.advance(); // }
        let root = if children.len() == 1 {
            children.into_iter().next().unwrap()
        } else {
            UIComponent::Container { flex: Some(FlexDirection::Column), children }
        };
        Ok(TopLevel::UILayout(UILayoutDef { name, root }))
    }

    fn parse_ui_component(&mut self) -> Result<UIComponent, String> {
        let kw = match self.peek().clone() {
            TokKind::Ident(id) => id,
            _ => return Err(format!("Ekspektasi komponen UI di baris {}", self.line())),
        };
        match kw.as_str() {
            "container" => {
                self.advance();
                self.expect(&TokKind::LBrace, "'{'")?;
                let mut flex = None;
                let mut children = Vec::new();
                // Phase 1: parse properties (flex, etc.)
                loop {
                    if self.peek() == &TokKind::RBrace || self.peek() == &TokKind::Eof {
                        break;
                    }
                    if let TokKind::Ident(ref id) = self.peek() {
                        if id == "text"
                            || id == "button"
                            || id == "input"
                            || id == "container"
                            || id == "card"
                            || id == "modal"
                            || id == "navbar"
                            || id == "footer"
                        {
                            break;
                        }
                    }
                    let prop = self.expect_ident("properti container")?;
                    match prop.as_str() {
                        "flex" => {
                            let dir = self.expect_ident("arah flex (row/column)")?;
                            flex = Some(match dir.as_str() {
                                "row" => FlexDirection::Row,
                                "column" => FlexDirection::Column,
                                other => {
                                    return Err(format!(
                                        "Flex direction tidak dikenal '{other}' di baris {}",
                                        self.line()
                                    ));
                                }
                            });
                        }
                        _ => {
                            let _ = self.parse_expr();
                        }
                    }
                }
                // Phase 2: parse children
                while self.peek() != &TokKind::RBrace
                    && self.peek() != &TokKind::Eof
                    && !matches!(self.peek(), TokKind::Ident(ref i)
                        if matches!(i.as_str(), "ui_layout" | "spatial_3d" | "world"))
                    && self.peek() != &TokKind::At
                {
                    children.push(self.parse_ui_component()?);
                }
                self.advance(); // }
                Ok(UIComponent::Container { flex, children })
            }
            "text" => {
                self.advance();
                let content = self.expect_str()?;
                Ok(UIComponent::Text { content })
            }
            "button" => {
                self.advance();
                let label = self.expect_str()?;
                let mut onClick = None;
                if self.peek() == &TokKind::Ident("onClick".to_string()) {
                    self.advance();
                    onClick = Some(self.expect_ident("nama handler")?);
                }
                Ok(UIComponent::Button { label, onClick })
            }
            "input" => {
                self.advance();
                let name = self.expect_str()?;
                let mut placeholder = None;
                let mut bind = None;
                let mut validate = None;
                loop {
                    match self.peek().clone() {
                        TokKind::Ident(id) if id == "placeholder" => {
                            self.advance();
                            placeholder = Some(self.expect_str()?);
                        }
                        TokKind::Ident(id) if id == "bind" => {
                            self.advance();
                            if self.peek() == &TokKind::Colon {
                                self.advance();
                            }
                            if self.peek() == &TokKind::At {
                                self.advance();
                                let mut path = self.expect_ident("nama state (setelah @)")?;
                                while self.peek() == &TokKind::Dot {
                                    self.advance();
                                    path.push('.');
                                    path.push_str(&self.expect_ident("segmen path state")?);
                                }
                                bind = Some(path);
                            } else {
                                bind = Some(self.expect_str()?);
                            }
                        }
                        TokKind::Ident(id) if id == "validate" => {
                            self.advance();
                            if self.peek() == &TokKind::Colon {
                                self.advance();
                            }
                            validate = Some(self.expect_str()?);
                        }
                        _ => break,
                    }
                }
                Ok(UIComponent::Input { name, placeholder, bind, validate })
            }
            "card" | "modal" => {
                let is_card = kw == "card";
                self.advance();
                let mut title = None;
                if let TokKind::Str(_) = self.peek() {
                    title = Some(self.expect_str()?);
                }
                self.expect(&TokKind::LBrace, "'{'")?;
                let mut children = Vec::new();
                while self.peek() != &TokKind::RBrace {
                    if self.peek() == &TokKind::Eof {
                        return Err(format!("'{kw}' tidak ditutup, baris {}", self.line()));
                    }
                    children.push(self.parse_ui_component()?);
                }
                self.advance(); // }
                if is_card {
                    Ok(UIComponent::Card { title, children })
                } else {
                    Ok(UIComponent::Modal { title, children })
                }
            }
            "navbar" => {
                self.advance();
                let mut title = None;
                if let TokKind::Str(_) = self.peek() {
                    title = Some(self.expect_str()?);
                }
                Ok(UIComponent::Navbar { title })
            }
            "footer" => {
                self.advance();
                let content = self.expect_str()?;
                Ok(UIComponent::Footer { content })
            }
            other => Err(format!(
                "Komponen UI tidak dikenal '{other}' di baris {}",
                self.line()
            )),
        }
    }

    // ── spatial_3d / world ────────────────────────────────────────────────
    fn parse_spatial_3d(&mut self, is_world_alias: bool) -> Result<TopLevel, String> {
        let name = if is_world_alias {
            self.advance(); // world
            self.expect_str()?
        } else {
            self.advance(); // spatial_3d
            self.expect_str()?
        };
        self.expect(&TokKind::LBrace, "'{'")?;
        let mut items = Vec::new();
        while self.peek() != &TokKind::RBrace {
            if self.peek() == &TokKind::Eof {
                return Err(format!(
                    "'{}' tidak ditutup, baris {}",
                    if is_world_alias { "world" } else { "spatial_3d" },
                    self.line()
                ));
            }
            items.push(self.parse_spatial_item()?);
        }
        self.advance(); // }
        let def = Spatial3DDef { name, items };
        if is_world_alias {
            Ok(TopLevel::World(def))
        } else {
            Ok(TopLevel::Spatial3D(def))
        }
    }

    fn parse_spatial_item(&mut self) -> Result<SpatialItem, String> {
        let kw = match self.peek().clone() {
            TokKind::Ident(id) => id,
            _ => return Err(format!("Ekspektasi deklarasi spatial di baris {}", self.line())),
        };
        match kw.as_str() {
            "camera" => {
                self.advance();
                let id = self.expect_str()?;
                let props = self.parse_prop_block()?;
                Ok(SpatialItem::Camera(CameraDef { id, props }))
            }
            "light" => {
                self.advance();
                let id = self.expect_str()?;
                let props = self.parse_prop_block()?;
                Ok(SpatialItem::Light(LightDef { id, props }))
            }
            "entity" => {
                self.advance();
                let id = self.expect_str()?;
                let mut props = Vec::new();
                let mut handlers = Vec::new();
                self.expect(&TokKind::LBrace, "'{'")?;
                while self.peek() != &TokKind::RBrace {
                    if self.peek() == &TokKind::Eof {
                        return Err(format!("entity tidak ditutup, baris {}", self.line()));
                    }
                    if self.peek() == &TokKind::Ident("on".to_string()) {
                        handlers.push(self.parse_handler()?);
                    } else {
                        props.push(self.parse_prop()?);
                    }
                }
                self.advance();
                Ok(SpatialItem::Entity(EntityDef { id, props, handlers }))
            }
            "let" => {
                self.advance();
                let name = self.expect_ident("nama variabel")?;
                self.expect(&TokKind::Assign, "'='")?;
                let value = self.parse_expr()?;
                Ok(SpatialItem::Let { name, value })
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
                Ok(SpatialItem::Func(FuncDef { name, params, body }))
            }
            "on" => Ok(SpatialItem::Handler(self.parse_handler()?)),
            other => Err(format!("Deklarasi spatial tidak dikenal '{other}' di baris {}", self.line())),
        }
    }

    // ── Handler ───────────────────────────────────────────────────────────
    fn parse_handler(&mut self) -> Result<Handler, String> {
        self.advance(); // on
        let ev = self.expect_ident("event (frame/speak/silent/click)")?;
        let event = match ev.as_str() {
            "frame" => EventKind::Frame,
            "speak" => EventKind::Speak,
            "silent" => EventKind::Silent,
            "click" => EventKind::Click,
            other => {
                return Err(format!("Event tidak dikenal '{other}' di baris {}", self.line()));
            }
        };
        let body = self.parse_block()?;
        Ok(Handler { event, body })
    }

    // ── Property block ────────────────────────────────────────────────────
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

    fn parse_prop_value(&mut self) -> Result<Expr, String> {
        match self.peek().clone() {
            TokKind::Ident(id) if is_builder(&id) => self.parse_builder(&id),
            TokKind::Ident(id) => {
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

    // ── Block & Statement ─────────────────────────────────────────────────
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
            TokKind::At => {
                self.advance(); // @
                let name = self.expect_ident("directive statement (@navigate/@set_locale)")?;
                match name.as_str() {
                    "navigate" => {
                        self.expect(&TokKind::LParen, "'('")?;
                        let path = self.expect_str()?;
                        self.expect(&TokKind::RParen, "')'")?;
                        Ok(Stmt::Navigate { path })
                    }
                    "set_locale" => {
                        self.expect(&TokKind::LParen, "'('")?;
                        let locale = self.expect_str()?;
                        self.expect(&TokKind::RParen, "')'")?;
                        Ok(Stmt::SetLocale { locale })
                    }
                    other => {
                        // Directive generik (v1.13.0): @fetch_data(), @log_change(),
                        // @cleanup_state() — dipakai lifecycle hooks component.
                        self.expect(&TokKind::LParen, "'('")?;
                        let args = self.parse_paren_list()?;
                        Ok(Stmt::Directive { name: other.to_string(), args })
                    }
                }
            }
            TokKind::Ident(kw) => match kw.as_str() {
                "let" => {
                    self.advance();
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
                                    _ => {
                                        return Err(format!(
                                            "Ekspektasi angka setelah '-' di pattern match, baris {}",
                                            self.line()
                                        ));
                                    }
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
                                ));
                            }
                        };
                        self.expect(&TokKind::Arrow, "'=>' (arm match)")?;
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
                    self.advance();
                    let cond = self.parse_expr()?;
                    let body = self.parse_block()?;
                    Ok(Stmt::While { cond, body })
                }
                "for" => {
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

    // ── Expressions ──────────────────────────────────────────────────────
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

    fn expect_str(&mut self) -> Result<String, String> {
        match self.peek().clone() {
            TokKind::Str(s) => {
                self.advance();
                Ok(s)
            }
            _ => Err(format!("Ekspektasi string di baris {}", self.line())),
        }
    }
}

fn is_builder(id: &str) -> bool {
    crate::registry::is_builder(id)
}
