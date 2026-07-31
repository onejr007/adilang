// adilang-opt — CLI token compactor ADILang (roadmap §3, v1.7.0).
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).
//
// Build:  cargo build --release  →  target/release/adilang-opt.exe
// Usage:  adilang-opt <file.adi> [--verify]
//   (tanpa --verify)  cetak hasil optimize (rename + kompak) ke stdout.
//   --verify          parse ulang hasil & bandingkan AST (semantik terjaga?)
//                     → "OK: AST identik" / "FAIL: AST berubah".
// Exit: 0 = sukses, 1 = parse error / AST berubah, 2 = usage.

use std::env;
use std::fs;
use std::process;

use adilang::compactor::optimize_src;
use adilang::parser::parse;

fn main() {
    let args: Vec<String> = env::args().collect();
    let verify = args.iter().any(|a| a == "--verify");
    let paths: Vec<&String> = args
        .iter()
        .skip(1)
        .filter(|a| *a != "--verify")
        .collect();
    if paths.is_empty() {
        eprintln!("usage: adilang-opt <file.adi> [--verify]");
        process::exit(2);
    }
    let mut bad = false;
    for path in paths {
        let src = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{path}: tidak dapat membaca: {e}");
                bad = true;
                continue;
            }
        };
        let opt = match optimize_src(&src) {
            Ok(o) => o,
            Err(e) => {
                eprintln!("{path}: [ERROR] {e}");
                bad = true;
                continue;
            }
        };
        if verify {
            let before = parse(&src);
            let after = parse(&opt);
            match (before, after) {
                (Ok(a), Ok(b)) if a == b => {
                    println!(
                        "{path}: OK — AST identik ({}/{} byte)",
                        opt.len(),
                        src.len()
                    );
                }
                (Ok(_), Ok(_)) => {
                    println!("{path}: FAIL — AST BERUBAH (semantik tidak terjaga!)");
                    bad = true;
                }
                _ => {
                    println!("{path}: FAIL — hasil optimize tidak dapat di-parse");
                    bad = true;
                }
            }
        } else {
            print!("{opt}");
        }
    }
    process::exit(if bad { 1 } else { 0 });
}
