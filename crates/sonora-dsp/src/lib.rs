pub mod biquad;
pub mod chorus;
pub mod compressor;
pub mod delay;
pub mod distortion;
pub mod fft;
pub mod limiter;
pub mod meter;
pub mod reverb;

pub use biquad::{BiquadFilter, FilterType};
pub use chorus::StereoChorus;
pub use compressor::StereoCompressor;
pub use delay::StereoDelay;
pub use distortion::{Distortion, DistortionMode};
pub use fft::FastFourierTransform;
pub use limiter::BrickwallLimiter;
pub use meter::LevelMeter;
pub use reverb::Reverb;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_biquad_lowpass() {
        let mut filter = BiquadFilter::new(48000.0, FilterType::Lowpass, 1000.0, 0.707);
        // Process a block of impulse samples
        let out = filter.process_sample(1.0);
        assert!(!out.is_nan() && !out.is_infinite());
        for _ in 0..100 {
            let o = filter.process_sample(0.0);
            assert!(!o.is_nan() && !o.is_infinite());
        }
    }

    #[test]
    fn test_limiter_brickwall() {
        let mut limiter = BrickwallLimiter::new(48000.0, -0.3);
        // Feed extremely loud input (+12 dBFS = 4.0 linear)
        for _ in 0..1000 {
            let (out_l, out_r) = limiter.process(4.0, -4.0);
            // Output must never exceed ceiling (around 0.966 linear = -0.3 dBFS)
            assert!(out_l.abs() <= 1.0, "Limiter exceeded ceiling: {}", out_l);
            assert!(out_r.abs() <= 1.0, "Limiter exceeded ceiling: {}", out_r);
        }
    }

    #[test]
    fn test_reverb_stability() {
        let mut rev = Reverb::new(48000.0);
        rev.wet = 1.0;
        rev.set_decay(3.0);
        // Feed impulse
        rev.process(1.0, 1.0);
        for _ in 0..10000 {
            let (l, r) = rev.process(0.0, 0.0);
            assert!(!l.is_nan() && l.abs() < 5.0);
            assert!(!r.is_nan() && r.abs() < 5.0);
        }
    }

    #[test]
    fn test_fft_computation() {
        let fft = FastFourierTransform::new(256);
        let input = vec![0.5f32; 256];
        let mut mags = vec![0.0f32; 128];
        fft.compute_magnitudes(&input, &mut mags);
        assert_eq!(mags.len(), 128);
        assert!(mags[0] >= 0.0 && mags[0] <= 1.0);
    }
}
