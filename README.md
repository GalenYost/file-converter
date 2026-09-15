# File Converter

A fast, lightweight, cross-platform desktop media converter written in Rust using [Iced](https://github.com/iced-rs/iced) and [FFmpeg](https://ffmpeg.org/).

---

## Features

- **Wide Format Support**:
  - **Video**: MP4, MKV, WebM, AVI, MOV, FLV, WMV, TS.
  - **Audio**: MP3, WAV, FLAC, AAC, OGG, M4A, Opus, WMA.
  - **Images & Animations**: GIF, WebP, PNG, JPG, BMP, ICO.
- **Batch Processing & Parallel Queue**: Convert multiple files at once with a configurable number of parallel worker threads.
- **Live Progress Tracking**: Accurate frame-by-frame progress, processing speed multiplier (e.g. `2.5x`), and dynamic ETA countdowns.
- **Multi-Language UI**: Built-in support for:
  - English
  - Ukrainian (Українська)
  - Russian (Русский)
- **Persistent Settings**: Saves output directory preferences, UI language, and concurrency limits across application launches.
- **Responsive Layout**: Compact, scalable user interface designed to fit comfortably on small windows and high-DPI displays.

---

## Architecture & Codebase

The project is organized as a Cargo workspace with separation between core logic and the graphical interface:

```text
file-converter/
├── converter-core/          # Pure media transcoding engine (no GUI dependencies)
│   ├── src/
│   │   ├── format.rs        # Media format definitions & compatibility matrix
│   │   ├── probe.rs         # Metadata probe using ffprobe / ffmpeg
│   │   ├── progress.rs      # Real-time FFmpeg -progress pipe:1 stream parser
│   │   ├── job.rs           # ConversionJob, JobStatus, and JobEvent definitions
│   │   ├── engine.rs        # Parallel throttled worker pool & event broadcaster
│   │   └── lib.rs           # Public API exports
│   └── tests/               # Integration tests (real audio transcoding)
│
├── converter-app/           # Graphical User Interface (Iced)
│   ├── src/
│   │   ├── i18n.rs          # Localization dictionary (EN, UK, RU)
│   │   ├── config.rs        # Persistent user settings in OS config directory
│   │   └── main.rs          # Application state, event subscription, and views
│
└── .github/workflows/       # Multi-platform CI/CD release workflow
    └── release.yml          # Windows (x86_64), macOS (ARM64), and Linux (x86_64) builds
```

---

## Prerequisites

- **Rust**: Latest stable Rust toolchain (edition 2024 / 2021).
- **FFmpeg**: `ffmpeg` and `ffprobe` binaries installed and available in your system `PATH`.

### Linux Build Dependencies
On Ubuntu/Debian systems, install the standard graphics and sound development headers:
```bash
sudo apt-get update && sudo apt-get install -y \
  libasound2-dev \
  libudev-dev \
  libwayland-dev \
  libxkbcommon-dev \
  libx11-dev
```

---

## Building and Running

### Run the Application
```bash
cargo run -p converter-app
```

### Build Optimized Release Binary
```bash
cargo build --release -p converter-app
```
The compiled binary will be located at `target/release/converter-app` (or `converter-app.exe` on Windows).

### Run Tests
```bash
cargo test --workspace
```

---

## Usage

1. **Add Files**: Click `+ Add Files` to select one or multiple audio, video, or image files using the native file picker.
2. **Choose Format**: Select your desired target format from the `To:` dropdown (or set a default in Settings).
3. **Select Output Folder**: Click on the output directory path button to customize where converted files will be saved.
4. **Convert**:
   - Click `▶ Convert All` to process the entire queue in parallel.
   - Or click `Start` on an individual item.
5. **Settings (`⚙`)**: Switch UI language, adjust maximum parallel conversions (1–8), and configure defaults.

---

## License

This project is licensed under the [MIT License](LICENSE) - see the [LICENSE](LICENSE) file for details.
