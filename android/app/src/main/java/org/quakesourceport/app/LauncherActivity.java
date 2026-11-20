package org.quakesourceport.app;

import android.app.Activity;
import android.content.ActivityNotFoundException;
import android.content.Context;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.util.Log;
import android.view.Gravity;
import android.view.View;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.ProgressBar;
import android.widget.ScrollView;
import android.widget.TextView;

import java.io.File;
import java.io.IOException;

public final class LauncherActivity extends Activity {
    private static final int CHOOSE_FOLDER = 1;
    private static final String TAG = "QuakeLauncher";
    private static ImportJob activeImport;

    private GameData data;
    private TextView status;
    private TextView message;
    private Button play;
    private Button choose;
    private ProgressBar progress;

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        data = new GameData(this);

        int padding = Math.round(24 * getResources().getDisplayMetrics().density);
        LinearLayout content = new LinearLayout(this);
        content.setOrientation(LinearLayout.VERTICAL);
        content.setGravity(Gravity.CENTER_HORIZONTAL);
        content.setPadding(padding, padding, padding, padding);

        TextView title = new TextView(this);
        title.setText(R.string.app_name);
        title.setTextAppearance(android.R.style.TextAppearance_Material_Headline);
        content.addView(title);

        status = new TextView(this);
        status.setGravity(Gravity.CENTER);
        status.setPadding(0, padding, 0, padding);
        content.addView(status);

        choose = new Button(this);
        choose.setText(R.string.choose_quake_folder);
        choose.setOnClickListener(view -> chooseFolder());
        content.addView(choose);

        play = new Button(this);
        play.setText(R.string.play);
        play.setOnClickListener(view -> play());
        content.addView(play);

        progress = new ProgressBar(this);
        content.addView(progress);

        message = new TextView(this);
        message.setGravity(Gravity.CENTER);
        message.setAccessibilityLiveRegion(TextView.ACCESSIBILITY_LIVE_REGION_POLITE);
        content.addView(message);
        if (state != null) message.setText(state.getCharSequence("message"));

        ScrollView scroll = new ScrollView(this);
        scroll.addView(content);
        setContentView(scroll);
        if (activeImport != null) activeImport.launcher = this;
    }

    @Override
    protected void onResume() {
        super.onResume();
        showStatus();
    }

    @Override
    protected void onSaveInstanceState(Bundle state) {
        super.onSaveInstanceState(state);
        state.putCharSequence("message", message.getText());
    }

    @Override
    protected void onDestroy() {
        if (activeImport != null && activeImport.launcher == this) activeImport.launcher = null;
        super.onDestroy();
    }

    private void chooseFolder() {
        Intent picker = new Intent(Intent.ACTION_OPEN_DOCUMENT_TREE);
        picker.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
        try {
            startActivityForResult(picker, CHOOSE_FOLDER);
            choose.setEnabled(false);
        } catch (ActivityNotFoundException e) {
            message.setText(R.string.picker_unavailable);
        }
    }

    @Override
    protected void onActivityResult(int request, int result, Intent intent) {
        super.onActivityResult(request, result, intent);
        if (request != CHOOSE_FOLDER || result != RESULT_OK || intent == null
                || intent.getData() == null || activeImport != null) return;

        message.setText("");
        activeImport = new ImportJob(this);
        showStatus();
        activeImport.start(getApplicationContext(), intent.getData());
    }

    private void showStatus() {
        boolean importing = activeImport != null;
        choose.setEnabled(!importing);
        progress.setVisibility(importing ? View.VISIBLE : View.GONE);
        File imported = importing ? null : data.importedBase();
        File existing = importing || imported != null ? null : data.legacyBase();
        play.setEnabled(imported != null || existing != null);
        if (importing) {
            status.setText(R.string.importing_paks);
        } else if (imported != null) {
            status.setText(R.string.imported_paks_ready);
        } else if (existing != null) {
            status.setText(R.string.using_app_files);
        } else {
            status.setText(R.string.choose_pak_folder_hint);
        }
    }

    private void play() {
        File base = data.importedBase();
        if (base == null) base = data.legacyBase();
        if (base == null) {
            showStatus();
            return;
        }
        play.setEnabled(false);
        startActivity(new Intent(this, GameActivity.class).putExtra(GameActivity.BASEDIR, base.getPath()));
    }

    private static final class ImportJob {
        // The main thread reconnects the launcher if Android recreates it mid-import.
        LauncherActivity launcher;

        ImportJob(LauncherActivity launcher) {
            this.launcher = launcher;
        }

        void start(Context context, Uri tree) {
            Handler main = new Handler(Looper.getMainLooper());
            new Thread(() -> {
                String error = "";
                try {
                    new GameData(context).importPaks(tree);
                } catch (IOException | RuntimeException e) {
                    Log.w(TAG, "Quake PAK import failed", e);
                    error = e instanceof IOException && e.getMessage() != null
                            ? e.getMessage() : context.getString(R.string.folder_unreadable);
                }
                String result = error;
                main.post(() -> {
                    activeImport = null;
                    if (launcher != null) {
                        launcher.showStatus();
                        launcher.message.setText(result);
                    }
                });
            }, "Quake PAK import").start();
        }
    }
}
