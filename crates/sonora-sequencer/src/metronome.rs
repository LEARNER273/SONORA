use std::f32::consts::PI;

#[derive(Debug, Clone)]
pub struct Metronome {
    sample_rate: f32,
    active: bool,
    time: f32,
    freq: f32,
    phase: f32,
    pub volume_db: f32,
    pub enabled: bool,
}

impl Metronome {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            active: false,
            time: 0.0,
            freq: 800.0,
            phase: 0.0,
            volume_db: -6.0,
            enabled: false,
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        if sample_rate > 0.0 {
            self.sample_rate = sample_rate;
        }
    }

    pub fn trigger_beat(&mut self, is_downbeat: bool) {
        if !self.enabled {
            return;
        }
        self.active = true;
        self.time = 0.0;
        self.phase = 0.0;
        self.freq = if is_downbeat { 1200.0 } else { 800.0 };
    }

    #[inline(always)]
    pub fn process_sample(&mut self) -> f32 {
        if !self.active || !self.enabled {
            return 0.0;
        }

        let dt = 1.0 / self.sample_rate;
        self.time += dt;

        let decay_time = 0.035; // 35ms tight click
        let env = (1.0 - (self.time / decay_time)).max(0.0);
        if env <= 0.001 {
            self.active = false;
            return 0.0;
        }

        let phase_inc = (2.0 * PI * self.freq) / self.sample_rate;
        self.phase = (self.phase + phase_inc) % (2.0 * PI);

        let gain = 10.0f32.powf(self.volume_db / 20.0);
        self.phase.sin() * env * gain
    }
}
