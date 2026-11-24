use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const TICKS_PER_BEAT: u64 = 960;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeSignature {
    pub numerator: u8,
    pub denominator: u8,
}

impl Default for TimeSignature {
    fn default() -> Self {
        Self {
            numerator: 4,
            denominator: 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrackType {
    Piano,
    Guitar,
    Bass,
    Synth,
    Drums,
    Audio,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MidiNote {
    pub note_number: u8,
    pub start_tick: u64,
    pub duration_ticks: u64,
    pub velocity: u8,
}

impl MidiNote {
    pub fn new(note_number: u8, start_tick: u64, duration_ticks: u64, velocity: u8) -> Self {
        Self {
            note_number,
            start_tick,
            duration_ticks,
            velocity: velocity.clamp(1, 127),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrumStep {
    pub channel: u8, // 0 = Kick, 1 = Snare, 2 = Closed Hat, 3 = Open Hat, 4 = High Tom, 5 = Low Tom, 6 = Crash, 7 = Clap
    pub step: u32,
    pub velocity: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrumPatternData {
    pub step_count: u32,
    pub kit_name: String,
    pub steps: Vec<DrumStep>,
}

impl Default for DrumPatternData {
    fn default() -> Self {
        Self {
            step_count: 16,
            kit_name: "studio".to_string(),
            steps: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioClipData {
    pub file_path: String,
    pub offset_samples: u64,
    pub length_samples: u64,
    pub gain: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum ClipData {
    Midi(Vec<MidiNote>),
    DrumPattern(DrumPatternData),
    Audio(AudioClipData),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Clip {
    pub id: String,
    pub name: String,
    pub start_tick: u64,
    pub duration_ticks: u64,
    pub data: ClipData,
}

impl Clip {
    pub fn new_midi(
        name: impl Into<String>,
        start_tick: u64,
        duration_ticks: u64,
        notes: Vec<MidiNote>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            start_tick,
            duration_ticks,
            data: ClipData::Midi(notes),
        }
    }

    pub fn new_drum(
        name: impl Into<String>,
        start_tick: u64,
        duration_ticks: u64,
        pattern: DrumPatternData,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            start_tick,
            duration_ticks,
            data: ClipData::DrumPattern(pattern),
        }
    }

    pub fn new_audio(
        name: impl Into<String>,
        start_tick: u64,
        duration_ticks: u64,
        audio: AudioClipData,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            start_tick,
            duration_ticks,
            data: ClipData::Audio(audio),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub id: String,
    pub name: String,
    pub track_type: TrackType,
    pub muted: bool,
    pub soloed: bool,
    pub volume_db: f32,
    pub pan: f32, // -1.0 to +1.0
    pub preset_name: String,
    pub clips: Vec<Clip>,
}

impl Track {
    pub fn new(
        name: impl Into<String>,
        track_type: TrackType,
        preset_name: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            track_type,
            muted: false,
            soloed: false,
            volume_db: 0.0,
            pan: 0.0,
            preset_name: preset_name.into(),
            clips: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MasterEffectsState {
    pub chorus_wet: f32,
    pub chorus_rate: f32,
    pub chorus_depth: f32,
    pub distortion_wet: f32,
    pub distortion_amount: f32,
    pub filter_cutoff: f32,
    pub filter_q: f32,
    pub delay_wet: f32,
    pub delay_time_sec: f32,
    pub delay_feedback: f32,
    pub reverb_wet: f32,
    pub reverb_decay: f32,
    pub reverb_predelay: f32,
    pub compressor_threshold: f32,
    pub compressor_ratio: f32,
    pub limiter_ceiling_db: f32,
}

impl Default for MasterEffectsState {
    fn default() -> Self {
        Self {
            chorus_wet: 0.0,
            chorus_rate: 1.4,
            chorus_depth: 0.5,
            distortion_wet: 0.0,
            distortion_amount: 0.25,
            filter_cutoff: 20000.0,
            filter_q: 0.707,
            delay_wet: 0.0,
            delay_time_sec: 0.25,
            delay_feedback: 0.35,
            reverb_wet: 0.15,
            reverb_decay: 2.2,
            reverb_predelay: 0.015,
            compressor_threshold: -18.0,
            compressor_ratio: 3.0,
            limiter_ceiling_db: -0.3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SongSession {
    pub id: String,
    pub name: String,
    pub bpm: f32,
    pub time_signature: TimeSignature,
    pub swing: f32,
    pub loop_enabled: bool,
    pub loop_start_bar: u32,
    pub loop_end_bar: u32,
    pub master_volume_db: f32,
    pub master_effects: MasterEffectsState,
    pub tracks: Vec<Track>,
}

impl Default for SongSession {
    fn default() -> Self {
        let mut session = Self {
            id: Uuid::new_v4().to_string(),
            name: "New Project".to_string(),
            bpm: 120.0,
            time_signature: TimeSignature::default(),
            swing: 0.0,
            loop_enabled: true,
            loop_start_bar: 0,
            loop_end_bar: 8,
            master_volume_db: 0.0,
            master_effects: MasterEffectsState::default(),
            tracks: Vec::new(),
        };

        // Create standard production default tracks
        session
            .tracks
            .push(Track::new("Piano", TrackType::Piano, "grand"));
        session
            .tracks
            .push(Track::new("Drums", TrackType::Drums, "studio"));
        session
            .tracks
            .push(Track::new("Bass", TrackType::Bass, "electric"));
        session
            .tracks
            .push(Track::new("Synth", TrackType::Synth, "pluto"));
        session
            .tracks
            .push(Track::new("Guitar", TrackType::Guitar, "acoustic"));

        // Starter loop: 8 bars of real, editable content (I–V–vi–IV in C) so
        // the first press of Play makes music. Notes end 20 ticks before each
        // bar line so note-offs never land exactly on the loop wrap point.
        let tpb = TICKS_PER_BEAT; // 960
        let bar = tpb * 4; // 4/4
        let total = bar * 8;

        let piano_voices: [[u8; 3]; 8] = [
            [60, 64, 67],
            [59, 62, 67],
            [57, 60, 64],
            [57, 60, 65], // C  G  Am F
            [60, 64, 67],
            [59, 62, 67],
            [57, 60, 64],
            [57, 60, 65], // C  G  Am F
        ];
        let guitar_voices: [[u8; 3]; 8] = [
            [48, 55, 64],
            [43, 55, 62],
            [45, 52, 60],
            [41, 53, 60],
            [48, 55, 64],
            [43, 55, 62],
            [45, 52, 60],
            [41, 53, 60],
        ];
        let bass_roots: [u8; 8] = [36, 31, 33, 29, 36, 31, 33, 29]; // C2 G1 A1 F1

        // Piano — one whole-bar chord per bar
        let mut piano_notes = Vec::new();
        for (b, voices) in piano_voices.iter().enumerate() {
            for &n in voices {
                piano_notes.push(MidiNote::new(n, b as u64 * bar, bar - 20, 85));
            }
        }
        session.tracks[0].clips.push(Clip::new_midi(
            "Grand Chord Progression",
            0,
            total,
            piano_notes,
        ));

        // Drums — 16-step pattern across all 8 bars, crash on the first downbeat
        let mut steps = Vec::new();
        for b in 0..8u32 {
            let base = b * 16;
            for &s in &[0u32, 8] {
                steps.push(DrumStep {
                    channel: 0,
                    step: base + s,
                    velocity: 110,
                }); // kick
            }
            for &s in &[4u32, 12] {
                steps.push(DrumStep {
                    channel: 1,
                    step: base + s,
                    velocity: 100,
                }); // snare
            }
            for s in (0..16).step_by(2) {
                steps.push(DrumStep {
                    channel: 2,
                    step: base + s,
                    velocity: 55,
                }); // closed hat
            }
            if b % 2 == 1 {
                steps.push(DrumStep {
                    channel: 3,
                    step: base + 14,
                    velocity: 70,
                }); // open hat
            }
            if b == 0 {
                steps.push(DrumStep {
                    channel: 6,
                    step: 0,
                    velocity: 90,
                }); // crash
            }
        }
        session.tracks[1].clips.push(Clip::new_drum(
            "Studio 16-Step Beat",
            0,
            total,
            DrumPatternData {
                step_count: 16,
                kit_name: "studio".to_string(),
                steps,
            },
        ));

        // Bass — quarter-note roots
        let mut bass_notes = Vec::new();
        for (b, &root) in bass_roots.iter().enumerate() {
            for beat in 0..4u64 {
                bass_notes.push(MidiNote::new(
                    root,
                    b as u64 * bar + beat * tpb,
                    tpb - 20,
                    90,
                ));
            }
        }
        session.tracks[2].clips.push(Clip::new_midi(
            "Electric Root Bassline",
            0,
            total,
            bass_notes,
        ));

        // Synth — sparse lead hook over the first 4 bars
        let synth_notes = vec![
            MidiNote::new(76, 0, 1900, 90),              // E5
            MidiNote::new(79, 1920, 1900, 90),           // G5
            MidiNote::new(74, bar, 3800, 90),            // D5
            MidiNote::new(72, bar * 2, 1900, 90),        // C5
            MidiNote::new(76, bar * 2 + 1920, 1900, 90), // E5
            MidiNote::new(69, bar * 3, 3800, 90),        // A4
        ];
        session.tracks[3]
            .clips
            .push(Clip::new_midi("Pluto Lead Hook", 0, total, synth_notes));

        // Guitar — two strums per bar with a 20-tick stagger between strings
        let mut guitar_notes = Vec::new();
        for (b, voices) in guitar_voices.iter().enumerate() {
            for &strum_tick in &[0u64, 1920] {
                for (k, &n) in voices.iter().enumerate() {
                    let start = b as u64 * bar + strum_tick + k as u64 * 20;
                    guitar_notes.push(MidiNote::new(n, start, 1900 - k as u64 * 20, 75));
                }
            }
        }
        session.tracks[4]
            .clips
            .push(Clip::new_midi("Acoustic Strum", 0, total, guitar_notes));

        session
    }
}

impl SongSession {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    pub fn ticks_per_bar(&self) -> u64 {
        TICKS_PER_BEAT * self.time_signature.numerator as u64
    }

    pub fn total_duration_ticks(&self) -> u64 {
        let mut max_tick = self.loop_end_bar as u64 * self.ticks_per_bar();
        for track in &self.tracks {
            for clip in &track.clips {
                let end = clip.start_tick + clip.duration_ticks;
                if end > max_tick {
                    max_tick = end;
                }
            }
        }
        max_tick
    }

    pub fn ticks_to_seconds(&self, ticks: u64) -> f64 {
        let beats = ticks as f64 / TICKS_PER_BEAT as f64;
        let seconds_per_beat = 60.0 / self.bpm as f64;
        beats * seconds_per_beat
    }

    pub fn seconds_to_ticks(&self, seconds: f64) -> u64 {
        let seconds_per_beat = 60.0 / self.bpm as f64;
        let beats = seconds / seconds_per_beat;
        (beats * TICKS_PER_BEAT as f64).round() as u64
    }

    pub fn ticks_to_frames(&self, ticks: u64, sample_rate: u32) -> u64 {
        let seconds = self.ticks_to_seconds(ticks);
        (seconds * sample_rate as f64).round() as u64
    }

    pub fn frames_to_ticks(&self, frames: u64, sample_rate: u32) -> u64 {
        let seconds = frames as f64 / sample_rate as f64;
        self.seconds_to_ticks(seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timing_conversions() {
        let session = SongSession::default();
        assert_eq!(session.bpm, 120.0);
        // At 120 BPM, 1 beat = 0.5s, 1 bar (4 beats) = 2.0s = 3840 ticks
        assert_eq!(session.ticks_per_bar(), 3840);
        let sec = session.ticks_to_seconds(3840);
        assert!((sec - 2.0).abs() < 1e-6);

        let ticks = session.seconds_to_ticks(2.0);
        assert_eq!(ticks, 3840);

        let frames = session.ticks_to_frames(3840, 48000);
        assert_eq!(frames, 96000); // 2.0 seconds * 48000
    }

    #[test]
    fn test_session_serialization() {
        let session = SongSession::new("Test Song");
        let json = serde_json::to_string_pretty(&session).expect("serialize");
        let restored: SongSession = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(session.name, restored.name);
        assert_eq!(session.tracks.len(), restored.tracks.len());
    }
}
