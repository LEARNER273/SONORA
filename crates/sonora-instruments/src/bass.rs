use crate::piano::midi_to_freq;
use std::f32::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BassPreset {
    Electric,
    Sub,
    Synth,
    E808,
}

impl BassPreset {
    pub fn from_name(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "sub" => Self::Sub,
            "synth" => Self::Synth,
            "e808" | "808" => Self::E808,
            _ => Self::Electric,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BassEngine {
    sample_rate: f32,
    preset: BassPreset,

    current_freq: f32,
    target_freq: f32,
    phase: f32,
    phase2: f32,
    active: bool,
    velocity: f32,

    // Envelope
    env_time: f32,
    released: bool,
    release_time: f32,

    // 4-pole Ladder filter state
    d0: f32,
    d1: f32,
    d2: f32,
    d3: f32,
}

impl BassEngine {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            preset: BassPreset::Electric,
            current_freq: 55.0,
            target_freq: 55.0,
            phase: 0.0,
            phase2: 0.0,
            active: false,
            velocity: 0.0,
            env_time: 0.0,
            released: false,
            release_time: 0.0,
            d0: 0.0,
            d1: 0.0,
            d2: 0.0,
            d3: 0.0,
        }
    }

    pub fn set_preset(&mut self, preset_name: &str) {
        self.preset = BassPreset::from_name(preset_name);
    }

    pub fn note_on(&mut self, note: u8, velocity: u8) {
        let freq = midi_to_freq(note);
        if !self.active {
            self.current_freq = freq;
        }
        self.target_freq = freq;
        self.velocity = (velocity as f32 / 127.0).clamp(0.1, 1.0);
        self.active = true;
        self.released = false;
        self.env_time = 0.0;
        self.release_time = 0.0;
    }

    pub fn note_off(&mut self, note: u8) {
        let freq = midi_to_freq(note);
        if (self.target_freq - freq).abs() < 1.0 {
            self.released = true;
        }
    }

    pub fn all_notes_off(&mut self) {
        self.active = false;
        self.d0 = 0.0;
        self.d1 = 0.0;
        self.d2 = 0.0;
        self.d3 = 0.0;
    }

    #[inline(always)]
    pub fn process_sample(&mut self) -> (f32, f32) {
        if !self.active {
            return (0.0, 0.0);
        }

        let dt = 1.0 / self.sample_rate;
        self.env_time += dt;

        // Portamento smoothing (slew target frequency)
        let glide_speed = match self.preset {
            BassPreset::Sub | BassPreset::Synth => 0.015,
            _ => 0.05,
        };
        self.current_freq += (self.target_freq - self.current_freq) * glide_speed;

        // Frequency envelope (for 808 pitch drop)
        let effective_freq = if self.preset == BassPreset::E808 {
            let pitch_drop = (-(self.env_time * 6.0)).exp() * 50.0;
            self.current_freq + pitch_drop
        } else {
            self.current_freq
        };

        let phase_inc = (2.0 * PI * effective_freq) / self.sample_rate;
        self.phase = (self.phase + phase_inc) % (2.0 * PI);
        self.phase2 = (self.phase2 + phase_inc * 1.003) % (2.0 * PI);

        // Amplitude envelope
        let (decay_rate, sustain_level) = match self.preset {
            BassPreset::Sub => (0.6, 0.8),
            BassPreset::E808 => (1.5, 0.1),
            BassPreset::Synth => (2.0, 0.5),
            BassPreset::Electric => (1.8, 0.6),
        };

        let base_env = (-self.env_time * decay_rate).exp() * (1.0 - sustain_level) + sustain_level;

        let amp = if self.released {
            self.release_time += dt;
            let rel_duration = 0.2;
            let rel_factor = (1.0 - (self.release_time / rel_duration)).max(0.0);
            if rel_factor <= 0.001 {
                self.active = false;
                return (0.0, 0.0);
            }
            base_env * rel_factor
        } else {
            base_env
        };

        // Raw oscillator waveform
        let raw_osc = match self.preset {
            BassPreset::Sub => self.phase.sin() + 0.15 * (self.phase * 2.0).sin(),
            BassPreset::E808 => (self.phase.sin() * 1.5).tanh(),
            BassPreset::Synth => {
                // Dual detuned saw waves
                let saw1 = 1.0 - (self.phase / PI);
                let saw2 = 1.0 - (self.phase2 / PI);
                (saw1 + saw2) * 0.5
            }
            BassPreset::Electric => {
                // Sawtooth with triangle sub
                let saw = 1.0 - (self.phase / PI);
                let sub = self.phase.sin();
                saw * 0.7 + sub * 0.5
            }
        };

        // Moog 4-pole ladder filter emulation
        let cutoff_hz = match self.preset {
            BassPreset::Sub => 180.0,
            BassPreset::E808 => 220.0,
            BassPreset::Synth => 400.0 + ((-self.env_time * 4.0).exp() * 2500.0),
            BassPreset::Electric => 300.0 + ((-self.env_time * 3.0).exp() * 1200.0),
        };

        let wc = (2.0 * PI * cutoff_hz.clamp(20.0, self.sample_rate * 0.45) / self.sample_rate)
            .clamp(0.01, 0.95);
        let q_res = match self.preset {
            BassPreset::Synth => 2.2,
            _ => 0.7,
        };

        let input = raw_osc - q_res * self.d3;
        self.d0 += wc * (input.tanh() - self.d0.tanh());
        self.d1 += wc * (self.d0 - self.d1);
        self.d2 += wc * (self.d1 - self.d2);
        self.d3 += wc * (self.d2 - self.d3);

        let filtered = self.d3;
        let out = filtered * amp * self.velocity * 0.5;

        (out, out)
    }
}
