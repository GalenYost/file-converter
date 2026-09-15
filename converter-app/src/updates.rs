use std::io::Write;
use std::path::{Path, PathBuf};

use semver::Version;
use serde_json::Value;

use tracing::{info, warn};

pub const REPO_OWNER: &str = "GalenYost";
pub const REPO_NAME: &str = "file-converter";
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

const USER_AGENT: &str = "file-converter-updater";

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub latest_version: Version,
    pub asset_name: String,
    pub asset_url: String,
    pub release_url: String,
}

fn platform_suffix() -> &'static str {
    if cfg!(target_os = "windows") {
        "-windows-x86_64.zip"
    } else if cfg!(target_os = "macos") {
        "-macos-arm64.tar.gz"
    } else {
        "-linux-x86_64.tar.gz"
    }
}

fn exe_suffix() -> &'static str {
    if cfg!(target_os = "windows") {
        ".exe"
    } else {
        ""
    }
}

fn bundled_file_names() -> &'static [&'static str] {
    &[
        "ffmpeg",
        "ffprobe",
    ]
}

/// Queries the GitHub Releases API for the latest tagged release and compares
/// it against the current build version. Returns `Some` only when a newer
/// version is available.
pub fn check_for_updates() -> Result<Option<UpdateInfo>, String> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/releases/latest",
        REPO_OWNER, REPO_NAME
    );
    let mut response = ureq::get(&url)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("Update check failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "Update check failed: HTTP {} ({})",
            response.status().as_u16(),
            url
        ));
    }

    let json: Value = response
        .body_mut()
        .read_json()
        .map_err(|e| format!("Update check failed: {}", e))?;

    let current = Version::parse(CURRENT_VERSION)
        .map_err(|e| format!("Invalid current version {}: {}", CURRENT_VERSION, e))?;

    let tag = json
        .get("tag_name")
        .and_then(Value::as_str)
        .ok_or_else(|| "Update check failed: no tag_name in response".to_string())?;
    let latest = Version::parse(tag.trim_start_matches('v'))
        .map_err(|e| format!("Invalid release tag {}: {}", tag, e))?;

    if latest <= current {
        return Ok(None);
    }

    let suffix = platform_suffix();
    let assets = json
        .get("assets")
        .and_then(Value::as_array)
        .ok_or_else(|| "Update check failed: no assets in response".to_string())?;

    let asset = assets
        .iter()
        .find(|asset| {
            asset
                .get("name")
                .and_then(Value::as_str)
                .is_some_and(|name| name.ends_with(suffix))
        })
        .ok_or_else(|| format!("Update check failed: no platform asset ({}*) found", suffix))?;

    let info = UpdateInfo {
        latest_version: latest,
        asset_name: asset
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        asset_url: asset
            .get("browser_download_url")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        release_url: json
            .get("html_url")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
    };

    info!("Found newer release v{} ({})", info.latest_version, info.asset_name);
    Ok(Some(info))
}

/// Downloads and extracts the release archive into `work_dir`, then replaces
/// the bundled tools and the application executable in place next to the
/// currently running binary. On non-Windows platforms the new binary is
/// relocated and relaunched immediately (the old process must then exit). On
/// Windows a helper `.bat` is left behind that waits for this process to exit,
/// swaps the executable, and relaunches it.
pub fn apply_update(info: &UpdateInfo) -> Result<(), String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("Could not locate current executable: {}", e))?;
    let install_dir = exe
        .parent()
        .ok_or_else(|| "Could not determine install directory".to_string())?
        .to_path_buf();

    let work_dir = std::env::temp_dir().join(format!("file-converter-update-{}", std::process::id()));
    std::fs::create_dir_all(&work_dir).map_err(|e| format!("Could not create temp dir: {}", e))?;

    let result = (|| {
        let archive_path = download_archive(info, &work_dir)?;
        let extracted_dir = extract_archive(&archive_path, &work_dir)?;
        replace_files(&extracted_dir, &install_dir, &exe)
    })();

    let _ = std::fs::remove_dir_all(&work_dir);
    result
}

fn download_archive(info: &UpdateInfo, work_dir: &Path) -> Result<PathBuf, String> {
    info!("Downloading {} ...", info.asset_name);
    let archive_path = work_dir.join(&info.asset_name);
    let mut response = ureq::get(&info.asset_url)
        .header("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| format!("Download failed: {}", e))?;
    if !response.status().is_success() {
        return Err(format!("Download failed: HTTP {}", response.status().as_u16()));
    }

    let mut file = std::fs::File::create(&archive_path)
        .map_err(|_e| format!("Could not create {}", archive_path.display()))?;
    std::io::copy(&mut response.body_mut().as_reader(), &mut file)
        .map_err(|e| format!("Download failed while writing file: {}", e))?;
    file.flush().map_err(|e| format!("Download failed while flushing file: {}", e))?;
    info!("Downloaded {} ({} bytes)", info.asset_name, file.metadata().map(|m| m.len()).unwrap_or(0));
    Ok(archive_path)
}

fn extract_archive(archive_path: &Path, work_dir: &Path) -> Result<PathBuf, String> {
    let out_dir = work_dir.join("extracted");
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("Could not create extract dir: {}", e))?;

    let file = std::fs::File::open(archive_path)
        .map_err(|e| format!("Could not open archive: {}", e))?;

    if archive_path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
    {
        let mut zip = zip::ZipArchive::new(file)
            .map_err(|e| format!("Could not read zip archive: {}", e))?;
        zip.extract(&out_dir)
            .map_err(|e| format!("Could not extract zip archive: {}", e))?;
    } else {
        let decoder = flate2::read::MultiGzDecoder::new(file);
        let mut tar = tar::Archive::new(decoder);
        tar.unpack(&out_dir)
            .map_err(|e| format!("Could not extract tar.gz archive: {}", e))?;
    }

    Ok(out_dir)
}

fn replace_files(extracted_dir: &Path, install_dir: &Path, exe: &Path) -> Result<(), String> {
    let exe_name = format!("converter-app{}", exe_suffix());
    let bin_file = extracted_dir.join(&exe_name);
    if !bin_file.exists() {
        return Err(format!("Archive does not contain {}", exe_name));
    }

    for tool in bundled_file_names() {
        let name = format!("{}{}", tool, exe_suffix());
        let src = extracted_dir.join(&name);
        if src.exists() {
            let dst = install_dir.join(&name);
            std::fs::copy(&src, &dst)
                .map_err(|e| format!("Could not replace {}: {}", name, e))?;
            #[cfg(unix)]
            make_executable(&dst)?;
            info!("Replaced bundled {}", name);
        } else {
            info!("Archive has no bundled {}, skipping", name);
        }
    }

    let ytdlp_name = format!("yt-dlp{}", exe_suffix());
    let src_ytdlp = extracted_dir.join(&ytdlp_name);
    if src_ytdlp.exists() {
        let dst = install_dir.join(&ytdlp_name);
        std::fs::copy(&src_ytdlp, &dst)
            .map_err(|e| format!("Could not replace {}: {}", ytdlp_name, e))?;
        #[cfg(unix)]
        make_executable(&dst)?;
    }

    let new_exe = extracted_dir.join(&exe_name);
    relocate_executable(&new_exe, exe, install_dir).map(|relaunched| {
        if relaunched {
            info!("Update applied; the running process should now exit");
        }
    })
}

/// Replaces the running executable and hands off to the new version. Returns
/// `true` when a fresh process was (or will be) started, meaning the current
/// process must exit right away.
fn relocate_executable(
    new_exe: &Path,
    current_exe: &Path,
    install_dir: &Path,
) -> Result<bool, String> {
    #[cfg(windows)]
    {
        let exe_name = current_exe
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("converter-app")
            .to_string();
        let staged = install_dir.join(format!("{}.new.exe", exe_name));
        std::fs::copy(new_exe, &staged)
            .map_err(|e| format!("Could not stage new executable: {}", e))?;

        let old = format!("{}.old.exe", exe_name);
        let staged_name = staged
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("converter-app.new.exe")
            .to_string();
        let bat = install_dir.join("update-restart.bat");
        let script = format!(
            "@echo off\r\n\
             cd /d \"%~dp0\"\r\n\
             :wait\r\n\
             tasklist /FI \"IMAGENAME eq {exe_name}\" 2>nul \
             | findstr /I \"{exe_name}\" >nul\r\n\
             if not errorlevel 1 (\r\n\
               timeout /t 1 /nobreak >nul\r\n\
               goto wait\r\n\
             )\r\n\
             move /Y \"{exe_name}\" \"{old}\" >nul\r\n\
             move /Y \"{staged_name}\" \"{exe_name}\" >nul\r\n\
             if exist \"{old}\" del /Q \"{old}\" >nul\r\n\
             start \"\" \"{exe_name}\"\r\n\
             del /Q \"%~f0\" >nul\r\n"
        );
        std::fs::write(&bat, script)
            .map_err(|e| format!("Could not write update script: {}", e))?;

        use std::os::windows::process::CommandExt;
        let spawned = std::process::Command::new("cmd")
            .arg("/C")
            .arg(&bat)
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
            .spawn()
            .map_err(|e| format!("Could not start update script: {}", e))?;
        info!("Update script {} launched (pid {:?})", bat.display(), spawned.id());
        Ok(true)
    }

    #[cfg(not(windows))]
    {
        let _ = install_dir;
        std::fs::rename(new_exe, current_exe)
            .map_err(|e| format!("Could not replace current executable: {}", e))?;
        #[cfg(unix)]
        make_executable(current_exe)?;

        let spawned = std::process::Command::new(current_exe)
            .spawn()
            .map_err(|e| format!("Could not relaunch application: {}", e))?;
        info!("Relaunched application (pid {:?})", spawned.id());
        Ok(true)
    }
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)
        .map_err(|e| format!("Could not read permissions of {}: {}", path.display(), e))?
        .permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms)
        .map_err(|e| format!("Could not set permissions of {}: {}", path.display(), e))
}

/// Cleans up stale files from a previous in-place update (Windows only). Safe to
/// call on every startup.
pub fn cleanup_stale_update_files() {
    if !cfg!(windows) {
        return;
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let old = dir.join("converter-app.old.exe");
        if old.exists() {
            warn!("Removing stale {}", old.display());
            let _ = std::fs::remove_file(&old);
        }
        let bat = dir.join("update-restart.bat");
        if bat.exists() {
            let _ = std::fs::remove_file(&bat);
        }
    }
}