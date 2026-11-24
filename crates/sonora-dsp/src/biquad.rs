use std::f32::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FilterType {
    Lowpass,
    Highpass,
    Bandpass,
    Notch,
    Peaking,
    LowShelf,
    HighShelf,
}

#[derive(Debug, Clone)]
pub struct BiquadFilter {
    filter_type: FilterType,
    sample_rate: f32,
    cutoff: f32,
    q: f32,
    gain_db: f32,

    // Target values for parameter smoothing
    target_cutoff: f32,
    target_q: f32,

    // Normalized coefficients
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,

    // State registers (Direct Form II Transposed)
    s1: f32,
    s2: f32,
}

impl BiquadFilter {
    pub fn new(sample_rate: f32, filter_type: FilterType, cutoff: f32, q: f32) -> Self {
        let mut filter = Self {
            filter_type,
            sample_rate: sample_rate.max(8000.0),
            cutoff: cutoff.clamp(10.0, sample_rate * 0.49),
            q: q.clamp(0.01, 40.0),
            gain_db: 0.0,
            target_cutoff: cutoff,
            target_q: q,
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            s1: 0.0,
            s2: 0.0,
        };
        filter.recalculate();
        filter
    }

    pub fn set_cutoff(&mut self, cutoff: f32) {
        self.target_cutoff = cutoff.clamp(10.0, self.sample_rate * 0.49);
    }

    pub fn set_q(&mut self, q: f32) {
        self.target_q = q.clamp(0.01, 40.0);
    }

    pub fn set_type(&mut self, filter_type: FilterType) {
        self.filter_type = filter_type;
        self.recalculate();
    }

    pub fn reset_state(&mut self) {
        self.s1 = 0.0;
        self.s2 = 0.0;
    }

    fn recalculate(&mut self) {
        let w0 = 2.0 * PI * self.cutoff / self.sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * self.q);

        let (b0, b1, b2, a0, a1, a2) = match self.filter_type {
            FilterType::Lowpass => {
                let b1 = 1.0 - cos_w0;
                let b0 = b1 * 0.5;
                let b2 = b0;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_w0;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::Highpass => {
                let b1 = -(1.0 + cos_w0);
                let b0 = (1.0 + cos_w0) * 0.5;
                let b2 = b0;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_w0;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::Bandpass => {
                let b0 = alpha;
                let b1 = 0.0;
                let b2 = -alpha;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_w0;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::Notch => {
                let b0 = 1.0;
                let b1 = -2.0 * cos_w0;
                let b2 = 1.0;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_w0;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::Peaking => {
                let a = 10.0f32.powf(self.gain_db / 40.0);
                let b0 = 1.0 + alpha * a;
                let b1 = -2.0 * cos_w0;
                let b2 = 1.0 - alpha * a;
                let a0 = 1.0 + alpha / a;
                let a1 = -2.0 * cos_w0;
                let a2 = 1.0 - alpha / a;
                (b0, b1, b2, a0, a1, a2)
            }
            _ => (1.0, 0.0, 0.0, 1.0, 0.0, 0.0),
        };

        let inv_a0 = 1.0 / a0;
        self.b0 = b0 * inv_a0;
        self.b1 = b1 * inv_a0;
        self.b2 = b2 * inv_a0;
        self.a1 = a1 * inv_a0;
        self.a2 = a2 * inv_a0;
    }

    #[inline(always)]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        // Parameter smoothing (slew rate ~0.005 per sample = smooth ~20ms response)
        if (self.cutoff - self.target_cutoff).abs() > 0.01 || (self.q - self.target_q).abs() > 0.001
        {
            self.cutoff += (self.target_cutoff - self.cutoff) * 0.005;
            self.q += (self.target_q - self.q) * 0.005;
            self.recalculate();
        }

        // Direct Form II Transposed
        let out = self.b0 * input + self.s1;
        self.s1 = self.b1 * input - self.a1 * out + self.s2;
        self.s2 = self.b2 * input - self.a2 * out;

        if out.is_nan() || out.is_infinite() {
            self.reset_state();
            0.0
        } else {
            out
        }
    }
}
