// ADILang scaffolder — `adi new <name> --template <minimal|spatial-3d|fullstack-agent>`.
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).
//
// Membuat proyek ADILang baru: adi.toml + src/main.adi dari template
// (minimal / spatial-3d / fullstack-agent). Template divalidasi oleh
// `check_src` agar hasil scaffold selalu bersih (tanpa diagnosa).

use std::path::Path;

pub const TEMPLATE_MINIMAL: &str = "minimal";
pub const TEMPLATE_SPATIAL_3D: &str = "spatial-3d";
pub const TEMPLATE_FULLSTACK_AGENT: &str = "fullstack-agent";

pub const TEMPLATE_NAMES: [&str; 3] =
    [TEMPLATE_MINIMAL, TEMPLATE_SPATIAL_3D, TEMPLATE_FULLSTACK_AGENT];

/// Konten template `.adi` (inline agar portable & tak bergantung path relatif).
pub fn template_source(name: &str) -> Option<&'static str> {
    match name {
        TEMPLATE_MINIMAL => Some(include_str!("../templates/minimal.adi")),
        TEMPLATE_SPATIAL_3D => Some(include_str!("../templates/spatial-3d.adi")),
        TEMPLATE_FULLSTACK_AGENT => Some(include_str!("../templates/fullstack-agent.adi")),
        _ => None,
    }
}

/// Validasi template: parse + checker tanpa diagnosa.
pub fn validate_template(name: &str) -> Result<(), String> {
    let src = template_source(name).ok_or_else(|| format!("Template '{name}' tidak dikenal"))?;
    let diags = crate::checker::check_src(src)?;
    if !diags.is_empty() {
        return Err(format!(
            "Template '{name}' menghasilkan diagnosa: {:?}",
            diags
        ));
    }
    Ok(())
}

/// Scaffold proyek ADILang baru di `target_dir` (dibuat bila belum ada).
/// Mengembalikan daftar file yang dibuat (path relatif). Gagal bila file
/// sudah ada (anti-overwrite, deterministik).
pub fn scaffold(
    project_name: &str,
    template: &str,
    target_dir: &Path,
) -> Result<Vec<String>, String> {
    validate_template(template)?;

    let dir_name = target_dir.join(project_name);
    if dir_name.exists() {
        return Err(format!(
            "Direktori '{}' sudah ada — pilih nama lain atau hapus dulu",
            dir_name.display()
        ));
    }
    std::fs::create_dir_all(dir_name.join("src"))
        .map_err(|e| format!("Gagal membuat direktori proyek: {e}"))?;

    let manifest = format!(
        "[package]\nname = \"{project_name}\"\nversion = \"0.1.0\"\nsource = \"adi.toml\"\n"
    );
    let source = template_source(template).unwrap();
    std::fs::write(dir_name.join("adi.toml"), manifest)
        .map_err(|e| format!("Gagal menulis adi.toml: {e}"))?;
    std::fs::write(dir_name.join("src").join("main.adi"), source)
        .map_err(|e| format!("Gagal menulis src/main.adi: {e}"))?;

    Ok(vec![
        format!("{project_name}/adi.toml"),
        format!("{project_name}/src/main.adi"),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semua_template_valid_dan_bersih() {
        for t in TEMPLATE_NAMES {
            validate_template(t).expect(&format!("template {t} harus valid & bersih"));
        }
    }

    #[test]
    fn scaffold_membuat_file_dan_menolak_overwrite() {
        let dir = std::env::temp_dir().join(format!(
            "adi_scaffold_test_{}",
            std::process::id()
        ));
        let target = dir.join("Proyek Baru");
        let files = scaffold("my-app", TEMPLATE_MINIMAL, &target).expect("scaffold ok");
        assert_eq!(files.len(), 2);
        let proj = target.join("my-app");
        assert!(proj.join("adi.toml").exists());
        assert!(proj.join("src").join("main.adi").exists());
        // Overwrite ditolak
        let err = scaffold("my-app", TEMPLATE_MINIMAL, &target).unwrap_err();
        assert!(err.contains("sudah ada"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
