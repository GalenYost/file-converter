package org.fileconverter.android;

import android.app.AlertDialog;
import android.content.Intent;
import android.content.pm.PackageInfo;
import android.net.Uri;
import android.os.Bundle;
import android.provider.Settings;
import android.widget.*;
import androidx.core.content.FileProvider;
import org.json.*;
import java.io.*;
import java.net.*;
import java.nio.file.*;
import java.util.Arrays;
import java.util.concurrent.*;

public final class SettingsActivity extends LocalizedActivity {
    private static final String RELEASES = "https://github.com/GalenYost/file-converter/releases/";
    private final ExecutorService worker = Executors.newSingleThreadExecutor();
    private Button check;
    private TextView status;
    private File pending;

    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        setTitle(R.string.settings);
        LinearLayout layout = new LinearLayout(this);
        layout.setOrientation(LinearLayout.VERTICAL);
        int pad = Math.round(20 * getResources().getDisplayMetrics().density);
        layout.setPadding(pad, pad, pad, pad);
        layout.setFitsSystemWindows(true);
        setContentView(layout);
        String[] codes = {"", "en", "uk", "ru"};
        String[] labels = {getString(R.string.system_language), "English", "Українська", "Русский"};
        Button language = new Button(this);
        int selected = Math.max(0, Arrays.asList(codes).indexOf(language(this)));
        language.setText(getString(R.string.language_choice, labels[selected]));
        layout.addView(language);
        language.setOnClickListener(v -> new AlertDialog.Builder(this).setTitle(R.string.language)
                .setSingleChoiceItems(labels, selected, (dialog, index) -> {
                    dialog.dismiss();
                    getSharedPreferences("settings", MODE_PRIVATE).edit().putString("language", codes[index]).apply();
                    recreate();
                }).show());
        check = new Button(this);
        check.setText(R.string.check_updates);
        layout.addView(check);
        status = new TextView(this);
        layout.addView(status);
        try {
            status.setText(getString(R.string.installed_version, installed().versionName));
        } catch (Exception e) { status.setText(e.getMessage()); }
        check.setOnClickListener(v -> checkForUpdates());
        if (state != null && state.getBoolean("permissionPending")) pending = apkFile();
    }

    private PackageInfo installed() throws Exception {
        return getPackageManager().getPackageInfo(getPackageName(), 0);
    }

    private static InputStream open(String url) throws IOException {
        URLConnection connection = new URL(url).openConnection();
        connection.setConnectTimeout(15000);
        connection.setReadTimeout(60000);
        connection.setRequestProperty("User-Agent", "File-Converter-Android");
        return connection.getInputStream();
    }

    // GitHub's latest release excludes prereleases. Compare numeric components, including 1.10 > 1.9.
    static boolean newer(String latest, String installed) {
        String[] a = latest.split("[+-]", 2)[0].split("\\.");
        String[] b = installed.split("[+-]", 2)[0].split("\\.");
        for (int i = 0; i < Math.max(a.length, b.length); i++) {
            int left = i < a.length ? Integer.parseInt(a[i]) : 0;
            int right = i < b.length ? Integer.parseInt(b[i]) : 0;
            if (left != right) return left > right;
        }
        return installed.contains("-") && !latest.contains("-");
    }

    private void checkForUpdates() {
        check.setEnabled(false);
        status.setText(R.string.checking);
        worker.execute(() -> {
            try (InputStream in = open("https://api.github.com/repos/GalenYost/file-converter/releases/latest");
                 BufferedReader reader = new BufferedReader(new InputStreamReader(in, "UTF-8"))) {
                StringBuilder json = new StringBuilder();
                String line;
                while ((line = reader.readLine()) != null) json.append(line);
                JSONObject release = new JSONObject(json.toString());
                String tag = release.getString("tag_name");
                if (!newer(tag.replaceFirst("^v", ""), installed().versionName)) {
                    message(getString(R.string.up_to_date));
                    return;
                }
                JSONArray assets = release.getJSONArray("assets");
                for (int i = 0; i < assets.length(); i++) {
                    JSONObject asset = assets.getJSONObject(i);
                    if ("file-converter-android.apk".equals(asset.getString("name"))) {
                        String url = asset.getString("browser_download_url");
                        if (!url.startsWith(RELEASES + "download/")) throw new IOException(url);
                        runOnUiThread(() -> offer(tag, url));
                        return;
                    }
                }
                message(getString(R.string.no_apk));
            } catch (Exception e) { message(getString(R.string.failure, e.getLocalizedMessage())); }
        });
    }

    private void offer(String tag, String url) {
        if (isFinishing() || isDestroyed()) return;
        check.setEnabled(true);
        status.setText(getString(R.string.available, tag));
        boolean debug = getPackageName().endsWith(".debug");
        new AlertDialog.Builder(this).setMessage(debug ? R.string.debug_release : R.string.install_prompt)
                .setPositiveButton(debug ? R.string.view_release : R.string.download, (d, w) -> {
                    if (debug) startActivity(new Intent(Intent.ACTION_VIEW, Uri.parse(RELEASES + "latest")));
                    else download(url);
                }).setNegativeButton(android.R.string.cancel, null).show();
    }

    private File apkFile() { return new File(getCacheDir(), "updates/file-converter.apk"); }

    private void download(String url) {
        check.setEnabled(false);
        status.setText(R.string.downloading);
        worker.execute(() -> {
            File apk = apkFile();
            File part = null;
            try (InputStream in = open(url)) {
                Files.createDirectories(apk.getParentFile().toPath());
                part = File.createTempFile("download-", ".apk", apk.getParentFile());
                Files.copy(in, part.toPath(), StandardCopyOption.REPLACE_EXISTING);
                PackageInfo candidate = getPackageManager().getPackageArchiveInfo(part.getPath(), 0);
                if (candidate == null || !getPackageName().equals(candidate.packageName)
                        || candidate.versionCode < installed().versionCode
                        || !newer(candidate.versionName, installed().versionName)) {
                    throw new IOException(getString(R.string.invalid_update));
                }
                Files.move(part.toPath(), apk.toPath(), StandardCopyOption.REPLACE_EXISTING);
                runOnUiThread(() -> {
                    if (isDestroyed() || isFinishing()) return;
                    pending = apk;
                    try {
                        if (getPackageManager().canRequestPackageInstalls()) install();
                        else startActivityForResult(new Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES,
                                Uri.parse("package:" + getPackageName())), 1);
                    } catch (RuntimeException e) { message(getString(R.string.failure, e.getMessage())); }
                });
            } catch (Exception e) { message(getString(R.string.failure, e.getLocalizedMessage())); }
            finally { if (part != null) part.delete(); }
        });
    }

    private void install() {
        Uri uri = FileProvider.getUriForFile(this, getPackageName() + ".updates", pending);
        pending = null;
        startActivity(new Intent(Intent.ACTION_VIEW).setDataAndType(uri, "application/vnd.android.package-archive")
                .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION));
        message(getString(R.string.installer));
    }

    @Override protected void onActivityResult(int request, int result, Intent data) {
        super.onActivityResult(request, result, data);
        if (request == 1 && pending != null) {
            if (getPackageManager().canRequestPackageInstalls()) {
                try { install(); } catch (RuntimeException e) { message(getString(R.string.failure, e.getMessage())); }
            } else {
                pending = null;
                message(getString(R.string.install_denied));
            }
        }
    }

    private void message(String text) {
        runOnUiThread(() -> {
            if (isDestroyed() || isFinishing()) return;
            status.setText(text);
            check.setEnabled(true);
        });
    }

    @Override protected void onSaveInstanceState(Bundle state) {
        super.onSaveInstanceState(state);
        state.putBoolean("permissionPending", pending != null);
    }

    @Override protected void onDestroy() { worker.shutdownNow(); super.onDestroy(); }
}
