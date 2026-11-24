#[derive(Debug, Clone)]
pub struct StereoDelay {
    sample_rate: f32,
    buffer_left: Vec<f32>,
    buffer_right: Vec<f32>,
    write_pos: usize,
    buffer_len: usize,

    pub time_sec: f32,
    pub feedback: f32,
    pub wet: f32,
    pub ping_pong: bool,

    // Feedback damping lowpass filter states
    filter_state_l: f32,
    filter_state_r: f32,
}

impl StereoDelay {
    pub fn new(sample_rate: f32, max_delay_sec: f32) -> Self {
        let max_samples = ((sample_rate * max_delay_sec) as usize).max(4800);
        Self {
            sample_rate,
            buffer_left: vec![0.0; max_samples],
            buffer_right: vec![0.0; max_samples],
            write_pos: 0,
            buffer_len: max_samples,
            time_sec: 0.25,
            feedback: 0.35,
            wet: 0.0,
            ping_pong: true,
            filter_state_l: 0.0,
            filter_state_r: 0.0,
        }
    }

    #[inline(always)]
    pub fn process(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        if self.wet <= 0.001 {
            return (in_l, in_r);
        }

        let delay_samples =
            ((self.time_sec * self.sample_rate) as usize).clamp(1, self.buffer_len - 1);
        let read_pos = if self.write_pos >= delay_samples {
            self.write_pos - delay_samples
        } else {
            self.buffer_len + self.write_pos - delay_samples
        };

        let delayed_l = self.buffer_left[read_pos];
        let delayed_r = self.buffer_right[read_pos];

        // 6dB/oct lowpass damping on feedback to simulate analog tape decay
        self.filter_state_l += (delayed_l - self.filter_state_l) * 0.45;
        self.filter_state_r += (delayed_r - self.filter_state_r) * 0.45;

        let damped_l = self.filter_state_l;
        let damped_r = self.filter_state_r;

        let fb = self.feedback.clamp(0.0, 0.95);

        // Ping-pong cross feedback or stereo feedback
        let (new_write_l, new_write_r) = if self.ping_pong {
            (in_l + damped_r * fb, in_r + damped_l * fb)
        } else {
            (in_l + damped_l * fb, in_r + damped_r * fb)
        };

        // Soft clip feedback to avoid runaway
        self.buffer_left[self.write_pos] = new_write_l.clamp(-2.0, 2.0);
        self.buffer_right[self.write_pos] = new_write_r.clamp(-2.0, 2.0);

        self.write_pos = (self.write_pos + 1) % self.buffer_len;

        let out_l = in_l * (1.0 - self.wet) + delayed_l * self.wet;
        let out_r = in_r * (1.0 - self.wet) + delayed_r * self.wet;

        (out_l, out_r)
    }

    pub fn reset(&mut self) {
        self.buffer_left.fill(0.0);
        self.buffer_right.fill(0.0);
        self.filter_state_l = 0.0;
        self.filter_state_r = 0.0;
        self.write_pos = 0;
    }
}
