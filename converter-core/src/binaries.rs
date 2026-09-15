use std::path::{Path, PathBuf};
use directories::ProjectDirs;
use tokio::process::Command;
use tracing::{info, warn};

/// Locate the ffmpeg binary (static, no execution check).
pub fn find_ffmpeg() -> PathBuf {
    find_executable("ffmpeg")
}

/// Locate the ffprobe binary (static, no execution check).
pub fn find_ffprobe() -> PathBuf {
    find_executable("ffprobe")
}

/// Locate the yt-dlp binary (static, no execution check).
pub fn find_ytdlp() -> PathBuf {
    find_executable("yt-dlp")
}

/// Resolve the ffmpeg binary with execution verification.
/// Tries the static path first; if that fails, tries the bundled copy
/// next to the running executable.
pub async fn find_ffmpeg_resolved() -> PathBuf {
    find_executable_resolved("ffmpeg").await
}

/// Resolve the ffprobe binary with execution verification.
pub async fn find_ffprobe_resolved() -> PathBuf {
    find_executable_resolved("ffprobe").await
}

/// Resolve the yt-dlp binary with execution verification.
pub async fn find_ytdlp_resolved() -> PathBuf {
    find_executable_resolved("yt-dlp").await
}

/// Static path resolution. Returns an absolute path when possible.
/// Searches: env var → next to exe → cwd/./bin → app data → PATH entries.
/// Falls back to bare name if nothing found.
fn find_executable(name: &str) -> PathBuf {
    let binary_name = binary_filename(name);

    // 1. Environment variable override (e.g. FFMPEG_PATH, FFPROBE_PATH)
    let env_var_name = format!("{}_PATH", name.to_uppercase());
    if let Ok(p) = std::env::var(&env_var_name) {
        let path = PathBuf::from(p);
        if path.is_file() {
            return path;
        }
    }

    // 2. Next to the running application binary
    if let Ok(current_exe) = std::env::current_exe()
        && let Some(parent) = current_exe.parent()
    {
        let candidate = parent.join(&binary_name);
        if candidate.is_file() {
            return candidate;
        }
    }

    // 3. Relative ./ or ./bin folder
    if let Ok(cwd) = std::env::current_dir() {
        let cwd_candidate = cwd.join(&binary_name);
        if cwd_candidate.is_file() {
            return cwd_candidate;
        }
        let bin_candidate = cwd.join("bin").join(&binary_name);
        if bin_candidate.is_file() {
            return bin_candidate;
        }
    }

    // 4. In application data directory
    if let Some(proj_dirs) = ProjectDirs::from("com", "GalenYost", "file-converter") {
        let app_data_bin = proj_dirs.data_local_dir().join("bin").join(&binary_name);
        if app_data_bin.is_file() {
            return app_data_bin;
        }
    }

    // 5. Walk PATH to find absolute path (not a bare name)
    if let Some(abs) = find_on_path(&binary_name) {
        return abs;
    }

    // 6. Bare name fallback (let Command::new resolve at spawn time)
    PathBuf::from(name)
}

/// Resolved (verified) path resolution.
/// First tries to execute the static candidate, then falls back to the
/// bundled copy next to the running application executable.
async fn find_executable_resolved(name: &str) -> PathBuf {
    let static_path = find_executable(name);

    // Try the static candidate
    if verify_binary(&static_path).await {
        return static_path;
    }

    warn!(
        "{} at {} is not executable, trying bundled copy",
        name,
        static_path.display()
    );

    // Try bundled next-to-exe
    let binary_name = binary_filename(name);
    if let Ok(current_exe) = std::env::current_exe()
        && let Some(parent) = current_exe.parent()
    {
        let candidate = parent.join(&binary_name);
        if candidate.is_file() && verify_binary(&candidate).await {
            info!("Using bundled {} at {}", name, candidate.display());
            return candidate;
        }
    }

    // Return static path as last resort
    static_path
}

/// Search PATH entries for the binary and return its absolute path.
fn find_on_path(binary_name: &str) -> Option<PathBuf> {
    let path_env = std::env::var("PATH").ok()?;
    for dir in std::env::split_paths(&path_env) {
        let candidate = dir.join(binary_name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn binary_filename(name: &str) -> String {
    if cfg!(windows) {
        format!("{}.exe", name)
    } else {
        name.to_string()
    }
}

/// Create a `tokio::process::Command` that will not flash a terminal window
/// on Windows. On non-Windows platforms this is identical to `Command::new`.
pub fn create_quiet_cmd(path: impl AsRef<Path>) -> Command {
    #[cfg_attr(not(windows), allow(unused_mut))]
    let mut cmd = Command::new(path.as_ref());
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Test whether the specified binary is executable by running `--version`.
pub async fn verify_binary(binary: &Path) -> bool {
    create_quiet_cmd(binary)
        .arg("--version")
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_binaries_not_empty() {
        let ffmpeg = find_ffmpeg();
        let ffprobe = find_ffprobe();
        assert!(!ffmpeg.as_os_str().is_empty());
        assert!(!ffprobe.as_os_str().is_empty());
    }

    #[test]
    fn test_find_on_path() {
        let binary = if cfg!(windows) { "cmd.exe" } else { "sh" };
        let result = find_on_path(binary);
        assert!(result.is_some(), "{} should be on PATH", binary);
        let path = result.unwrap();
        assert!(path.is_absolute(), "should return absolute path: {}", path.display());
    }
}
