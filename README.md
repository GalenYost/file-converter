# File Converter

File Converter converts video, audio, and images on **Windows, macOS, Linux, and Android**. Download the package for your device from [GitHub Releases](https://github.com/GalenYost/file-converter/releases).

| Platform | Release download |
| --- | --- |
| Windows (x86_64) | `file-converter-windows-x86_64.zip` |
| macOS (Apple silicon) | `file-converter-macos-arm64.tar.gz` |
| Linux (x86_64) | `file-converter-linux-x86_64.tar.gz` |
| Android 8.0 or newer | `file-converter-android.apk` |

On desktop, extract the entire archive and launch `converter-app` (`converter-app.exe` on Windows). The archive includes `ffmpeg`, `ffprobe`, and `yt-dlp` alongside the app. On Android, install the APK, which also includes its media tools.

---

## Desktop features

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

The desktop app uses a Rust Cargo workspace. The native Android app is a separate Gradle project.

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
├── android/                 # Native Android app and APK build
└── .github/workflows/
    ├── android.yml          # Build and verify Android APKs
    └── release.yml          # Package desktop binaries and publish releases with the Android APK
```

---

## Building the desktop app from source

Use the latest stable Rust toolchain. To run a local Cargo build, you can copy `ffmpeg` and `ffprobe` from the matching desktop release into the directory containing the compiled app, or use copies already on your `PATH`.

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

## Desktop build commands

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

## Android

The dark-themed Android app converts one file at a time on Android 8.0 (API 26) or newer. Choose a file, select a format, tap **Convert**, then **Save file**. For audio from a video, choose **MP3**. FFmpeg and its encoders are included in the APK. Keep the app open during conversion and allow space for temporary input and output files.

| Input | Android output |
| --- | --- |
| Video | MP4, MKV, MOV, AVI, WebM, FLV, WMV, TS, GIF, or any audio output below |
| Audio | MP3, M4A, AAC, WAV, FLAC, Opus, OGG, WMA |
| Image | PNG, JPEG, WebP, BMP, ICO, GIF |

**Download from TikTok** accepts a public HTTPS video link, including TikTok short links. The bundled yt-dlp downloads an MP4. Tap **Save file** to keep the MP4, or choose another format such as MP3 and tap **Convert**. Downloads depend on TikTok making the video available to the device.

Under **Settings**, choose the system language, English, Ukrainian, or Russian. **Check for updates** checks the latest stable GitHub Release. Release builds can download the signed Android APK and open the Android installer after you approve. Android may ask you to allow installations from File Converter. Debug builds open the release page because they use a separate application ID.

The Android FFmpeg and Python runtimes come from [youtubedl-android](https://github.com/yausername/youtubedl-android). The bundled extractor in `android/src/main/res/raw/ytdlp` is the official platform-independent zipapp from [yt-dlp 2026.10.26](https://github.com/yt-dlp/yt-dlp/releases/tag/2026.10.26).

### Build the APK locally

Install JDK 17 and the Android SDK with Platform 35 and Build Tools 34.0.0. Set `ANDROID_HOME` to the SDK directory or add `sdk.dir=/absolute/path/to/your/Android/Sdk` to `android/local.properties`. This file is Git-ignored. Then run:

```sh
cd android
./gradlew assembleDebug
```

The installable APK is in `android/build/outputs/apk/debug/`. Use `gradlew.bat` on Windows, or open the `android/` directory in Android Studio.

### APK builds and releases

Android pull requests, pushes to `master`, and manual Release Build runs produce a debug-signed APK in the **android-apk** workflow artifact. Master nightly releases also include it. Debug builds use a separate application ID and CI generates a fresh debug key, so replacing a previous nightly may require uninstalling it first.

A `v*` tag builds and signs **file-converter-android.apk** and attaches it to the same GitHub Release as the desktop binaries. Before tagging, configure these repository Actions secrets:

| Secret | Value |
| --- | --- |
| `ANDROID_KEYSTORE_BASE64` | Base64-encoded release keystore |
| `ANDROID_KEY_ALIAS` | Signing key alias |
| `ANDROID_STORE_PASSWORD` | Keystore password |
| `ANDROID_KEY_PASSWORD` | Signing key password |

Keep the same release key for future updates. Tag builds use the tag without its leading `v` as the Android version name and the Release Build workflow run number as the version code.

---

## License

This project is licensed under the [MIT License](LICENSE) - see the [LICENSE](LICENSE) file for details.
