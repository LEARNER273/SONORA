#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DistortionMode {
    SoftClip,
    Overdrive,
    Tube,
    HardClip,
}

#[derive(Debug, Clone)]
pub struct Distortion {
    pub wet: f32,
    pub drive: f32, // 0.0 to 1.0
    pub mode: DistortionMode,
}

impl Distortion {
    pub fn new() -> Self {
        Self {
            wet: 0.0,
            drive: 0.25,
            mode: DistortionMode::SoftClip,
        }
    }

    #[inline(always)]
    pub fn process_sample(&self, input: f32) -> f32 {
        if self.wet <= 0.001 {
            return input;
        }

        let gain = 1.0 + self.drive * 12.0;
        let x = input * gain;

        let shaped = match self.mode {
            DistortionMode::SoftClip => {
                // Hyperbolic tangent saturation
                x.tanh()
            }
            DistortionMode::Overdrive => {
                // Cubic soft clipper: x - x^3 / 3
                if x < -1.5 {
                    -1.0
                } else if x > 1.5 {
                    1.0
                } else {
                    x - (x * x * x) / 3.0
                }
            }
            DistortionMode::Tube => {
                // Asymmetric tube warmth
                if x > 0.0 {
                    1.0 - (-x).exp()
                } else {
                    -(-x).tanh()
                }
            }
            DistortionMode::HardClip => x.clamp(-1.0, 1.0),
        };

        // Makeup normalization
        let norm_factor = 1.0 / (1.0 + self.drive * 3.0);
        let distorted = shaped * norm_factor;

        input * (1.0 - self.wet) + distorted * self.wet
    }
}

impl Default for Distortion {
    fn default() -> Self {
        Self::new()
    }
}
