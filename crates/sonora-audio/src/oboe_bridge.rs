use crate::engine::AudioEngine;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioStreamState {
    Uninitialized,
    Starting,
    Running,
    Paused,
    Stopping,
    Closed,
    Error,
}

/// The audio driver owns the engine and all buffers needed by the real-time
/// callback. On Android the AAudio stream itself lives in `aaudio_android`
/// (statically hosted so the C callback can always reach it); the driver here
/// stays platform-neutral and is what the callback actually drives.
pub struct AudioDriver {
    engine: Arc<Mutex<AudioEngine>>,
    state: AudioStreamState,
    pub sample_rate: u32,
    pub channel_count: u32,
    /// Rate actually reported by the device stream (differs from requested sometimes)
    pub device_sample_rate: u32,
    pub frames_per_burst: u32,
    pub underruns: u32,
    /// Preallocated de-interleave scratch: zero allocation inside the callback
    scratch_l: Vec<f32>,
    scratch_r: Vec<f32>,
}

impl AudioDriver {
    pub fn new(sample_rate: u32, channel_count: u32) -> Self {
        let engine = Arc::new(Mutex::new(AudioEngine::new(sample_rate as f32)));
        Self {
            engine,
            state: AudioStreamState::Uninitialized,
            sample_rate,
            channel_count,
            device_sample_rate: 0,
            frames_per_burst: 0,
            underruns: 0,
            scratch_l: vec![0.0f32; crate::aaudio::MAX_SCRATCH_FRAMES],
            scratch_r: vec![0.0f32; crate::aaudio::MAX_SCRATCH_FRAMES],
        }
    }

    pub fn engine(&self) -> Arc<Mutex<AudioEngine>> {
        Arc::clone(&self.engine)
    }

    /// Host/test path: mark the stream running without any OS audio device.
    pub fn start(&mut self) -> Result<(), String> {
        self.state = AudioStreamState::Running;
        Ok(())
    }

    pub fn set_state(&mut self, state: AudioStreamState) {
        self.state = state;
    }

    pub fn state(&self) -> AudioStreamState {
        self.state
    }

    /// Adopt a device sample rate change (stream reopen on Android).
    pub fn adopt_device_rate(&mut self, rate: u32) {
        if rate > 0 && rate != self.sample_rate {
            self.sample_rate = rate;
            if let Ok(mut eng) = self.engine.lock() {
                eng.sample_rate = rate as f32;
                eng.sequencer.set_sample_rate(rate as f32);
            }
        }
        self.device_sample_rate = rate;
    }

    /// Maximum frames one scratch pass can process.
    pub fn max_scratch_frames(&self) -> usize {
        self.scratch_l.len()
    }

    /// Render an interleaved stereo block WITHOUT heap allocation.
    /// Real-time safe: try_lock only — under contention it emits digital silence
    /// instead of blocking the high-priority audio thread.
    pub fn render_interleaved_rt(&mut self, out: &mut [f32], frames: usize) {
        let total = frames.min(out.len() / 2);
        let mut done = 0usize;

        while done < total {
            let chunk = (total - done).min(self.scratch_l.len());

            if self.state == AudioStreamState::Running {
                if let Ok(mut eng) = self.engine.try_lock() {
                    {
                        let l = &mut self.scratch_l[0..chunk];
                        let r = &mut self.scratch_r[0..chunk];
                        eng.process_block(l, r);
                    }
                    let base = done * 2;
                    for i in 0..chunk {
                        out[base + i * 2] = self.scratch_l[i];
                        out[base + i * 2 + 1] = self.scratch_r[i];
                    }
                    done += chunk;
                    continue;
                }
            }

            // Paused / error / engine lock momentarily held → silence, never block.
            for i in 0..chunk {
                out[(done + i) * 2] = 0.0;
                out[(done + i) * 2 + 1] = 0.0;
            }
            done += chunk;
        }
    }

    /// Separate-buffer render for offline rendering and host tests.
    pub fn render_callback(&self, out_left: &mut [f32], out_right: &mut [f32]) {
        if self.state == AudioStreamState::Running {
            if let Ok(mut eng) = self.engine.lock() {
                eng.process_block(out_left, out_right);
                return;
            }
        }
        out_left.fill(0.0);
        out_right.fill(0.0);
    }
}
