package com.sonora.app;

import android.webkit.JavascriptInterface;
import android.webkit.WebView;

import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/**
 * JNI surface of the SONORA audio engine (libsonora.so) plus the object that is
 * exposed to the WebView as window.SonoraNative.
 *
 * The static native methods are linked by symbol name to the JNI layer compiled
 * in apps/sonora-android (jni.rs). The Bridge instance is what JavaScript talks
 * to; every call is a direct pass-through except exports, which are dispatched
 * to a background executor so a multi-second offline render never blocks the
 * JS bridge thread. Export results are delivered to JS via
 * window.SonoraUI.onExportResult(type, json).
 */
public final class SonoraNative {
    static {
        System.loadLibrary("sonora");
    }

    // ------------------------------------------------------------------
    // JNI methods (implemented in Rust, apps/sonora-android/src/jni.rs)
    // ------------------------------------------------------------------
    public static native boolean init(int sampleRate);
    public static native void play();
    public static native void pause();
    public static native void stop();
    public static native void serviceAudio();

    public static native void noteOn(int track, int note, int velocity);
    public static native void noteOff(int track, int note);
    public static native void triggerDrum(int channel, int velocity);
    public static native void setSustain(boolean on);
    public static native void setDrumStep(int clip, int channel, int step, boolean active, int velocity);

    public static native void setBpm(float bpm);
    public static native void setLoop(boolean enabled, int startBar, int endBar);
    public static native void setTrackVolume(int track, float volumeDb);
    public static native void setTrackPan(int track, float pan);
    public static native void setTrackMute(int track, boolean muted);
    public static native void setTrackSolo(int track, boolean soloed);
    public static native void setMasterVolume(float volumeDb);
    public static native void setMetronome(boolean enabled);
    public static native void setPreset(int instrument, String preset);
    public static native void setSynthLfo(float rateHz, float depth);

    public static native int getSampleRate();
    public static native boolean loadSession(String json);
    public static native String getSessionJson();
    public static native void setStorageRoot(String path);

    public static native String createProject(String name);
    public static native String listProjects();
    public static native String openProject(String path);
    public static native String saveProject();
    public static native String autosave();

    public static native String exportWav(int format); // 0=16-bit, 1=24-bit, 2=float32
    public static native String exportMidi();
    public static native String exportStems();

    public static native String getTelemetryJson(); // [peakL, peakR, rmsL, rmsR, tick]
    public static native String getWaveformJson();   // 256 samples
    public static native String getFftJson();        // 128 bins

    // ------------------------------------------------------------------
    // JavaScript bridge (window.SonoraNative)
    // ------------------------------------------------------------------
    public static final class Bridge {
        private final WebView webView;
        private final ExecutorService exportExecutor = Executors.newSingleThreadExecutor();

        public Bridge(WebView webView) {
            this.webView = webView;
        }

        // Named classes instead of anonymous ones: build-tools 34's d8 chokes
        // on anonymous inner classes emitted by modern javac (NPE in R0.a).

        // Transport ----------------------------------------------------
        @JavascriptInterface public boolean init(int sampleRate) { return SonoraNative.init(sampleRate); }
        @JavascriptInterface public void play() { SonoraNative.play(); }
        @JavascriptInterface public void pause() { SonoraNative.pause(); }
        @JavascriptInterface public void stop() { SonoraNative.stop(); }
        @JavascriptInterface public void serviceAudio() { SonoraNative.serviceAudio(); }

        // Instruments --------------------------------------------------
        @JavascriptInterface public void noteOn(int track, int note, int velocity) { SonoraNative.noteOn(track, note, velocity); }
        @JavascriptInterface public void noteOff(int track, int note) { SonoraNative.noteOff(track, note); }
        @JavascriptInterface public void triggerDrum(int channel, int velocity) { SonoraNative.triggerDrum(channel, velocity); }
        @JavascriptInterface public void setSustain(boolean on) { SonoraNative.setSustain(on); }
        @JavascriptInterface public void setDrumStep(int clip, int channel, int step, boolean active, int velocity) {
            SonoraNative.setDrumStep(clip, channel, step, active, velocity);
        }

        // Mixer / tempo -------------------------------------------------
        @JavascriptInterface public void setBpm(float bpm) { SonoraNative.setBpm(bpm); }
        @JavascriptInterface public void setLoop(boolean enabled, int startBar, int endBar) { SonoraNative.setLoop(enabled, startBar, endBar); }
        @JavascriptInterface public void setTrackVolume(int track, float volumeDb) { SonoraNative.setTrackVolume(track, volumeDb); }
        @JavascriptInterface public void setTrackPan(int track, float pan) { SonoraNative.setTrackPan(track, pan); }
        @JavascriptInterface public void setTrackMute(int track, boolean muted) { SonoraNative.setTrackMute(track, muted); }
        @JavascriptInterface public void setTrackSolo(int track, boolean soloed) { SonoraNative.setTrackSolo(track, soloed); }
        @JavascriptInterface public void setMasterVolume(float volumeDb) { SonoraNative.setMasterVolume(volumeDb); }
        @JavascriptInterface public void setMetronome(boolean enabled) { SonoraNative.setMetronome(enabled); }
        @JavascriptInterface public void setPreset(int instrument, String preset) { SonoraNative.setPreset(instrument, preset); }
        @JavascriptInterface public void setSynthLfo(float rateHz, float depth) { SonoraNative.setSynthLfo(rateHz, depth); }

        // Session / projects --------------------------------------------
        @JavascriptInterface public int getSampleRate() { return SonoraNative.getSampleRate(); }
        @JavascriptInterface public boolean loadSession(String json) { return SonoraNative.loadSession(json); }
        @JavascriptInterface public String getSessionJson() { return SonoraNative.getSessionJson(); }
        @JavascriptInterface public void setStorageRoot(String path) { SonoraNative.setStorageRoot(path); }
        @JavascriptInterface public String createProject(String name) { return SonoraNative.createProject(name); }
        @JavascriptInterface public String listProjects() { return SonoraNative.listProjects(); }
        @JavascriptInterface public String openProject(String path) { return SonoraNative.openProject(path); }
        @JavascriptInterface public String saveProject() { return SonoraNative.saveProject(); }
        @JavascriptInterface public String autosave() { return SonoraNative.autosave(); }

        // Telemetry -------------------------------------------------------
        // NOTE: @JavascriptInterface cannot marshal Java arrays to JS — they
        // arrive as undefined. JSON strings are the supported path.
        @JavascriptInterface public String getTelemetryJson() { return SonoraNative.getTelemetryJson(); }
        @JavascriptInterface public String getWaveformJson() { return SonoraNative.getWaveformJson(); }
        @JavascriptInterface public String getFftJson() { return SonoraNative.getFftJson(); }

        // Exports — CPU-heavy offline renders on a background thread ------
        @JavascriptInterface public void exportWavAsync(final int format) {
            exportExecutor.execute(new WavExportJob(webView, format));
        }

        @JavascriptInterface public void exportMidiAsync() {
            exportExecutor.execute(new MidiExportJob(webView));
        }

        @JavascriptInterface public void exportStemsAsync() {
            exportExecutor.execute(new StemsExportJob(webView));
        }
    }

    /** Renders offline on the export executor, then hops to the UI thread. */
    private abstract static class ExportJob implements Runnable {
        protected final WebView webView;
        private final String type;

        ExportJob(WebView webView, String type) {
            this.webView = webView;
            this.type = type;
        }

        @Override
        public final void run() {
            String json = render();
            webView.post(new DeliverJob(webView, type, json));
        }

        protected abstract String render();
    }

    private static final class WavExportJob extends ExportJob {
        private final int format;

        WavExportJob(WebView webView, int format) {
            super(webView, "wav");
            this.format = format;
        }

        @Override
        protected String render() {
            return SonoraNative.exportWav(format);
        }
    }

    private static final class MidiExportJob extends ExportJob {
        MidiExportJob(WebView webView) {
            super(webView, "midi");
        }

        @Override
        protected String render() {
            return SonoraNative.exportMidi();
        }
    }

    private static final class StemsExportJob extends ExportJob {
        StemsExportJob(WebView webView) {
            super(webView, "stems");
        }

        @Override
        protected String render() {
            return SonoraNative.exportStems();
        }
    }

    /** Delivers an export result to JavaScript on the UI thread. */
    private static final class DeliverJob implements Runnable {
        private final WebView webView;
        private final String type;
        private final String json;

        DeliverJob(WebView webView, String type, String json) {
            this.webView = webView;
            this.type = type;
            this.json = json;
        }

        @Override
        public void run() {
            // `json` is a serde-produced JSON object literal; embedding it
            // directly is a valid JS argument list.
            webView.evaluateJavascript(
                    "window.SonoraUI && window.SonoraUI.onExportResult('" + type + "', " + json + ");",
                    null);
        }
    }
}
