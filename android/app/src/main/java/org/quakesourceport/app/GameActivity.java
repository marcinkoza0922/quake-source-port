package org.quakesourceport.app;

import android.os.Process;

import org.libsdl.app.SDLActivity;

public final class GameActivity extends SDLActivity {
    static final String BASEDIR = "org.quakesourceport.app.BASEDIR";

    @Override
    protected void main() {
        // SDL has loaded its libraries and JNI bindings by this point.
        nativeSetenv("QUAKE_BASEDIR", getIntent().getStringExtra(BASEDIR));
        super.main();
    }

    @Override
    protected void onDestroy() {
        super.onDestroy();
        // SDL expects a fresh process for the next game. The launcher lives separately.
        Process.killProcess(Process.myPid());
    }
}
