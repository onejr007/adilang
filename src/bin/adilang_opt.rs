// adilang-opt — CLI token compactor ADILang (roadmap §3, v1.7.0 → v1.8.1 T-123).
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).
//
// Build:  cargo build --release  →  target/release/adilang-opt.exe
// Usage:  adilang-opt <file.adi> [--verify] [--report]
//   (tanpa flag)   cetak hasil optimize (rename + kompak) ke stdout.
//   --verify       parse ulang hasil & bandingkan AST (semantik terjaga?)
//                  → "OK: AST identik" / "FAIL: AST berubah".
//   --report       cetak statistik hemat byte/token per file (tanpa dump),
//                  plus TOTAL agregat di akhir.
// Exit: 0 = sukses, 1 = parse error / AST berubah, 2 = usage.

use std::env;
use std::fs;
use std::process;

use adilang::compactor::optimize_src;
use adilang::parser::parse;

/// Estimasi token cepat (~4 karakter/token) — konsisten dengan
/// core/adilang_llm_prompt.py::estimate_tokens.
fn est_tokens(chars: usize) -> usize {
    (chars + 3) / 4
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let verify = args.iter().any(|a| a == "--verify");
    let report = args.iter().any(|a| a == "--report");
    let paths: Vec<&String> = args
        .iter()
        .skip(1)
        .filter(|a| *a != "--verify" && *a != "--report")
        .collect();
    if paths.is_empty() {
        eprintln!("usage: adilang-opt <file.adi> [--verify] [--report]");
        process::exit(2);
    }
    let mut bad = false;
    let mut total_src = 0usize;
    let mut total_opt = 0usize;
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
        } else if report {
            let slen = src.len();
            let olen = opt.len();
            let saved = slen.saturating_sub(olen);
            let pct = if slen == 0 { 0 } else { saved * 100 / slen };
            let ts = est_tokens(slen);
            let to = est_tokens(olen);
            println!(
                "{path}: {slen} → {olen} byte | hemat {saved} byte ({pct}%) | \
                 est. token {ts} → {to} (hemat ~{})",
                ts.saturating_sub(to)
            );
            total_src += slen;
            total_opt += olen;
        } else {
            print!("{opt}");
        }
    }
    if report && total_src > 0 {
        let saved = total_src.saturating_sub(total_opt);
        let pct = saved * 100 / total_src;
        println!(
            "TOTAL: {total_src} → {total_opt} byte | hemat {saved} byte ({pct}%) | \
             est. token {} → {} (hemat ~{})",
            est_tokens(total_src),
            est_tokens(total_opt),
            est_tokens(total_src).saturating_sub(est_tokens(total_opt))
        );
    }
    process::exit(if bad { 1 } else { 0 });
}
