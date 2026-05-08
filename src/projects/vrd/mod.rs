//! `.vrd` project archive format — a zip file containing:
//!   manifest.json   — project metadata
//!   vinrouge.db     — the project SQLite database
//!   files/          — all uploaded project files

use std::io::{Read, Write};
use std::path::Path;
use zip::write::FileOptions;
use serde::{Deserialize, Serialize};

const VRD_VERSION: &str = "1";

#[derive(Serialize, Deserialize)]
struct VrdManifest {
    vrd_version: String,
    project_id: String,
    project_name: String,
    created_at: String,
    exported_at: String,
}

// ── Peek (read manifest without extracting) ───────────────────────────────────

/// Read the manifest from a `.vrd` file without extracting it.
/// The returned `Project` has `path = vrd_path` and `vrd_path` set,
/// so `open_project` can detect it and extract on demand.
pub fn peek_vrd_project(vrd_path: &Path) -> Result<super::Project, String> {
    let file = std::fs::File::open(vrd_path)
        .map_err(|e| format!("Cannot open .vrd: {e}"))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Not a valid .vrd archive: {e}"))?;
    let manifest: VrdManifest = {
        let mut mf = archive
            .by_name("manifest.json")
            .map_err(|_| "Missing manifest.json".to_string())?;
        let mut buf = String::new();
        mf.read_to_string(&mut buf).map_err(|e| e.to_string())?;
        serde_json::from_str(&buf).map_err(|e| format!("Invalid manifest: {e}"))?
    };
    let vrd_str = vrd_path.to_string_lossy().to_string();
    Ok(super::Project {
        id: manifest.project_id,
        name: manifest.project_name,
        path: vrd_str.clone(), // open_project will detect .vrd extension and extract
        created_at: manifest.created_at,
        vrd_path: Some(vrd_str),
    })
}

// ── Export ────────────────────────────────────────────────────────────────────

pub fn export_project_vrd(
    project_dir: &Path,
    project: &super::Project,
    dest_path: &Path,
) -> Result<(), String> {
    let file = std::fs::File::create(dest_path)
        .map_err(|e| format!("Cannot create .vrd file: {e}"))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // Write manifest
    let manifest = VrdManifest {
        vrd_version: VRD_VERSION.to_string(),
        project_id: project.id.clone(),
        project_name: project.name.clone(),
        created_at: project.created_at.clone(),
        exported_at: chrono::Utc::now().to_rfc3339(),
    };
    zip.start_file("manifest.json", options).map_err(|e| e.to_string())?;
    zip.write_all(
        serde_json::to_string_pretty(&manifest)
            .map_err(|e| e.to_string())?
            .as_bytes(),
    )
    .map_err(|e| e.to_string())?;

    // Write project database
    let db_path = project_dir.join("vinrouge.db");
    if db_path.exists() {
        let bytes = std::fs::read(&db_path).map_err(|e| e.to_string())?;
        zip.start_file("vinrouge.db", options).map_err(|e| e.to_string())?;
        zip.write_all(&bytes).map_err(|e| e.to_string())?;
    }

    // Write files/ directory
    let files_dir = project_dir.join("files");
    if files_dir.exists() {
        zip.add_directory("files/", options).map_err(|e| e.to_string())?;
        add_dir_to_zip(&mut zip, &files_dir, project_dir, options)?;
    }

    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}

fn add_dir_to_zip<W: Write + std::io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    dir: &Path,
    prefix: &Path,
    options: FileOptions,
) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| e.to_string())?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let rel = path
            .strip_prefix(prefix)
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .replace('\\', "/");

        if path.is_dir() {
            zip.add_directory(&format!("{rel}/"), options)
                .map_err(|e| e.to_string())?;
            add_dir_to_zip(zip, &path, prefix, options)?;
        } else {
            let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
            zip.start_file(&rel, options).map_err(|e| e.to_string())?;
            zip.write_all(&bytes).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

// ── Import ────────────────────────────────────────────────────────────────────

pub fn import_project_vrd(vrd_path: &Path, parent_dir: &Path) -> Result<super::Project, String> {
    let file = std::fs::File::open(vrd_path)
        .map_err(|e| format!("Cannot open .vrd file: {e}"))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Not a valid .vrd archive: {e}"))?;

    // Read manifest
    let manifest: VrdManifest = {
        let mut mf = archive
            .by_name("manifest.json")
            .map_err(|_| "Missing manifest.json — not a valid .vrd file".to_string())?;
        let mut buf = String::new();
        mf.read_to_string(&mut buf).map_err(|e| e.to_string())?;
        serde_json::from_str(&buf).map_err(|e| format!("Invalid manifest: {e}"))?
    };

    let project_dir = parent_dir.join(&manifest.project_name);
    std::fs::create_dir_all(&project_dir)
        .map_err(|e| format!("Cannot create project directory: {e}"))?;

    // Extract all entries
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        if entry.name() == "manifest.json" {
            continue; // already read; no need to write to disk
        }
        let out_path = project_dir.join(entry.name());
        if entry.name().ends_with('/') {
            std::fs::create_dir_all(&out_path).map_err(|e| e.to_string())?;
        } else {
            if let Some(p) = out_path.parent() {
                std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
            }
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
            std::fs::write(&out_path, &bytes).map_err(|e| e.to_string())?;
        }
    }

    // Re-open the extracted DB and fix absolute file paths to match the new location.
    // The exported DB may have paths rooted at the original machine's project dir;
    // we replace them using the file name (which is stable) into the new files/ dir.
    let new_files_dir = project_dir.join("files");
    let conn = super::db::open_project(&project_dir).map_err(|e| e.to_string())?;

    let file_names: Vec<(String, String)> = {
        let mut stmt = conn
            .prepare("SELECT id, name FROM files")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        rows
    };

    for (id, name) in file_names {
        let new_path = new_files_dir.join(&name).to_string_lossy().to_string();
        conn.execute(
            "UPDATE files SET path = ?1 WHERE id = ?2",
            rusqlite::params![new_path, id],
        )
        .map_err(|e| e.to_string())?;
    }

    // Register in the global index
    let home = super::vinrouge_home()?;
    let global_conn = super::db::open_global(&home).map_err(|e| e.to_string())?;

    let project = super::Project {
        id: manifest.project_id.clone(),
        name: manifest.project_name.clone(),
        path: project_dir.to_string_lossy().to_string(),
        created_at: manifest.created_at.clone(),
        vrd_path: None,
    };

    global_conn
        .execute(
            "INSERT OR REPLACE INTO projects (id, name, path, created_at) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![project.id, project.name, project.path, project.created_at],
        )
        .map_err(|e| format!("Failed to register imported project: {e}"))?;

    Ok(project)
}

// ── Bulk migration ────────────────────────────────────────────────────────────

/// One-time migration: package every project registered in the global index as
/// a `.vrd` file, then move (or delete) the original project folder.
///
/// * `output_dir`       — where to write the `.vrd` files; defaults to `~/VinRouge/vrd/`
/// * `delete_originals` — if `true`, remove original folders; otherwise move them
///                        to `~/VinRouge/_migrated_backup/`
/// * `progress`         — optional callback called for each line of output
pub fn migrate_projects_to_vrd(
    output_dir: Option<&std::path::Path>,
    delete_originals: bool,
    mut progress: impl FnMut(&str),
) -> Result<(usize, usize), String> {
    let home = super::vinrouge_home()?;

    let vrd_dir = output_dir
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| home.join("vrd"));
    std::fs::create_dir_all(&vrd_dir).map_err(|e| e.to_string())?;

    let backup_dir = home.join("_migrated_backup");
    if !delete_originals {
        std::fs::create_dir_all(&backup_dir).map_err(|e| e.to_string())?;
    }

    let projects = super::list_projects(&home)?;
    if projects.is_empty() {
        progress("No projects found in the global index — nothing to migrate.");
        return Ok((0, 0));
    }

    progress(&format!("Found {} project(s) to migrate.", projects.len()));

    let mut ok = 0usize;
    let mut skipped = 0usize;

    for project in &projects {
        let project_dir = std::path::PathBuf::from(&project.path);

        if !project_dir.exists() {
            progress(&format!(
                "  [SKIP] \"{}\" — folder not found at {}",
                project.name, project.path
            ));
            skipped += 1;
            continue;
        }

        // Sanitise the project name for use as a filename
        let safe_name = project
            .name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>()
            .trim()
            .to_string();

        let vrd_path = vrd_dir.join(format!("{safe_name}.vrd"));

        progress(&format!(
            "  Exporting \"{}\" → {} ...",
            project.name,
            vrd_path.display()
        ));

        if let Err(e) = export_project_vrd(&project_dir, project, &vrd_path) {
            progress(&format!("    FAILED: {e}"));
            skipped += 1;
            continue;
        }

        progress("    Export done.");

        if delete_originals {
            match std::fs::remove_dir_all(&project_dir) {
                Ok(_) => progress("    Original folder removed."),
                Err(e) => progress(&format!("    Could not remove original: {e}")),
            }
        } else {
            let backup_target = backup_dir.join(&safe_name);
            match std::fs::rename(&project_dir, &backup_target) {
                Ok(_) => progress(&format!("    Backed up to {}", backup_target.display())),
                Err(_) => {
                    // rename fails across filesystems — copy then delete
                    match copy_dir_all(&project_dir, &backup_target)
                        .and_then(|_| std::fs::remove_dir_all(&project_dir).map_err(|e| e.to_string()))
                    {
                        Ok(_) => progress(&format!("    Backed up to {}", backup_target.display())),
                        Err(e) => progress(&format!("    Backup failed: {e}")),
                    }
                }
            }
        }

        ok += 1;
    }

    progress(&format!(
        "\nMigration complete: {ok} exported, {skipped} skipped."
    ));
    if !delete_originals && ok > 0 {
        progress(&format!(
            "Originals backed up to: {}",
            backup_dir.display()
        ));
    }
    progress(&format!(".vrd files saved to: {}", vrd_dir.display()));

    Ok((ok, skipped))
}

fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let dst_path = dst.join(entry.file_name());
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            copy_dir_all(&entry.path(), &dst_path)?;
        } else {
            std::fs::copy(entry.path(), dst_path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
