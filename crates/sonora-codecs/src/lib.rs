pub mod wav;

pub use wav::{WavFormat, WavReader, WavWriter};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wav_roundtrip() {
        let sample_rate = 48000;
        let left: Vec<f32> = (0..480).map(|i| (i as f32 / 480.0) * 0.5).collect();
        let right: Vec<f32> = (0..480).map(|i| -(i as f32 / 480.0) * 0.5).collect();

        let encoded =
            WavWriter::encode_stereo(&left, &right, sample_rate, WavFormat::Pcm16).expect("encode");
        assert!(encoded.starts_with(b"RIFF"));

        let (dec_left, dec_right, dec_rate) = WavReader::decode(&encoded).expect("decode");
        assert_eq!(dec_rate, sample_rate);
        assert_eq!(dec_left.len(), left.len());
        // Verify audio values within 16-bit quantization error tolerance
        for i in 0..left.len() {
            assert!((dec_left[i] - left[i]).abs() < 0.001);
            assert!((dec_right[i] - right[i]).abs() < 0.001);
        }
    }
}
