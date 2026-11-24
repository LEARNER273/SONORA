# SONORA — System Architecture & Workspace Specification

## 1. Architectural Overview

**SONORA** is a professional-grade mobile Digital Audio Workstation (DAW) for Android, built as a
real-time audio engine written in Rust and compiled to a native `cdylib` (`libsonora.so`), driven by
a WebView control surface (HTML/CSS/JS) through a JNI/JavaScript bridge, with a thin Java platform
layer owning the `Activity` and the WebView host. Audio output is an **AAudio** stream opened
directly from Rust with an adaptive low-latency policy (exclusive/low-latency → shared/low-latency
→ shared/no-hint; the granted configuration is logged).

```
+-------------------------------------------------------------------------+
|                           SONORA Mobile UI                              |
|            WebView (assets/index.html + sonora.css, raw JS)             |
|  [Arrangement] [Piano Roll] [Instruments] [Drums] [Mixer] [Mastering]   |
+-------------------------------------------------------------------------+
       |   window.SonoraNative.* calls            ^  window.SonoraNative
       |   (@JavascriptInterface bridge)          |  .getTelemetryJson() polls:
       v                                          |  peak/RMS/tick JSON,
+-------------------------------------------------------------------------+
|                     Java Platform Layer (platform/java)                 |
|  MainActivity (WebView host, lifecycle)  SonoraNative (JNI natives +    |
|  Bridge + background export executor)                                    |
+-------------------------------------------------------------------------+
       |  JNI static natives (jni.rs)              ^  JNI arrays / JStrings
       v                                           |  (session JSON mirror)
+-------------------------------------------------------------------------+
|                    libsonora.so  (apps/sonora-android)                  |
|        C ABI (sonora_*)  +  JNI bindings  +  project/export logic       |
+-------------------------------------------------------------------------+
       |  AudioEngineCommand queue (drained at block boundaries)
       v
+-------------------------------------------------------------------------+
|                       SONORA Core & Sequencer                           |
|            (Song model, Tracks, Clips, sample-accurate events)          |
+-------------------------------------------------------------------------+
       |
       | Per-block render on the AAudio callback thread
       v
+-------------------------------------------------------------------------+
|                Real-Time Audio Processing (sonora-audio)                |
|                                                                         |
|  +--------------------+  +--------------------+  +-------------------+  |
|  | Instrument Engines |  | Channel DSP Chains |  | Master Bus        |  |
|  | Piano, Guitar,     |->| EQ, Delay, Reverb, |->| Chorus/Dist/Filt, |  |
|  | Bass, Synth, Drums |  | Chorus, Distortion |  | Delay, Reverb,    |  |
|  | (sonora-instruments)| | (sonora-dsp)       |  | Comp, Limiter     |  |
|  +--------------------+  +--------------------+  +-------------------+  |
+-------------------------------------------------------------------------+
       |
       v AAudio FFI (aaudio_android.rs, links -laaudio -llog)
+-------------------------------------------------------------------------+
|                   Android Hardware Audio HAL / AudioServer              |
|                     (Low-Latency Exclusive AAudio Stream)               |
+-------------------------------------------------------------------------+
```

The WebView is a **control surface, not an audio renderer**: there is no Web Audio fallback. Every
button, fader, and step in the UI maps to a real engine command or reads real engine telemetry. If
the page is opened outside the Android app (where `window.SonoraNative` is absent), it displays a
banner and disables the controls instead of faking audio.

---

## 2. Cargo Workspace & Repository Structure

```
sonora/
├── apps/
│   └── sonora-android/          # cdylib: C ABI (sonora_*), JNI bindings, project
│       ├── src/lib.rs           #   catalog, offline exports, telemetry, storage
│       ├── src/jni.rs           # JNI functions for com.sonora.app.SonoraNative
│       ├── assets/index.html    # The complete WebView UI (single file, vanilla JS)
│       └── styles/sonora.css    # Dark liquid-glass design system
│
├── crates/
│   ├── sonora-core/             # Universal data models (SongSession, Track, Clip,
│   │                            #   MidiNote, DrumPattern) + 8-bar demo content
│   ├── sonora-audio/            # AudioEngine, command queue, AudioDriver,
│   │                            #   AAudio stream host (aaudio_android.rs)
│   ├── sonora-dsp/              # Biquad, Reverb, Delay, Chorus, Distortion,
│   │                            #   Compressor, BrickwallLimiter, FFT, LevelMeter
│   ├── sonora-instruments/      # Piano, Guitar (Karplus-Strong), Bass, Synth,
│   │                            #   Drum kit engines, each with named presets
│   ├── sonora-sequencer/        # Transport clock, loop region, sample-accurate
│   │                            #   event dispatch (incl. loop-wrap regression test)
│   ├── sonora-midi/             # Standard MIDI File import/export
│   ├── sonora-project/          # SQLite catalog (rusqlite) + .sonora package IO
│   ├── sonora-codecs/           # WAV encode/decode (16/24-bit PCM, float32)
│   └── sonora-renderer/         # Offline faster-than-real-time song & stem render
│
├── platform/
│   ├── java/com/sonora/app/     # MainActivity + SonoraNative (JNI + JS bridge)
│   ├── AndroidManifest.xml      # RECORD_AUDIO only — deliberately NO INTERNET
│   └── res/                     # Styles, colors, adaptive launcher icon
│
├── scripts/
│   ├── build_apk.sh             # No-Gradle pipeline: cargo ndk → aapt2 → javac
│   │                            #   → d8 → zip pack → zipalign → apksigner
│   └── gen_icons.py             # Launcher icon generator (PIL)
│
├── docs/                        # Engineering documentation (this directory)
├── artifacts/apk/               # SONORA-debug.apk output
├── Cargo.toml                   # Root workspace manifest
└── README.md
```

---

## 3. Concurrency Model (as implemented)

The audio callback runs on AAudio's high-priority callback thread. The honest model:

1. **Command queue**: `AudioEngineCommand` values are pushed by the bridge/UI side into an
   `Arc<Mutex<VecDeque<...>>>` and drained by the engine **between render blocks** (never mid-block).
   This is not yet a lock-free SPSC ring — it is a short, bounded critical section taken once per
   block, which keeps sample-level rendering allocation-free. Replacing it with a lock-free SPSC
   ring is a planned optimization, not a regression.
2. **Parameter changes are block-quantized**: volume/pan/mute/solo/BPM/loop/preset changes take
   effect at the next block boundary; a click-free ramp is applied by the DSP where relevant.
3. **No allocation in the sample loop**: voices, delay lines, reverb buffers, and the command queue
   are pre-allocated at init; the render path performs no `Vec::push`, `String`, or `malloc`.
4. **Telemetry**: the master `LevelMeter` (peak + RMS) and FFT magnitudes are updated by the render
   path into plain buffers; the JS bridge polls them (~10 Hz) through the engine mutex. Polls are
   read-only and short; the audio thread only ever blocks for the duration of a lock acquisition.
5. **Audio thread prohibitions**: no file I/O, no network, no logging (`__android_log_print` is only
   used on the setup/supervisor threads), no unbounded work.

---

## 4. Audio Focus & Android Lifecycle Management

Implemented in `MainActivity` + `aaudio_android.rs`:

- **App pause (onPause)**: the output stream is paused — transport position is preserved in the
  engine, and `onResume` resumes output via `serviceAudio()`.
- **Device change / stream error**: the AAudio error/disconnect callback sets a restart flag; a
  supervisor thread (and the on-demand `serviceAudio` call) closes and reopens the stream, adopting
  the device's real sample rate (`adopt_device_rate`) and frames-per-burst.
- **Back press**: `moveTaskToBack(true)` — a DAW must not lose its session to a stray back gesture.
- **Screen**: `FLAG_KEEP_SCREEN_ON` while the app is foregrounded.
- **Planned (not yet wired)**: audio focus requests and the headphone-unplug instant-pause broadcast;
  microphone capture (the `RECORD_AUDIO` permission is declared but no input stream exists yet).

---

## 5. Storage Architecture

All storage is app-private (`filesDir`); the manifest declares **no `INTERNET` permission**, so the
app cannot exfiltrate anything and works fully offline.

1. **SQLite catalog (`sonora-project`, rusqlite)**: `sonora_catalog.db` holds the project index,
   name, BPM, track count, timestamps and paths for the Projects screen.
2. **`.sonora` directory package**: one directory per project containing:
   - `manifest.json`: schema version, song name, BPM, timestamps.
   - `session.json`: complete track/clip/note/drum/mixer state (snake_case serde model; written
     atomically via `session.json.tmp` + rename).
   - `backups/`: timestamped autosave rollbacks.
3. **`exports/`**: WAV (16/24/32-float), `.mid`, and per-track stems written by the offline
   renderer, one timestamped file per export.

---

## 6. JNI / Bridge Surface

`SonoraNative.java` declares static natives linked by symbol name to `Java_com_sonora_app_SonoraNative_*`
functions in `jni.rs`. The `Bridge` inner class is registered as `window.SonoraNative`.

- **Commands**: one native call per UI gesture (play/pause/stop, noteOn/noteOff, triggerDrum,
  setDrumStep, setTrackVolume/Pan/Mute/Solo, setMasterVolume, setBpm, setLoop, setMetronome,
  setPreset, setSynthLfo, setSustain).
- **Session JSON**: `getSessionJson()` returns the engine's `SongSession` (the mirror of truth);
  `loadSession(json)` replaces it. The UI renders everything (arrangement, mixer, drum grid,
  piano roll, preset selects) from this JSON.
- **Telemetry**: `getTelemetryJson()` → `"[peakL,peakR,rmsL,rmsR,tick]"` (JSON string — Java arrays
  are not marshalled to JS by `@JavascriptInterface`); `getWaveformJson()` → 256 samples;
  `getFftJson()` → 128 magnitudes; `getSampleRate()` → the device rate the stream actually opened
  with.
- **Projects**: `createProject/listProjects/openProject/saveProject/autosave` return JSON envelopes.
- **Exports**: `exportWavAsync(format)` / `exportMidiAsync()` / `exportStemsAsync()` run on a
  dedicated Java single-thread executor (multi-second offline renders never block the bridge
  thread); results are delivered to JS via `window.SonoraUI.onExportResult(type, json)`.

---

## 7. Engine Truth & Session Mirror

The engine's `SongSession` is the single source of truth. The UI keeps a parsed copy (`state.session`)
used for rendering and optimistic updates, refreshed from the engine after every project open/create
and reconciled incrementally after each edit command. Drum grid toggles, preset changes, mixer
moves, BPM and loop edits are all engine commands; the session mirror is updated in the same
 gesture so the UI never shows state the engine does not have.
