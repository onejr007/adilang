// ADILang CLI — `adi` (v1.13.0).
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).
//
// Subcommand:
//   adi init [--name N]            buat adi.toml di direktori saat ini
//   adi new <name> [--template T]  scaffold proyek baru (minimal|spatial-3d|fullstack-agent)
//   adi add <pkg>[@version]        tambah/perbarui dependency
//   adi remove <pkg>               hapus dependency
//   adi install                    resolve deps → local_modules/<name>/
//   adi list                       daftar dependency + status terpasang
//   adi test <file.adi>            headless test (parse/check/struktur/simulasi)
//   adi dev [--port N]             DevServer + HMR (WebSocket HMR_RELOAD)
//   adi build [--release] [--pwa] [--ci]   build produksi → dist/
//
// Tanpa dependency eksternal (tanpa clap/serde dari binary — std only).

use std::fs;
use std::path::Path;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

use adilang::pkg::{self, PackageManifest};
use adilang::tester;

const MANIFEST_FILE: &str = "adi.toml";
const MODULES_DIR: &str = "local_modules";

fn usage() {
    println!("adi — ADILang CLI (package manager + tester + dev + build)");
    println!("  adi init [--name N]");
    println!("  adi new <name> [--template minimal|spatial-3d|fullstack-agent]");
    println!("  adi add <pkg>[@version]");
    println!("  adi remove <pkg>");
    println!("  adi install");
    println!("  adi list");
    println!("  adi test <file.adi>");
    println!("  adi dev [--port N]");
    println!("  adi build [--release] [--pwa] [--ci] [--runtime <js>] [--wasm <path>]");
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first().map(|s| s.as_str()) else {
        usage();
        std::process::exit(2);
    };
    let rest = &args[1..];
    let code = match cmd {
        "init" => cmd_init(rest),
        "new" => cmd_new(rest),
        "add" => cmd_add(rest),
        "remove" => cmd_remove(rest),
        "install" => cmd_install(rest),
        "list" => cmd_list(rest),
        "test" => cmd_test(rest),
        #[cfg(not(target_arch = "wasm32"))]
        "dev" => cmd_dev(rest),
        #[cfg(not(target_arch = "wasm32"))]
        "build" => cmd_build(rest),
        "help" | "--help" | "-h" => {
            usage();
            0
        }
        other => {
            eprintln!("adi: perintah tak dikenal '{other}'");
            usage();
            2
        }
    };
    std::process::exit(code);
}

fn read_manifest_or_default() -> Result<PackageManifest, String> {
    if !Path::new(MANIFEST_FILE).exists() {
        return Err(format!(
            "'{MANIFEST_FILE}' tidak ditemukan — jalankan `adi init` dulu"
        ));
    }
    let text = fs::read_to_string(MANIFEST_FILE).map_err(|e| e.to_string())?;
    pkg::parse_manifest(&text)
}

fn write_manifest(m: &PackageManifest) -> Result<(), String> {
    fs::write(MANIFEST_FILE, pkg::render_manifest(m)).map_err(|e| e.to_string())
}

fn cmd_init(args: &[String]) -> i32 {
    let mut name = None;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--name" {
            if i + 1 < args.len() {
                name = Some(args[i + 1].clone());
                i += 2;
            } else {
                eprintln!("adi: --name butuh nilai");
                return 2;
            }
        } else {
            eprintln!("adi: argumen tak dikenal '{}'", args[i]);
            return 2;
        }
    }
    let name = name.unwrap_or_else(|| {
        std::env::current_dir()
            .ok()
            .and_then(|d| d.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "adilang-app".to_string())
    });
    let m = PackageManifest {
        name,
        version: "0.1.0".to_string(),
        description: Some("Aplikasi ADILang".to_string()),
        deps: Vec::new(),
    };
    match write_manifest(&m) {
        Ok(()) => {
            println!("Membuat {MANIFEST_FILE} (nama: {}, versi {})", m.name, m.version);
            0
        }
        Err(e) => {
            eprintln!("adi: gagal menulis {MANIFEST_FILE}: {e}");
            1
        }
    }
}

fn cmd_new(args: &[String]) -> i32 {
    // adi new <name> [--template T]
    let mut name: Option<String> = None;
    let mut template = String::from(adilang::scaffolder::TEMPLATE_MINIMAL);
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--template" => {
                if i + 1 < args.len() {
                    template = args[i + 1].clone();
                    i += 2;
                } else {
                    eprintln!("adi: --template butuh nilai (minimal|spatial-3d|fullstack-agent)");
                    return 2;
                }
            }
            "--name" => {
                if i + 1 < args.len() {
                    name = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    eprintln!("adi: --name butuh nilai");
                    return 2;
                }
            }
            other if other.starts_with('-') => {
                eprintln!("adi: argumen tak dikenal '{other}'");
                return 2;
            }
            other => {
                if name.is_some() {
                    eprintln!("adi: hanya satu nama proyek yang diizinkan");
                    return 2;
                }
                name = Some(other.to_string());
                i += 1;
            }
        }
    }
    let Some(project) = name else {
        eprintln!("adi: `adi new <name> [--template T]` butuh nama proyek");
        return 2;
    };

    let target = Path::new(".");
    let files = match adilang::scaffolder::scaffold(&project, &template, target) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("adi: gagal scaffold: {e}");
            return 1;
        }
    };
    println!("adi new: proyek '{project}' dibuat (template: {template})");
    for f in &files {
        println!("  + {f}");
    }
    println!("Lanjutkan: cd {project} && adi test src/main.adi && adi dev");
    0
}

fn cmd_add(args: &[String]) -> i32 {
    if args.len() != 1 {
        eprintln!("adi: `adi add <pkg>[@version]` butuh tepat satu argumen");
        return 2;
    }
    let spec = &args[0];
    let (name, version) = match spec.split_once('@') {
        Some((n, v)) => (n.to_string(), v.to_string()),
        None => (spec.clone(), "0.1.0".to_string()),
    };
    let mut m = match read_manifest_or_default() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("adi: {e}");
            return 1;
        }
    };
    if let Err(e) = pkg::add_dependency(&mut m, &name, &version) {
        eprintln!("adi: {e}");
        return 1;
    }
    if let Err(e) = write_manifest(&m) {
        eprintln!("adi: {e}");
        return 1;
    }
    println!("adipm: menambahkan {name}@{version} ke {MANIFEST_FILE}");
    0
}

fn cmd_remove(args: &[String]) -> i32 {
    if args.len() != 1 {
        eprintln!("adi: `adi remove <pkg>` butuh tepat satu argumen");
        return 2;
    }
    let mut m = match read_manifest_or_default() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("adi: {e}");
            return 1;
        }
    };
    if pkg::remove_dependency(&mut m, &args[0]) {
        let _ = write_manifest(&m);
        println!("adipm: menghapus {}", args[0]);
        0
    } else {
        eprintln!("adipm: dependency '{}' tidak ada", args[0]);
        1
    }
}

fn cmd_install(args: &[String]) -> i32 {
    if !args.is_empty() {
        eprintln!("adi: `adi install` tidak menerima argumen");
        return 2;
    }
    let m = match read_manifest_or_default() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("adi: {e}");
            return 1;
        }
    };
    let _ = fs::create_dir_all(MODULES_DIR);
    let mut installed = 0usize;
    for (name, version) in &m.deps {
        let dir = Path::new(MODULES_DIR).join(name);
        if let Err(e) = fs::create_dir_all(&dir) {
            eprintln!("adipm: gagal membuat {dir:?}: {e}");
            return 1;
        }
        let mod_toml = format!(
            "[package]\nname = \"{name}\"\nversion = \"{version}\"\nsource = \"adi.toml\"\n"
        );
        if let Err(e) = fs::write(dir.join("adi.mod.toml"), mod_toml) {
            eprintln!("adipm: gagal menulis modul {name}: {e}");
            return 1;
        }
        installed += 1;
    }
    if installed == 0 {
        println!("adipm: tidak ada dependency untuk dipasang");
    } else {
        println!("adipm: {installed} modul terpasang ke {MODULES_DIR}/");
    }
    0
}

fn cmd_list(args: &[String]) -> i32 {
    if !args.is_empty() {
        eprintln!("adi: `adi list` tidak menerima argumen");
        return 2;
    }
    let m = match read_manifest_or_default() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("adi: {e}");
            return 1;
        }
    };
    println!("{} v{}", m.name, m.version);
    if m.deps.is_empty() {
        println!("  (tanpa dependency)");
        return 0;
    }
    for (name, version) in &m.deps {
        let marker = if Path::new(MODULES_DIR).join(name).join("adi.mod.toml").exists() {
            "[terpasang]"
        } else {
            "[belum]"
        };
        println!("  {name}@{version} {marker}");
    }
    0
}

fn cmd_test(args: &[String]) -> i32 {
    if args.len() != 1 {
        eprintln!("adi: `adi test <file.adi>` butuh tepat satu argumen");
        return 2;
    }
    let path = &args[0];
    let src = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("adi: tidak bisa membaca '{path}': {e}");
            return 1;
        }
    };
    let rep = tester::test_program(&src);
    println!("TAP — {path} ({})", rep.program);
    for (i, c) in rep.checks.iter().enumerate() {
        let status = if c.ok { "ok" } else { "not ok" };
        println!("  {status} {} - {}{}", i + 1, c.name, if c.message.is_empty() { String::new() } else { format!(" ({})", c.message) });
    }
    println!(
        "  # {passed} pass, {failed} fail",
        passed = rep.passed,
        failed = rep.failed
    );
    if rep.failed == 0 {
        0
    } else {
        1
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn cmd_dev(args: &[String]) -> i32 {
    let mut port: u16 = 8080;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--port" => {
                if i + 1 < args.len() {
                    port = match args[i + 1].parse() {
                        Ok(p) => p,
                        Err(_) => {
                            eprintln!("adi: port tidak valid '{}'", args[i + 1]);
                            return 2;
                        }
                    };
                    i += 2;
                } else {
                    eprintln!("adi: --port butuh nilai");
                    return 2;
                }
            }
            other => {
                eprintln!("adi: argumen tak dikenal '{other}' (hanya --port)");
                return 2;
            }
        }
    }
    let root = Path::new(".");
    match adilang::devserver::serve(port, root) {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("adi dev: {e}");
            1
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn cmd_build(args: &[String]) -> i32 {
    let mut release = false;
    let mut pwa = false;
    let mut ci = false;
    let mut runtime: Option<String> = None;
    let mut wasm: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--release" => {
                release = true;
                i += 1;
            }
            "--pwa" => {
                pwa = true;
                i += 1;
            }
            "--ci" => {
                ci = true;
                i += 1;
            }
            "--runtime" => {
                if i + 1 < args.len() {
                    runtime = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    eprintln!("adi: --runtime butuh path ke adilang_web.js");
                    return 2;
                }
            }
            "--wasm" => {
                if i + 1 < args.len() {
                    wasm = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    eprintln!("adi: --wasm butuh path ke adilang_bg.wasm");
                    return 2;
                }
            }
            other => {
                eprintln!("adi: argumen tak dikenal '{other}'");
                return 2;
            }
        }
    }
    let opts = adilang::build::BuildOptions {
        runtime_js: runtime.map(PathBuf::from),
        wasm: wasm.map(PathBuf::from),
        pwa,
        ci,
        title: None,
    };
    match adilang::build::build_project(Path::new("."), &opts) {
        Ok(rep) => {
            println!("adi build: selesai → dist/");
            println!(
                "  DCE token-level: {} → {} byte (rename + strip komentar, kanonik)",
                rep.source_bytes, rep.compact_bytes
            );
            println!(
                "  DCE wire-level: source {} → bytecode .adib {} byte ({}%)",
                rep.source_bytes,
                rep.binary_bytes,
                adilang::build::savings_percent(rep.source_bytes, rep.binary_bytes)
            );
            if let Some(w) = &rep.wasm_opt {
                println!("  wasm-opt: {}", w.note);
            }
            if release {
                println!(
                    "  hint: --release memakai wasm-opt --dce; wasm disediakan via --wasm"
                );
            }
            0
        }
        Err(e) => {
            eprintln!("adi build: {e}");
            1
        }
    }
}
