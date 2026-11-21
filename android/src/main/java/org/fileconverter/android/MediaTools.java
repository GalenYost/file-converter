package org.fileconverter.android;

import android.content.Context;
import android.net.Uri;

import com.yausername.ffmpeg.FFmpeg;
import com.yausername.youtubedl_android.YoutubeDL;
import com.yausername.youtubedl_android.YoutubeDLRequest;

import java.io.*;
import java.nio.file.Files;
import java.util.*;
import java.util.concurrent.atomic.AtomicReference;

final class MediaTools {
    enum Format {
        MP4("video/mp4", "-c:v", "libx264", "-preset", "veryfast", "-c:a", "aac"),
        MKV("video/x-matroska", "-c:v", "libx264", "-preset", "veryfast", "-c:a", "aac"),
        MOV("video/quicktime", "-c:v", "libx264", "-preset", "veryfast", "-c:a", "aac"),
        AVI("video/x-msvideo", "-c:v", "mpeg4", "-q:v", "4", "-c:a", "libmp3lame"),
        WEBM("video/webm", "-c:v", "libvpx-vp9", "-cpu-used", "5", "-c:a", "libopus"),
        FLV("video/x-flv", "-c:v", "flv", "-c:a", "libmp3lame", "-ar", "44100"),
        WMV("video/x-ms-wmv", "-c:v", "wmv2", "-c:a", "wmav2"),
        TS("video/mp2t", "-c:v", "libx264", "-preset", "veryfast", "-c:a", "aac"),
        MP3("audio/mpeg", "-c:a", "libmp3lame", "-q:a", "2"),
        M4A("audio/mp4", "-c:a", "aac"), AAC("audio/aac", "-c:a", "aac"),
        WAV("audio/wav", "-c:a", "pcm_s16le"), FLAC("audio/flac", "-c:a", "flac"),
        OPUS("audio/ogg", "-c:a", "libopus"), OGG("audio/ogg", "-c:a", "libvorbis"),
        WMA("audio/x-ms-wma", "-c:a", "wmav2"),
        PNG("image/png"), JPEG("image/jpeg"), WEBP("image/webp"),
        BMP("image/bmp"), ICO("image/x-icon", "-vf", "scale=256:256:force_original_aspect_ratio=decrease"), GIF("image/gif");

        final String mime;
        final String[] options;

        Format(String mime, String... options) {
            this.mime = mime;
            this.options = options;
        }

        String extension() {
            return this == JPEG ? "jpg" : name().toLowerCase(Locale.ROOT);
        }

        static Format[] targets(String mime) {
            List<Format> result = new ArrayList<>();
            boolean image = mime != null && mime.startsWith("image/");
            boolean audio = mime != null && mime.startsWith("audio/");
            for (Format f : values()) {
                if (image ? f.mime.startsWith("image/") : f.mime.startsWith("audio/")
                        || (!audio && (f.mime.startsWith("video/") || f == GIF))) result.add(f);
            }
            return result.toArray(new Format[0]);
        }
    }

    private final Context context;
    private final AtomicReference<Process> process = new AtomicReference<>();
    private final String downloadId = UUID.randomUUID().toString();

    MediaTools(Context context) {
        this.context = context.getApplicationContext();
    }

    void cancel() {
        Process running = process.get();
        if (running != null) running.destroyForcibly();
        YoutubeDL.getInstance().destroyProcessById(downloadId);
    }

    void convert(Uri uri, Format format, File output) throws Exception {
        File input = File.createTempFile("source-", null, context.getCacheDir());
        try {
            try (InputStream stream = context.getContentResolver().openInputStream(uri)) {
                if (stream == null) throw new IOException();
                Files.copy(stream, input.toPath(), java.nio.file.StandardCopyOption.REPLACE_EXISTING);
            }
            // The wrapper ships FFmpeg's C++ runtime in its Python package.
            YoutubeDL.getInstance().init(context);
            FFmpeg.getInstance().init(context);
            File executable = new File(context.getApplicationInfo().nativeLibraryDir, "libffmpeg.so");
            List<String> args = new ArrayList<>(Arrays.asList(executable.getPath(),
                    "-nostdin", "-hide_banner", "-loglevel", "error", "-y",
                    "-protocol_whitelist", "file,pipe", "-i", input.getPath()));
            if (format.mime.startsWith("audio/")) Collections.addAll(args, "-map", "0:a:0", "-vn");
            else if (format.mime.startsWith("video/")) Collections.addAll(args, "-map", "0:v:0", "-map", "0:a:0?");
            else if (format != Format.GIF) Collections.addAll(args, "-frames:v", "1");
            Collections.addAll(args, format.options);
            args.add(output.getPath());
            ProcessBuilder builder = new ProcessBuilder(args).redirectErrorStream(true);
            File packages = new File(context.getNoBackupFilesDir(), "youtubedl-android/packages");
            builder.environment().put("LD_LIBRARY_PATH", new File(packages, "python/usr/lib").getPath()
                    + File.pathSeparator + new File(packages, "ffmpeg/usr/lib").getPath());
            Process child = builder.start();
            process.set(child);
            try {
                if (Thread.currentThread().isInterrupted()) throw new InterruptedException();
                StringBuilder error = new StringBuilder();
                try (BufferedReader reader = new BufferedReader(new InputStreamReader(child.getInputStream()))) {
                    String line;
                    while ((line = reader.readLine()) != null) {
                        if (error.length() > 800) error.setLength(0);
                        error.append(line).append('\n');
                    }
                }
                if (child.waitFor() != 0 || output.length() == 0) throw new IOException(error.toString().trim());
            } finally {
                child.destroyForcibly();
                process.set(null);
            }
        } finally {
            input.delete();
        }
    }

    static boolean isTikTokUrl(String value) {
        Uri uri = Uri.parse(value);
        String host = uri.getHost();
        return "https".equalsIgnoreCase(uri.getScheme()) && host != null
                && (host.equalsIgnoreCase("tiktok.com") || host.toLowerCase(Locale.ROOT).endsWith(".tiktok.com"));
    }

    // The wrapper normally retains its extracted script across APK upgrades.
    private static boolean downloaderReady;
    static synchronized YoutubeDL downloader(Context context) throws Exception {
        YoutubeDL yt = YoutubeDL.getInstance();
        yt.init(context);
        if (!downloaderReady) {
            File dir = new File(context.getNoBackupFilesDir(), "youtubedl-android/yt-dlp");
            File script = File.createTempFile("extractor-", null, dir);
            try (InputStream in = context.getResources().openRawResource(R.raw.ytdlp)) {
                Files.copy(in, script.toPath(), java.nio.file.StandardCopyOption.REPLACE_EXISTING);
                Files.move(script.toPath(), new File(dir, "yt-dlp").toPath(),
                        java.nio.file.StandardCopyOption.ATOMIC_MOVE);
                downloaderReady = true;
            } finally {
                script.delete();
            }
        }
        return yt;
    }

    void download(String url, File output) throws Exception {
        YoutubeDL yt = downloader(context);
        FFmpeg.getInstance().init(context);
        YoutubeDLRequest request = new YoutubeDLRequest(url);
        request.addOption("--no-playlist");
        request.addOption("--no-part");
        request.addOption("--force-overwrites");
        request.addOption("-f", "best[ext=mp4][vcodec!=none][acodec!=none]/best[ext=mp4]");
        request.addOption("-o", output.getAbsolutePath());
        Files.delete(output.toPath()); // yt-dlp treats an existing output as an already completed download.
        if (Thread.currentThread().isInterrupted()) throw new InterruptedException();
        yt.execute(request, downloadId, null);
        if (output.length() == 0) throw new IOException();
    }
}
