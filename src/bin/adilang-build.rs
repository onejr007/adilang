// ADILang build tool — `adilang-build` (v1.12.0).
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).
//
//   adilang-build --target gh-pages [--pwa] [--title T] [--theme #hex] \
//                 [--js <runtime.js>] [--out <dir>] [input.adi]
//
// Menghasilkan situs statis (index.html + adilang_web.js + icon.svg +
// optional manifest.json + sw.js untuk PWA) siap deploy ke GitHub Pages.

use std::fs;
use std::path::Path;

use adilang::exporter::{self, ExportOptions};

const DEFAULT_RUNTIME: &str = "web/adilang_web.js";
const DEFAULT_OUT: &str = "dist";

fn usage() {
    println!("adilang-build — build tool ADILang");
    println!("  --target gh-pages   target ekspor (wajib)");
    println!("  --pwa               aktifkan manifest.json + sw.js (offline)");
    println!("  --title <T>         judul situs (fallback: nama program)");
    println!("  --theme <#hex>      warna tema PWA");
    println!("  --js <file>         runtime adilang_web.js (default: {DEFAULT_RUNTIME})");
    println!("  --out <dir>         direktori output (default: {DEFAULT_OUT})");
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut target: Option<String> = None;
    let mut opts = ExportOptions::default();
    let mut runtime_path = DEFAULT_RUNTIME.to_string();
    let mut out_dir = DEFAULT_OUT.to_string();
    let mut input: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        match a.as_str() {
            "--target" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("adilang-build: --target butuh nilai");
                    usage();
                    std::process::exit(2);
                }
                target = Some(args[i].clone());
            }
            "--pwa" => opts.pwa = true,
            "--title" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("adilang-build: --title butuh nilai");
                    std::process::exit(2);
                }
                opts.title = Some(args[i].clone());
            }
            "--theme" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("adilang-build: --theme butuh nilai");
                    std::process::exit(2);
                }
                opts.theme_color = Some(args[i].clone());
            }
            "--js" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("adilang-build: --js butuh nilai");
                    std::process::exit(2);
                }
                runtime_path = args[i].clone();
            }
            "--out" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("adilang-build: --out butuh nilai");
                    std::process::exit(2);
                }
                out_dir = args[i].clone();
            }
            "--help" | "-h" => {
                usage();
                std::process::exit(0);
            }
            other => {
                if other.starts_with("--") {
                    eprintln!("adilang-build: argumen tak dikenal '{other}'");
                    usage();
                    std::process::exit(2);
                }
                if input.is_none() {
                    input = Some(other.to_string());
                } else {
                    eprintln!("adilang-build: input ganda");
                    std::process::exit(2);
                }
            }
        }
        i += 1;
    }

    if target.as_deref() != Some("gh-pages") {
        eprintln!("adilang-build: hanya target 'gh-pages' yang didukung");
        usage();
        std::process::exit(2);
    }
    let input = match input {
        Some(p) => p,
        None => {
            eprintln!("adilang-build: butuh berkas input .adi");
            usage();
            std::process::exit(2);
        }
    };

    let src = match fs::read_to_string(&input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("adilang-build: tidak bisa membaca '{input}': {e}");
            std::process::exit(1);
        }
    };
    let runtime = match fs::read_to_string(&runtime_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "adilang-build: tidak bisa membaca runtime '--js {runtime_path}': {e}"
            );
            std::process::exit(1);
        }
    };

    let files = match exporter::export_gh_pages(&src, &runtime, &opts) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("adilang-build: ekspor gagal: {e}");
            std::process::exit(1);
        }
    };

    if let Err(e) = fs::create_dir_all(&out_dir) {
        eprintln!("adilang-build: gagal membuat {out_dir}: {e}");
        std::process::exit(1);
    }
    for (path, content) in &files {
        let full = Path::new(&out_dir).join(path);
        if let Err(e) = fs::write(&full, content) {
            eprintln!("adilang-build: gagal menulis {:?}: {e}", full);
            std::process::exit(1);
        }
        println!("  menulis {out_dir}/{path} ({} byte)", content.len());
    }
    println!(
        "adilang-build: {}{} → {out_dir}/",
        if opts.pwa { "PWA " } else { "" },
        target.as_deref().unwrap_or("gh-pages")
    );
}
