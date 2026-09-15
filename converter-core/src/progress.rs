use serde::{Deserialize, Serialize};

/// Detailed progress information during a media conversion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversionProgress {
    /// Percentage completed between 0.0 and 100.0.
    pub percentage: f32,
    /// Current output timestamp in seconds.
    pub current_time_seconds: f64,
    /// Total duration in seconds (if known).
    pub total_duration_seconds: Option<f64>,
    /// Processing speed multiplier (e.g. 2.4 for 2.4x real-time).
    pub speed: Option<f32>,
    /// Current frame number (for video).
    pub frame: Option<u64>,
    /// Processing frame rate (for video).
    pub fps: Option<f32>,
    /// Current output bitrate in kbps.
    pub bitrate_kbps: Option<f64>,
    /// Estimated time remaining in seconds (if total duration and speed are known).
    pub eta_seconds: Option<u64>,
}

impl Default for ConversionProgress {
    fn default() -> Self {
        Self {
            percentage: 0.0,
            current_time_seconds: 0.0,
            total_duration_seconds: None,
            speed: None,
            frame: None,
            fps: None,
            bitrate_kbps: None,
            eta_seconds: None,
        }
    }
}

/// Helper to parse incremental key-value pairs from FFmpeg `-progress pipe:1`.
#[derive(Debug, Default)]
pub struct FfmpegProgressParser {
    total_duration_seconds: Option<f64>,
    current_time_seconds: f64,
    speed: Option<f32>,
    frame: Option<u64>,
    fps: Option<f32>,
    bitrate_kbps: Option<f64>,
}

impl FfmpegProgressParser {
    pub fn new(total_duration_seconds: Option<f64>) -> Self {
        Self {
            total_duration_seconds,
            ..Default::default()
        }
    }

    /// Process a single line of key=value output from ffmpeg.
    /// Returns Some(ConversionProgress) when a `progress=continue` or `progress=end` marker is encountered.
    pub fn parse_line(&mut self, line: &str) -> Option<ConversionProgress> {
        let trimmed = line.trim();
        if let Some((key, val)) = trimmed.split_once('=') {
            let key = key.trim();
            let val = val.trim();

            match key {
                "frame" => {
                    if let Ok(f) = val.parse::<u64>() {
                        self.frame = Some(f);
                    }
                }
                "fps" => {
                    if let Ok(f) = val.parse::<f32>() {
                        self.fps = Some(f);
                    }
                }
                "out_time_us" => {
                    if let Ok(us) = val.parse::<i64>() {
                        if us >= 0 {
                            self.current_time_seconds = us as f64 / 1_000_000.0;
                        }
                    }
                }
                "out_time_ms" => {
                    if let Ok(us) = val.parse::<i64>() {
                        if us >= 0 {
                            self.current_time_seconds = us as f64 / 1_000_000.0;
                        }
                    }
                }
                "bitrate" => {
                    // e.g. " 1200.0kbits/s" or "128.0kbits/s" or "N/A"
                    if let Some(num_str) = val.strip_suffix("kbits/s") {
                        if let Ok(br) = num_str.trim().parse::<f64>() {
                            self.bitrate_kbps = Some(br);
                        }
                    }
                }
                "speed" => {
                    // e.g. "2.45x" or "0.85x" or "N/A"
                    if let Some(num_str) = val.strip_suffix('x') {
                        if let Ok(sp) = num_str.trim().parse::<f32>() {
                            self.speed = Some(sp);
                        }
                    }
                }
                "progress" => {
                    if val == "continue" || val == "end" {
                        let is_end = val == "end";
                        let percentage = if is_end {
                            100.0
                        } else if let Some(total) = self.total_duration_seconds {
                            if total > 0.0 {
                                ((self.current_time_seconds / total) * 100.0).clamp(0.0, 99.9) as f32
                            } else {
                                0.0
                            }
                        } else {
                            0.0
                        };

                        let eta_seconds = if is_end {
                            Some(0)
                        } else if let (Some(total), Some(spd)) = (self.total_duration_seconds, self.speed) {
                            if spd > 0.01 && total > self.current_time_seconds {
                                let remaining_media_time = total - self.current_time_seconds;
                                Some((remaining_media_time / spd as f64).round() as u64)
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        return Some(ConversionProgress {
                            percentage,
                            current_time_seconds: self.current_time_seconds,
                            total_duration_seconds: self.total_duration_seconds,
                            speed: self.speed,
                            frame: self.frame,
                            fps: self.fps,
                            bitrate_kbps: self.bitrate_kbps,
                            eta_seconds,
                        });
                    }
                }
                _ => {}
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_parsing() {
        let mut parser = FfmpegProgressParser::new(Some(10.0));

        let lines = [
            "frame=150",
            "fps=30.0",
            "out_time_us=5000000",
            "speed=2.0x",
            "bitrate= 1500.0kbits/s",
            "progress=continue",
        ];

        let mut last_progress = None;
        for line in lines {
            if let Some(p) = parser.parse_line(line) {
                last_progress = Some(p);
            }
        }

        let p = last_progress.expect("Should have emitted progress");
        assert_eq!(p.frame, Some(150));
        assert_eq!(p.fps, Some(30.0));
        assert_eq!(p.current_time_seconds, 5.0);
        assert!((p.percentage - 50.0).abs() < 0.1);
        assert_eq!(p.speed, Some(2.0));
        assert_eq!(p.eta_seconds, Some(3)); // (10 - 5) / 2.0 = 2.5 -> 3s
    }

    #[test]
    fn test_progress_end() {
        let mut parser = FfmpegProgressParser::new(Some(10.0));
        let p = parser
            .parse_line("progress=end")
            .expect("Should emit on end");
        assert_eq!(p.percentage, 100.0);
        assert_eq!(p.eta_seconds, Some(0));
    }
}
