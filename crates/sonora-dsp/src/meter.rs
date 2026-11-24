#[derive(Debug, Clone)]
pub struct LevelMeter {
    peak_l: f32,
    peak_r: f32,
    rms_acc_l: f32,
    rms_acc_r: f32,
    sample_count: usize,
    decay_coeff: f32,
}

impl LevelMeter {
    pub fn new(sample_rate: f32) -> Self {
        let decay_ms = 300.0;
        let decay_coeff = (-1.0 / (decay_ms * 0.001 * sample_rate)).exp();
        Self {
            peak_l: 0.0,
            peak_r: 0.0,
            rms_acc_l: 0.0,
            rms_acc_r: 0.0,
            sample_count: 0,
            decay_coeff,
        }
    }

    #[inline(always)]
    pub fn process_sample(&mut self, l: f32, r: f32) {
        let abs_l = l.abs();
        let abs_r = r.abs();

        if abs_l > self.peak_l {
            self.peak_l = abs_l;
        } else {
            self.peak_l = abs_l + self.decay_coeff * (self.peak_l - abs_l);
        }

        if abs_r > self.peak_r {
            self.peak_r = abs_r;
        } else {
            self.peak_r = abs_r + self.decay_coeff * (self.peak_r - abs_r);
        }

        self.rms_acc_l += l * l;
        self.rms_acc_r += r * r;
        self.sample_count += 1;
    }

    pub fn read_and_reset(&mut self) -> (f32, f32, f32, f32) {
        let count = self.sample_count.max(1) as f32;
        let rms_l = (self.rms_acc_l / count).sqrt();
        let rms_r = (self.rms_acc_r / count).sqrt();

        let peak_l_norm = self.linear_to_normalized(self.peak_l);
        let peak_r_norm = self.linear_to_normalized(self.peak_r);
        let rms_l_norm = self.linear_to_normalized(rms_l);
        let rms_r_norm = self.linear_to_normalized(rms_r);

        self.rms_acc_l = 0.0;
        self.rms_acc_r = 0.0;
        self.sample_count = 0;

        (peak_l_norm, peak_r_norm, rms_l_norm, rms_r_norm)
    }

    fn linear_to_normalized(&self, val: f32) -> f32 {
        if val <= 1e-4 {
            0.0
        } else {
            let db = 20.0 * val.log10();
            ((db + 60.0) / 60.0).clamp(0.0, 1.0)
        }
    }
}
