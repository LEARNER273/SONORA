use std::f32::consts::PI;

pub fn midi_to_freq(note: u8) -> f32 {
    440.0 * 2.0f32.powf((note as f32 - 69.0) / 12.0)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PianoPreset {
    Grand,
    Soft,
    Bright,
    Electric,
    Cinematic,
}

impl PianoPreset {
    pub fn from_name(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "soft" => Self::Soft,
            "bright" => Self::Bright,
            "electric" => Self::Electric,
            "cinematic" => Self::Cinematic,
            _ => Self::Grand,
        }
    }
}

#[derive(Debug, Clone)]
struct PianoVoice {
    note: u8,
    active: bool,
    sustained: bool,
    age: u64,
    phase: f32,
    phase_inc: f32,
    fm_phase: f32,
    velocity: f32,
    env_time: f32,
    released: bool,
    release_time: f32,
    filter_state: f32,
}

impl PianoVoice {
    fn new() -> Self {
        Self {
            note: 0,
            active: false,
            sustained: false,
            age: 0,
            phase: 0.0,
            phase_inc: 0.0,
            fm_phase: 0.0,
            velocity: 0.0,
            env_time: 0.0,
            released: false,
            release_time: 0.0,
            filter_state: 0.0,
        }
    }

    fn start(&mut self, note: u8, velocity: u8, sample_rate: f32, age: u64) {
        self.note = note;
        self.active = true;
        self.sustained = false;
        self.age = age;
        let freq = midi_to_freq(note);
        self.phase = 0.0;
        self.phase_inc = (2.0 * PI * freq) / sample_rate;
        self.fm_phase = 0.0;
        self.velocity = (velocity as f32 / 127.0).clamp(0.01, 1.0);
        self.env_time = 0.0;
        self.released = false;
        self.release_time = 0.0;
        self.filter_state = 0.0;
    }

    fn release(&mut self, sustain_pedal: bool) {
        if sustain_pedal {
            self.sustained = true;
        } else {
            self.released = true;
        }
    }
}

#[derive(Debug, Clone)]
pub struct PianoEngine {
    sample_rate: f32,
    voices: Vec<PianoVoice>,
    sustain_pedal: bool,
    preset: PianoPreset,
    voice_counter: u64,
}

impl PianoEngine {
    pub fn new(sample_rate: f32) -> Self {
        let max_polyphony = 16;
        let voices = (0..max_polyphony).map(|_| PianoVoice::new()).collect();
        Self {
            sample_rate,
            voices,
            sustain_pedal: false,
            preset: PianoPreset::Grand,
            voice_counter: 0,
        }
    }

    pub fn set_preset(&mut self, preset_name: &str) {
        self.preset = PianoPreset::from_name(preset_name);
    }

    pub fn set_sustain(&mut self, sustain: bool) {
        self.sustain_pedal = sustain;
        if !sustain {
            for v in &mut self.voices {
                if v.sustained {
                    v.sustained = false;
                    v.released = true;
                }
            }
        }
    }

    pub fn note_on(&mut self, note: u8, velocity: u8) {
        self.voice_counter += 1;
        let counter = self.voice_counter;

        // Try finding inactive voice
        if let Some(v) = self.voices.iter_mut().find(|v| !v.active) {
            v.start(note, velocity, self.sample_rate, counter);
            return;
        }

        // Voice stealing: Find oldest released or oldest voice
        let oldest_idx = self
            .voices
            .iter()
            .enumerate()
            .min_by_key(|(_, v)| (if v.released { 0 } else { 1 }, v.age))
            .map(|(i, _)| i)
            .unwrap_or(0);

        self.voices[oldest_idx].start(note, velocity, self.sample_rate, counter);
    }

    pub fn note_off(&mut self, note: u8) {
        for v in &mut self.voices {
            if v.active && v.note == note && !v.released {
                v.release(self.sustain_pedal);
            }
        }
    }

    pub fn all_notes_off(&mut self) {
        for v in &mut self.voices {
            v.active = false;
            v.released = false;
            v.sustained = false;
        }
    }

    #[inline(always)]
    pub fn process_sample(&mut self) -> (f32, f32) {
        let mut sum_l = 0.0f32;
        let mut sum_r = 0.0f32;
        let dt = 1.0 / self.sample_rate;

        for v in &mut self.voices {
            if !v.active {
                continue;
            }

            v.env_time += dt;

            // Amplitude envelope calculation
            let decay_rate = match self.preset {
                PianoPreset::Soft => 1.8,
                PianoPreset::Bright => 1.2,
                PianoPreset::Electric => 0.9,
                PianoPreset::Cinematic => 0.5,
                PianoPreset::Grand => 1.4,
            };

            let base_env = (-v.env_time * decay_rate).exp();

            let amp = if v.released {
                v.release_time += dt;
                let rel_duration = match self.preset {
                    PianoPreset::Cinematic => 0.8,
                    _ => 0.15,
                };
                let rel_factor = (1.0 - (v.release_time / rel_duration)).max(0.0);
                if rel_factor <= 0.001 {
                    v.active = false;
                    continue;
                }
                base_env * rel_factor
            } else {
                base_env
            };

            if amp <= 0.0001 {
                v.active = false;
                continue;
            }

            // Synthesize tone based on preset
            let sample = match self.preset {
                PianoPreset::Electric => {
                    // FM Synthesis: Harmonicity 3.0, Mod index 8.0
                    let mod_inc = v.phase_inc * 3.0;
                    v.fm_phase = (v.fm_phase + mod_inc) % (2.0 * PI);
                    let modulator = v.fm_phase.sin() * 4.0 * amp;
                    (v.phase + modulator).sin()
                }
                _ => {
                    // Triangle + Sine overtones with velocity-dependent brightness
                    let s1 = v.phase.sin();
                    let s2 = (v.phase * 2.0).sin() * 0.4;
                    let s3 = (v.phase * 3.0).sin() * 0.2 * v.velocity;
                    let s4 = (v.phase * 4.0).sin() * 0.1 * v.velocity;
                    s1 + s2 + s3 + s4
                }
            };

            // Dynamic lowpass filter tracking note velocity
            let cutoff_coeff = match self.preset {
                PianoPreset::Soft => 0.15,
                PianoPreset::Bright => 0.65,
                PianoPreset::Electric => 0.40,
                PianoPreset::Cinematic => 0.30,
                PianoPreset::Grand => 0.35 + v.velocity * 0.25,
            };
            v.filter_state += (sample - v.filter_state) * cutoff_coeff;

            let out_voice = v.filter_state * amp * v.velocity * 0.35;

            // Stereo panning based on note keyboard position (low notes left, high notes right)
            let pan = ((v.note as f32 - 60.0) / 48.0).clamp(-0.8, 0.8);
            let gain_l = ((1.0 - pan) * 0.5).sqrt();
            let gain_r = ((1.0 + pan) * 0.5).sqrt();

            sum_l += out_voice * gain_l;
            sum_r += out_voice * gain_r;

            v.phase = (v.phase + v.phase_inc) % (2.0 * PI);
        }

        (sum_l, sum_r)
    }
}
