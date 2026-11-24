pub mod bass;
pub mod drums;
pub mod guitar;
pub mod piano;
pub mod synth;

pub use bass::{BassEngine, BassPreset};
pub use drums::{DrumEngine, DrumKit};
pub use guitar::{GuitarEngine, GuitarPreset};
pub use piano::{midi_to_freq, PianoEngine, PianoPreset};
pub use synth::{SynthEngine, SynthPreset};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_piano_polyphony() {
        let mut piano = PianoEngine::new(48000.0);
        piano.note_on(60, 100);
        piano.note_on(64, 100);
        piano.note_on(67, 100);

        for _ in 0..1000 {
            let (l, r) = piano.process_sample();
            assert!(!l.is_nan() && !r.is_nan());
        }
        piano.note_off(60);
        piano.note_off(64);
        piano.note_off(67);
    }

    #[test]
    fn test_guitar_karplus_strong() {
        let mut guitar = GuitarEngine::new(48000.0);
        guitar.note_on(64, 110);
        for _ in 0..2000 {
            let (l, r) = guitar.process_sample();
            assert!(!l.is_nan() && !r.is_nan());
        }
    }

    #[test]
    fn test_drum_channels() {
        let mut drums = DrumEngine::new(48000.0);
        for ch in 0..8 {
            drums.trigger_channel(ch, 100);
            for _ in 0..500 {
                let (l, r) = drums.process_sample();
                assert!(!l.is_nan() && !r.is_nan());
            }
        }
    }
}
