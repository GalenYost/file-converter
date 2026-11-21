package org.fileconverter.android;

import android.app.Activity;
import android.content.Context;
import android.content.res.Configuration;
import java.util.Locale;

abstract class LocalizedActivity extends Activity {
    static String language(Context context) {
        return context.getSharedPreferences("settings", MODE_PRIVATE).getString("language", "");
    }

    @Override protected void attachBaseContext(Context base) {
        String language = language(base);
        if (!language.isEmpty()) {
            Configuration config = new Configuration(base.getResources().getConfiguration());
            config.setLocale(Locale.forLanguageTag(language));
            base = base.createConfigurationContext(config);
        }
        super.attachBaseContext(base);
    }
}
