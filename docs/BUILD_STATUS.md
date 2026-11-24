# SONORA — Build Status (honest ledger)

_Last verified: 2026-11-19 (Android 11 x86_64 emulator, host GPU). Every claim below was reproduced
by running the listed commands on this machine. Nothing in this document is aspirational;
unimplemented work is listed in "Not done yet"._

## Ship-ready

**`artifacts/apk/SONORA-debug.apk`** — installable universal debug APK (arm64-v8a + x86_64),
signed (SHA-256 cert `d898487b…`), zipaligned, 2.76 MB.

- Manifest: `com.sonora.app` v1.0.0, minSdk 26, targetSdk 34.
- Permissions: **`RECORD_AUDIO` only** — no `INTERNET`, no storage permissions. The app cannot
  make network connections (offline-first by construction).
- Contains: `lib/arm64-v8a/libsonora.so` + `lib/x86_64-linux-android/libsonora.so` (Rust engine,
  release build — the x86_64 slice makes it installable on emulators), `classes.dex` (Java host +
  JS bridge), `assets/index.html` + `assets/sonora.css` (UI), adaptive launcher icons.
- Rebuild from clean source: `./scripts/build_apk.sh` (deterministic, no Gradle).

### Final APK (honest checksums)

```
artifacts/apk/SONORA-debug.apk
SHA-256: bb77d78d335319f7d88889efcf8f81aa5bb4d3f57319f3e16995104f75c87919
```
Packaged as `artifacts/SONORA-1.0.0-debug.zip` (APK + `SHA256SUMS.txt` + both docs).

## Launch — "Signal Boot" intro (~5 s, self-contained, tap-to-skip)

The intro is a pure-CSS-overlay play in `apps/sonora-android/assets/index.html` — no images,
no fonts, no network (the app is offline by construction):

1. 0.0–0.5 s — thin cyan signal line draws across the centre.
2. 0.2–1.9 s — 26 cyan→violet→magenta EQ bars rise from the centre outwards.
3. 1.3–2.7 s — gradient waveform path draws across the bars, peak dot pulses.
4. 1.95–2.9 s — the SONORA wordmark reveals letter-by-letter with per-letter gradient.
5. 2.4–3.5 s — glow underline sweeps.
6. 3.15–3.35 s — the tagline fades up.
7. ~4.0–4.5 s — bars collapse back, wordmark glows, then the overlay fades out (0.44 s) and
   the DAW is revealed.

Behaviour details (all in `index.html`):

- **Runs once per cold start.** `sessionStorage['sonora_boot_seen']` is set at controller parse;
  killing the process (force-stop / reboot) clears it, so it replays on a true cold launch only.
  A same-session reload shows the overlay only via `display:none` flash guard.
- **Starts after the system splash lifts.** Android's API 31+ splash is backed by
  `platform/res/values-v31/styles.xml` (same `sonora_bg` canvas + launcher mark). The page-side
  timeline starts on two animation frames plus a 320 ms beat — the first frame is when Android
  lifts the splash and presents the WebView, so the sequence is never playing behind the splash
  or on a blank gap. Verified on the emulator: first frame at +4.3 s cold-start.
- **Tap anywhere on the overlay skips it** (`pointerdown` → `endBoot`, 0.44 s fade-out; verified
  via CDP-dispatched taps — boot element removed within a second of the tap).
- **Reduced motion** (`prefers-reduced-motion: reduce`, e.g. animation scales at 0): the
  sequence collapses to a ~0.95 s static version of the final intro (bars + word visible
  immediately, no fade timeline). Emulator animator scales are set back to 1.
- **Telemetry:** `pollTelemetry` skips while `state.introActive` (set at overlay parse, cleared
  after the fade); `state.lastTimeText` is reset on hand-off so the playhead re-paints.
- **No fake anything:** the intro uses no buttons/meters/exports; the Rust engine boots before
  the page loads, so first audio works while the intro plays.

Honest stream wording (the old UI text claimed "exclusive/low-latency"; the Rust side grants
the first mode it can, so the UI now reports the actual state):

- `AAudio output · device running at <rate> Hz`
- `AAudio output (stream idle)`

## Audio engine (Rust, real)

- Instruments with named presets: piano (grand/soft/bright/electric/cinematic), guitar
  (acoustic/clean/warm/rock/ambient — Karplus-Strong), bass (electric/sub/synth/808), synth
  (pluto/nebula/stardust/voyager) with global LFO (rate + depth), drum kits (studio/trap/acoustic/
  hiphop/electronic/lofi) over 8 channels.
- Channel + master DSP: EQ, delay, reverb, chorus, distortion, filter, compressor, brickwall
  limiter with 5 ms lookahead. True-peak/RMS metering, FFT, waveform ring.
- Sequencer: sample-accurate event dispatch, loop region with correct wrap semantics (regression
  test `test_loop_wrap_fires_downbeat_events`), metronome, BPM 30–300, 4/4 grid.
- Offline renderer: full-song WAV (16-bit / 24-bit / 32-bit float), per-track stems, Standard MIDI
  File export — all faster than real-time, all written to `exports/` with timestamped names.
- Projects: SQLite catalog + `.sonora` package (manifest.json + atomic session.json + backups),
  create/open/save/autosave.
- AAudio output: adaptive open — tries exclusive/low-latency, shared/low-latency, then
  shared/no-hint, and logs the granted configuration. AAudio formats/performance constants verified
  against NDK r26b `AAudio.h` (`PCM_FLOAT = 2`, performance modes `NONE/POWER_SAVING/LOW_LATENCY =
  10/11/12` — the Oboe-style 0/1/2 values are rejected by the platform). Stream rebuild on device
  change (supervisor thread + on-demand `serviceAudio`), device rate adopted. Boot is idempotent
  (activity recreation reuses the live stream instead of leaking a second one).
- Telemetry bridge: JSON strings over `@JavascriptInterface` (Java arrays are not marshalled to JS
  and arrive as `undefined`): `[peakL, peakR, rmsL, rmsR, tick]`, waveform, FFT.

## UI (WebView, all real wiring)

14 screens. Every visible control is connected to the engine or renders from engine truth
(`getSessionJson` snake_case session). No fake meters, no fake exports, no placeholder buttons:

- Transport: play/pause/stop, BPM, metronome, loop toggle, playhead time from engine tick.
- Arrangement: tracks + clips rendered from session, loop region overlay, moving playhead.
- Piano/Guitar/Bass/Synth keyboards (real note on/off per track), sustain pedal, octave shift,
  chord strums.
- Drum machine: session-backed 16-step grid × 8 channels, step edit → engine command, current-step
  highlight while playing.
- Mixer: rendered from session, dB faders (−60…+6), pan, mute/solo, master.
- Mastering: live spectrum (engine FFT), true peak + RMS dBFS readouts, device rate info.
- Export center: 5 real export buttons with progress lock + result log.
- Projects: create/open/save/autosave (60 s) against the SQLite catalog.
- Sound library: accurate inventory of engine presets. Settings: real device/sample info.
- Recording screen: **honestly disabled** ("Not available in this build") — see below.

## Verification status

| Check | Result |
| :--- | :--- |
| `cargo test --workspace` | 21 passed, 0 failed |
| `cargo check --workspace` | 0 warnings |
| `cargo ndk --platform 26 -t arm64-v8a -t x86_64 check -p sonora-android` | 0 warnings |
| `cargo clippy --workspace --all-targets` | 0 warnings |
| `cargo fmt --all --check` | clean |
| `apksigner verify` | passes (debug cert) |
| `zipalign -c 4` | passes |
| Emulator install + launch | Android 11 (API 31) x86_64, `am start` → process alive, UI rendered |
| AAudio streaming | stream open 48 kHz / burst 960 / shared / performance NONE; first data callback 960 frames; AudioFlinger track active |
| Engine playback | telemetry tick 3882 → 8211 → 12061 while `peak ≈ 0.92`, meters move, waveform scope draws |
| Launch intro | Cold start: "Signal Boot" intro (26 bars + 6-letter SONORA wordmark) plays fully visible on the emulator — bars from +5.4 s, wordmark by +6.8 s, overlay removed +8.9 s after `am start`; taps anywhere during the intro skip it |
| Telemetry while intro | `pollTelemetry` returns early while `state.introActive`; playhead text re-paints on hand-off (`state.lastTimeText = null`) — verified via CDP: after the overlay is removed the telemetry channel reads again |
| Transport controls | pause freezes tick (5871 → 5871 over 2 s), resume continues, stop resets to 0, play after stop restarts the stream (no `-895`) |
| Keyboard preview while stopped | held piano key → peak 0.76 with natural decay, tick stays 0 |
| On-device WAV export | 3,072,044 bytes, 16 s, 48 kHz stereo, peak −1.9 dBFS, RMS −19.0 dBFS — same as the host render except ±1 LSB (max −90 dBFS) CPU rounding |

## Not done yet (do not ship claims about these)

1. **Microphone recording** — `RECORD_AUDIO` is declared but no input stream is wired; the UI says
   so instead of pretending.
2. **Physical-phone testing** — not performed (the user's phone display is broken). Everything in
   the table above was verified on the Android 11 x86_64 emulator with the exact APK that ships;
   arm64 hardware should be checked once a usable device/panel is available.
3. **Piano-roll note editing** — viewer only for now.
4. **LUFS integrated loudness metering** — peak/RMS only.
5. **Audio focus + headphone-unplug handling** — lifecycle pause/resume and stream rebuild are
   implemented; focus requests are not. The device stream intentionally keeps running while the
   transport is stopped so instrument previews work; it is not paused when the app is backgrounded
   (only the transport pauses).
6. **Release signing / release APK** — debug keystore only.
7. **Undo/redo, automation curves, audio-track clips** — model has audio clip type, no editor.
8. **Emulator performance caveat** — with `-gpu swiftshader_indirect` (software rendering) the
   Android UI can ANR under first-boot load; the app itself was responsive once the emulator
   settled. Testing used `-gpu host` (Intel HD 3000) plus a 720×1560 display override.

## Build environment

- NDK r26b: `/home/ansh/Android/android-ndk-r26b` (`ANDROID_NDK_HOME` must be exported)
- SDK: `/home/ansh/Android/Sdk` (build-tools 34.0.0, android-34)
- JDK 25 (javac `--release 11`); build-tools 34 `d8` requires the Java sources to avoid anonymous
  classes/lambdas (they are written as named static classes for this reason)
- Keystore: `artifacts/keystores/debug.keystore` (auto-generated, pass `android`)
