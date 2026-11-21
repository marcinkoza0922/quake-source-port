package org.quakesourceport.app;

import android.annotation.TargetApi;
import android.os.Build;
import android.os.Bundle;
import android.os.Process;
import android.util.DisplayMetrics;
import android.view.KeyEvent;
import android.window.OnBackInvokedCallback;
import android.window.OnBackInvokedDispatcher;

import org.libsdl.app.SDLActivity;

public final class GameActivity extends SDLActivity {
    static final String BASEDIR = "org.quakesourceport.app.BASEDIR";
    private Runnable unregisterBackCallback;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        if (!mBrokenLibraries && Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            registerBackCallback();
        }
    }

    @TargetApi(Build.VERSION_CODES.TIRAMISU)
    private void registerBackCallback() {
        OnBackInvokedDispatcher dispatcher = getOnBackInvokedDispatcher();
        OnBackInvokedCallback callback = this::sendBackToSDL;
        dispatcher.registerOnBackInvokedCallback(OnBackInvokedDispatcher.PRIORITY_DEFAULT, callback);
        unregisterBackCallback = () -> dispatcher.unregisterOnBackInvokedCallback(callback);
    }

    @Override
    protected void main() {
        // SDL has loaded its libraries and JNI bindings by this point.
        nativeSetenv("QUAKE_BASEDIR", getIntent().getStringExtra(BASEDIR));
        DisplayMetrics metrics = new DisplayMetrics();
        getWindowManager().getDefaultDisplay().getRealMetrics(metrics);
        nativeSetenv("QUAKE_DISPLAY_DPI_X", Float.toString(metrics.xdpi));
        nativeSetenv("QUAKE_DISPLAY_DPI_Y", Float.toString(metrics.ydpi));
        super.main();
    }

    @Override
    public void onBackPressed() {
        if (mBrokenLibraries) {
            super.onBackPressed();
        } else {
            sendBackToSDL();
        }
    }

    private void sendBackToSDL() {
        // Gesture navigation can deliver Back without a key event.
        onNativeKeyDown(KeyEvent.KEYCODE_BACK);
        onNativeKeyUp(KeyEvent.KEYCODE_BACK);
    }

    @Override
    protected void onDestroy() {
        if (unregisterBackCallback != null) {
            unregisterBackCallback.run();
        }
        super.onDestroy();
        // SDL expects a fresh process for the next game. The launcher lives separately.
        Process.killProcess(Process.myPid());
    }
}
