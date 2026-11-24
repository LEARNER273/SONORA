package com.sonora.app;

import android.app.Activity;
import android.os.Bundle;
import android.view.WindowManager;
import android.webkit.WebChromeClient;
import android.webkit.WebSettings;
import android.webkit.WebView;

/**
 * SONORA — single-activity host for the WebView control surface backed by the
 * Rust audio engine (AAudio + cdylib). No INTERNET permission: everything is
 * offline and local to app-private storage.
 */
public class MainActivity extends Activity {
    private WebView webView;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);

        webView = new WebView(this);
        WebSettings settings = webView.getSettings();
        settings.setJavaScriptEnabled(true);
        settings.setDomStorageEnabled(true);
        settings.setAllowFileAccess(true);
        webView.setBackgroundColor(0xFF06060A);
        webView.setWebChromeClient(new WebChromeClient());

        // Storage + engine must be ready before the UI asks for the session.
        SonoraNative.setStorageRoot(getFilesDir().getAbsolutePath());
        SonoraNative.init(48000);

        webView.addJavascriptInterface(new SonoraNative.Bridge(webView), "SonoraNative");
        setContentView(webView);
        webView.loadUrl("file:///android_asset/index.html");
    }

    @Override
    protected void onPause() {
        super.onPause();
        // Background: stop audible output (transport position is preserved).
        SonoraNative.pause();
    }

    @Override
    protected void onResume() {
        super.onResume();
        // Foreground: repair the output stream if the device changed while away.
        SonoraNative.serviceAudio();
    }

    @Override
    public void onBackPressed() {
        // A DAW must not lose its session to a stray back press.
        moveTaskToBack(true);
    }

    @Override
    protected void onDestroy() {
        if (webView != null) {
            webView.destroy();
            webView = null;
        }
        super.onDestroy();
    }
}
