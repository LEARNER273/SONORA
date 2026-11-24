use sonora_core::{ClipData, SongSession, Track, TICKS_PER_BEAT};

#[derive(Debug, Clone, PartialEq)]
pub struct RawMidiEvent {
    pub tick: u64,
    pub data: Vec<u8>,
}

pub struct MidiFile;

impl MidiFile {
    /// Encode a variable-length quantity into standard MIDI bytes
    fn write_var_len(val: u64, out: &mut Vec<u8>) {
        let mut buffer = [0u8; 8];
        let mut count = 0;
        let mut v = val;

        buffer[count] = (v & 0x7F) as u8;
        v >>= 7;

        while v > 0 {
            count += 1;
            buffer[count] = ((v & 0x7F) | 0x80) as u8;
            v >>= 7;
        }

        while count > 0 {
            out.push(buffer[count]);
            count -= 1;
        }
        out.push(buffer[0]);
    }

    /// Serializes a complete SongSession into Standard MIDI File Format 1 binary bytes
    pub fn export_smf(session: &SongSession) -> Vec<u8> {
        let mut out = Vec::new();

        // Count tracks with MIDI or Drum data
        let valid_tracks: Vec<&Track> = session
            .tracks
            .iter()
            .filter(|t| {
                t.clips.iter().any(|c| match &c.data {
                    ClipData::Midi(notes) => !notes.is_empty(),
                    ClipData::DrumPattern(p) => !p.steps.is_empty(),
                    _ => false,
                })
            })
            .collect();

        let num_tracks = (valid_tracks.len() + 1) as u16; // Conductor track + Instrument tracks

        // 1. MThd Header Chunk
        out.extend_from_slice(b"MThd");
        out.extend_from_slice(&6u32.to_be_bytes()); // Chunk length = 6
        out.extend_from_slice(&1u16.to_be_bytes()); // Format 1 (synchronous multitrack)
        out.extend_from_slice(&num_tracks.to_be_bytes());
        out.extend_from_slice(&(TICKS_PER_BEAT as u16).to_be_bytes()); // PPQN = 960

        // 2. Track 0: Conductor Track (Tempo & Time Signature)
        let mut track0_data = Vec::new();

        // Time Signature: 00 FF 58 04 nn dd cc bb
        track0_data.push(0x00); // delta 0
        track0_data.extend_from_slice(&[0xFF, 0x58, 0x04]);
        track0_data.push(session.time_signature.numerator);
        let denom_exp = (session.time_signature.denominator as f32).log2().round() as u8;
        track0_data.push(denom_exp);
        track0_data.push(24); // 24 MIDI clocks per metronome click
        track0_data.push(8); // 8 32nd notes per quarter note

        // Tempo: microseconds per quarter note = 60,000,000 / BPM
        let us_per_quarter = (60_000_000.0 / session.bpm.max(20.0)) as u32;
        track0_data.push(0x00); // delta 0
        track0_data.extend_from_slice(&[0xFF, 0x51, 0x03]);
        track0_data.push((us_per_quarter >> 16) as u8);
        track0_data.push((us_per_quarter >> 8) as u8);
        track0_data.push(us_per_quarter as u8);

        // Track 0 End of Track
        track0_data.push(0x00);
        track0_data.extend_from_slice(&[0xFF, 0x2F, 0x00]);

        // Write Track 0 Header & Body
        out.extend_from_slice(b"MTrk");
        out.extend_from_slice(&(track0_data.len() as u32).to_be_bytes());
        out.extend_from_slice(&track0_data);

        // 3. Instrument Tracks
        for (i, track) in valid_tracks.iter().enumerate() {
            let channel = (i as u8) % 16;
            let mut track_data = Vec::new();

            // Track Name
            track_data.push(0x00); // delta 0
            track_data.extend_from_slice(&[0xFF, 0x03]);
            track_data.push(track.name.len() as u8);
            track_data.extend_from_slice(track.name.as_bytes());

            // Collect all note events
            let mut events: Vec<RawMidiEvent> = Vec::new();

            for clip in &track.clips {
                let clip_start = clip.start_tick;
                match &clip.data {
                    ClipData::Midi(notes) => {
                        for note in notes {
                            let on_tick = clip_start + note.start_tick;
                            let off_tick = on_tick + note.duration_ticks;

                            events.push(RawMidiEvent {
                                tick: on_tick,
                                data: vec![0x90 | channel, note.note_number, note.velocity],
                            });
                            events.push(RawMidiEvent {
                                tick: off_tick,
                                data: vec![0x80 | channel, note.note_number, 0],
                            });
                        }
                    }
                    ClipData::DrumPattern(pat) => {
                        let step_ticks = TICKS_PER_BEAT / 4;
                        for step in &pat.steps {
                            let on_tick = clip_start + (step.step as u64 * step_ticks);
                            let off_tick = on_tick + (step_ticks / 2);

                            // General MIDI drum map (Kick = 36, Snare = 38, Closed Hat = 42, etc.)
                            let drum_note = match step.channel {
                                0 => 36, // Bass Drum 1
                                1 => 38, // Acoustic Snare
                                2 => 42, // Closed Hi-Hat
                                3 => 46, // Open Hi-Hat
                                4 => 50, // High Tom
                                5 => 45, // Low Tom
                                6 => 49, // Crash Cymbal
                                7 => 39, // Hand Clap
                                _ => 36,
                            };

                            events.push(RawMidiEvent {
                                tick: on_tick,
                                data: vec![0x99, drum_note, step.velocity], // Channel 10 (0x99) for drums
                            });
                            events.push(RawMidiEvent {
                                tick: off_tick,
                                data: vec![0x89, drum_note, 0],
                            });
                        }
                    }
                    _ => {}
                }
            }

            // Sort events by tick time (note offs before note ons if same tick)
            events.sort_by(|a, b| {
                if a.tick == b.tick {
                    let a_is_off = (a.data[0] & 0xF0) == 0x80;
                    let b_is_off = (b.data[0] & 0xF0) == 0x80;
                    b_is_off.cmp(&a_is_off)
                } else {
                    a.tick.cmp(&b.tick)
                }
            });

            // Write delta-time encoded events
            let mut last_tick = 0u64;
            for ev in events {
                let delta = ev.tick.saturating_sub(last_tick);
                last_tick = ev.tick;
                Self::write_var_len(delta, &mut track_data);
                track_data.extend_from_slice(&ev.data);
            }

            // End of Track event
            Self::write_var_len(0, &mut track_data);
            track_data.extend_from_slice(&[0xFF, 0x2F, 0x00]);

            // Append Track Chunk
            out.extend_from_slice(b"MTrk");
            out.extend_from_slice(&(track_data.len() as u32).to_be_bytes());
            out.extend_from_slice(&track_data);
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sonora_core::{Clip, MidiNote, SongSession};

    #[test]
    fn test_smf_export() {
        let mut session = SongSession::new("MIDI Test");
        let notes = vec![
            MidiNote::new(60, 0, 480, 100),
            MidiNote::new(64, 480, 480, 100),
        ];
        session.tracks[0]
            .clips
            .push(Clip::new_midi("Melody", 0, 960, notes));

        let smf_bytes = MidiFile::export_smf(&session);

        // Verify MThd header
        assert!(smf_bytes.starts_with(b"MThd"));
        assert!(smf_bytes.len() > 30);
    }
}
