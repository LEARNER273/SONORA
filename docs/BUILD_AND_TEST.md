# SONORA — Build, Test & Deployment Guide

## 1. Environment & Prerequisites

### Toolchain Requirements
- **Rust toolchain**: 1.80+ (installed via rustup; `source $HOME/.cargo/env` first)
- **Compilation targets**:
  - `x86_64-unknown-linux-gnu` — host development & test suite
  - `aarch64-linux-android` + `x86_64-linux-android` — the APK ships both (arm64-v8a for phones,
    x86_64 for the emulator)
- **JDK**: any modern JDK works for `javac --release 11` (system has JDK 25)
- **Android NDK**: r26b at `/home/ansh/Android/android-ndk-r26b`
  (`export ANDROID_NDK_HOME=/home/ansh/Android/android-ndk-r26b` — required)
- **Android SDK**: `/home/ansh/Android/Sdk` with `build-tools/34.0.0` and `platforms/android-34`
- **cargo-ndk**: `~/.cargo/bin/cargo-ndk`
- **python3 + Pillow**: only for `scripts/gen_icons.py` (already generated)

> **Important**: `cargo ndk` must be invoked with `--platform 26`. `libaaudio.so` only exists in the
> API 26+ NDK sysroot, so linking fails without it (minSdk is 26 anyway).

---

## 2. Compilation Workflows

### 2.1. Host build & full test suite
```bash
source $HOME/.cargo/env
cd /home/ansh/Downloads/sonora
cargo test --workspace
```
All 21 tests must pass with zero warnings (`cargo check --workspace` → 0 warnings).

### 2.2. Android-target validation (compiles the cfg(android) code)
```bash
source $HOME/.cargo/env
export ANDROID_NDK_HOME=/home/ansh/Android/android-ndk-r26b
cd /home/ansh/Downloads/sonora
cargo ndk --platform 26 -t arm64-v8a -t x86_64 check -p sonora-android
```
The host check never sees `jni.rs` / `aaudio_android.rs` — this is the check that catches Android-only
errors (private-field access, JNI signatures, `-laaudio` linking).

### 2.3. Assembling the APK (no Gradle)
```bash
./scripts/build_apk.sh
```
The pipeline (`scripts/build_apk.sh`):
1. `cargo ndk --platform 26 -t arm64-v8a -t x86_64 build --release` → both `libsonora.so` slices
2. `scripts/gen_icons.py` → adaptive launcher icons
3. `aapt2 compile/link` → resources, manifest, assets (`index.html` + `sonora.css`)
4. `javac --release 11` → `platform/java` + generated `R.java`
5. `d8` → `classes.dex` (build-tools 34 d8 requires **no anonymous classes/lambdas** in the Java
   sources — they are written as named static classes for exactly this reason)
6. Python zip append → `classes.dex` + `lib/arm64-v8a/libsonora.so` +
   `lib/x86_64-linux-android/libsonora.so` into the aapt2 base APK
7. `zipalign -f 4`
8. `apksigner sign` with the auto-generated debug keystore
   (`artifacts/keystores/debug.keystore`, store/key pass `android`)
9. `apksigner verify --print-certs`

Output (the only artifact produced):
```
artifacts/apk/SONORA-debug.apk
```

### 2.4. Packaged deliverable
```
artifacts/SONORA-1.0.0-debug.zip
  SONORA-debug.apk
  SHA256SUMS.txt
  docs/BUILD_STATUS.md
  docs/BUILD_AND_TEST.md
```
The canonical working copy stays at `artifacts/apk/SONORA-debug.apk`; the zip is the
packaged package for sharing. `SHA256SUMS.txt` contains the SHA-256 of `SONORA-debug.apk`:
```
bb77d78d335319f7d88889efcf8f81aa5bb4d3f57319f3e16995104f75c87919  SONORA-debug.apk
```
Verify:
```bash
unzip -l artifacts/SONORA-1.0.0-debug.zip
shasum -a 256 artifacts/apk/SONORA-debug.apk   # must match SHA256SUMS.txt
```

### 2.5. Lint & formatting
```bash
cargo fmt --all            # rustfmt
cargo clippy --workspace   # must report 0 warnings
```

---

## 3. Test Matrix (actual tests, as in the workspace)

| Test | Crate | Verifies |
| :--- | :--- | :--- |
| `test_biquad_lowpass` | sonora-dsp | Filter coefficient stability, frequency response |
| `test_reverb_stability` | sonora-dsp | Schroeder/Freeverb feedback decays to silence |
| `test_limiter_brickwall` | sonora-dsp | Master ceiling never exceeded on hot input |
| `test_fft_computation` | sonora-dsp | FFT magnitude correctness |
| `test_piano_polyphony` / `test_guitar_karplus_strong` / `test_drum_channels` | sonora-instruments | Voice engines |
| `test_sequencer_transport_and_events` | sonora-sequencer | Transport, note on/off scheduling |
| `test_loop_wrap_fires_downbeat_events` | sonora-sequencer | Downbeat events re-fire on every loop wrap (regression) |
| `test_timing_conversions` | sonora-core | Tick↔frame↔second conversions |
| `test_session_serialization` | sonora-core | SongSession JSON round-trip |
| `test_wav_roundtrip` | sonora-codecs | WAV encode/decode (16/24/float32) |
| `test_smf_export` | sonora-midi | Standard MIDI File 1 export |
| `test_project_db_and_package` | sonora-project | SQLite catalog + `.sonora` package open/save |
| `test_offline_song_render` | sonora-renderer | Faster-than-real-time full-song render |
| `test_audio_driver_lifecycle` / `test_audio_engine_block_processing` / `test_interleaved_rt_render` | sonora-audio | Driver + engine block path |
| `test_c_bridge_interleaved_rendering` / `test_session_json_roundtrip` / `test_project_lifecycle` | sonora-android | C ABI surface (serialized via `TEST_SERIAL` mutex) |

---

## 4. Deployment & Testing

### 4.1. One-command Android emulator (no physical device)

```bash
./scripts/run_emulator.sh
```

Creates the AVD on first run (Android 11 / API 31, `google_apis`, x86_64), boots it, installs
`artifacts/apk/SONORA-debug.apk`, launches the app and verifies the process. Ctrl+C shuts it down.
Prerequisites: SDK packages `emulator` and `system-images;android-31;google_apis;x86_64`.

Manual equivalent:
```bash
$HOME/Android/Sdk/emulator/emulator -avd sonora -no-snapshot -no-boot-anim -gpu host -cores 4
adb install -r artifacts/apk/SONORA-debug.apk
adb shell am start -n com.sonora.app/.MainActivity
adb logcat -s SonoraAudio:V AAudio:V
```

**Signal Boot intro (verified on the emulator, 2026-11-19):** after `am start` the app shows the
5 s launch animation in full — confirmed via CDP timeline: intro visible ~+4 s after launch,
all 26 EQ bars + the 6-letter SONORA wordmark fully visible by +6.8 s, overlay removed +8.9 s;
tapping during the intro skips it immediately. `adb shell settings put global
animator_duration_scale 0` triggers the reduced-motion fallback (static ~0.95 s).

Expected engine log (the first two lines prove the HAL is pumping the callback):
```
SonoraAudio: data callback active: first block = 960 frames
SonoraAudio: stream open: rate=48000 burst=960 sharing=1 performance=10
SonoraAudio: transport play requested
```
`sharing=1` = shared, `performance=10` = no performance hint — the third fallback of the adaptive
open. On many phones the second or first attempt is granted instead (the log states what was
actually granted).

Emulator notes:
- Use `-gpu host` for a responsive UI; `-gpu swiftshader_indirect` (software) can trigger system/UI
  ANRs under first-boot load on slow hosts.
- A 720×1560 override (`adb shell wm size 720x1560 && adb shell wm density 320`) reduces the
  software-rendering cost; the app works at any size.
- Debug telemetry without the UI: enable WebView devtools, then evaluate
  `window.SonoraNative.getTelemetryJson()` via the DevTools websocket:
  `adb forward tcp:9333 localabstract:webview_devtools_remote_$(adb shell pidof com.sonora.app)`.

### 4.2. Physical device

```bash
adb devices -l
adb install -r artifacts/apk/SONORA-debug.apk
adb shell am start -n com.sonora.app/.MainActivity
adb logcat -s SonoraAudio:V AAudio:V
```

### Verified on the emulator (2026-11-19)

| Area | Result |
| :--- | :--- |
| Install + launch | process alive, UI rendered at 720×1560 |
| AAudio | 48 kHz, 960-frame bursts, shared/no-hint, active AudioFlinger track, 0 underruns at idle |
| Playback | tick 3882 → 8211 → 12061, peaks ≈ 0.92, meters and scope live |
| Pause/resume/stop | pause freezes tick, resume continues, stop resets to 0, play after stop restarts the stream |
| Keyboard preview | held key while stopped → peak 0.76, natural decay |
| WAV export | 16 s 48 kHz stereo file, peak −1.9 dBFS, RMS −19.0 dBFS, matching the host render ±1 LSB |

### Suggested on-device checklist
1. First launch → tap **Play**: the shipped 8-bar demo loop (piano I–V–vi–IV, drums, bass, synth,
   guitar) must be audible immediately; meters and playhead move.
2. **Piano/Guitar/Bass/Synth** screens: keys/strings play the right instrument; preset changes
   audibly switch and survive project save/open.
3. **Drum Machine**: toggling steps updates the pattern; grid highlights follow the transport.
4. **Mixer**: fader/pan/mute/solo audibly affect each track.
5. **Projects**: create, save, close app, reopen, open project — state restored.
6. **Export Center**: WAV 16/24/32, MIDI, stems — each writes a file and logs success.
7. **Settings**: device sample rate matches the stream (typically 48000 Hz).

---

## 5. Known Limitations (honest list)

- Microphone recording UI is intentionally disabled — the input stream is not wired yet.
- Piano roll is a read-only viewer; note editing lands with the audio-track system.
- LUFS integrated metering is not implemented (peak/RMS only).
- Command queue is mutex-based (drained per block), not yet a lock-free SPSC ring.
- `@JavascriptInterface` arrays do not cross into JS (they arrive as `undefined`); telemetry,
  waveform and FFT are returned as JSON strings instead.
- The AAudio device stream keeps running while the transport is stopped (so keyboard previews
  work) and is not paused when the app is backgrounded — only the transport pauses.
- Physical-phone (arm64) testing is still pending; the exact shipping APK was verified on an
  Android 11 x86_64 emulator.
