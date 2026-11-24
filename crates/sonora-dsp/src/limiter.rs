#[derive(Debug, Clone)]
pub struct BrickwallLimiter {
    ceiling_linear: f32,
    release_coeff: f32,

    // Lookahead ring buffers (5 ms lookahead)
    lookahead_len: usize,
    buffer_l: Vec<f32>,
    buffer_r: Vec<f32>,
    write_idx: usize,

    current_gain: f32,
}

impl BrickwallLimiter {
    pub fn new(sample_rate: f32, ceiling_db: f32) -> Self {
        let lookahead_samples = ((sample_rate * 0.005) as usize).max(64); // 5 ms
        let ceiling_linear = 10.0f32.powf(ceiling_db.min(0.0) / 20.0);
        let release_ms = 80.0;
        let release_coeff = (-1.0 / (release_ms * 0.001 * sample_rate)).exp();

        Self {
            ceiling_linear,
            release_coeff,
            lookahead_len: lookahead_samples,
            buffer_l: vec![0.0; lookahead_samples],
            buffer_r: vec![0.0; lookahead_samples],
            write_idx: 0,
            current_gain: 1.0,
        }
    }

    #[inline(always)]
    pub fn process(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        // Write incoming audio to lookahead delay
        self.buffer_l[self.write_idx] = in_l;
        self.buffer_r[self.write_idx] = in_r;

        let read_idx = (self.write_idx + 1) % self.lookahead_len;
        let delayed_l = self.buffer_l[read_idx];
        let delayed_r = self.buffer_r[read_idx];

        // Measure future peak from incoming sample
        let peak = in_l.abs().max(in_r.abs());
        let target_gain = if peak > self.ceiling_linear && peak > 1e-6 {
            self.ceiling_linear / peak
        } else {
            1.0
        };

        // Instant attack (if target gain is lower, apply immediately), smooth release
        if target_gain < self.current_gain {
            self.current_gain = target_gain;
        } else {
            self.current_gain =
                target_gain + self.release_coeff * (self.current_gain - target_gain);
        }

        self.write_idx = read_idx;

        let out_l =
            (delayed_l * self.current_gain).clamp(-self.ceiling_linear, self.ceiling_linear);
        let out_r =
            (delayed_r * self.current_gain).clamp(-self.ceiling_linear, self.ceiling_linear);

        (out_l, out_r)
    }

    pub fn reset(&mut self) {
        self.buffer_l.fill(0.0);
        self.buffer_r.fill(0.0);
        self.write_idx = 0;
        self.current_gain = 1.0;
    }
}
