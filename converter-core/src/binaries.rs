use std::path::PathBuf;
use directories::ProjectDirs;
use tokio::process::Command;

/// Locate the ffmpeg binary, checking local exe dir, app data dir, and PATH.
pub fn find_ffmpeg() -> PathBuf {
    find_executable("ffmpeg")
}

/// Locate the ffprobe binary, checking local exe dir, app data dir, and PATH.
pub fn find_ffprobe() -> PathBuf {
    find_executable("ffprobe")
}

fn find_executable(name: &str) -> PathBuf {
    let binary_name = if cfg!(windows) {
        format!("{}.exe", name)
    } else {
        name.to_string()
    };

    // 1. Environment variable override (e.g. FFMPEG_PATH, FFPROBE_PATH)
    let env_var_name = format!("{}_PATH", name.to_uppercase());
    if let Ok(p) = std::env::var(&env_var_name) {
        let path = PathBuf::from(p);
        if path.is_file() {
            return path;
        }
    }

    // 2. Next to the running application binary
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            let candidate = parent.join(&binary_name);
            if candidate.is_file() {
                return candidate;
            }
        }
    }

    // 3. Relative ./ or ./bin folder
    let cwd_candidate = PathBuf::from(&binary_name);
    if cwd_candidate.is_file() {
        return cwd_candidate;
    }
    let bin_candidate = PathBuf::from("bin").join(&binary_name);
    if bin_candidate.is_file() {
        return bin_candidate;
    }

    // 4. In application data directory
    if let Some(proj_dirs) = ProjectDirs::from("com", "GalenYost", "file-converter") {
        let app_data_bin = proj_dirs.data_local_dir().join("bin").join(&binary_name);
        if app_data_bin.is_file() {
            return app_data_bin;
        }
    }

    // 5. Fallback to system PATH
    PathBuf::from(name)
}

/// Test whether the specified binary is executable.
pub async fn verify_binary(binary: &PathBuf) -> bool {
    Command::new(binary)
        .arg("-version")
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
}
