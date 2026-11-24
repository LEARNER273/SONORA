use std::f32::consts::PI;

#[derive(Debug, Clone)]
pub struct StereoChorus {
    sample_rate: f32,
    buffer_l: Vec<f32>,
    buffer_r: Vec<f32>,
    write_pos: usize,
    buffer_len: usize,

    lfo_phase: f32,
    pub rate_hz: f32,
    pub depth: f32, // 0.0 to 1.0
    pub wet: f32,
}

impl StereoChorus {
    pub fn new(sample_rate: f32) -> Self {
        let max_samples = ((sample_rate * 0.05) as usize).max(2400); // 50ms buffer
        Self {
            sample_rate,
            buffer_l: vec![0.0; max_samples],
            buffer_r: vec![0.0; max_samples],
            write_pos: 0,
            buffer_len: max_samples,
            lfo_phase: 0.0,
            rate_hz: 1.4,
            depth: 0.5,
            wet: 0.0,
        }
    }

    #[inline(always)]
    pub fn process(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        if self.wet <= 0.001 {
            return (in_l, in_r);
        }

        self.buffer_l[self.write_pos] = in_l;
        self.buffer_r[self.write_pos] = in_r;

        // Base delay 15ms, modulation depth +/- 5ms
        let base_delay = 0.015 * self.sample_rate;
        let mod_range = 0.005 * self.sample_rate * self.depth;

        // Quadrature LFO (90 degree phase difference between L and R)
        let lfo_l = (self.lfo_phase).sin();
        let lfo_r = (self.lfo_phase + PI * 0.5).sin();

        let delay_l = base_delay + lfo_l * mod_range;
        let delay_r = base_delay + lfo_r * mod_range;

        // Interpolated read from circular buffers
        let out_l = self.read_interpolated(&self.buffer_l, delay_l);
        let out_r = self.read_interpolated(&self.buffer_r, delay_r);

        // Advance LFO
        let phase_inc = (2.0 * PI * self.rate_hz) / self.sample_rate;
        self.lfo_phase = (self.lfo_phase + phase_inc) % (2.0 * PI);

        self.write_pos = (self.write_pos + 1) % self.buffer_len;

        (
            in_l * (1.0 - self.wet) + out_l * self.wet,
            in_r * (1.0 - self.wet) + out_r * self.wet,
        )
    }

    #[inline(always)]
    fn read_interpolated(&self, buffer: &[f32], delay_samples: f32) -> f32 {
        let read_pos = (self.write_pos as f32) - delay_samples;
        let read_pos = if read_pos < 0.0 {
            read_pos + (self.buffer_len as f32)
        } else {
            read_pos
        };

        let idx0 = read_pos.floor() as usize % self.buffer_len;
        let idx1 = (idx0 + 1) % self.buffer_len;
        let frac = read_pos.fract();

        buffer[idx0] * (1.0 - frac) + buffer[idx1] * frac
    }

    pub fn reset(&mut self) {
        self.buffer_l.fill(0.0);
        self.buffer_r.fill(0.0);
        self.write_pos = 0;
        self.lfo_phase = 0.0;
    }
}
