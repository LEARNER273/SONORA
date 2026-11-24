# SONORA — Audio Engine & DSP Technical Specification

## 1. Audio Processing Pipeline

The SONORA audio engine processes audio in fixed or variable sample blocks negotiated with the Android AAudio/Oboe subsystem. The standard processing block size is typically 64, 128, or 256 frames depending on device performance and buffer size configuration (e.g. 48,000 Hz @ 128 frames = 2.67 ms hardware buffer).

```
[MIDI & Sequencer Events]
          |
          v
+-------------------------------------------------------------+
|                     Voice Allocation                        |
|  - Piano (16 voices)        - Guitar (6 physical strings)   |
|  - Bass (Monophonic legato) - Synth (8 polyphonic voices)   |
|  - Drums (8 percussion channels)                            |
+-------------------------------------------------------------+
          |
          | Per-Instrument Stereo Audio Stream
          v
+-------------------------------------------------------------+
|                      Mixer Channels                         |
|  For each Track:                                            |
|    Insert DSP: Chorus -> Distortion -> Filter -> Delay      |
|    Channel Fader (Gain) & Pan (Equal Power)                 |
|    Mute / Solo Logic                                        |
+-------------------------------------------------------------+
          |
          | Summed Stereo Master Bus
          v
+-------------------------------------------------------------+
|                     Master Processing                       |
|  - Master Parametric EQ / Filter                            |
|  - Global Reverb Send / Return                              |
|  - Master Bus Compressor                                    |
|  - Master Gain Fader                                        |
|  - Lookahead Brickwall Peak Limiter (-0.3 dBFS ceiling)     |
+-------------------------------------------------------------+
          |
          +------------------------+
          |                        |
          v                        v
+---------------------+  +------------------------------------+
| Hardware DAC Output |  | Telemetry Ring Buffer              |
| (Oboe / AAudio)     |  | - 1024-sample Waveform Buffer      |
|                     |  | - 256-band Hann-Windowed FFT Bins  |
|                     |  | - Channel Peak / RMS Levels        |
+---------------------+  +------------------------------------+
```

---

## 2. High-Precision Timing & Sequencer Clock

Timing within SONORA is calculated in pure integer audio frames rather than floating-point seconds or wall-clock system time. This ensures 100% sample-accurate playback and complete immunity to clock drift, audio buffer resize events, or thread scheduling jitter.

$$\text{Frames Per Beat} = \frac{\text{Sample Rate} \times 60}{\text{BPM}}$$
$$\text{Ticks Per Frame} = \frac{\text{PPQN} \times \text{BPM}}{\text{Sample Rate} \times 60}$$

Where SONORA uses standard **960 PPQN (Pulses Per Quarter Note)** for micro-timing resolution.

---

## 3. Instrument Synthesis Engines

### 3.1. Piano Engine
- **Architecture**: Polyphonic additive and modal physical modeling.
- **Polyphony**: 16 dynamic voices with Least-Recently-Used (LRU) voice stealing and release dampening.
- **Sound Generation**: Fundamental sine-triangle blend with weighted harmonic overtones (2nd to 7th harmonics) dynamically scaled by note velocity.
- **Physical Modeling**: Transient hammer-strike click impulse, velocity-dependent lowpass filter tracking, and sympathetic string resonance simulation.
- **Sustain Handling**: Dedicated sustain pedal buffer. Notes released while sustain is active continue in sustain state until pedal lift.

### 3.2. Guitar Engine (Karplus-Strong Plucked String Synthesis)
- **Architecture**: Digital waveguide physical modeling for 6 independent strings (E2, A2, D3, G3, B3, E4 base tunings).
- **Delay Line**: Sample-accurate circular buffer whose length corresponds to the target string pitch:
  $$L = \frac{f_s}{f_0} - \frac{1}{2}$$
- **Loop Filter**: Single-pole lowpass filter modeling string stiffness and internal friction:
  $$y[n] = \rho \cdot \frac{x[n] + x[n-1]}{2}$$
  Where $\rho$ is the string decay factor ($0.98 - 0.998$).
- **Excitation**: Band-limited noise burst shaped by pick velocity and contact position.

### 3.3. Bass Engine
- **Architecture**: Monophonic dual-oscillator subtractive synthesizer with portamento.
- **Oscillators**: Sawtooth, Square, and deep Sub-Sine wave.
- **Filter**: 4-pole 24 dB/octave resonant lowpass filter modeled after the Moog ladder filter, with non-linear saturation in the resonance loop:
  $$\frac{dy}{dt} = \omega_c \left( \tanh(x - 4k \cdot y) - \tanh(y) \right)$$
- **Portamento**: Exponential pitch slewing configurable between 0 and 200 ms.

### 3.4. Synthesizer Laboratory
- **Architecture**: 8-voice polyphonic synthesizer with multi-waveform oscillators (Saw, Triangle, Pulse with PWM, Sine, Noise).
- **Modulation**: Dedicated LFO (Sine, Triangle, Saw, Random S&H) routable to Pitch (Vibrato), Filter Cutoff (Wah), and Amplitude (Tremolo).
- **Envelopes**: Dual exponential ADSR envelopes (Amplitude ADSR and Filter ADSR).

### 3.5. Drum Engine
- **Kick**: Dual-stage pitch drop oscillator (high transient click at 150 Hz rapidly dropping to fundamental 40-55 Hz) with soft clipping.
- **Snare**: Resonant body bandpass filter (180 Hz) mixed with enveloped white noise through a highpass filter (1.8 - 2.6 kHz).
- **Closed / Open Hi-Hat**: 6 detuned square-wave oscillators creating a complex metallic inharmonic cluster, passed through a 7 kHz highpass filter with fast exponential decay.
- **Toms**: Pitched sine drop with stick transient click and medium decay (400-600 ms).
- **Crash / Ride**: Multi-band metallic noise generator with extended decay (1.2 - 2.5 s).

---

## 4. Professional DSP Effects

1. **Parametric Equalizer / Biquad Filters**: Robert Bristow-Johnson Audio EQ Cookbook formulas implemented with transposed Direct Form II to minimize roundoff noise.
2. **Ping-Pong Delay**: Dual delay lines with cross-feedback, host tempo synchronization (1/4, 1/8, 1/8 dot, 1/16), and lowpass feedback dampening.
3. **Schroeder-Freeverb Reverb**: 8 parallel Feedback Comb Filters (FBCF) tuned to prime delay lengths, followed by 4 series Allpass Diffusers, with stereo spread and high-frequency absorption.
4. **Distortion / Saturation**: Antialiased oversampled waveshaping featuring soft tanh saturation, asymmetric tube emulation, and hard clipping options.
5. **Stereo Chorus**: Quadrature-modulated delay line (0.1 - 5 Hz LFO) with feedback and dry/wet blend.
6. **Master Limiter**: Transparent lookahead peak limiter with 5ms lookahead buffer, zero overshoot, and smooth program-dependent release.
