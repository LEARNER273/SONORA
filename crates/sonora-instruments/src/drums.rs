use std::f32::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DrumKit {
    Acoustic,
    Studio,
    Trap,
    HipHop,
    Electronic,
    LoFi,
}

impl DrumKit {
    pub fn from_name(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "acoustic" => Self::Acoustic,
            "trap" => Self::Trap,
            "hiphop" => Self::HipHop,
            "electronic" => Self::Electronic,
            "lofi" => Self::LoFi,
            _ => Self::Studio,
        }
    }
}

#[derive(Debug, Clone)]
struct PercussionVoice {
    active: bool,
    time: f32,
    velocity: f32,
    phase: f32,
    phase2: f32,
    noise_seed: u32,
    filter_state: f32,
    channel: u8,
}

impl PercussionVoice {
    fn new(channel: u8) -> Self {
        Self {
            active: false,
            time: 0.0,
            velocity: 0.0,
            phase: 0.0,
            phase2: 0.0,
            noise_seed: 12345 + (channel as u32 * 6789),
            filter_state: 0.0,
            channel,
        }
    }

    fn trigger(&mut self, velocity: u8) {
        self.active = true;
        self.time = 0.0;
        self.velocity = (velocity as f32 / 127.0).clamp(0.1, 1.0);
        self.phase = 0.0;
        self.phase2 = 0.0;
        self.filter_state = 0.0;
    }

    #[inline(always)]
    fn next_noise(&mut self) -> f32 {
        self.noise_seed = self
            .noise_seed
            .wrapping_mul(1664525)
            .wrapping_add(1013904223);
        ((self.noise_seed >> 16) as f32 / 32768.0) - 1.0
    }
}

#[derive(Debug, Clone)]
pub struct DrumEngine {
    sample_rate: f32,
    kit: DrumKit,
    voices: Vec<PercussionVoice>, // 8 channels
}

impl DrumEngine {
    pub fn new(sample_rate: f32) -> Self {
        let voices = (0..8).map(PercussionVoice::new).collect();
        Self {
            sample_rate,
            kit: DrumKit::Studio,
            voices,
        }
    }

    pub fn set_kit(&mut self, kit_name: &str) {
        self.kit = DrumKit::from_name(kit_name);
    }

    pub fn trigger_channel(&mut self, channel: u8, velocity: u8) {
        if let Some(v) = self.voices.get_mut(channel as usize) {
            v.trigger(velocity);
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

        for v in &mut self.voices {
            if !v.active {
                continue;
            }

            v.time += dt;

            let sample = match v.channel {
                0 => {
                    // Kick
                    let (base_pitch, decay) = match self.kit {
                        DrumKit::Trap => (42.0, 0.9),
                        DrumKit::Acoustic => (55.0, 0.35),
                        DrumKit::HipHop => (48.0, 0.5),
                        _ => (50.0, 0.4),
                    };

                    let env = (-v.time * (1.0 / decay)).exp();
                    if env < 0.001 {
                        v.active = false;
                        continue;
                    }

                    let pitch = base_pitch + ((-v.time * 28.0).exp() * 120.0);
                    let phase_inc = (2.0 * PI * pitch) / self.sample_rate;
                    v.phase = (v.phase + phase_inc) % (2.0 * PI);

                    // Click transient in the first 8ms
                    let click = if v.time < 0.008 {
                        v.next_noise() * 0.4
                    } else {
                        0.0
                    };

                    (v.phase.sin() + click).tanh() * env
                }
                1 => {
                    // Snare
                    let (snare_freq, decay) = match self.kit {
                        DrumKit::Trap => (2600.0, 0.22),
                        DrumKit::Acoustic => (1800.0, 0.18),
                        _ => (2000.0, 0.20),
                    };

                    let env = (-v.time * (1.0 / decay)).exp();
                    if env < 0.001 {
                        v.active = false;
                        continue;
                    }

                    // Tonal body (180 Hz drop)
                    let body_pitch = 180.0 + ((-v.time * 30.0).exp() * 80.0);
                    let phase_inc = (2.0 * PI * body_pitch) / self.sample_rate;
                    v.phase = (v.phase + phase_inc) % (2.0 * PI);
                    let body = v.phase.sin() * (-v.time * 15.0).exp();

                    // Noise snap
                    let noise = v.next_noise();
                    let norm_cut = (2.0 * PI * snare_freq / self.sample_rate).clamp(0.05, 0.9);
                    v.filter_state += norm_cut * (noise - v.filter_state);
                    let snap = noise - v.filter_state; // Highpass noise

                    (body * 0.6 + snap * 0.7) * env
                }
                2 => {
                    // Closed Hi-Hat
                    let decay = match self.kit {
                        DrumKit::Trap => 0.05,
                        _ => 0.08,
                    };
                    let env = (-v.time * (1.0 / decay)).exp();
                    if env < 0.001 {
                        v.active = false;
                        continue;
                    }

                    // Inharmonic metallic square cluster
                    let noise = v.next_noise();
                    let norm_cut = (2.0 * PI * 7000.0 / self.sample_rate).clamp(0.1, 0.9);
                    v.filter_state += norm_cut * (noise - v.filter_state);
                    let metallic = noise - v.filter_state;

                    metallic * env * 0.6
                }
                3 => {
                    // Open Hi-Hat
                    let env = (-v.time * (1.0 / 0.45)).exp();
                    if env < 0.001 {
                        v.active = false;
                        continue;
                    }
                    let noise = v.next_noise();
                    let norm_cut = (2.0 * PI * 6000.0 / self.sample_rate).clamp(0.1, 0.9);
                    v.filter_state += norm_cut * (noise - v.filter_state);
                    let metallic = noise - v.filter_state;

                    metallic * env * 0.65
                }
                4 => {
                    // High Tom
                    let env = (-v.time * (1.0 / 0.35)).exp();
                    if env < 0.001 {
                        v.active = false;
                        continue;
                    }
                    let pitch = 180.0 + ((-v.time * 18.0).exp() * 90.0);
                    let phase_inc = (2.0 * PI * pitch) / self.sample_rate;
                    v.phase = (v.phase + phase_inc) % (2.0 * PI);
                    v.phase.sin() * env * 0.7
                }
                5 => {
                    // Low Tom
                    let env = (-v.time * (1.0 / 0.45)).exp();
                    if env < 0.001 {
                        v.active = false;
                        continue;
                    }
                    let pitch = 95.0 + ((-v.time * 14.0).exp() * 60.0);
                    let phase_inc = (2.0 * PI * pitch) / self.sample_rate;
                    v.phase = (v.phase + phase_inc) % (2.0 * PI);
                    v.phase.sin() * env * 0.7
                }
                6 => {
                    // Crash
                    let env = (-v.time * (1.0 / 1.5)).exp();
                    if env < 0.001 {
                        v.active = false;
                        continue;
                    }
                    let noise = v.next_noise();
                    let norm_cut = (2.0 * PI * 4500.0 / self.sample_rate).clamp(0.05, 0.9);
                    v.filter_state += norm_cut * (noise - v.filter_state);
                    (noise - v.filter_state) * env * 0.5
                }
                7 => {
                    // Clap
                    let env = (-v.time * (1.0 / 0.25)).exp();
                    if env < 0.001 {
                        v.active = false;
                        continue;
                    }
                    // Multi-burst triggers at 0ms, 12ms, 24ms
                    let burst_gate =
                        if v.time < 0.010 || (v.time > 0.012 && v.time < 0.022) || v.time > 0.024 {
                            1.0
                        } else {
                            0.1
                        };
                    let noise = v.next_noise();
                    noise * burst_gate * env * 0.6
                }
                _ => 0.0,
            };

            let out_voice = sample * v.velocity * 0.4;
            // Panning for drum kit realism
            let pan: f32 = match v.channel {
                2 | 4 => -0.3, // Hat / High Tom slightly left
                3 | 5 => 0.3,  // Open Hat / Low Tom slightly right
                6 => -0.5,     // Crash left
                _ => 0.0,      // Kick, Snare, Clap center
            };

            let gain_l = ((1.0 - pan) * 0.5).sqrt();
            let gain_r = ((1.0 + pan) * 0.5).sqrt();

            sum_l += out_voice * gain_l;
            sum_r += out_voice * gain_r;
        }

        (sum_l, sum_r)
    }
}
