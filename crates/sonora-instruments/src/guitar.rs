use crate::piano::midi_to_freq;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GuitarPreset {
    Acoustic,
    CleanElectric,
    WarmElectric,
    Rock,
    Ambient,
}

impl GuitarPreset {
    pub fn from_name(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "cleanelectric" => Self::CleanElectric,
            "warmelectric" => Self::WarmElectric,
            "rock" => Self::Rock,
            "ambient" => Self::Ambient,
            _ => Self::Acoustic,
        }
    }
}

#[derive(Debug, Clone)]
struct StringModel {
    active: bool,
    buffer: Vec<f32>,
    write_idx: usize,
    buffer_len: usize,
    prev_sample: f32,
    damping: f32,
    pluck_len: usize,
    pluck_count: usize,
    velocity: f32,
}

impl StringModel {
    fn new() -> Self {
        Self {
            active: false,
            buffer: vec![0.0; 2048],
            write_idx: 0,
            buffer_len: 2048,
            prev_sample: 0.0,
            damping: 0.992,
            pluck_len: 0,
            pluck_count: 0,
            velocity: 0.0,
        }
    }

    fn pluck(&mut self, freq: f32, sample_rate: f32, velocity: u8, damping: f32) {
        let period_samples = (sample_rate / freq).round() as usize;
        self.buffer_len = period_samples.clamp(16, 2048);
        self.buffer.resize(self.buffer_len, 0.0);
        self.write_idx = 0;
        self.prev_sample = 0.0;
        self.damping = damping.clamp(0.90, 0.999);
        self.velocity = (velocity as f32 / 127.0).clamp(0.1, 1.0);
        self.active = true;

        // Initialize delay line with band-limited pink/white noise excitation
        let mut seed = (freq * 1337.0) as u32;
        for i in 0..self.buffer_len {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let noise = ((seed >> 16) as f32 / 32768.0) - 1.0;
            self.buffer[i] = noise * self.velocity;
        }
        self.pluck_len = self.buffer_len;
        self.pluck_count = 0;
    }

    #[inline(always)]
    fn process(&mut self) -> f32 {
        if !self.active {
            return 0.0;
        }

        let curr = self.buffer[self.write_idx];

        // Lowpass feedback loop filter (Karplus-Strong)
        let filtered = (curr + self.prev_sample) * 0.5 * self.damping;
        self.prev_sample = curr;

        self.buffer[self.write_idx] = filtered;
        self.write_idx = (self.write_idx + 1) % self.buffer_len;

        self.pluck_count += 1;
        if self.pluck_count > self.buffer_len * 200 && filtered.abs() < 1e-4 {
            self.active = false;
        }

        filtered
    }
}

#[derive(Debug, Clone)]
pub struct GuitarEngine {
    sample_rate: f32,
    strings: Vec<StringModel>,
    preset: GuitarPreset,
}

impl GuitarEngine {
    pub fn new(sample_rate: f32) -> Self {
        let strings = (0..6).map(|_| StringModel::new()).collect();
        Self {
            sample_rate,
            strings,
            preset: GuitarPreset::Acoustic,
        }
    }

    pub fn set_preset(&mut self, preset_name: &str) {
        self.preset = GuitarPreset::from_name(preset_name);
    }

    pub fn note_on(&mut self, note: u8, velocity: u8) {
        let freq = midi_to_freq(note);

        let damping = match self.preset {
            GuitarPreset::Acoustic => 0.992,
            GuitarPreset::CleanElectric => 0.996,
            GuitarPreset::WarmElectric => 0.988,
            GuitarPreset::Rock => 0.995,
            GuitarPreset::Ambient => 0.998,
        };

        // Assign to first inactive string or oldest string
        let string_idx = self.strings.iter().position(|s| !s.active).unwrap_or(0);

        self.strings[string_idx].pluck(freq, self.sample_rate, velocity, damping);
    }

    pub fn note_off(&mut self, _note: u8) {
        // Guitar strings damp naturally on release
        for s in &mut self.strings {
            if s.active {
                s.damping *= 0.95;
            }
        }
    }

    pub fn all_notes_off(&mut self) {
        for s in &mut self.strings {
            s.active = false;
        }
    }

    #[inline(always)]
    pub fn process_sample(&mut self) -> (f32, f32) {
        let mut mono_sum = 0.0f32;
        for s in &mut self.strings {
            mono_sum += s.process();
        }

        // Apply preset shaping
        let shaped = match self.preset {
            GuitarPreset::Rock => {
                // Saturated overdrive
                (mono_sum * 2.2).tanh() * 0.45
            }
            _ => mono_sum * 0.35,
        };

        (shaped, shaped)
    }
}
