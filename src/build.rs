// ADILang production build optimizer — `adi build [--release]` (v1.13.0).
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).
//
// Tahap build (deterministik, tanpa GPU/browser):
//   1. Discover semua `*.adi` di `src/` → validasi (check_src) + kompak
//      (compactor — DCE token-level: rename variabel & buang whitespace).
//   2. Gabung source → dist/app.adi (bundled source, siap `adi dev`/JIT).
//   3. Encode AST → bytecode Zero-Token-Waste → dist/app.adib.
//   4. Ekspor situs statis (index.html + runtime) memakai exporter.
//   5. (--release) wasm-opt --dce pada wasm bila tersedia (Dead Code
//      Elimination level binary), salin web/adilang_web.js.
//   6. (--ci) generate .github/workflows/deploy.yml dari template.

use std::path::{Path, PathBuf};

/// Opsi build produksi.
#[derive(Debug, Clone)]
pub struct BuildOptions {
    /// Path ke adilang_web.js (runtime). Bila None, dicari di
    /// `web/adilang_web.js` relatif ke root.
    pub runtime_js: Option<PathBuf>,
    /// Path wasm hasil `cargo build --release --target wasm32-unknown-unknown`.
    /// Di-optimasi dengan wasm-opt (bila tersedia di PATH).
    pub wasm: Option<PathBuf>,
    /// Sertakan PWA (manifest + sw) pada ekspor statis.
    pub pwa: bool,
    /// Generate `.github/workflows/deploy.yml` (CI/CD template).
    pub ci: bool,
    /// Judul situs (fallback nama program).
    pub title: Option<String>,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            runtime_js: None,
            wasm: None,
            pwa: false,
            ci: false,
            title: None,
        }
    }
}

/// Laporan build: hasil + statistik penghematan (DCE).
#[derive(Debug, Clone)]
pub struct BuildReport {
    pub dist: Vec<PathBuf>,
    pub source_bytes: usize,
    pub compact_bytes: usize,
    pub binary_bytes: usize,
    pub wasm_opt: Option<WasmOptReport>,
}

#[derive(Debug, Clone)]
pub struct WasmOptReport {
    pub input_bytes: usize,
    pub output_bytes: usize,
    pub skipped: bool,
    pub note: String,
}

/// Persen penghematan (dikunci ≥ 0): compact/source.
pub fn savings_percent(source: usize, compact: usize) -> usize {
    if source == 0 || compact >= source {
        0
    } else {
        100 - compact * 100 / source
    }
}

/// Bangun proyek di `root` → direktori `dist/`.
pub fn build_project(root: &Path, opts: &BuildOptions) -> Result<BuildReport, String> {
    let src_dir = root.join("src");
    if !src_dir.is_dir() {
        return Err(format!("Tidak ada direktori '{}'", src_dir.display()));
    }
    let files = collect_adi_files(&src_dir);
    if files.is_empty() {
        return Err(format!("Tidak ada file *.adi di '{}'", src_dir.display()));
    }

    // 1. Validasi + kompak per file (DCE token-level deterministik).
    let mut compact_parts = Vec::new();
    let mut source_bytes = 0usize;
    for f in &files {
        let raw = std::fs::read_to_string(f)
            .map_err(|e| format!("Gagal membaca '{}': {e}", f.display()))?;
        source_bytes += raw.len();
        let diags = crate::checker::check_src(&raw)?;
        if !diags.is_empty() {
            return Err(format!(
                "File '{}' tidak valid: {:?}",
                f.display(),
                diags
            ));
        }
        compact_parts.push(crate::compactor::optimize_src(&raw)?);
    }
    let merged = compact_parts.join("\n");
    let compact_bytes = merged.len();

    // Parse gabungan untuk bundling bytecode.
    let program = crate::parser::parse(&merged)?;

    let dist = root.join("dist");
    std::fs::create_dir_all(&dist).map_err(|e| format!("Gagal buat dist/: {e}"))?;

    // 2. Bundled source.
    let app_adi = dist.join("app.adi");
    std::fs::write(&app_adi, &merged).map_err(|e| e.to_string())?;

    // 3. Bytecode Zero-Token-Waste.
    let bin = crate::bytecode::encode_program(&program)?;
    let binary_bytes = bin.len();
    let app_adib = dist.join("app.adib");
    std::fs::write(&app_adib, &bin).map_err(|e| e.to_string())?;

    // 4. Situs statis + runtime.
    let runtime_path = opts
        .runtime_js
        .clone()
        .or_else(|| find_runtime_js(root));
    let runtime = match runtime_path {
        Some(p) => {
            let js = std::fs::read_to_string(&p)
                .map_err(|e| format!("Gagal baca runtime '{}': {e}", p.display()))?;
            let dest = dist.join("adilang_web.js");
            std::fs::write(&dest, &js).map_err(|e| e.to_string())?;
            js
        }
        None => {
            return Err("adilang_web.js tidak ditemukan (beri --runtime <path>)".to_string());
        }
    };
    let export_opts = crate::exporter::ExportOptions {
        pwa: opts.pwa,
        title: opts.title.clone(),
        theme_color: None,
    };
    for (name, content) in crate::exporter::export_gh_pages(&merged, &runtime, &export_opts)? {
        let p = dist.join(name);
        std::fs::write(&p, content).map_err(|e| e.to_string())?;
    }

    // 5. wasm-opt --dce (hanya bila --release & wasm disediakan).
    let mut wasm_opt = None;
    if let Some(wasm_path) = &opts.wasm {
        wasm_opt = Some(optimize_wasm(wasm_path, &dist)?);
    }

    // 6. CI template.
    if opts.ci {
        let ci_dir = root.join(".github").join("workflows");
        std::fs::create_dir_all(&ci_dir).map_err(|e| e.to_string())?;
        std::fs::write(ci_dir.join("deploy.yml"), CI_TEMPLATE)
            .map_err(|e| format!("Gagal tulis deploy.yml: {e}"))?;
    }

    let mut out = vec![app_adi, app_adib, dist.join("index.html"), dist.join("adilang_web.js")];
    if opts.pwa {
        out.push(dist.join("manifest.json"));
        out.push(dist.join("sw.js"));
        out.push(dist.join("icon.svg"));
    }
    if let Some(w) = &wasm_opt {
        if !w.skipped {
            out.push(dist.join("adilang_optimized.wasm"));
        }
    }

    Ok(BuildReport {
        dist: out,
        source_bytes,
        compact_bytes,
        binary_bytes,
        wasm_opt,
    })
}

fn optimize_wasm(wasm: &Path, dist: &Path) -> Result<WasmOptReport, String> {
    let input_bytes = std::fs::metadata(wasm)
        .map(|m| m.len() as usize)
        .unwrap_or(0);
    let out_path = dist.join("adilang_optimized.wasm");
    let tmp_path = dist.join(".adilang_opt_tmp.wasm");
    let status = std::process::Command::new("wasm-opt")
        .args(["-Oz", "--dce"])
        .arg(wasm)
        .arg("-o")
        .arg(&tmp_path)
        .status();
    match status {
        Ok(s) if s.success() => {
            let output_bytes = std::fs::metadata(&tmp_path).map(|m| m.len() as usize).unwrap_or(0);
            std::fs::rename(&tmp_path, &out_path).map_err(|e| e.to_string())?;
            Ok(WasmOptReport {
                input_bytes,
                output_bytes,
                skipped: false,
                note: format!(
                    "wasm-opt --dce: {input_bytes} → {output_bytes} byte ({}%)",
                    if input_bytes > 0 {
                        100 - output_bytes * 100 / input_bytes
                    } else {
                        0
                    }
                ),
            })
        }
        _ => {
            let _ = std::fs::remove_file(&tmp_path);
            Ok(WasmOptReport {
                input_bytes,
                output_bytes: input_bytes,
                skipped: true,
                note: "wasm-opt tidak tersedia di PATH — lewati optimasi wasm (pasang binaryen untuk --release penuh)".to_string(),
            })
        }
    }
}

fn collect_adi_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    fn walk(d: &Path, out: &mut Vec<PathBuf>) {
        if let Ok(entries) = std::fs::read_dir(d) {
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().map(|x| x == "adi").unwrap_or(false) {
                    out.push(p);
                }
            }
        }
    }
    walk(dir, &mut out);
    out.sort();
    out
}

fn find_runtime_js(root: &Path) -> Option<PathBuf> {
    let web = root.join("web").join("adilang_web.js");
    if web.exists() {
        return Some(web);
    }
    let pkg = root.join("adilang").join("web").join("adilang_web.js");
    if pkg.exists() {
        return Some(pkg);
    }
    None
}

/// Template CI/CD (GitHub Actions) — dipakai `--ci`.
pub const CI_TEMPLATE: &str = r#"name: Deploy ADILang

on:
  push:
    branches: [main]
  workflow_dispatch:

permissions:
  contents: write
  pages: write
  id-token: write

jobs:
  build-and-deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable

      - name: Setup Node
        uses: actions/setup-node@v4
        with:
          node-version: 20

      - name: Cargo test (lib)
        run: cargo test --lib
        working-directory: adilang

      - name: Web SDK selfTest
        run: node -e "const A=require('./adilang/web/adilang_web.js');const r=A.selfTest();if(!r.ok){process.exit(1)}"

      - name: ADI build
        run: adilang-build --target gh-pages --pwa

      - name: Deploy to GitHub Pages
        uses: peaceiris/actions-gh-pages@v4
        with:
          github_token: ${{ secrets.GITHUB_TOKEN }}
          publish_dir: ./dist
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_minimal_menghasilkan_dist_lengkap() {
        let dir = std::env::temp_dir().join(format!("adi_build_test_{}", std::process::id()));
        let root = dir.join("proj");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join("web")).unwrap();
        std::fs::write(
            root.join("src").join("main.adi"),
            r#"
            ui_layout "home" {
                text "Halo"
                button "Go" onClick go
            }
            routes {
                route "/" layout "home" transition "fade"
            }
            world "T" {
                entity "e" {
                    on frame {
                        rotate(0.1, (0 1 0))
                    }
                }
            }
            "#,
        )
        .unwrap();
        std::fs::write(root.join("web").join("adilang_web.js"), "/* runtime */\n").unwrap();

        let opts = BuildOptions { pwa: true, ..Default::default() };
        let rep = build_project(&root, &opts).expect("build ok");
        assert!(rep.dist.iter().any(|p| p.file_name().unwrap() == "app.adib"));
        assert!(rep.dist.iter().any(|p| p.file_name().unwrap() == "manifest.json"));
        assert!(rep.dist.iter().any(|p| p.file_name().unwrap() == "sw.js"));
        assert!(rep.compact_bytes > 0);
        assert!(rep.binary_bytes > 0);
        // DCE wire-level: bytecode .adib harus lebih kecil dari source asli.
        assert!(
            rep.binary_bytes <= rep.source_bytes,
            "bytecode ({}) harus <= source ({})",
            rep.binary_bytes,
            rep.source_bytes
        );
        // helper persen: tidak overflow & terkunci ≥ 0
        assert_eq!(savings_percent(0, 0), 0);
        assert_eq!(savings_percent(100, 120), 0);
        assert_eq!(savings_percent(100, 50), 50);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ci_generate_deploy_yml() {
        let dir = std::env::temp_dir().join(format!("adi_ci_test_{}", std::process::id()));
        let root = dir.join("proj");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join("web")).unwrap();
        std::fs::write(root.join("src").join("a.adi"), "ui_layout \"x\" { text \"a\" }\n").unwrap();
        std::fs::write(root.join("web").join("adilang_web.js"), "/* r */\n").unwrap();
        let opts = BuildOptions { ci: true, ..Default::default() };
        let _ = build_project(&root, &opts).expect("build ok");
        let yml = std::fs::read_to_string(root.join(".github").join("workflows").join("deploy.yml"))
            .expect("deploy.yml tertulis");
        assert!(yml.contains("peaceiris/actions-gh-pages@v4"));
        assert!(yml.contains("publish_dir: ./dist"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn build_menolak_src_kosong() {
        let dir = std::env::temp_dir().join(format!("adi_empty_{}", std::process::id()));
        let root = dir.join("proj");
        std::fs::create_dir_all(root.join("src")).unwrap();
        let err = build_project(&root, &BuildOptions::default()).unwrap_err();
        assert!(err.contains("tidak ada file") || err.contains("Tidak ada"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
