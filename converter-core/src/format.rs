use std::path::Path;
use serde::{Deserialize, Serialize};

/// High-level category of media.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MediaCategory {
    Video,
    Audio,
    Image,
}

impl MediaCategory {
    pub fn name(&self) -> &'static str {
        match self {
            MediaCategory::Video => "Video",
            MediaCategory::Audio => "Audio",
            MediaCategory::Image => "Image",
        }
    }
}

/// Supported media formats for transcoding and probing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MediaFormat {
    // Video Formats
    Mp4,
    Mkv,
    Webm,
    Avi,
    Mov,
    Flv,
    Wmv,
    Ts,

    // Audio Formats
    Mp3,
    Wav,
    Flac,
    Aac,
    Ogg,
    M4a,
    Opus,
    Wma,

    // Image / Animation Formats
    Gif,
    Png,
    Jpg,
    Webp,
    Bmp,
    Ico,
}

impl MediaFormat {
    /// Return the standard file extension (without dot).
    pub fn extension(&self) -> &'static str {
        match self {
            MediaFormat::Mp4 => "mp4",
            MediaFormat::Mkv => "mkv",
            MediaFormat::Webm => "webm",
            MediaFormat::Avi => "avi",
            MediaFormat::Mov => "mov",
            MediaFormat::Flv => "flv",
            MediaFormat::Wmv => "wmv",
            MediaFormat::Ts => "ts",
            MediaFormat::Mp3 => "mp3",
            MediaFormat::Wav => "wav",
            MediaFormat::Flac => "flac",
            MediaFormat::Aac => "aac",
            MediaFormat::Ogg => "ogg",
            MediaFormat::M4a => "m4a",
            MediaFormat::Opus => "opus",
            MediaFormat::Wma => "wma",
            MediaFormat::Gif => "gif",
            MediaFormat::Png => "png",
            MediaFormat::Jpg => "jpg",
            MediaFormat::Webp => "webp",
            MediaFormat::Bmp => "bmp",
            MediaFormat::Ico => "ico",
        }
    }

    /// User-friendly display label (e.g. "MP4 (Video)", "FLAC (Lossless Audio)").
    pub fn display_label(&self) -> &'static str {
        match self {
            MediaFormat::Mp4 => "MP4 (H.264/AAC)",
            MediaFormat::Mkv => "MKV (Matroska)",
            MediaFormat::Webm => "WebM (VP9/Opus)",
            MediaFormat::Avi => "AVI",
            MediaFormat::Mov => "MOV (QuickTime)",
            MediaFormat::Flv => "FLV (Flash Video)",
            MediaFormat::Wmv => "WMV (Windows Media)",
            MediaFormat::Ts => "TS (MPEG Transport Stream)",
            MediaFormat::Mp3 => "MP3 (MPEG Audio)",
            MediaFormat::Wav => "WAV (Uncompressed PCM)",
            MediaFormat::Flac => "FLAC (Lossless Audio)",
            MediaFormat::Aac => "AAC (Advanced Audio Coding)",
            MediaFormat::Ogg => "OGG (Vorbis Audio)",
            MediaFormat::M4a => "M4A (Apple Audio)",
            MediaFormat::Opus => "Opus (High Efficiency Audio)",
            MediaFormat::Wma => "WMA (Windows Media Audio)",
            MediaFormat::Gif => "GIF (Animated Image)",
            MediaFormat::Png => "PNG (Lossless Image)",
            MediaFormat::Jpg => "JPG (JPEG Image)",
            MediaFormat::Webp => "WebP (Modern Image)",
            MediaFormat::Bmp => "BMP (Bitmap Image)",
            MediaFormat::Ico => "ICO (Icon File)",
        }
    }

    /// Media category for this format.
    pub fn category(&self) -> MediaCategory {
        match self {
            MediaFormat::Mp4
            | MediaFormat::Mkv
            | MediaFormat::Webm
            | MediaFormat::Avi
            | MediaFormat::Mov
            | MediaFormat::Flv
            | MediaFormat::Wmv
            | MediaFormat::Ts => MediaCategory::Video,

            MediaFormat::Mp3
            | MediaFormat::Wav
            | MediaFormat::Flac
            | MediaFormat::Aac
            | MediaFormat::Ogg
            | MediaFormat::M4a
            | MediaFormat::Opus
            | MediaFormat::Wma => MediaCategory::Audio,

            MediaFormat::Gif
            | MediaFormat::Png
            | MediaFormat::Jpg
            | MediaFormat::Webp
            | MediaFormat::Bmp
            | MediaFormat::Ico => MediaCategory::Image,
        }
    }

    /// Detect format from a file extension.
    pub fn from_extension(ext: &str) -> Option<Self> {
        let clean = ext.trim().trim_start_matches('.').to_ascii_lowercase();
        match clean.as_str() {
            "mp4" | "m4v" => Some(MediaFormat::Mp4),
            "mkv" => Some(MediaFormat::Mkv),
            "webm" => Some(MediaFormat::Webm),
            "avi" => Some(MediaFormat::Avi),
            "mov" | "qt" => Some(MediaFormat::Mov),
            "flv" => Some(MediaFormat::Flv),
            "wmv" => Some(MediaFormat::Wmv),
            "ts" | "m2ts" | "mts" => Some(MediaFormat::Ts),
            "mp3" => Some(MediaFormat::Mp3),
            "wav" | "wave" => Some(MediaFormat::Wav),
            "flac" => Some(MediaFormat::Flac),
            "aac" => Some(MediaFormat::Aac),
            "ogg" | "oga" => Some(MediaFormat::Ogg),
            "m4a" => Some(MediaFormat::M4a),
            "opus" => Some(MediaFormat::Opus),
            "wma" => Some(MediaFormat::Wma),
            "gif" => Some(MediaFormat::Gif),
            "png" => Some(MediaFormat::Png),
            "jpg" | "jpeg" => Some(MediaFormat::Jpg),
            "webp" => Some(MediaFormat::Webp),
            "bmp" => Some(MediaFormat::Bmp),
            "ico" => Some(MediaFormat::Ico),
            _ => None,
        }
    }

    /// Detect format from a filesystem path.
    pub fn from_path(path: &Path) -> Option<Self> {
        path.extension()
            .and_then(|ext| ext.to_str())
            .and_then(Self::from_extension)
    }

    /// List all supported target formats.
    pub fn all() -> &'static [MediaFormat] {
        &[
            MediaFormat::Mp4,
            MediaFormat::Mkv,
            MediaFormat::Webm,
            MediaFormat::Avi,
            MediaFormat::Mov,
            MediaFormat::Flv,
            MediaFormat::Wmv,
            MediaFormat::Ts,
            MediaFormat::Mp3,
            MediaFormat::Wav,
            MediaFormat::Flac,
            MediaFormat::Aac,
            MediaFormat::Ogg,
            MediaFormat::M4a,
            MediaFormat::Opus,
            MediaFormat::Wma,
            MediaFormat::Gif,
            MediaFormat::Png,
            MediaFormat::Jpg,
            MediaFormat::Webp,
            MediaFormat::Bmp,
            MediaFormat::Ico,
        ]
    }

    /// List suggested target formats for a given source format.
    pub fn compatible_targets(&self) -> Vec<MediaFormat> {
        match self.category() {
            MediaCategory::Video => {
                // Video can be converted to other videos, extracted to audio, or animated GIF
                vec![
                    MediaFormat::Mp4,
                    MediaFormat::Mkv,
                    MediaFormat::Webm,
                    MediaFormat::Mov,
                    MediaFormat::Avi,
                    MediaFormat::Mp3,
                    MediaFormat::Wav,
                    MediaFormat::Flac,
                    MediaFormat::Aac,
                    MediaFormat::Ogg,
                    MediaFormat::M4a,
                    MediaFormat::Gif,
                ]
            }
            MediaCategory::Audio => {
                // Audio converts to other audio formats
                vec![
                    MediaFormat::Mp3,
                    MediaFormat::Wav,
                    MediaFormat::Flac,
                    MediaFormat::Aac,
                    MediaFormat::Ogg,
                    MediaFormat::M4a,
                    MediaFormat::Opus,
                    MediaFormat::Wma,
                ]
            }
            MediaCategory::Image => {
                // Image converts to other image formats or GIF
                vec![
                    MediaFormat::Png,
                    MediaFormat::Jpg,
                    MediaFormat::Webp,
                    MediaFormat::Bmp,
                    MediaFormat::Ico,
                    MediaFormat::Gif,
                    MediaFormat::Mp4,
                ]
            }
        }
    }
}

impl std::fmt::Display for MediaFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.extension().to_uppercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_extension_detection() {
        assert_eq!(MediaFormat::from_extension("mp4"), Some(MediaFormat::Mp4));
        assert_eq!(MediaFormat::from_extension(".JPEG"), Some(MediaFormat::Jpg));
        assert_eq!(MediaFormat::from_extension("flac"), Some(MediaFormat::Flac));
        assert_eq!(MediaFormat::from_extension("unknown_ext"), None);
    }

    #[test]
    fn test_path_detection() {
        assert_eq!(
            MediaFormat::from_path(Path::new("/path/to/song.opus")),
            Some(MediaFormat::Opus)
        );
        assert_eq!(
            MediaFormat::from_path(Path::new("video.m4v")),
            Some(MediaFormat::Mp4)
        );
    }

    #[test]
    fn test_categories() {
        assert_eq!(MediaFormat::Mp4.category(), MediaCategory::Video);
        assert_eq!(MediaFormat::Mp3.category(), MediaCategory::Audio);
        assert_eq!(MediaFormat::Png.category(), MediaCategory::Image);
    }
}
