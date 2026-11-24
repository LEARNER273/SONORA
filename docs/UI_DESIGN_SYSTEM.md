# SONORA — Mobile UI/UX & Liquid-Glass Design System

## 1. Design Identity & Aesthetic Principles

SONORA embodies a premium, futuristic, dark liquid-glass aesthetic tailored specifically for mobile touchscreens. It rejects cluttered desktop DAW paradigms and avoids excessive neon glow or flat generic interfaces.

### Core Visual Principles
- **Atmospheric Depth**: Multi-layered surfaces floating over near-black and deep-graphite backdrops.
- **Controlled Translucency**: Frosted-glass panels with subtle edge highlights (`1px solid rgba(255, 255, 255, 0.09)`).
- **Tactile Precision**: Every touchable button, fader, key, and pad provides instantaneous visual feedback and adheres to Android's minimum 48dp touch target standard.
- **Zero Runtime Dependencies**: All typography (`Space Grotesk`, `Inter`) and icons are bundled locally in the APK with zero external network calls.

---

## 2. Design Tokens

### 2.1. Color System
```css
:root {
  /* Surface Layers */
  --bg-0: #06060a;              /* Deep canvas black */
  --bg-1: #0a0a12;              /* Secondary studio background */
  --bg-2: #101018;              /* Card & panel base */
  --bg-3: #16161f;              /* Elevated surface / inputs */

  /* Liquid Glass Tokens */
  --glass: rgba(255, 255, 255, 0.045);
  --glass-strong: rgba(255, 255, 255, 0.08);
  --glass-border: rgba(255, 255, 255, 0.09);
  --glass-border-hover: rgba(255, 255, 255, 0.20);

  /* Typography */
  --text-0: #f5f5fa;            /* Primary titles & readings */
  --text-1: #b3b3c2;            /* Secondary labels */
  --text-2: #74748a;            /* Subdued metadata & units */

  /* Accents */
  --accent-cyan: #22d3ee;       /* Primary highlight & active notes */
  --accent-violet: #a78bfa;     /* Secondary audio accents */
  --accent-magenta: #f472b6;    /* Automation & playhead */
  --accent-gradient: linear-gradient(120deg, #22d3ee, #a78bfa 55%, #f472b6);

  /* Functional Status */
  --success: #34d399;           /* Saved, ready, verified */
  --danger: #f87171;            /* Recording active, clip delete */
  --warn: #fbbf24;              /* Limiter gain reduction, clip warn */

  /* Radii & Shadows */
  --radius-sm: 8px;
  --radius-md: 14px;
  --radius-lg: 22px;
  --radius-pill: 9999px;
  --shadow-soft: 0 10px 40px rgba(0, 0, 0, 0.45);
}
```

---

## 3. The 16 Studio Screens

SONORA organizes mobile music production into 16 dedicated, accessible screens:

1. **Home Dashboard**: Quick project launch, recent tracks, audio hardware status, demo sessions.
2. **Project Browser**: Searchable list of saved `.sonora` projects, sorting, duplicates, export backups.
3. **Project Creation**: New song setup with configurable BPM, Time Signature, Key, and template tracks.
4. **Song Arrangement Studio**: Multi-track timeline (Audio & MIDI tracks), zoomable beat ruler, clip drag/drop, split, loop marker.
5. **Piano Roll**: Touch-optimized MIDI note editor with grid snap (1/4 to 1/32), velocity faders, note duration handles.
6. **Piano Instrument**: Multi-octave responsive keyboard, octave switch, sustain pedal toggle, voice selector (Grand, Soft, Bright, Electric, Cinematic).
7. **Guitar Instrument**: 6-string playable fretboard with realistic string vibration animations, strum mode, chord pads.
8. **Bass Instrument**: Slap/finger bass touch interface with low-register keys, slide/portamento control, 808/sub/synth presets.
9. **Synthesizer Laboratory**: Interactive sound design lab with waveform selectors, visual ADSR envelope graph, filter cutoff pad, LFO routing.
10. **Drum Machine**: 16/32-step drum sequencer matrix, 8-channel drum kit selector, swing control, tap tempo, step velocity.
11. **Mixer**: Full-featured vertical channel strips, pan pots, mute/solo buttons, peak level meters, master fader.
12. **Recording Studio**: Microphone recording booth with input gain meter, countdown count-in, latency calibration, take browser.
13. **Sound Library**: Preset browser, imported audio sample browser, drum kit management.
14. **Mastering Laboratory**: Real-time 256-band FFT spectrum analyzer, LUFS integrated/short-term meter, stereo phase correlation meter, brickwall ceiling.
15. **Export Center**: High-resolution audio rendering dialogue (WAV 16/24/32-bit float, MP3/FLAC), MIDI export, stem packager.
16. **Application Settings**: Audio buffer size, sample rate, theme tuning, latency compensation, storage paths.

---

## 4. Mobile Ergonomics & Gesture Controls

- **Edge Insets**: Automatic padding for Android system navigation bar, gesture pill, and camera punch hole (`env(safe-area-inset-top)`).
- **Multi-Touch Piano**: Up to 10 simultaneous touches tracked with distinct pointer IDs, velocity sensitive based on vertical touch position.
- **Scroll vs. Play Isolation**: Timeline horizontal panning is locked while touching clips to prevent accidental scrolling during edits.
- **Haptic Feedback**: Subtle vibration cues on snap-to-grid, loop boundary hit, and record toggle (using Android HapticFeedbackConstants).
