# SONORA — Feature Audit & Migration Specification

## 1. Executive Summary

This document details the reverse engineering, audit, and architectural migration path for **SONORA**, transitioning from the reference browser-based client application (Tone.js + Web Audio + IndexedDB) to a high-performance native Android Digital Audio Workstation (DAW) built in Rust with Dioxus Mobile, native Oboe/AAudio audio processing, and local SQLite/filesystem persistence.

The reference bundle was extracted and audited from `/home/ansh/Downloads/sonora/reference_bundle/`, consisting of:
- `index.html`: Entry structure, meta definitions, viewport settings
- `assets/index-dmNgwm_6.js`: Core application logic, instruments, presets, sequencer, mixer, and export
- `assets/tone-eT4RWXPy.js`: Tone.js Web Audio library wrapper
- `assets/vendor-BidpXJVQ.js`: React 19, Lucide icons, Framer Motion
- `assets/index-CgiHuZLU.css`: Dark liquid-glass styling system
- `favicon.svg`: Brand vector icon

---

## 2. Feature Matrix: Web Reference vs. Native Rust SONORA

| Subsystem | Web Reference (Existing) | Native Rust Target (SONORA) | Migration Strategy |
| :--- | :--- | :--- | :--- |
| **Audio Core** | Web Audio API / Tone.js via browser thread | Native Rust DSP engine running on dedicated real-time audio thread (`oboe`/`AAudio`) | Full native DSP reimplementation. Lock-free SPSC ring buffers, zero-allocation callback, atomic parameter updates. |
| **UI Framework** | React 19 + Framer Motion | Dioxus Mobile (Rust) with native webview host | Idiomatic Rust components, signals, reactive state management, CSS styling. |
| **Sample Rate** | Fixed by browser context (44.1k / 48k) | Dynamic hardware negotiated (44.1 kHz, 48 kHz, etc.) | High-performance sample rate converters, buffer-agnostic block processing. |
| **Piano** | 5 voices: Grand, Soft, Bright, Electric, Cinematic | Polyphonic multi-oscillator synthesized piano + physical hammer modeling + sustain | Native Rust polyphonic voice allocator with voice stealing, velocity curves, sustain pedal state. |
| **Guitar** | 5 presets: Acoustic, Clean, Warm, Rock, Ambient | 6-string physical modeling (Karplus-Strong plucked string synthesis) | Native delay-line string modeling with string dampening, body resonance, fretboard touch interaction. |
| **Bass** | 4 presets: Electric, Sub, Synth, 808 Bass | Multi-algorithm bass synthesis (sub-sine, saw wave, 808 pitch-decay kick-bass) | Dedicated bass voice engine with saturation and 24dB/oct resonant ladder filter. |
| **Synthesizer** | 4 presets: Pluto Lead, Nebula Pad, Stardust Pluck, Voyager Bass | Synthesizer Laboratory: Multi-oscillator, ADSR envelopes, LFO modulation, filter routing | Polyphonic subtractive & FM synth engine with real-time parameter smoothing. |
| **Drum Machine** | 6 kits (Acoustic, Studio, Trap, Hip-Hop, Electronic, Lo-Fi) | 6 expanded kits with synthesized & bundled sample voices | Step sequencer (16/32 steps), swing timing, per-step velocity, pattern chaining. |
| **Sequencer** | Tone.Transport tick scheduling | Lock-free audio timeline tick engine synchronized directly to audio frame clock | Microsecond-accurate sample-level timing; immune to UI frame drops or garbage collection. |
| **Arrangement** | Single pattern playback + take recording | Multi-track arrangement timeline (Instrument, Audio, MIDI tracks) | Clip movement, resizing, looping, splitting, track mute/solo/pan/gain. |
| **Piano Roll** | Basic note viewer | Full interactive Piano Roll touch editor | Note drawing, drag move, resize duration, velocity edit, snap-to-grid, quantization. |
| **Mixer** | Channel strips, master gain, limiter | Multi-track channel strips with inserts, sends, master limiter, true peak & LUFS meters | SIMD-accelerated mixing bus, pan law (equal power 3dB), channel mute/solo/fader. |
| **DSP Effects** | Tone.js effects: Chorus, Distortion, Filter, Delay, Reverb, Compressor | Native Rust DSP library (`sonora-dsp`) | High-quality biquad filters, stereo ping-pong delay, Schroeder/Freeverb algorithmic reverb, soft-knee compressor. |
| **Audio Recording** | Browser MediaRecorder | Low-latency AAudio microphone capture directly to memory buffer & WAV | Lock-free input stream recording, take management, peak level metering. |
| **Audio Import** | Browser File API (`AudioContext.decodeAudioData`) | Pure Rust audio decoders (`symphonia`) supporting WAV, FLAC, OGG, MP3 | Direct decoding into 32-bit floating-point audio buffers for timeline placement. |
| **Mastering Lab** | Basic web visualizer | Spectrum analyzer (FFT 256/1024), True Peak meter, Loudness (LUFS), stereo correlation meter | Real-time lock-free audio analysis ring buffer streaming to UI visualizers. |
| **Export** | Browser Blob URL download (.wav, .mid, .json) | Multi-format background exporter (WAV 16/24/32-bit, MIDI SMF, Stem export, Project archive) | Background thread offline rendering running faster than real-time into `.sonora` package and external storage. |
| **Storage** | IndexedDB | SQLite catalogue (`rusqlite`) + `.sonora` bundle directory | Atomic file writes, project schema migrations, autosave, crash recovery. |
| **Network** | Google Fonts runtime dependency | 100% Offline: Bundled local typography and SVG icon assets | Zero external runtime network requests; no telemetry, tracking, or cloud lock-in. |

---

## 3. Reference Presets Specifications

### Piano Presets
1. **Grand Piano**: Triangle/Sine blend with attack envelope (4ms attack, 1.3s decay, 0.4 sustain, 0.8s release), cutoff 5000 Hz.
2. **Soft Piano**: Mellow harmonic spectrum, lowpass filter at 2200 Hz, gentle velocity response.
3. **Bright Piano**: Sawtooth/Square subtle overtone mix, filter cutoff at 8000 Hz, crisp transient.
4. **Electric Piano**: 2-operator FM synthesis, harmonicity ratio 3, modulation index 8, gentle tremolo.
5. **Cinematic Piano**: Long release (3.2s), wide stereo detuning, subtle pad layering, filter at 3000 Hz.

### Guitar Presets
1. **Acoustic Guitar**: Karplus-Strong string synthesis, dampening 4000 Hz, resonance 0.92, acoustic body impulse.
2. **Clean Electric**: High resonance (0.95), bright pickup profile, chorus send, dampening 6500 Hz.
3. **Warm Electric**: Rolled-off highs (dampening 3200 Hz), tube-style warm saturation.
4. **Rock Guitar**: Karplus-Strong string into wave-shaping overdrive distortion, cabinet filter emulation.
5. **Ambient Guitar**: Extended sustain, delay send (250ms ping-pong), large hall reverb send.

### Bass Presets
1. **Electric Bass**: Sawtooth waveform through dynamic envelope filter (attack 10ms, decay 250ms, sustain 0.6), base cutoff 150 Hz.
2. **Sub Bass**: Pure sine wave with subtle 2nd harmonic saturation, portamento 20ms, extended sub-bass power (30-80 Hz).
3. **Synth Bass**: Dual-oscillator detuned saw waves with resonant lowpass sweep (Q=2.5), snappy decay.
4. **808 Bass**: Deep sine drop with pitch decay envelope (55 Hz decaying to 32 Hz over 800ms) with soft-clipping drive.

### Synthesizer Laboratory Presets
1. **Pluto Lead**: Fast attack saw lead, filter cutoff 3500 Hz, Q 1.0, light delay and reverb sends.
2. **Nebula Pad**: Slow attack (150ms), dual saw + triangle, slow LFO filter modulation, wide stereo chorus.
3. **Stardust Pluck**: Instantaneous attack (1ms), fast exponential decay (180ms), zero sustain, snappy resonant filter.
4. **Voyager Bass**: Heavy square/saw blend, aggressive filter drive, tight release.

### Drum Kits
1. **Acoustic**: Natural acoustic drum simulation (Kick: 55 Hz pitch, 350ms decay; Snare: 1800 Hz noise/tone blend; Hat: 90ms metallic decay; Crash: 1.6s).
2. **Studio**: Tight modern studio kit (Kick: 50 Hz, 400ms decay; Snare: 2000 Hz; Hat: 80ms decay; Crash: 1.4s).
3. **Trap**: Sub-heavy 808 kick (42 Hz, 900ms decay), crispy layered snare (2600 Hz), rapid 1/32 roll-ready hats (50ms decay).
4. **Hip-Hop**: Boomy kick (48 Hz), punchy snare with rim shot, vintage vinyl noise profile.
5. **Electronic**: Synthesized techno/house kit, aggressive transient click, resonant noise sweep claps.
6. **Lo-Fi**: Bandpass-filtered kit, bitcrush saturation, subtle pitch flutter.

---

## 4. Preservation & Modernization Requirements

1. **Brand Continuity**: Maintain the name **SONORA**, the brand typography, the clean icon aesthetic, and the dark liquid-glass UI identity.
2. **Preset Compatibility**: Ensure all presets from the reference application sound faithful or superior when running on the native Rust DSP engine.
3. **Project File Interoperability**: Support importing legacy `.sonora.json` web project export files into the new native `.sonora` package format.
4. **Real-time Safety**: Enforce strict DAW audio thread safety (no malloc, no mutex, no syscalls in the audio callback).
