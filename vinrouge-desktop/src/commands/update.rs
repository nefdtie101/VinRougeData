use anyhow::{bail, Context};
use serde::Serialize;
use std::io::Write;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

const GITHUB_API_LATEST: &str = "https://api.github.com/repos/Johannnefdt/VinRouge/releases/latest";
const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub download_url: String,
    pub asset_name: String,
}

#[derive(Debug, serde::Deserialize)]
struct GitHubRelease {
    tag_name: String,
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, serde::Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

/// Check whether a newer desktop release exists on GitHub.
#[tauri::command]
pub async fn check_for_update() -> Result<Option<UpdateInfo>, String> {
    let current = env!("CARGO_PKG_VERSION").to_string();

    println!("Checking for desktop updates...");
    let client = reqwest::Client::new();
    let release: GitHubRelease = client
        .get(GITHUB_API_LATEST)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("failed to contact GitHub API: {e}"))?
        .error_for_status()
        .map_err(|e| format!("GitHub API error: {e}"))?
        .json()
        .await
        .map_err(|e| format!("failed to parse GitHub release: {e}"))?;

    let latest = normalize_version(&release.tag_name);
    if !is_newer(&latest, &current) {
        println!("Desktop app is up to date ({current})");
        return Ok(None);
    }

    let asset_name = desktop_asset_name();
    let asset = release
        .assets
        .iter()
        .find(|a| a.name == asset_name)
        .ok_or_else(|| format!("no asset '{}' found in release {}", asset_name, latest))?;

    Ok(Some(UpdateInfo {
        current_version: current,
        latest_version: latest,
        download_url: asset.browser_download_url.clone(),
        asset_name: asset.name.clone(),
    }))
}

/// Download the update asset and return the local file path.
#[tauri::command]
pub async fn download_update(url: String, app_handle: AppHandle) -> Result<String, String> {
    println!("Downloading update from {url}");

    let client = reqwest::Client::new();
    let mut resp = client
        .get(&url)
        .header("User-Agent", USER_AGENT)
        .send()
        .await
        .map_err(|e| format!("failed to start download: {e}"))?
        .error_for_status()
        .map_err(|e| format!("download request failed: {e}"))?;

    let total = resp.content_length().unwrap_or(0);

    let app_data = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("app data dir not available: {e}"))?;
    std::fs::create_dir_all(&app_data)
        .map_err(|e| format!("failed to create app data dir: {e}"))?;

    let file_name = url.rsplit('/').next().unwrap_or("update");
    let out_path = app_data.join(file_name);
    let mut file = std::fs::File::create(&out_path)
        .map_err(|e| format!("failed to create download file: {e}"))?;

    let mut downloaded: u64 = 0;
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| format!("download error: {e}"))?
    {
        file.write_all(&chunk)
            .map_err(|e| format!("failed to write chunk: {e}"))?;
        downloaded += chunk.len() as u64;

        let percent = if total > 0 {
            ((downloaded as f64 / total as f64) * 100.0) as u8
        } else {
            0
        };
        let _ = app_handle.emit(
            "update-progress",
            serde_json::json!({
                "percent": percent,
                "status": format!("Downloading {}...", file_name),
                "done": false,
            }),
        );
    }

    let _ = app_handle.emit(
        "update-progress",
        serde_json::json!({
            "percent": 100,
            "status": "Download complete",
            "done": true,
        }),
    );

    Ok(out_path.to_string_lossy().to_string())
}

/// Install the downloaded update.
#[tauri::command]
pub async fn install_update(download_path: String) -> Result<(), String> {
    let path = PathBuf::from(download_path);
    println!("Installing update from {:?}", path);

    #[cfg(target_os = "macos")]
    {
        install_dmg(&path).map_err(|e| format!("macOS install failed: {e}"))?;
    }

    #[cfg(target_os = "linux")]
    {
        if let Err(e) = install_linux(&path).await {
            return Err(format!("Linux install failed: {e}"));
        }
    }

    #[cfg(target_os = "windows")]
    {
        install_windows(&path).map_err(|e| format!("Windows install failed: {e}"))?;
    }

    Ok(())
}

// ── Platform-specific installers ─────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn install_dmg(dmg_path: &Path) -> anyhow::Result<()> {
    use std::process::Command;

    let mount_point = "/Volumes/VinRouge";

    // Attach DMG silently.
    let out = Command::new("hdiutil")
        .args([
            "attach",
            "-nobrowse",
            "-quiet",
            dmg_path.to_str().unwrap_or(""),
        ])
        .output()
        .context("hdiutil attach failed")?;
    if !out.status.success() {
        bail!(
            "hdiutil attach failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    // Copy app to /Applications.
    let src = format!("{}/VinRouge.app", mount_point);
    let dest = "/Applications/VinRouge.app";
    let out = Command::new("cp")
        .args(["-R", &src, dest])
        .output()
        .context("cp failed")?;
    if !out.status.success() {
        // Try to detach before erroring.
        let _ = Command::new("hdiutil")
            .args(["detach", "-quiet", mount_point])
            .output();
        bail!("cp failed: {}", String::from_utf8_lossy(&out.stderr));
    }

    // Detach DMG.
    let out = Command::new("hdiutil")
        .args(["detach", "-quiet", mount_point])
        .output()
        .context("hdiutil detach failed")?;
    if !out.status.success() {
        bail!(
            "hdiutil detach failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    Ok(())
}

#[cfg(target_os = "linux")]
async fn install_linux(path: &Path) -> anyhow::Result<()> {
    let current_exe = std::env::current_exe().context("failed to get current exe")?;

    if current_exe.to_string_lossy().ends_with(".AppImage") {
        // Replace the running AppImage.
        let backup = current_exe.with_extension("old");
        std::fs::rename(&current_exe, &backup).context("failed to rename current AppImage")?;
        std::fs::copy(path, &current_exe).context("failed to copy new AppImage")?;
        std::fs::set_permissions(&current_exe, std::fs::Permissions::from_mode(0o755))
            .context("failed to chmod new AppImage")?;
        let _ = std::fs::remove_file(&backup);
        return Ok(());
    }

    // Otherwise assume DEB package.
    let out = tokio::process::Command::new("pkexec")
        .args(["dpkg", "-i", path.to_str().unwrap_or("")])
        .output()
        .await
        .context("dpkg failed")?;
    if !out.status.success() {
        bail!("dpkg failed: {}", String::from_utf8_lossy(&out.stderr));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn install_windows(path: &Path) -> anyhow::Result<()> {
    use std::process::Command;

    let path_str = path.to_string_lossy();

    // If it's the Inno Setup installer, run it silently.
    if path_str.ends_with(".exe") {
        let mut cmd = Command::new(path_str.as_ref());
        cmd.arg("/SILENT");
        cmd.spawn().context("failed to spawn installer")?;
        return Ok(());
    }

    // If it's a ZIP, extract over the installation directory.
    if path_str.ends_with(".zip") {
        let exe = std::env::current_exe().context("failed to get current exe")?;
        let install_dir = exe.parent().context("failed to get install dir")?;

        let tmp_extract = tempfile::tempdir().context("failed to create temp dir")?;
        extract_zip(path, tmp_extract.path()).context("failed to extract zip")?;

        // The ZIP contains a staged folder (see CI). Find the actual files.
        let stage = find_single_subdir(tmp_extract.path()).unwrap_or(tmp_extract.path());

        // Rename current exe so we can overwrite it.
        let backup = exe.with_extension("old");
        std::fs::rename(&exe, &backup).context("failed to rename current exe")?;

        // Copy all extracted files into install dir.
        for entry in std::fs::read_dir(stage).context("failed to read extracted dir")? {
            let entry = entry?;
            let dest = install_dir.join(entry.file_name());
            if entry.file_type()?.is_dir() {
                copy_dir_all(entry.path(), dest).context("failed to copy directory")?;
            } else {
                std::fs::copy(entry.path(), dest).context("failed to copy file")?;
            }
        }

        // Spawn cleanup bat to delete the old exe after we exit.
        let backup_str = backup.to_string_lossy().to_string();
        let cleanup_script = format!(
            r#"@echo off
:retry
timeout /t 1 /nobreak >nul
del "{}" 2>nul
if exist "{}" goto retry
"#,
            backup_str, backup_str
        );
        let tmp = tempfile::Builder::new()
            .suffix(".bat")
            .temp_file()
            .context("failed to create cleanup bat")?;
        std::fs::write(tmp.path(), cleanup_script)?;

        let mut cmd = Command::new("cmd");
        cmd.args(["/C", tmp.path().to_string_lossy().as_ref()]);
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x00000008); // DETACHED_PROCESS
        cmd.spawn().context("failed to spawn cleanup")?;
        let _ = tmp.into_temp_path();

        return Ok(());
    }

    bail!("unsupported Windows update file: {}", path_str);
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn normalize_version(tag: &str) -> String {
    tag.trim_start_matches('v').to_string()
}

fn is_newer(latest: &str, current: &str) -> bool {
    match (
        semver::Version::parse(latest),
        semver::Version::parse(current),
    ) {
        (Ok(l), Ok(c)) => l > c,
        _ => latest != current,
    }
}

fn desktop_asset_name() -> String {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return "VinRouge-macos-aarch64.dmg".to_string();
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return "VinRouge-linux-x86_64.AppImage".to_string();
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    return "VinRouge-windows-x64.zip".to_string();
    #[cfg(not(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "x86_64"),
    )))]
    compile_error!("unsupported target platform for desktop self-update");
}

#[cfg(target_os = "windows")]
fn extract_zip(archive: &Path, out_dir: &Path) -> anyhow::Result<()> {
    use std::io::BufReader;
    let file = std::fs::File::open(archive).context("failed to open zip")?;
    let mut archive = zip::ZipArchive::new(BufReader::new(file)).context("invalid zip")?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).context("failed to read zip entry")?;
        let out_path = out_dir.join(entry.mangled_name());
        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out = std::fs::File::create(&out_path)?;
            std::io::copy(&mut entry, &mut out)?;
        }
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn find_single_subdir(dir: &Path) -> Option<&Path> {
    let mut entries = std::fs::read_dir(dir).ok()?;
    let first = entries.next()?.ok()?;
    if first.file_type().ok()?.is_dir() && entries.next().is_none() {
        Some(&first.path())
    } else {
        Some(dir)
    }
}

#[cfg(target_os = "windows")]
fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> anyhow::Result<()> {
    std::fs::create_dir_all(&dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}
