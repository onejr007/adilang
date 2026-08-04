// ADILang build exporter — `adilang-build --target gh-pages` (v1.15.0).
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).
//
// Menghasilkan situs statis siap-hosting (GitHub Pages) yang menyajikan
// program ADILang sebagai dokumen teks yang bisa dibaca (render minimal):
//   - index.html         — menampilkan source ADILang dalam <pre> (tanpa
//                          runtime JS — ADILang TIDAK lagi dipakai untuk
//                          membangun website; ia adalah bahasa komunikasi
//                          antar-AI/LLM).
//   - icon.svg           — ikon PWA (vektor, dihasilkan programatik).
//   - (--pwa) manifest.json + sw.js — instalable + offline.
//
// Tidak ada GPU/browser diperlukan: build murni transformasi string.

/// Opsi ekspor.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportOptions {
    /// Aktifkan PWA: manifest.json + sw.js (offline installable).
    pub pwa: bool,
    /// Judul situs (fallback: nama program).
    pub title: Option<String>,
    /// Warna tema PWA (fallback: #0f172a).
    pub theme_color: Option<String>,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            pwa: false,
            title: None,
            theme_color: None,
        }
    }
}

/// Output statis: (path relatif → konten UTF-8).
pub type ExportFiles = Vec<(String, String)>;

/// Ekspor program ADILang → situs statis (render minimal, tanpa runtime JS).
pub fn export_gh_pages(
    src: &str,
    opts: &ExportOptions,
) -> Result<ExportFiles, String> {
    if src.trim().is_empty() {
        return Err("sumber ADILang kosong".to_string());
    }
    let prog_name = crate::parser::parse(src)
        .map(|p| p.name)
        .unwrap_or_else(|_| "adilang-app".to_string());
    let title = opts
        .title
        .clone()
        .unwrap_or_else(|| prog_name.clone());
    let theme = opts
        .theme_color
        .clone()
        .unwrap_or_else(|| "#0f172a".to_string());

    let icon = generate_icon_svg(&prog_name);

    let mut files: ExportFiles = Vec::new();
    files.push(("index.html".to_string(), render_index(&title, src, opts.pwa, &theme)));
    files.push(("icon.svg".to_string(), icon));
    if opts.pwa {
        files.push(("manifest.json".to_string(), render_manifest_json(&title, &theme)));
        files.push(("sw.js".to_string(), render_sw().to_string()));
    }
    Ok(files)
}

/// Escape teks agar aman ditampilkan di dalam `<pre>` HTML.
fn escape_html(src: &str) -> String {
    src.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn render_index(title: &str, src: &str, pwa: bool, theme: &str) -> String {
    let pwa_head = if pwa {
        format!(
            r#"    <link rel="manifest" href="manifest.json">
    <meta name="theme-color" content="{theme}">
    <link rel="icon" type="image/svg+xml" href="icon.svg">
"#
        )
    } else {
        String::new()
    };
    let sw_boot = if pwa {
        r#"    <script>
      if ('serviceWorker' in navigator) {
        navigator.serviceWorker.register('./sw.js').catch(function () {});
      }
    </script>
"#
        .to_string()
    } else {
        String::new()
    };
    let body = escape_html(src);
    format!(
        r#"<!doctype html>
<html lang="id">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{title}</title>
{pwa_head}  <style>
    :root {{ color-scheme: dark; }}
    body {{ margin: 0; font-family: system-ui, sans-serif; background: {theme}; color: #e2e8f0; }}
    header {{ padding: 1.25rem 1.5rem; border-bottom: 1px solid rgba(226,232,240,.15); }}
    header h1 {{ margin: 0; font-size: 1.1rem; font-weight: 600; }}
    main {{ max-width: 52rem; margin: 0 auto; padding: 1.5rem; }}
    pre {{ background: rgba(255,255,255,.06); border: 1px solid rgba(226,232,240,.15);
           border-radius: .5rem; padding: 1.25rem; overflow: auto;
           font-size: .85rem; line-height: 1.5; }}
    footer {{ text-align: center; padding: 1rem; color: rgba(226,232,240,.5); font-size: .75rem; }}
  </style>
</head>
<body>
  <header>
    <h1>{title}</h1>
  </header>
  <main>
    <pre>{body}</pre>
  </main>
  <footer>Dokumen ADILang — bahasa komunikasi antar-AI.</footer>
{sw_boot}</body>
</html>
"#
    )
}

fn render_manifest_json(title: &str, theme: &str) -> String {
    format!(
        r#"{{
  "name": "{title}",
  "short_name": "{title}",
  "start_url": "./",
  "scope": "./",
  "display": "standalone",
  "orientation": "any",
  "theme_color": "{theme}",
  "background_color": "{theme}",
  "icons": [
    {{ "src": "icon.svg", "sizes": "any", "type": "image/svg+xml", "purpose": "any" }}
  ]
}}
"#
    )
}

fn render_sw() -> &'static str {
    r#"const CACHE = 'adilang-v1';
const CORE = ['./', 'index.html', 'manifest.json', 'icon.svg'];

self.addEventListener('install', function (event) {
  event.waitUntil(
    caches.open(CACHE).then(function (cache) {
      return cache.addAll(CORE);
    }).then(function () {
      return self.skipWaiting();
    })
  );
});

self.addEventListener('activate', function (event) {
  event.waitUntil(
    caches.keys().then(function (keys) {
      return Promise.all(
        keys.filter(function (k) { return k !== CACHE; }).map(function (k) { return caches.delete(k); })
      );
    }).then(function () {
      return self.clients.claim();
    })
  );
});

self.addEventListener('fetch', function (event) {
  var req = event.request;
  if (req.method !== 'GET') return;
  event.respondWith(
    caches.match(req).then(function (hit) {
      if (hit) return hit;
      return fetch(req).then(function (res) {
        if (res.ok) {
          var copy = res.clone();
          caches.open(CACHE).then(function (cache) { cache.put(req, copy); });
        }
        return res;
      });
    })
  );
});
"#
}

/// Ikon vektor programatik — inisial nama program di atas kotak bertema ADI.
fn generate_icon_svg(name: &str) -> String {
    let initial = name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "A".to_string());
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512">
  <rect width="512" height="512" rx="96" fill="#0f172a"/>
  <rect x="96" y="96" width="320" height="320" rx="64" fill="none" stroke="#38bdf8" stroke-width="24"/>
  <text x="256" y="330" font-family="system-ui, sans-serif" font-size="220" font-weight="bold"
        fill="#e2e8f0" text-anchor="middle">{initial}</text>
</svg>
"##
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
        world "Demo" {
            entity "box" { on frame { rotate(0.1, (0 1 0)) } }
        }
        ui_layout "home" {
            text "Halo"
            button "Go" onClick go
        }
        @i18n {
            locale "en" { hello "Hello" }
        }
        routes { route "/" layout "home" transition "fade" }
    "#;

    #[test]
    fn ekspor_html_valid() {
        let files = export_gh_pages(SAMPLE, &ExportOptions::default()).unwrap();
        let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["index.html", "icon.svg"]);
        let html = &files[0].1;
        assert!(html.contains("world \"Demo\""));
        assert!(html.contains("<pre>"));
        assert!(html.contains("<title>Demo</title>"));
        assert!(!html.contains("adilang_web.js"), "tanpa runtime JS (v6.18.0)");
        assert!(!html.contains("manifest.json"), "non-pwa tidak boleh punya manifest");
    }

    #[test]
    fn ekspor_pwa_menghasilkan_manifest_dan_sw() {
        let mut opts = ExportOptions::default();
        opts.pwa = true;
        let files = export_gh_pages(SAMPLE, &opts).unwrap();
        let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["index.html", "icon.svg", "manifest.json", "sw.js"]);
        let manifest = files.iter().find(|(n, _)| n == "manifest.json").unwrap().1.clone();
        assert!(manifest.contains("\"start_url\": \"./\","));
        assert!(manifest.contains("\"display\": \"standalone\""));
        assert!(manifest.contains("icon.svg"));
        let sw = files.iter().find(|(n, _)| n == "sw.js").unwrap().1.clone();
        assert!(sw.contains("adilang-v1"));
        assert!(sw.contains("fetch"));
        assert!(!sw.contains("adilang_web.js"), "sw tanpa runtime JS");
        let html = &files[0].1;
        assert!(html.contains("manifest.json"));
        assert!(html.contains("serviceWorker"));
        assert!(html.contains("theme-color"));
    }

    #[test]
    fn source_aman_di_dalam_pre() {
        let evil = "world \"x\" { entity \"e\" { on frame { let s = \"<b>\" } } }";
        let files = export_gh_pages(evil, &ExportOptions::default()).unwrap();
        let html = &files[0].1;
        assert!(!html.contains("<b>"), "tag harus di-escape di dalam <pre>");
        assert!(html.contains("&lt;b&gt;"));
    }

    #[test]
    fn sumber_kosong_ditolak() {
        assert!(export_gh_pages("   \n", &ExportOptions::default()).is_err());
    }

    #[test]
    fn ikon_memakai_inisial_nama_program() {
        let files = export_gh_pages(SAMPLE, &ExportOptions::default()).unwrap();
        let icon = files.iter().find(|(n, _)| n == "icon.svg").unwrap().1.clone();
        assert!(icon.contains(">D</text>"), "inisial program 'Demo' → 'D'");
    }
}
