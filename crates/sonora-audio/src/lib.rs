pub mod aaudio;
#[cfg(target_os = "android")]
pub mod aaudio_android;
pub mod command;
pub mod engine;
pub mod oboe_bridge;

pub use command::AudioEngineCommand;
pub use engine::AudioEngine;
pub use oboe_bridge::{AudioDriver, AudioStreamState};

/// Route a log line into Android logcat (no-op on host builds).
pub fn android_log(msg: &str) {
    #[cfg(target_os = "android")]
    {
        use std::ffi::CString;
        if let Ok(tag) = CString::new("SonoraAudio") {
            if let Ok(text) = CString::new(msg.replace('\n', " ")) {
                unsafe {
                    __android_log_print(4, tag.as_ptr(), b"%s\0".as_ptr() as _, text.as_ptr());
                }
            }
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = msg;
    }
}

#[cfg(target_os = "android")]
extern "C" {
    fn __android_log_print(
        prio: i32,
        tag: *const std::os::raw::c_char,
        fmt: *const std::os::raw::c_char,
        ...
    ) -> i32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_engine_block_processing() {
        let mut engine = AudioEngine::new(48000.0);
        let mut left = vec![0.0f32; 256];
        let mut right = vec![0.0f32; 256];

        engine.send_command(AudioEngineCommand::NoteOn {
            track_idx: 0,
            note: 60,
            velocity: 100,
        });

        engine.process_block(&mut left, &mut right);

        // Verify audio was rendered
        let max_val = left.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(max_val > 0.0, "Expected audio generation from NoteOn");
        assert!(max_val <= 1.0, "Expected limiter to prevent clipping");
    }

    #[test]
    fn test_audio_driver_lifecycle() {
        let mut driver = AudioDriver::new(48000, 2);
        assert_eq!(driver.state(), AudioStreamState::Uninitialized);
        driver.start().expect("start");
        assert_eq!(driver.state(), AudioStreamState::Running);

        let mut left = vec![0.0f32; 128];
        let mut right = vec![0.0f32; 128];
        driver.render_callback(&mut left, &mut right);

        driver.set_state(AudioStreamState::Closed);
        assert_eq!(driver.state(), AudioStreamState::Closed);
    }

    #[test]
    fn test_interleaved_rt_render() {
        let mut driver = AudioDriver::new(48000, 2);
        driver.start().expect("start");

        let mut buf = vec![0.0f32; 512 * 2];
        driver.render_interleaved_rt(&mut buf, 512);

        driver.set_state(AudioStreamState::Closed);
    }
}
