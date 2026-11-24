use sonora_audio::{AudioEngine, AudioEngineCommand};
use sonora_codecs::{WavFormat, WavWriter};
use sonora_core::SongSession;

pub struct SongRenderer;

impl SongRenderer {
    /// Renders the entire song session to a stereo WAV file faster than real-time
    pub fn render_song<F>(
        session: &SongSession,
        sample_rate: u32,
        format: WavFormat,
        mut progress_cb: F,
    ) -> Result<Vec<u8>, String>
    where
        F: FnMut(f32),
    {
        let total_ticks = session
            .total_duration_ticks()
            .max(session.ticks_per_bar() * 4);
        let total_frames = session.ticks_to_frames(total_ticks, sample_rate) as usize;

        let mut left_channel = vec![0.0f32; total_frames];
        let mut right_channel = vec![0.0f32; total_frames];

        let mut engine = AudioEngine::new(sample_rate as f32);
        engine.send_command(AudioEngineCommand::LoadSession(session.clone()));
        engine.send_command(AudioEngineCommand::Play);

        let block_size = 256usize;
        let mut block_l = vec![0.0f32; block_size];
        let mut block_r = vec![0.0f32; block_size];

        let mut frame_offset = 0;
        while frame_offset < total_frames {
            let chunk_len = (total_frames - frame_offset).min(block_size);
            let active_l = &mut block_l[0..chunk_len];
            let active_r = &mut block_r[0..chunk_len];

            engine.process_block(active_l, active_r);

            left_channel[frame_offset..frame_offset + chunk_len].copy_from_slice(active_l);
            right_channel[frame_offset..frame_offset + chunk_len].copy_from_slice(active_r);

            frame_offset += chunk_len;
            progress_cb((frame_offset as f32 / total_frames as f32).min(1.0));
        }

        WavWriter::encode_stereo(&left_channel, &right_channel, sample_rate, format)
    }

    /// Renders individual stems for each track in the session
    pub fn render_stems(
        session: &SongSession,
        sample_rate: u32,
        format: WavFormat,
    ) -> Result<Vec<(String, Vec<u8>)>, String> {
        let mut stems = Vec::new();

        for (i, track) in session.tracks.iter().enumerate() {
            let mut solo_session = session.clone();
            for (t_idx, t) in solo_session.tracks.iter_mut().enumerate() {
                t.soloed = t_idx == i;
            }

            let wav_bytes = Self::render_song(&solo_session, sample_rate, format, |_| {})?;
            stems.push((track.name.clone(), wav_bytes));
        }

        Ok(stems)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sonora_core::{Clip, MidiNote, SongSession};

    #[test]
    fn test_offline_song_render() {
        let mut session = SongSession::new("Render Test");
        let notes = vec![
            MidiNote::new(60, 0, 480, 100),
            MidiNote::new(64, 480, 480, 100),
        ];
        session.tracks[0]
            .clips
            .push(Clip::new_midi("Notes", 0, 960, notes));

        let wav =
            SongRenderer::render_song(&session, 48000, WavFormat::Pcm16, |_| {}).expect("render");
        assert!(wav.starts_with(b"RIFF"));
        assert!(wav.len() > 1000);
    }
}
