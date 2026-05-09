use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};
use tracing::info;

const GITHUB_API_LATEST: &str = "https://api.github.com/repos/Johannnefdt/VinRouge/releases/latest";
const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

/// Information about an available update.
#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub download_url: String,
    pub asset_name: String,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

/// Check whether a newer release exists on GitHub.
pub async fn check_for_update() -> Result<Option<UpdateInfo>> {
    let current = env!("CARGO_PKG_VERSION").to_string();
    let client = reqwest::Client::new();

    info!("Checking for updates from GitHub...");
    let release: GitHubRelease = client
        .get(GITHUB_API_LATEST)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .context("failed to contact GitHub API")?
        .error_for_status()
        .context("GitHub API returned an error")?
        .json()
        .await
        .context("failed to parse GitHub release JSON")?;

    let latest = normalize_version(&release.tag_name);
    info!("Current version: {current}, latest release: {latest}");

    if !is_newer(&latest, &current) {
        info!("Already up to date");
        return Ok(None);
    }

    let target = target_triple();
    let expected_name = format!("vinrouge-{}{}", target, archive_extension());

    let asset = release
        .assets
        .iter()
        .find(|a| a.name == expected_name)
        .with_context(|| {
            format!(
                "no asset named '{}' found in release {}",
                expected_name, latest
            )
        })?;

    Ok(Some(UpdateInfo {
        current_version: current,
        latest_version: latest,
        download_url: asset.browser_download_url.clone(),
        asset_name: asset.name.clone(),
    }))
}

/// Download an update asset to a temporary file.
pub async fn download_update(
    info: &UpdateInfo,
    mut on_progress: impl FnMut(u64, u64),
) -> Result<PathBuf> {
    info!("Downloading {}...", info.asset_name);

    let client = reqwest::Client::new();
    let mut resp = client
        .get(&info.download_url)
        .header("User-Agent", USER_AGENT)
        .send()
        .await
        .context("failed to start download")?
        .error_for_status()
        .context("download request failed")?;

    let total = resp.content_length().unwrap_or(0);

    let tmp_dir = tempfile::tempdir().context("failed to create temp directory")?;
    let out_path = tmp_dir.path().join(&info.asset_name);
    let mut file = std::fs::File::create(&out_path)
        .with_context(|| format!("failed to create temp file at {:?}", out_path))?;

    let mut downloaded: u64 = 0;
    while let Some(chunk) = resp.chunk().await.context("error while downloading")? {
        file.write_all(&chunk)
            .context("failed to write download chunk")?;
        downloaded += chunk.len() as u64;
        on_progress(downloaded, total);
    }
    file.flush().context("failed to flush download file")?;

    // Leak the temp dir so the file survives this function.
    let path = out_path.clone();
    let _ = tmp_dir.keep();
    Ok(path)
}

/// Extract the `vinrouge` binary from the downloaded archive and replace the
/// current executable.
pub async fn install_update(archive_path: &Path) -> Result<()> {
    let current_exe = std::env::current_exe().context("failed to locate current executable")?;
    let exe_name = current_exe
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("vinrouge");

    info!("Extracting update from {:?}...", archive_path);
    let tmp_dir = tempfile::tempdir().context("failed to create temp directory")?;
    extract_archive(archive_path, tmp_dir.path())?;

    // Find the extracted binary.
    let extracted = find_binary(tmp_dir.path(), exe_name)?;
    info!("Found extracted binary at {:?}", extracted);

    // On Unix we can chmod +x immediately.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&extracted)?.permissions();
        perms.set_mode(perms.mode() | 0o111);
        std::fs::set_permissions(&extracted, perms)?;
    }

    replace_executable(&current_exe, &extracted).await?;

    info!("Update installed successfully");
    Ok(())
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

fn archive_extension() -> &'static str {
    if cfg!(target_os = "windows") {
        ".zip"
    } else {
        ".tar.gz"
    }
}

fn extract_archive(archive: &Path, out_dir: &Path) -> Result<()> {
    if cfg!(target_os = "windows") {
        extract_zip(archive, out_dir)
    } else {
        extract_tar_gz(archive, out_dir)
    }
}

fn extract_zip(archive: &Path, out_dir: &Path) -> Result<()> {
    let file = std::fs::File::open(archive).context("failed to open zip archive")?;
    let mut archive = zip::ZipArchive::new(BufReader::new(file)).context("invalid zip archive")?;

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

fn extract_tar_gz(archive: &Path, out_dir: &Path) -> Result<()> {
    let file = std::fs::File::open(archive).context("failed to open tar.gz archive")?;
    let gz = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(gz);
    archive
        .unpack(out_dir)
        .context("failed to unpack tar.gz archive")?;
    Ok(())
}

fn find_binary(dir: &Path, name: &str) -> Result<PathBuf> {
    let expected = if cfg!(target_os = "windows") {
        format!("{}.exe", name)
    } else {
        name.to_string()
    };

    // Most CLI archives contain the binary at the root.
    let direct = dir.join(&expected);
    if direct.exists() {
        return Ok(direct);
    }

    // Fallback: shallow recursive search.
    for entry in std::fs::read_dir(dir).context("failed to read extraction directory")? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let nested = path.join(&expected);
            if nested.exists() {
                return Ok(nested);
            }
        }
    }
    bail!("could not find '{}' inside extracted archive", expected);
}

fn target_triple() -> &'static str {
    // Compile-time target triple mapping for the platforms we CI-build.
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return "x86_64-unknown-linux-gnu";
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return "aarch64-apple-darwin";
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    return "x86_64-pc-windows-msvc";
    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "windows", target_arch = "x86_64"),
    )))]
    compile_error!("unsupported target platform for self-update");
}

async fn replace_executable(current: &Path, new: &Path) -> Result<()> {
    let backup = current.with_extension("old");

    #[cfg(unix)]
    {
        std::fs::rename(current, &backup).context("failed to rename current executable")?;
        std::fs::copy(new, current).context("failed to copy new executable into place")?;
        let _ = std::fs::remove_file(&backup);
        Ok(())
    }

    #[cfg(windows)]
    {
        // Windows locks the running EXE, so we rename it out of the way,
        // copy the new one in, then spawn a detached process to clean up the
        // old file after we exit.
        std::fs::rename(current, &backup).context("failed to rename current executable")?;
        std::fs::copy(new, current).context("failed to copy new executable into place")?;

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

        let mut cmd = std::process::Command::new("cmd");
        cmd.args(["/C", tmp.path().to_string_lossy().as_ref()]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x00000008); // DETACHED_PROCESS
        }
        cmd.spawn().context("failed to spawn cleanup process")?;

        // Leak the temp file so the bat can read it.
        let _ = tmp.into_temp_path();
        Ok(())
    }
}
