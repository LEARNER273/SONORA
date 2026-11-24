use crate::piano::midi_to_freq;
use std::f32::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Waveform {
    Sawtooth,
    Square,
    Triangle,
    Sine,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SynthPreset {
    Pluto,
    Nebula,
    Stardust,
    Voyager,
}

impl SynthPreset {
    pub fn from_name(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "nebula" => Self::Nebula,
            "stardust" => Self::Stardust,
            "voyager" => Self::Voyager,
            _ => Self::Pluto,
        }
    }
}

#[derive(Debug, Clone)]
struct SynthVoice {
    note: u8,
    active: bool,
    age: u64,
    phase: f32,
    phase_detuned: f32,
    freq: f32,
    velocity: f32,

    // Envelope
    env_time: f32,
    released: bool,
    release_time: f32,
    release_start_amp: f32,

    // Filter state
    filter_s1: f32,
}

impl SynthVoice {
    fn new() -> Self {
        Self {
            note: 0,
            active: false,
            age: 0,
            phase: 0.0,
            phase_detuned: 0.0,
            freq: 440.0,
            velocity: 0.0,
            env_time: 0.0,
            released: false,
            release_time: 0.0,
            release_start_amp: 0.0,
            filter_s1: 0.0,
        }
    }

    fn start(&mut self, note: u8, velocity: u8, age: u64) {
        self.note = note;
        self.active = true;
        self.age = age;
        self.freq = midi_to_freq(note);
        self.phase = 0.0;
        self.phase_detuned = 0.0;
        self.velocity = (velocity as f32 / 127.0).clamp(0.1, 1.0);
        self.env_time = 0.0;
        self.released = false;
        self.release_time = 0.0;
        self.release_start_amp = 0.0;
        self.filter_s1 = 0.0;
    }

    fn release(&mut self, current_amp: f32) {
        self.released = true;
        self.release_start_amp = current_amp;
        self.release_time = 0.0;
    }
}

#[derive(Debug, Clone)]
pub struct SynthEngine {
    sample_rate: f32,
    voices: Vec<SynthVoice>,
    preset: SynthPreset,
    voice_counter: u64,

    // Global LFO
    lfo_phase: f32,
    pub lfo_rate_hz: f32,
    pub lfo_depth: f32,
}

impl SynthEngine {
    pub fn new(sample_rate: f32) -> Self {
        let max_polyphony = 8;
        let voices = (0..max_polyphony).map(|_| SynthVoice::new()).collect();
        Self {
            sample_rate,
            voices,
            preset: SynthPreset::Pluto,
            voice_counter: 0,
            lfo_phase: 0.0,
            lfo_rate_hz: 2.0,
            lfo_depth: 0.2,
        }
    }

    pub fn set_preset(&mut self, preset_name: &str) {
        self.preset = SynthPreset::from_name(preset_name);
    }

    pub fn note_on(&mut self, note: u8, velocity: u8) {
        self.voice_counter += 1;
        let counter = self.voice_counter;

        if let Some(v) = self.voices.iter_mut().find(|v| !v.active) {
            v.start(note, velocity, counter);
            return;
        }

        // Steal oldest
        let oldest_idx = self
            .voices
            .iter()
            .enumerate()
            .min_by_key(|(_, v)| (if v.released { 0 } else { 1 }, v.age))
            .map(|(i, _)| i)
            .unwrap_or(0);

        self.voices[oldest_idx].start(note, velocity, counter);
    }

    pub fn note_off(&mut self, note: u8) {
        for v in &mut self.voices {
            if v.active && v.note == note && !v.released {
                v.release(1.0);
            }
        }
    }

    pub fn all_notes_off(&mut self) {
        for v in &mut self.voices {
            v.active = false;
        }
    }

    #[inline(always)]
    pub fn process_sample(&mut self) -> (f32, f32) {
        let mut sum_l = 0.0f32;
        let mut sum_r = 0.0f32;
        let dt = 1.0 / self.sample_rate;

        // Advance global LFO
        let lfo_inc = (2.0 * PI * self.lfo_rate_hz) / self.sample_rate;
        self.lfo_phase = (self.lfo_phase + lfo_inc) % (2.0 * PI);
        let lfo_val = self.lfo_phase.sin() * self.lfo_depth;

        let (attack_s, decay_s, sustain_level, release_s, base_cutoff, waveform): (
            f32,
            f32,
            f32,
            f32,
            f32,
            Waveform,
        ) = match self.preset {
            SynthPreset::Pluto => (0.01, 0.25, 0.5, 0.3, 3500.0, Waveform::Sawtooth),
            SynthPreset::Nebula => (0.35, 0.8, 0.7, 1.2, 1800.0, Waveform::Sawtooth),
            SynthPreset::Stardust => (0.002, 0.18, 0.0, 0.1, 4500.0, Waveform::Triangle),
            SynthPreset::Voyager => (0.005, 0.35, 0.6, 0.25, 1200.0, Waveform::Square),
        };

        for v in &mut self.voices {
            if !v.active {
                continue;
            }

            v.env_time += dt;

            // ADSR Amplitude Envelope
            let amp = if v.released {
                v.release_time += dt;
                let rel_prog = v.release_time / release_s.max(0.01);
                if rel_prog >= 1.0 {
                    v.active = false;
                    continue;
                }
                v.release_start_amp.max(sustain_level) * (1.0 - rel_prog)
            } else if v.env_time < attack_s {
                v.env_time / attack_s.max(0.001)
            } else {
                let decay_time = v.env_time - attack_s;
                let decay_prog = (decay_time / decay_s.max(0.01)).min(1.0);
                1.0 - (1.0 - sustain_level) * decay_prog
            };

            if amp <= 0.0001 {
                v.active = false;
                continue;
            }

            // Dual oscillator with detune
            let detune_ratio = match self.preset {
                SynthPreset::Nebula => 1.005,
                SynthPreset::Pluto => 1.002,
                _ => 1.001,
            };

            let phase_inc = (2.0 * PI * v.freq) / self.sample_rate;
            let phase_inc2 = phase_inc * detune_ratio;

            v.phase = (v.phase + phase_inc) % (2.0 * PI);
            v.phase_detuned = (v.phase_detuned + phase_inc2) % (2.0 * PI);

            let s1 = match waveform {
                Waveform::Sawtooth => 1.0 - (v.phase / PI),
                Waveform::Square => {
                    if v.phase < PI {
                        0.8
                    } else {
                        -0.8
                    }
                }
                Waveform::Triangle => (v.phase / PI - 1.0).abs() * 2.0 - 1.0,
                Waveform::Sine => v.phase.sin(),
            };

            let s2 = match waveform {
                Waveform::Sawtooth => 1.0 - (v.phase_detuned / PI),
                Waveform::Square => {
                    if v.phase_detuned < PI {
                        0.8
                    } else {
                        -0.8
                    }
                }
                Waveform::Triangle => (v.phase_detuned / PI - 1.0).abs() * 2.0 - 1.0,
                Waveform::Sine => v.phase_detuned.sin(),
            };

            let mixed = (s1 + s2) * 0.5;

            // Lowpass filter modulated by LFO and envelope
            let env_cutoff = base_cutoff * (0.3 + 0.7 * amp) * (1.0 + lfo_val);
            let cutoff_norm = (2.0 * PI * env_cutoff / self.sample_rate).clamp(0.01, 0.95);
            v.filter_s1 += cutoff_norm * (mixed - v.filter_s1);

            let voice_out = v.filter_s1 * amp * v.velocity * 0.35;

            sum_l += voice_out;
            sum_r += voice_out;
        }

        (sum_l, sum_r)
    }
}
