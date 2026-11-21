package org.fileconverter.android;

import android.app.AlertDialog;
import android.content.Intent;
import android.database.Cursor;
import android.net.Uri;
import android.os.Bundle;
import android.provider.OpenableColumns;
import android.text.InputType;
import android.view.View;
import android.widget.*;
import java.io.*;
import java.nio.file.Files;
import java.util.concurrent.*;
import org.fileconverter.android.MediaTools.Format;

public final class MainActivity extends LocalizedActivity {
    private static final int OPEN = 1, SAVE = 2;
    private final ExecutorService worker = Executors.newSingleThreadExecutor();
    private MediaTools media;
    private Uri source;
    private String name, mime, activeLanguage;
    private File output, downloaded;
    private Button choose, convert, download, settings;
    private Spinner formats;
    private TextView filename, status;
    private ProgressBar progress;
    private boolean busy, ready;

    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        media = new MediaTools(this);
        activeLanguage = language(this);
        LinearLayout form = new LinearLayout(this);
        form.setOrientation(LinearLayout.VERTICAL);
        int pad = Math.round(20 * getResources().getDisplayMetrics().density);
        form.setPadding(pad, pad, pad, pad);
        ScrollView scroll = new ScrollView(this);
        scroll.setFitsSystemWindows(true);
        scroll.addView(form);
        setContentView(scroll);

        choose = button(form, R.string.choose);
        download = button(form, R.string.tiktok);
        filename = new TextView(this);
        filename.setPadding(0, pad, 0, pad);
        form.addView(filename);
        formats = new Spinner(this);
        formats.setContentDescription(getString(R.string.format));
        formats.setOnItemSelectedListener(new AdapterView.OnItemSelectedListener() {
            @Override public void onNothingSelected(AdapterView<?> parent) {}
            @Override public void onItemSelected(AdapterView<?> parent, View view, int index, long id) {
                if (!busy && downloaded != null && (output == null || downloaded.equals(output))) {
                    ready = formats.getSelectedItem() == Format.MP4;
                    output = ready ? downloaded : null;
                    refresh();
                }
            }
        });
        form.addView(formats);
        convert = button(form, R.string.convert);
        progress = new ProgressBar(this);
        form.addView(progress);
        status = new TextView(this);
        status.setAccessibilityLiveRegion(View.ACCESSIBILITY_LIVE_REGION_POLITE);
        form.addView(status);
        settings = button(form, R.string.settings);

        choose.setOnClickListener(v -> startActivityForResult(
                new Intent(Intent.ACTION_OPEN_DOCUMENT).addCategory(Intent.CATEGORY_OPENABLE)
                        .setType("*/*").putExtra(Intent.EXTRA_MIME_TYPES,
                                new String[]{"audio/*", "video/*", "image/*"}), OPEN));
        download.setOnClickListener(v -> askForUrl());
        settings.setOnClickListener(v -> startActivity(new Intent(this, SettingsActivity.class)));
        convert.setOnClickListener(v -> {
            if (ready) chooseDestination();
            else startConversion();
        });
        if (state != null) {
            source = state.getParcelable("source");
            name = state.getString("name");
            mime = state.getString("mime");
            output = cached(state, "output");
            downloaded = cached(state, "downloaded");
            ready = output != null;
            if (source != null && "file".equals(source.getScheme()) && downloaded == null) source = null;
        }
        showSource();
        if (state != null && source != null) formats.setSelection(state.getInt("format"));
        refresh();
    }

    private File cached(Bundle state, String key) {
        String value = state.getString(key);
        if (value == null) return null;
        File file = new File(getCacheDir(), value);
        return file.isFile() ? file : null;
    }

    private Button button(LinearLayout form, int label) {
        Button button = new Button(this);
        button.setText(label);
        form.addView(button);
        return button;
    }

    @Override protected void onResume() {
        super.onResume();
        if (!language(this).equals(activeLanguage)) recreate();
    }

    private void showSource() {
        filename.setText(source == null ? getString(R.string.choose_hint) : name);
        formats.setAdapter(new ArrayAdapter<>(this, android.R.layout.simple_spinner_dropdown_item, Format.targets(mime)));
    }

    private void refresh() {
        choose.setEnabled(!busy);
        download.setEnabled(!busy);
        settings.setEnabled(!busy);
        formats.setVisibility(source == null ? View.GONE : View.VISIBLE);
        formats.setEnabled(!busy && (!ready || output.equals(downloaded)));
        convert.setText(ready ? R.string.save : R.string.convert);
        convert.setEnabled(source != null && !busy);
        progress.setVisibility(busy ? View.VISIBLE : View.GONE);
    }

    private void askForUrl() {
        EditText input = new EditText(this);
        input.setSingleLine(true);
        input.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_URI);
        input.setHint(R.string.url_hint);
        new AlertDialog.Builder(this).setTitle(R.string.tiktok).setView(input)
                .setNegativeButton(android.R.string.cancel, null).setPositiveButton(R.string.download, (d, w) -> {
                    String url = input.getText().toString().trim();
                    if (!MediaTools.isTikTokUrl(url)) { status.setText(R.string.invalid_url); return; }
                    begin(R.string.downloading);
                    worker.execute(() -> {
                        File file = null;
                        try {
                            file = File.createTempFile("tiktok-", ".mp4", getCacheDir());
                            media.download(url, file);
                            File completed = file;
                            runOnUiThread(() -> {
                                if (isDestroyed() || isFinishing()) { completed.delete(); return; }
                                clearSource();
                                downloaded = output = completed;
                                source = Uri.fromFile(completed);
                                name = "TikTok.mp4";
                                mime = Format.MP4.mime;
                                ready = true;
                                showSource();
                                finish(R.string.complete);
                            });
                        } catch (Exception e) {
                            if (file != null) file.delete();
                            failed(e);
                        }
                    });
                }).show();
    }

    @Override protected void onActivityResult(int request, int result, Intent data) {
        super.onActivityResult(request, result, data);
        if (result != RESULT_OK || data == null || data.getData() == null) return;
        Uri uri = data.getData();
        if (request == OPEN) {
            try {
                String displayName = "converted";
                try (Cursor cursor = getContentResolver().query(uri,
                        new String[]{OpenableColumns.DISPLAY_NAME}, null, null, null)) {
                    if (cursor != null && cursor.moveToFirst() && !cursor.isNull(0)) displayName = cursor.getString(0);
                }
                String type = getContentResolver().getType(uri);
                clearSource();
                source = uri;
                name = displayName;
                mime = type;
                status.setText("");
                showSource();
                refresh();
            } catch (RuntimeException e) { failed(e); }
        } else if (request == SAVE && ready) {
            begin(R.string.saving);
            File file = output;
            worker.execute(() -> {
                try (OutputStream stream = getContentResolver().openOutputStream(uri, "wt")) {
                    if (stream == null) throw new IOException();
                    Files.copy(file.toPath(), stream);
                } catch (Exception e) { failed(e); return; }
                runOnUiThread(() -> {
                    if (isDestroyed()) return;
                    clearOutput();
                    finish(R.string.saved);
                });
            });
        }
    }

    private void chooseDestination() {
        Format format = (Format) formats.getSelectedItem();
        int dot = name.lastIndexOf('.');
        String stem = dot > 0 ? name.substring(0, dot) : name;
        startActivityForResult(new Intent(Intent.ACTION_CREATE_DOCUMENT)
                .addCategory(Intent.CATEGORY_OPENABLE).setType(format.mime)
                .putExtra(Intent.EXTRA_TITLE, stem + "." + format.extension()), SAVE);
    }

    private void begin(int message) {
        busy = true;
        status.setText(message);
        refresh();
    }

    private void finish(int message) {
        busy = false;
        status.setText(message);
        refresh();
    }

    private void startConversion() {
        begin(R.string.converting);
        Format format = (Format) formats.getSelectedItem();
        Uri input = source;
        worker.execute(() -> {
            File temp = null;
            try {
                temp = File.createTempFile("converted-", "." + format.extension(), getCacheDir());
                media.convert(input, format, temp);
                File completed = temp;
                runOnUiThread(() -> {
                    if (isDestroyed() || isFinishing()) { completed.delete(); return; }
                    output = completed;
                    ready = true;
                    finish(R.string.complete);
                });
            } catch (Exception e) {
                if (temp != null) temp.delete();
                failed(e);
            }
        });
    }

    private void failed(Exception e) {
        runOnUiThread(() -> {
            if (isDestroyed() || isFinishing()) return;
            busy = false;
            status.setText(getString(R.string.failure, String.valueOf(e.getLocalizedMessage())));
            refresh();
        });
    }

    private void clearOutput() {
        if (output != null && !output.equals(downloaded)) output.delete();
        output = null;
        ready = false;
    }

    private void clearSource() {
        clearOutput();
        if (downloaded != null) downloaded.delete();
        downloaded = null;
    }

    @Override protected void onSaveInstanceState(Bundle state) {
        super.onSaveInstanceState(state);
        state.putParcelable("source", source);
        state.putString("name", name);
        state.putString("mime", mime);
        state.putInt("format", formats.getSelectedItemPosition());
        if (ready) state.putString("output", output.getName());
        if (downloaded != null) state.putString("downloaded", downloaded.getName());
    }

    @Override protected void onDestroy() {
        worker.shutdownNow();
        media.cancel();
        if (isFinishing()) clearSource();
        super.onDestroy();
    }
}
