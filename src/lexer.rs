// ADILang lexer — memecah source menjadi token.
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).

#[derive(Debug, Clone, PartialEq)]
pub enum TokKind {
    Num(f64),
    Str(String),
    Ident(String),
    // Symbols
    At,            // @ — payload block prefix (v2.0.0)
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,     // [ — list literal (v1.6.0)
    RBracket,     // ]
    Colon,        // : — map literal key separator (v1.6.0)
    Arrow,        // => — match arm (v1.6.0)
    Dot,          // . — path state di bind (v1.12.0)
    Comma,
    Assign,       // =
    Plus,         // +
    Minus,        // -
    Star,         // *
    Slash,        // /
    Percent,      // %
    Eq,           // ==
    Ne,           // !=
    Lt,           // <
    Gt,           // >
    Le,           // <=
    Ge,           // >=
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokKind,
    pub line: usize,
    pub col: usize,
}

pub fn tokenize(src: &str) -> Result<Vec<Token>, String> {
    // Toleransi BOM UTF-8 (umum pada editor Windows) — dibuang di awal.
    let src = src.strip_prefix('\u{feff}').unwrap_or(src);
    let chars: Vec<char> = src.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0usize;
    let mut line = 1usize;
    let mut col = 1usize;

    let mut bump = |i: &mut usize, line: &mut usize, col: &mut usize| {
        if *i < chars.len() {
            if chars[*i] == '\n' {
                *line += 1;
                *col = 1;
            } else {
                *col += 1;
            }
            *i += 1;
        }
    };

    while i < chars.len() {
        let c = chars[i];
        // Whitespace
        if c.is_whitespace() {
            bump(&mut i, &mut line, &mut col);
            continue;
        }
        // Line comment
        if c == '#' {
            while i < chars.len() && chars[i] != '\n' {
                bump(&mut i, &mut line, &mut col);
            }
            continue;
        }
        // Block comment
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
            bump(&mut i, &mut line, &mut col);
            bump(&mut i, &mut line, &mut col);
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                bump(&mut i, &mut line, &mut col);
            }
            if i + 1 < chars.len() {
                bump(&mut i, &mut line, &mut col);
                bump(&mut i, &mut line, &mut col);
            }
            continue;
        }
        let tok_line = line;
        let tok_col = col;

        // String
        if c == '"' {
            bump(&mut i, &mut line, &mut col);
            let mut s = String::new();
            while i < chars.len() && chars[i] != '"' {
                let ch = chars[i];
                if ch == '\\' && i + 1 < chars.len() {
                    bump(&mut i, &mut line, &mut col);
                    let esc = chars[i];
                    match esc {
                        'n' => s.push('\n'),
                        't' => s.push('\t'),
                        '\\' => s.push('\\'),
                        '"' => s.push('"'),
                        other => {
                            s.push('\\');
                            s.push(other);
                        }
                    }
                    bump(&mut i, &mut line, &mut col);
                } else {
                    s.push(ch);
                    bump(&mut i, &mut line, &mut col);
                }
            }
            if i >= chars.len() {
                return Err(format!("String tidak ditutup pada baris {tok_line}"));
            }
            bump(&mut i, &mut line, &mut col); // closing quote
            tokens.push(Token { kind: TokKind::Str(s), line: tok_line, col: tok_col });
            continue;
        }

        // Number (termasuk 0x hex). Unary minus di-handle parser.
        if c.is_ascii_digit() {
            let mut num_str = String::new();
            if i < chars.len() && chars[i] == '0' && i + 1 < chars.len() && (chars[i + 1] == 'x' || chars[i + 1] == 'X') {
                bump(&mut i, &mut line, &mut col); // 0
                bump(&mut i, &mut line, &mut col); // x
                let mut hex = String::new();
                while i < chars.len() && chars[i].is_ascii_hexdigit() {
                    hex.push(chars[i]);
                    bump(&mut i, &mut line, &mut col);
                }
                let v = u64::from_str_radix(&hex, 16).map_err(|_| format!("Hex invalid baris {tok_line}"))? as f64;
                tokens.push(Token { kind: TokKind::Num(v), line: tok_line, col: tok_col });
                continue;
            }
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                num_str.push(chars[i]);
                bump(&mut i, &mut line, &mut col);
            }
            // exponent
            if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') && i + 1 < chars.len() && (chars[i + 1].is_ascii_digit() || ((chars[i + 1] == '+' || chars[i + 1] == '-') && i + 2 < chars.len() && chars[i + 2].is_ascii_digit())) {
                num_str.push(chars[i]);
                bump(&mut i, &mut line, &mut col);
                if i < chars.len() && (chars[i] == '+' || chars[i] == '-') {
                    num_str.push(chars[i]);
                    bump(&mut i, &mut line, &mut col);
                }
                while i < chars.len() && chars[i].is_ascii_digit() {
                    num_str.push(chars[i]);
                    bump(&mut i, &mut line, &mut col);
                }
            }
            if num_str.is_empty() {
                return Err(format!("Angka invalid baris {tok_line}"));
            }
            let v: f64 = num_str.parse().map_err(|_| format!("Angka invalid '{num_str}' baris {tok_line}"))?;
            tokens.push(Token { kind: TokKind::Num(v), line: tok_line, col: tok_col });
            continue;
        }

        // Identifiers / keywords (keywords di-handle parser)
        if c.is_alphabetic() || c == '_' {
            let mut id = String::new();
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                id.push(chars[i]);
                bump(&mut i, &mut line, &mut col);
            }
            tokens.push(Token { kind: TokKind::Ident(id), line: tok_line, col: tok_col });
            continue;
        }

        // Symbols
        match c {
            '@' => { tokens.push(Token { kind: TokKind::At, line, col }); bump(&mut i, &mut line, &mut col); }
            '(' => { tokens.push(Token { kind: TokKind::LParen, line, col }); bump(&mut i, &mut line, &mut col); }
            ')' => { tokens.push(Token { kind: TokKind::RParen, line, col }); bump(&mut i, &mut line, &mut col); }
            '{' => { tokens.push(Token { kind: TokKind::LBrace, line, col }); bump(&mut i, &mut line, &mut col); }
            '}' => { tokens.push(Token { kind: TokKind::RBrace, line, col }); bump(&mut i, &mut line, &mut col); }
            '[' => { tokens.push(Token { kind: TokKind::LBracket, line, col }); bump(&mut i, &mut line, &mut col); }
            ']' => { tokens.push(Token { kind: TokKind::RBracket, line, col }); bump(&mut i, &mut line, &mut col); }
            ':' => { tokens.push(Token { kind: TokKind::Colon, line, col }); bump(&mut i, &mut line, &mut col); }
            '.' => { tokens.push(Token { kind: TokKind::Dot, line, col }); bump(&mut i, &mut line, &mut col); }
            ',' => { tokens.push(Token { kind: TokKind::Comma, line, col }); bump(&mut i, &mut line, &mut col); }
            '=' => {
                if i + 1 < chars.len() && chars[i + 1] == '>' {
                    tokens.push(Token { kind: TokKind::Arrow, line, col });
                    bump(&mut i, &mut line, &mut col);
                    bump(&mut i, &mut line, &mut col);
                } else if i + 1 < chars.len() && chars[i + 1] == '=' {
                    tokens.push(Token { kind: TokKind::Eq, line, col });
                    bump(&mut i, &mut line, &mut col);
                    bump(&mut i, &mut line, &mut col);
                } else {
                    tokens.push(Token { kind: TokKind::Assign, line, col });
                    bump(&mut i, &mut line, &mut col);
                }
            }
            '!' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    tokens.push(Token { kind: TokKind::Ne, line, col });
                    bump(&mut i, &mut line, &mut col);
                    bump(&mut i, &mut line, &mut col);
                } else {
                    return Err(format!("Simbol '!' tidak dikenal baris {line}"));
                }
            }
            '<' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    tokens.push(Token { kind: TokKind::Le, line, col });
                    bump(&mut i, &mut line, &mut col);
                    bump(&mut i, &mut line, &mut col);
                } else {
                    tokens.push(Token { kind: TokKind::Lt, line, col });
                    bump(&mut i, &mut line, &mut col);
                }
            }
            '>' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    tokens.push(Token { kind: TokKind::Ge, line, col });
                    bump(&mut i, &mut line, &mut col);
                    bump(&mut i, &mut line, &mut col);
                } else {
                    tokens.push(Token { kind: TokKind::Gt, line, col });
                    bump(&mut i, &mut line, &mut col);
                }
            }
            '+' => { tokens.push(Token { kind: TokKind::Plus, line, col }); bump(&mut i, &mut line, &mut col); }
            '-' => { tokens.push(Token { kind: TokKind::Minus, line, col }); bump(&mut i, &mut line, &mut col); }
            '*' => { tokens.push(Token { kind: TokKind::Star, line, col }); bump(&mut i, &mut line, &mut col); }
            '/' => { tokens.push(Token { kind: TokKind::Slash, line, col }); bump(&mut i, &mut line, &mut col); }
            '%' => { tokens.push(Token { kind: TokKind::Percent, line, col }); bump(&mut i, &mut line, &mut col); }
            other => return Err(format!("Karakter tidak dikenal '{other}' baris {line} kolom {col}")),
        }
    }

    tokens.push(Token { kind: TokKind::Eof, line, col });
    Ok(tokens)
}
