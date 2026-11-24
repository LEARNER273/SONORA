#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WavFormat {
    Pcm16,
    Pcm24,
    Float32,
}

pub struct WavWriter;

impl WavWriter {
    /// Encodes stereo audio slices (left and right channels) into standard WAV binary
    pub fn encode_stereo(
        left: &[f32],
        right: &[f32],
        sample_rate: u32,
        format: WavFormat,
    ) -> Result<Vec<u8>, String> {
        if left.len() != right.len() {
            return Err("Left and right channels must have identical frame lengths".to_string());
        }

        let num_frames = left.len();
        let num_channels = 2u16;

        let (audio_format, bits_per_sample, bytes_per_sample) = match format {
            WavFormat::Pcm16 => (1u16, 16u16, 2usize),
            WavFormat::Pcm24 => (1u16, 24u16, 3usize),
            WavFormat::Float32 => (3u16, 32u16, 4usize),
        };

        let block_align = num_channels * (bits_per_sample / 8);
        let byte_rate = sample_rate * (block_align as u32);
        let data_size = (num_frames * 2 * bytes_per_sample) as u32;
        let riff_chunk_size = 36 + data_size;

        let mut out = Vec::with_capacity(44 + data_size as usize);

        // RIFF Header
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&riff_chunk_size.to_le_bytes());
        out.extend_from_slice(b"WAVE");

        // "fmt " Subchunk
        out.extend_from_slice(b"fmt ");
        out.extend_from_slice(&16u32.to_le_bytes()); // Subchunk1Size for PCM
        out.extend_from_slice(&audio_format.to_le_bytes());
        out.extend_from_slice(&num_channels.to_le_bytes());
        out.extend_from_slice(&sample_rate.to_le_bytes());
        out.extend_from_slice(&byte_rate.to_le_bytes());
        out.extend_from_slice(&block_align.to_le_bytes());
        out.extend_from_slice(&bits_per_sample.to_le_bytes());

        // "data" Subchunk
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_size.to_le_bytes());

        // Interleaved sample writing
        for i in 0..num_frames {
            let l = left[i].clamp(-1.0, 1.0);
            let r = right[i].clamp(-1.0, 1.0);

            match format {
                WavFormat::Pcm16 => {
                    let s_l = (l * 32767.0).round() as i16;
                    let s_r = (r * 32767.0).round() as i16;
                    out.extend_from_slice(&s_l.to_le_bytes());
                    out.extend_from_slice(&s_r.to_le_bytes());
                }
                WavFormat::Pcm24 => {
                    let s_l = (l * 8388607.0).round() as i32;
                    let s_r = (r * 8388607.0).round() as i32;
                    out.extend_from_slice(&s_l.to_le_bytes()[0..3]);
                    out.extend_from_slice(&s_r.to_le_bytes()[0..3]);
                }
                WavFormat::Float32 => {
                    out.extend_from_slice(&left[i].to_le_bytes());
                    out.extend_from_slice(&right[i].to_le_bytes());
                }
            }
        }

        Ok(out)
    }
}

pub struct WavReader;

impl WavReader {
    pub fn decode(bytes: &[u8]) -> Result<(Vec<f32>, Vec<f32>, u32), String> {
        if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
            return Err("Invalid RIFF/WAVE header".to_string());
        }

        let num_channels = u16::from_le_bytes([bytes[22], bytes[23]]);
        let sample_rate = u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]);
        let bits_per_sample = u16::from_le_bytes([bytes[34], bytes[35]]);

        // Search for 'data' chunk
        let mut idx = 36;
        while idx + 8 <= bytes.len() {
            if &bytes[idx..idx + 4] == b"data" {
                let data_size = u32::from_le_bytes([
                    bytes[idx + 4],
                    bytes[idx + 5],
                    bytes[idx + 6],
                    bytes[idx + 7],
                ]) as usize;
                let payload = &bytes[idx + 8..idx + 8 + data_size.min(bytes.len() - (idx + 8))];

                let bytes_per_sample = (bits_per_sample / 8) as usize;
                let block_align = (num_channels as usize) * bytes_per_sample;
                let num_frames = payload.len() / block_align;

                let mut left = Vec::with_capacity(num_frames);
                let mut right = Vec::with_capacity(num_frames);

                for f in 0..num_frames {
                    let offset = f * block_align;
                    let l_sample = match bits_per_sample {
                        16 => {
                            let raw = i16::from_le_bytes([payload[offset], payload[offset + 1]]);
                            (raw as f32) / 32768.0
                        }
                        32 => f32::from_le_bytes([
                            payload[offset],
                            payload[offset + 1],
                            payload[offset + 2],
                            payload[offset + 3],
                        ]),
                        _ => 0.0,
                    };

                    let r_sample = if num_channels > 1 {
                        let r_offset = offset + bytes_per_sample;
                        match bits_per_sample {
                            16 => {
                                let raw =
                                    i16::from_le_bytes([payload[r_offset], payload[r_offset + 1]]);
                                (raw as f32) / 32768.0
                            }
                            32 => f32::from_le_bytes([
                                payload[r_offset],
                                payload[r_offset + 1],
                                payload[r_offset + 2],
                                payload[r_offset + 3],
                            ]),
                            _ => l_sample,
                        }
                    } else {
                        l_sample
                    };

                    left.push(l_sample);
                    right.push(r_sample);
                }

                return Ok((left, right, sample_rate));
            }
            idx += 1;
        }

        Err("Data chunk not found in WAV file".to_string())
    }
}
