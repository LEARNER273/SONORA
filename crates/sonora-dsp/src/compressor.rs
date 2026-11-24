#[derive(Debug, Clone)]
pub struct StereoCompressor {
    sample_rate: f32,
    pub threshold_db: f32,
    pub ratio: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    pub makeup_db: f32,

    envelope: f32,
    attack_coeff: f32,
    release_coeff: f32,
}

impl StereoCompressor {
    pub fn new(sample_rate: f32) -> Self {
        let mut comp = Self {
            sample_rate,
            threshold_db: -18.0,
            ratio: 3.0,
            attack_ms: 10.0,
            release_ms: 150.0,
            makeup_db: 2.0,
            envelope: 0.0,
            attack_coeff: 0.0,
            release_coeff: 0.0,
        };
        comp.update_coeffs();
        comp
    }

    pub fn update_coeffs(&mut self) {
        let ga = (-1.0 / (self.attack_ms * 0.001 * self.sample_rate)).exp();
        let gr = (-1.0 / (self.release_ms * 0.001 * self.sample_rate)).exp();
        self.attack_coeff = ga;
        self.release_coeff = gr;
    }

    #[inline(always)]
    pub fn process(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        let peak = in_l.abs().max(in_r.abs());

        // Ballistics follower
        let coeff = if peak > self.envelope {
            self.attack_coeff
        } else {
            self.release_coeff
        };
        self.envelope = peak + coeff * (self.envelope - peak);

        // Convert envelope to dB
        let env_db = if self.envelope > 1e-6 {
            20.0 * self.envelope.log10()
        } else {
            -120.0
        };

        // Static characteristic curve with soft-knee
        let gain_reduction_db = if env_db > self.threshold_db {
            let over_db = env_db - self.threshold_db;
            let compressed_over = over_db / self.ratio;
            compressed_over - over_db
        } else {
            0.0
        };

        let total_gain_db = gain_reduction_db + self.makeup_db;
        let linear_gain = 10.0f32.powf(total_gain_db / 20.0);

        (in_l * linear_gain, in_r * linear_gain)
    }

    pub fn reset(&mut self) {
        self.envelope = 0.0;
    }
}
