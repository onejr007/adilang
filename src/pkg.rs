// ADILang Package Manager — adipm (v1.12.0).
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).
//
// Package manager & manifest ADILang: `adi.toml` adalah source of truth
// ketergantungan proyek (mirip Cargo.toml/pyproject.toml). Parser TOML di
// sini minimal namun deterministik — HANYA subset yang dibutuhkan adipm:
//   [package]
//   name = "my-app"
//   version = "0.1.0"
//   description = "..."
//
//   [dependencies]
//   adi-ui = "1.0.0"
//   my-lib = "0.2.1"
//
// Semua operasi (parse/add/install/list) dapat diuji headless tanpa jaringan;
// `adi install` menyelesaikan dep ke local_modules/<name>/ (registry offline).

/// Manifest proyek ADILang hasil parse `adi.toml`.
#[derive(Debug, Clone, PartialEq)]
pub struct PackageManifest {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    /// (nama, versi) — urutan sumber dipertahankan (deterministik).
    pub deps: Vec<(String, String)>,
}

impl Default for PackageManifest {
    fn default() -> Self {
        Self {
            name: String::new(),
            version: "0.1.0".to_string(),
            description: None,
            deps: Vec::new(),
        }
    }
}

/// Parse `adi.toml` → manifest. `Err` = TOML tidak valid untuk adipm.
pub fn parse_manifest(text: &str) -> Result<PackageManifest, String> {
    let mut m = PackageManifest::default();
    let mut section = String::new();
    for (idx, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            if !line.ends_with(']') {
                return Err(format!("adi.toml:{idx}: baris section tidak ditutup ']'"));
            }
            section = line[1..line.len() - 1].trim().to_string();
            continue;
        }
        // key = "value" atau key = "value" (toleransi spasi)
        let eq = line.find('=').ok_or_else(|| {
            format!("adi.toml:{idx}: ekspektasi 'key = value', dapat '{line}'")
        })?;
        let key = line[..eq].trim().to_string();
        let value = parse_toml_value(&line[eq + 1..].trim())
            .map_err(|e| format!("adi.toml:{idx}: {e}"))?;
        match (section.as_str(), key.as_str()) {
            ("package", "name") => m.name = value,
            ("package", "version") => m.version = value,
            ("package", "description") => m.description = Some(value),
            ("dependencies", dep) => m.deps.push((dep.to_string(), value)),
            ("", _) => {
                return Err(format!(
                    "adi.toml:{idx}: '{}' di luar section (butuh [package] / [dependencies])",
                    key
                ));
            }
            _ => {
                return Err(format!(
                    "adi.toml:{idx}: kunci '{key}' tidak dikenal di section [{section}]"
                ));
            }
        }
    }
    if m.name.is_empty() {
        return Err("adi.toml: wajib punya [package] name".to_string());
    }
    if m.version.is_empty() {
        return Err("adi.toml: wajib punya [package] version".to_string());
    }
    Ok(m)
}

/// Render manifest kembali ke adi.toml (deterministik, idempotent).
pub fn render_manifest(m: &PackageManifest) -> String {
    let mut out = String::from("[package]\n");
    out.push_str(&format!("name = {:?}\n", m.name));
    out.push_str(&format!("version = {:?}\n", m.version));
    if let Some(d) = &m.description {
        out.push_str(&format!("description = {:?}\n", d));
    }
    if !m.deps.is_empty() {
        out.push_str("\n[dependencies]\n");
        for (name, version) in &m.deps {
            out.push_str(&format!("{name} = {:?}\n", version));
        }
    }
    out
}

/// Tambah (atau perbarui versi) satu dependency. Mengembalikan manifest baru.
/// Dep diurutkan ulang alfabetis setelah operasi (deterministik).
pub fn add_dependency(
    m: &mut PackageManifest,
    name: &str,
    version: &str,
) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        return Err(format!("Nama paket tidak valid '{name}'"));
    }
    let version = version.trim();
    if version.is_empty() || !version.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return Err(format!("Versi tidak valid '{version}'"));
    }
    if let Some(slot) = m.deps.iter_mut().find(|(n, _)| n == name) {
        slot.1 = version.to_string();
    } else {
        m.deps.push((name.to_string(), version.to_string()));
    }
    m.deps.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(())
}

/// Hapus satu dependency. `false` = nama tidak ada di manifest.
pub fn remove_dependency(m: &mut PackageManifest, name: &str) -> bool {
    let before = m.deps.len();
    m.deps.retain(|(n, _)| n != name);
    m.deps.len() != before
}

/// Ada atau tidak nama dependency? (case-sensitive, deterministik)
pub fn has_dependency(m: &PackageManifest, name: &str) -> bool {
    m.deps.iter().any(|(n, _)| n == name)
}

/// Parse satu nilai TOML scalar: string ber-quote `"..."` atau bare token.
fn parse_toml_value(s: &str) -> Result<String, String> {
    let s = s.trim();
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        let inner = &s[1..s.len() - 1];
        return Ok(inner.to_string());
    }
    if s.is_empty() {
        return Err("nilai kosong".to_string());
    }
    Ok(s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
        # contoh manifest
        [package]
        name = "my-app"
        version = "0.1.0"
        description = "Aplikasi demo ADILang"

        [dependencies]
        adi-ui = "1.0.0"
        my-lib = "0.2.1"
    "#;

    #[test]
    fn parse_manifest_dasar() {
        let m = parse_manifest(SAMPLE).expect("parse ok");
        assert_eq!(m.name, "my-app");
        assert_eq!(m.version, "0.1.0");
        assert_eq!(m.description.as_deref(), Some("Aplikasi demo ADILang"));
        assert_eq!(
            m.deps,
            vec![
                ("adi-ui".to_string(), "1.0.0".to_string()),
                ("my-lib".to_string(), "0.2.1".to_string()),
            ]
        );
    }

    #[test]
    fn render_idempotent() {
        let m = parse_manifest(SAMPLE).unwrap();
        let text = render_manifest(&m);
        assert_eq!(parse_manifest(&text).unwrap(), m, "render→parse harus identik");
    }

    #[test]
    fn add_dependency_update_dan_sort() {
        let mut m = parse_manifest(SAMPLE).unwrap();
        add_dependency(&mut m, "zeta-lib", "3.0.0").unwrap();
        add_dependency(&mut m, "adi-ui", "2.0.0").unwrap();
        assert!(has_dependency(&m, "zeta-lib"));
        let names: Vec<&str> = m.deps.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["adi-ui", "my-lib", "zeta-lib"], "deps harus terurut alfabetis");
        let adi_ui = m.deps.iter().find(|(n, _)| n == "adi-ui").unwrap();
        assert_eq!(adi_ui.1, "2.0.0", "add harus memperbarui versi dep yang ada");
    }

    #[test]
    fn remove_dependency_works() {
        let mut m = parse_manifest(SAMPLE).unwrap();
        assert!(remove_dependency(&mut m, "my-lib"));
        assert!(!has_dependency(&m, "my-lib"));
        assert!(!remove_dependency(&mut m, "my-lib"), "hapus dua kali = false");
    }

    #[test]
    fn manifest_tanpa_nama_ditolak() {
        assert!(parse_manifest("[package]\nversion = \"1.0.0\"\n").is_err());
        let m = parse_manifest("[package]\nname = \"x\"\n").expect("tanpa version memakai default");
        assert_eq!(m.version, "0.1.0");
    }

    #[test]
    fn nama_versi_invalid_ditolak() {
        let mut m = PackageManifest::default();
        assert!(add_dependency(&mut m, "bad name", "1.0.0").is_err());
        assert!(add_dependency(&mut m, "ok-name", "v1.0").is_err());
        assert!(add_dependency(&mut m, "ok-name", "1.0.0").is_ok());
    }
}
