package org.fileconverter.android;

import android.content.Context;
import android.media.MediaExtractor;
import android.media.MediaFormat;
import android.net.Uri;
import androidx.test.platform.app.InstrumentationRegistry;
import com.yausername.youtubedl_android.YoutubeDL;
import com.yausername.youtubedl_android.YoutubeDLRequest;
import org.junit.Test;
import org.junit.Assume;

import java.io.*;
import java.nio.file.*;
import java.util.Collections;

import static org.junit.Assert.*;

public class MediaToolsTest {
    @Test public void downloadsTikTokMp4WhenTestUrlProvided() throws Exception {
        String url = InstrumentationRegistry.getArguments().getString("tiktokUrl");
        Assume.assumeTrue(url != null);
        assertTrue(MediaTools.isTikTokUrl(url));
        Context app = InstrumentationRegistry.getInstrumentation().getTargetContext();
        File output = File.createTempFile("download-", ".mp4", app.getCacheDir());
        MediaExtractor extractor = new MediaExtractor();
        try {
            new MediaTools(app).download(url, output);
            extractor.setDataSource(output.getPath());
            for (int i = 0; i < extractor.getTrackCount(); i++) {
                if (extractor.getTrackFormat(i).getString(MediaFormat.KEY_MIME).startsWith("video/")) return;
            }
            fail("No video track in downloaded MP4");
        } finally {
            extractor.release();
            output.delete();
        }
    }

    @Test public void convertsMp4AudioWithPackagedMp3Encoder() throws Exception {
        Context app = InstrumentationRegistry.getInstrumentation().getTargetContext();
        File input = File.createTempFile("fixture-", ".mp4", app.getCacheDir());
        File output = File.createTempFile("result-", ".mp3", app.getCacheDir());
        MediaExtractor extractor = new MediaExtractor();
        try (InputStream stream = InstrumentationRegistry.getInstrumentation().getContext()
                .getAssets().open("tone.mp4")) {
            Files.copy(stream, input.toPath(), StandardCopyOption.REPLACE_EXISTING);
            new MediaTools(app).convert(Uri.fromFile(input), MediaTools.Format.MP3, output);
            extractor.setDataSource(output.getPath());
            assertEquals(1, extractor.getTrackCount());
            assertEquals("audio/mpeg", extractor.getTrackFormat(0).getString(MediaFormat.KEY_MIME));
            extractor.selectTrack(0);
            assertTrue(extractor.getSampleTime() >= 0);
        } finally {
            extractor.release();
            input.delete();
            output.delete();
        }
    }

    @Test public void startsPackagedDownloaderOffline() throws Exception {
        Context app = InstrumentationRegistry.getInstrumentation().getTargetContext();
        YoutubeDL yt = MediaTools.downloader(app);
        YoutubeDLRequest request = new YoutubeDLRequest(Collections.emptyList());
        request.addOption("--version");
        assertEquals("2026.10.26", yt.execute(request, null, null).getOut().trim());
    }
}
