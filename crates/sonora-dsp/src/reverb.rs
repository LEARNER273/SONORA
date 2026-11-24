// Freeverb Algorithmic Reverberator
// 8 parallel Feedback Comb Filters (FBCF) + 4 series Allpass Diffusers (APF) per channel

const COMB_TUNING_L: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASS_TUNING_L: [usize; 4] = [556, 441, 341, 225];
const STEREO_SPREAD: usize = 23;

#[derive(Debug, Clone)]
struct CombFilter {
    buffer: Vec<f32>,
    buf_size: usize,
    buf_idx: usize,
    feedback: f32,
    filter_store: f32,
    damp: f32,
}

impl CombFilter {
    fn new(size: usize) -> Self {
        Self {
            buffer: vec![0.0; size],
            buf_size: size,
            buf_idx: 0,
            feedback: 0.8,
            filter_store: 0.0,
            damp: 0.2,
        }
    }

    #[inline(always)]
    fn process(&mut self, input: f32) -> f32 {
        let output = self.buffer[self.buf_idx];
        self.filter_store = output * (1.0 - self.damp) + self.filter_store * self.damp;
        self.buffer[self.buf_idx] = input + self.filter_store * self.feedback;
        self.buf_idx = (self.buf_idx + 1) % self.buf_size;
        output
    }

    fn clear(&mut self) {
        self.buffer.fill(0.0);
        self.filter_store = 0.0;
    }
}

#[derive(Debug, Clone)]
struct AllpassFilter {
    buffer: Vec<f32>,
    buf_size: usize,
    buf_idx: usize,
    feedback: f32,
}

impl AllpassFilter {
    fn new(size: usize) -> Self {
        Self {
            buffer: vec![0.0; size],
            buf_size: size,
            buf_idx: 0,
            feedback: 0.5,
        }
    }

    #[inline(always)]
    fn process(&mut self, input: f32) -> f32 {
        let buf_out = self.buffer[self.buf_idx];
        let output = -input + buf_out;
        self.buffer[self.buf_idx] = input + buf_out * self.feedback;
        self.buf_idx = (self.buf_idx + 1) % self.buf_size;
        output
    }

    fn clear(&mut self) {
        self.buffer.fill(0.0);
    }
}

#[derive(Debug, Clone)]
pub struct Reverb {
    comb_l: Vec<CombFilter>,
    comb_r: Vec<CombFilter>,
    allpass_l: Vec<AllpassFilter>,
    allpass_r: Vec<AllpassFilter>,

    pub wet: f32,
    pub room_size: f32,
    pub damping: f32,
}

impl Reverb {
    pub fn new(sample_rate: f32) -> Self {
        let scale = sample_rate / 44100.0;

        let comb_l = COMB_TUNING_L
            .iter()
            .map(|&len| CombFilter::new(((len as f32) * scale).round() as usize))
            .collect();

        let comb_r = COMB_TUNING_L
            .iter()
            .map(|&len| CombFilter::new((((len + STEREO_SPREAD) as f32) * scale).round() as usize))
            .collect();

        let allpass_l = ALLPASS_TUNING_L
            .iter()
            .map(|&len| AllpassFilter::new(((len as f32) * scale).round() as usize))
            .collect();

        let allpass_r = ALLPASS_TUNING_L
            .iter()
            .map(|&len| {
                AllpassFilter::new((((len + STEREO_SPREAD) as f32) * scale).round() as usize)
            })
            .collect();

        let mut rev = Self {
            comb_l,
            comb_r,
            allpass_l,
            allpass_r,
            wet: 0.15,
            room_size: 0.75,
            damping: 0.25,
        };
        rev.update_params();
        rev
    }

    pub fn set_decay(&mut self, decay: f32) {
        // Map decay (0.1 .. 10.0 sec) to room_size (0.4 .. 0.96)
        self.room_size = (0.4 + (decay / 10.0) * 0.56).clamp(0.2, 0.96);
        self.update_params();
    }

    pub fn set_damping(&mut self, damp: f32) {
        self.damping = damp.clamp(0.0, 1.0);
        self.update_params();
    }

    fn update_params(&mut self) {
        let fb = self.room_size;
        let damp = self.damping;
        for c in &mut self.comb_l {
            c.feedback = fb;
            c.damp = damp;
        }
        for c in &mut self.comb_r {
            c.feedback = fb;
            c.damp = damp;
        }
    }

    #[inline(always)]
    pub fn process(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        if self.wet <= 0.001 {
            return (in_l, in_r);
        }

        let input_mixed = (in_l + in_r) * 0.015;

        // Parallel Comb filters
        let mut out_l = 0.0f32;
        let mut out_r = 0.0f32;

        for c in &mut self.comb_l {
            out_l += c.process(input_mixed);
        }
        for c in &mut self.comb_r {
            out_r += c.process(input_mixed);
        }

        // Series Allpass filters
        for a in &mut self.allpass_l {
            out_l = a.process(out_l);
        }
        for a in &mut self.allpass_r {
            out_r = a.process(out_r);
        }

        let wet_gain = self.wet * 3.0;
        let dry_gain = 1.0 - (self.wet * 0.5);

        (
            in_l * dry_gain + out_l * wet_gain,
            in_r * dry_gain + out_r * wet_gain,
        )
    }

    pub fn reset(&mut self) {
        for c in &mut self.comb_l {
            c.clear();
        }
        for c in &mut self.comb_r {
            c.clear();
        }
        for a in &mut self.allpass_l {
            a.clear();
        }
        for a in &mut self.allpass_r {
            a.clear();
        }
    }
}
