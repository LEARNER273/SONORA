use sonora_core::SongSession;

#[derive(Debug, Clone)]
pub enum AudioEngineCommand {
    Play,
    Pause,
    Stop,
    SeekTick(u64),
    LoadSession(SongSession),
    NoteOn {
        track_idx: usize,
        note: u8,
        velocity: u8,
    },
    NoteOff {
        track_idx: usize,
        note: u8,
    },
    TriggerDrum {
        channel: u8,
        velocity: u8,
    },
    SetTrackVolume {
        track_idx: usize,
        volume_db: f32,
    },
    SetTrackPan {
        track_idx: usize,
        pan: f32,
    },
    SetTrackMute {
        track_idx: usize,
        muted: bool,
    },
    SetTrackSolo {
        track_idx: usize,
        soloed: bool,
    },
    SetMasterVolume(f32),
    SetMetronomeEnabled(bool),
    SetMetronomeVolume(f32),
    SetSustainPedal(bool),
    SetBpm(f32),
    SetLoop {
        enabled: bool,
        start_bar: u32,
        end_bar: u32,
    },
    SetDrumStep {
        clip_idx: usize,
        channel: u8,
        step: u32,
        active: bool,
        velocity: u8,
    },
    AddMidiNotes {
        track_idx: usize,
        notes: Vec<sonora_core::MidiNote>,
    },
    /// Select an instrument preset. `instrument`: 0=piano, 1=guitar, 2=bass, 3=synth, 4=drum kit.
    SetPreset {
        instrument: u8,
        preset: String,
    },
    /// Global synth LFO (Hz + depth 0..1).
    SetSynthLfo {
        rate_hz: f32,
        depth: f32,
    },
}
