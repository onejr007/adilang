// adilang-check — CLI static analyzer ADILang (roadmap §3, v1.7.0).
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).
//
// Build:  cargo build --release  →  target/release/adilang-check.exe
// Usage:  adilang-check file.adi [file2.adi ...]
// Exit:   0 = semua bersih / hanya warning; 1 = ada ERROR/syntax error; 2 = usage.
//
// Offline & instan — tanpa evaluasi, tanpa WebGL. Cocok untuk pre-commit hook:
//   git hook:  adilang-check worlds/*.adi

use std::env;
use std::fs;
use std::process;

use adilang::checker::{check_src, Severity};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: adilang-check <file.adi> [file2.adi ...]");
        process::exit(2);
    }
    let mut any_error = false;
    for path in &args[1..] {
        let src = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                println!("{path}: [ERROR] tidak dapat membaca: {e}");
                any_error = true;
                continue;
            }
        };
        match check_src(&src) {
            Err(err) => {
                println!("{path}: [ERROR] {err}");
                any_error = true;
            }
            Ok(diags) => {
                if diags.is_empty() {
                    println!("{path}: OK (bersih, tanpa temuan)");
                    continue;
                }
                for d in &diags {
                    let sev = d.severity.as_str();
                    println!(
                        "{path}:{}: [{sev}] {} — {}",
                        d.line, d.message, d.hint
                    );
                    if d.severity == Severity::Error {
                        any_error = true;
                    }
                }
            }
        }
    }
    process::exit(if any_error { 1 } else { 0 });
}
