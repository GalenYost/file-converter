use std::path::Path;
use serde::{Deserialize, Serialize};

/// Metadata probed from a media file.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MediaInfo {
    pub duration_seconds: Option<f64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub bitrate_kbps: Option<f64>,
    pub format_name: Option<String>,
    pub size_bytes: Option<u64>,
}

#[derive(Deserialize)]
struct FfprobeOutput {
    format: Option<FfprobeFormat>,
    streams: Option<Vec<FfprobeStream>>,
}

#[derive(Deserialize)]
struct FfprobeFormat {
    duration: Option<String>,
    bit_rate: Option<String>,
    format_name: Option<String>,
    size: Option<String>,
}

#[derive(Deserialize)]
struct FfprobeStream {
    codec_type: Option<String>,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    duration: Option<String>,
}

/// Probe a media file asynchronously using `ffprobe` (or `ffmpeg` fallback).
pub async fn probe_file(path: &Path) -> Result<MediaInfo, String> {
    if !path.exists() {
        return Err(format!("File does not exist: {}", path.display()));
    }

    // Attempt probe using ffprobe
    if let Ok(info) = probe_with_ffprobe(path).await {
        return Ok(info);
    }

    // Fallback probe with ffmpeg -i
    probe_with_ffmpeg(path).await
}

async fn probe_with_ffprobe(path: &Path) -> Result<MediaInfo, String> {
    let bin = crate::binaries::find_ffprobe();
    let output = crate::binaries::create_quiet_cmd(&bin)
        .arg("-v")
        .arg("quiet")
        .arg("-print_format")
        .arg("json")
        .arg("-show_format")
        .arg("-show_streams")
        .arg(path)
        .output()
        .await
        .map_err(|e| format!("Failed to spawn ffprobe: {}", e))?;

    if !output.status.success() {
        return Err("ffprobe returned non-zero status".into());
    }

    let parsed: FfprobeOutput = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse ffprobe json: {}", e))?;

    let mut info = MediaInfo::default();

    if let Some(fmt) = parsed.format {
        info.duration_seconds = fmt.duration.and_then(|d| d.parse::<f64>().ok());
        info.bitrate_kbps = fmt.bit_rate.and_then(|b| b.parse::<f64>().ok().map(|br| br / 1000.0));
        info.format_name = fmt.format_name;
        info.size_bytes = fmt.size.and_then(|s| s.parse::<u64>().ok());
    }

    if let Some(streams) = parsed.streams {
        for stream in streams {
            match stream.codec_type.as_deref() {
                Some("video") if info.video_codec.is_none() => {
                    info.video_codec = stream.codec_name;
                    info.width = stream.width;
                    info.height = stream.height;
                    if info.duration_seconds.is_none() {
                        info.duration_seconds = stream.duration.and_then(|d| d.parse::<f64>().ok());
                    }
                }
                Some("audio") if info.audio_codec.is_none() => {
                    info.audio_codec = stream.codec_name;
                    if info.duration_seconds.is_none() {
                        info.duration_seconds = stream.duration.and_then(|d| d.parse::<f64>().ok());
                    }
                }
                _ => {}
            }
        }
    }

    Ok(info)
}

async fn probe_with_ffmpeg(path: &Path) -> Result<MediaInfo, String> {
    let bin = crate::binaries::find_ffmpeg();
    let output = crate::binaries::create_quiet_cmd(&bin)
        .arg("-i")
        .arg(path)
        .output()
        .await
        .map_err(|e| format!("Failed to spawn ffmpeg for probe: {}", e))?;

    // ffmpeg -i outputs info to stderr
    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut info = MediaInfo::default();

    // Parse Duration: 00:01:23.45, start: ...
    if let Some(duration_idx) = stderr.find("Duration: ") {
        let snippet = &stderr[duration_idx + 10..];
        if let Some(comma_idx) = snippet.find(',') {
            let time_str = snippet[..comma_idx].trim();
            info.duration_seconds = parse_time_string(time_str);
        }
    }

    Ok(info)
}

fn parse_time_string(s: &str) -> Option<f64> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() == 3 {
        let hours: f64 = parts[0].parse().ok()?;
        let minutes: f64 = parts[1].parse().ok()?;
        let seconds: f64 = parts[2].parse().ok()?;
        Some(hours * 3600.0 + minutes * 60.0 + seconds)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_time_string() {
        assert_eq!(parse_time_string("00:00:10.50"), Some(10.5));
        assert_eq!(parse_time_string("01:02:03.00"), Some(3723.0));
        assert_eq!(parse_time_string("invalid"), None);
    }
}
