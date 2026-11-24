//! Android AAudio output stream host.
//!
//! Owns the AAudio stream, registers the process-wide [`AudioSystem`], and pumps
//! the high-priority AAudio data callback into `AudioDriver::render_interleaved_rt`.
//! Device disconnects are signalled via the error callback and repaired by a
//! lightweight supervisor thread (never from inside the callback itself).

#![cfg(target_os = "android")]

use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use crate::aaudio::*;
use crate::oboe_bridge::AudioDriver;

/// The one audio system per process: the shared driver that both the AAudio
/// callback and the UI/JNI layer talk to. Never moved after init.
pub struct AudioSystem {
    pub driver: Arc<Mutex<AudioDriver>>,
}

unsafe impl Sync for AudioSystem {}
unsafe impl Send for AudioSystem {}

static AUDIO_SYSTEM: OnceLock<AudioSystem> = OnceLock::new();
static RESTART_REQUESTED: AtomicBool = AtomicBool::new(false);
static STREAM: AtomicPtr<AAudioStream> = AtomicPtr::new(ptr::null_mut());

fn log(msg: &str) {
    // Always visible under `adb logcat -s SonoraAudio` — lifecycle events only,
    // never called from the audio callback.
    crate::android_log(&format!("SonoraAudio: {}", msg));
}

/// Fetch (or create) the process-wide audio system.
pub fn ensure_system(sample_rate: u32, channels: u32) -> &'static AudioSystem {
    AUDIO_SYSTEM.get_or_init(|| AudioSystem {
        driver: Arc::new(Mutex::new(AudioDriver::new(sample_rate, channels))),
    })
}

// ---------------------------------------------------------------------------
// AAudio callbacks — real-time critical
// ---------------------------------------------------------------------------

extern "C" fn data_callback(
    _stream: *mut AAudioStream,
    _user_data: *mut std::ffi::c_void,
    audio_data: *mut std::ffi::c_void,
    num_frames: i32,
) -> aaudio_result_t {
    // Log the first callback exactly once — confirms the HAL is actually
    // pumping the stream (an OPEN+STARTED stream can still sit idle).
    static CALLBACK_SEEN: AtomicBool = AtomicBool::new(false);
    if !CALLBACK_SEEN.swap(true, Ordering::Relaxed) {
        log(&format!(
            "data callback active: first block = {} frames",
            num_frames
        ));
    }

    let frames = num_frames.max(0) as usize;
    let out =
        unsafe { std::slice::from_raw_parts_mut(audio_data as *mut f32, frames.saturating_mul(2)) };

    if let Some(sys) = AUDIO_SYSTEM.get() {
        if let Ok(mut driver) = sys.driver.try_lock() {
            driver.render_interleaved_rt(out, frames);
            return AAUDIO_OK;
        }
    }

    // Engine busy or not yet up: emit silence rather than block the audio thread.
    for sample in out.iter_mut() {
        *sample = 0.0;
    }
    AAUDIO_OK
}

extern "C" fn error_callback(
    _stream: *mut AAudioStream,
    _user_data: *mut std::ffi::c_void,
    error: aaudio_result_t,
) {
    // NEVER touch the stream or allocate here — just request a supervised rebuild.
    log(&format!("stream error {}, scheduling restart", error));
    RESTART_REQUESTED.store(true, Ordering::SeqCst);
}

// ---------------------------------------------------------------------------
// Stream lifecycle
// ---------------------------------------------------------------------------

struct BuilderGuard(*mut AAudioStreamBuilder);
impl BuilderGuard {
    fn new() -> Option<Self> {
        let mut builder: *mut AAudioStreamBuilder = ptr::null_mut();
        if unsafe { AAudio_createStreamBuilder(&mut builder) } == AAUDIO_OK && !builder.is_null() {
            Some(BuilderGuard(builder))
        } else {
            None
        }
    }
}
impl Drop for BuilderGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { AAudioStreamBuilder_delete(self.0) };
        }
    }
}

/// One configured open attempt: build, open, start, adopt granted parameters.
fn open_and_start(
    requested_rate: u32,
    channels: u32,
    sharing: aaudio_sharing_mode_t,
    performance: aaudio_performance_mode_t,
) -> Result<(u32, u32), String> {
    let builder =
        BuilderGuard::new().ok_or_else(|| "AAudio_createStreamBuilder failed".to_string())?;
    let b = builder.0;

    unsafe {
        AAudioStreamBuilder_setDirection(b, AAUDIO_DIRECTION_OUTPUT);
        AAudioStreamBuilder_setSharingMode(b, sharing);
        AAudioStreamBuilder_setPerformanceMode(b, performance);
        AAudioStreamBuilder_setSampleRate(b, requested_rate as i32);
        AAudioStreamBuilder_setChannelCount(b, channels as i32);
        AAudioStreamBuilder_setFormat(b, AAUDIO_FORMAT_PCM_FLOAT);
        // No explicit buffer capacity: AAudio sizes it from the device burst,
        // which avoids burst-alignment rejections on stricter HALs.
        AAudioStreamBuilder_setDataCallback(b, Some(data_callback));
        AAudioStreamBuilder_setErrorCallback(b, Some(error_callback));

        let mut stream: *mut AAudioStream = ptr::null_mut();
        let result = AAudioStreamBuilder_openStream(b, &mut stream);
        if result != AAUDIO_OK || stream.is_null() {
            return Err(format!("openStream: {}", result_text(result)));
        }

        let start_result = AAudioStream_requestStart(stream);
        if start_result != AAUDIO_OK {
            AAudioStream_close(stream);
            return Err(format!("requestStart: {}", result_text(start_result)));
        }

        let granted_rate = AAudioStream_getSampleRate(stream).max(1) as u32;
        let burst = AAudioStream_getFramesPerBurst(stream).max(1) as u32;
        // Pace the callback at ~3 bursts: enough lookahead for stability,
        // small enough for low latency. The HAL may clamp the value.
        let _ = AAudioStream_setBufferSizeInFrames(stream, (burst * 3) as i32);

        // Defensive: never overwrite a live stream pointer without closing it
        // (activity recreation used to leak one stream per init).
        let previous = STREAM.swap(stream, Ordering::SeqCst);
        if !previous.is_null() {
            AAudioStream_requestStop(previous);
            AAudioStream_close(previous);
        }
        log(&format!(
            "stream open: rate={} burst={} sharing={} performance={}",
            granted_rate,
            burst,
            AAudioStream_getSharingMode(stream),
            AAudioStream_getPerformanceMode(stream),
        ));
        Ok((granted_rate, burst))
    }
}

/// Opens and starts the output stream. On success returns (device_sample_rate, frames_per_burst).
///
/// Low-latency configurations are attempted first, degrading gracefully: some
/// HALs (the emulator, several budget devices) reject requests that the AAudio
/// spec treats as mere hints (e.g. LOW_LATENCY), and the granted configuration
/// is always logged so the device behaviour is visible.
pub fn open_output_stream(requested_rate: u32, channels: u32) -> Result<(u32, u32), String> {
    let attempts: [(aaudio_sharing_mode_t, aaudio_performance_mode_t); 3] = [
        (
            AAUDIO_SHARING_MODE_EXCLUSIVE,
            AAUDIO_PERFORMANCE_MODE_LOW_LATENCY,
        ),
        (
            AAUDIO_SHARING_MODE_SHARED,
            AAUDIO_PERFORMANCE_MODE_LOW_LATENCY,
        ),
        (AAUDIO_SHARING_MODE_SHARED, AAUDIO_PERFORMANCE_MODE_NONE),
    ];

    let mut last_err = "AAudio open failed".to_string();
    for &(sharing, performance) in &attempts {
        match open_and_start(requested_rate, channels, sharing, performance) {
            Ok((rate, burst)) => return Ok((rate, burst)),
            Err(e) => {
                log(&format!(
                    "open attempt failed (sharing={}, performance={}): {}",
                    sharing, performance, e
                ));
                last_err = e;
            }
        }
    }
    Err(last_err)
}

/// Supervised stream repair after a device change / disconnect.
/// Shared by the background supervisor thread and the on-demand [`service`] call.
fn rebuild_stream(driver: &Arc<Mutex<AudioDriver>>) -> Result<(), String> {
    close_stream();
    let rate = driver.lock().map(|d| d.sample_rate).unwrap_or(48_000);
    match open_output_stream(rate, 2) {
        Ok((device_rate, burst)) => {
            if let Ok(mut d) = driver.lock() {
                d.adopt_device_rate(device_rate);
                d.frames_per_burst = burst;
                d.set_state(crate::oboe_bridge::AudioStreamState::Running);
            }
            log("stream rebuilt successfully");
            Ok(())
        }
        Err(e) => {
            if let Ok(mut d) = driver.lock() {
                d.set_state(crate::oboe_bridge::AudioStreamState::Error);
            }
            log(&format!("stream rebuild failed: {}", e));
            Err(e)
        }
    }
}

fn supervisor(driver: Arc<Mutex<AudioDriver>>) {
    loop {
        std::thread::sleep(std::time::Duration::from_millis(300));
        if RESTART_REQUESTED.swap(false, Ordering::SeqCst) {
            let _ = rebuild_stream(&driver);
        }
    }
}

/// On-demand repair pass (invoked from JNI `serviceAudio`, e.g. on app resume).
/// Rebuilds the stream if the error callback scheduled a restart.
pub fn service() {
    if let Some(system) = AUDIO_SYSTEM.get() {
        if RESTART_REQUESTED.swap(false, Ordering::SeqCst) {
            let _ = rebuild_stream(&system.driver);
        }
    }
}

pub fn close_stream() {
    let old = STREAM.swap(ptr::null_mut(), Ordering::SeqCst);
    if !old.is_null() {
        unsafe {
            AAudioStream_requestStop(old);
            AAudioStream_close(old);
        }
    }
}

pub fn request_pause() {
    let s = STREAM.load(Ordering::SeqCst);
    if !s.is_null() {
        unsafe {
            AAudioStream_requestPause(s);
        }
    }
}

pub fn request_resume() -> Result<(), String> {
    let s = STREAM.load(Ordering::SeqCst);
    if s.is_null() {
        return Err("no stream".to_string());
    }
    unsafe {
        let mut state = AAudioStream_getState(s);
        // Already running (the common case — the stream stays up while the
        // transport is stopped): nothing to do. Flushing a STARTED stream is
        // illegal and used to make every Play press log error -895.
        if state == AAUDIO_STREAM_STATE_STARTED || state == AAUDIO_STREAM_STATE_STARTING {
            return Ok(());
        }
        // A pause/stop transition may still be in flight (Stop → Play quickly).
        if state == AAUDIO_STREAM_STATE_PAUSING || state == AAUDIO_STREAM_STATE_STOPPING {
            for _ in 0..40 {
                std::thread::sleep(std::time::Duration::from_millis(5));
                state = AAudioStream_getState(s);
                if state != AAUDIO_STREAM_STATE_PAUSING && state != AAUDIO_STREAM_STATE_STOPPING {
                    break;
                }
            }
        }
        if state == AAUDIO_STREAM_STATE_STARTED || state == AAUDIO_STREAM_STATE_STARTING {
            return Ok(());
        }
        // Flush is only legal in PAUSED/OPEN/STOPPED/FLUSHED.
        if state == AAUDIO_STREAM_STATE_PAUSED
            || state == AAUDIO_STREAM_STATE_OPEN
            || state == AAUDIO_STREAM_STATE_STOPPED
            || state == AAUDIO_STREAM_STATE_FLUSHED
        {
            let _ = AAudioStream_requestFlush(s);
        }
        let r = AAudioStream_requestStart(s);
        if r != AAUDIO_OK {
            return Err(format!("resume failed: {}", result_text(r)));
        }
    }
    Ok(())
}

pub fn request_stop() {
    let s = STREAM.load(Ordering::SeqCst);
    if !s.is_null() {
        unsafe {
            AAudioStream_requestStop(s);
        }
    }
}

fn result_text(r: aaudio_result_t) -> String {
    unsafe {
        let p = AAudio_convertResultToText(r);
        if p.is_null() {
            format!("code {}", r)
        } else {
            std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned()
        }
    }
}

/// Boot the whole audio stack: system + driver state + supervisor thread.
///
/// Idempotent: activity recreation (rotation, resize, process re-init) calls the
/// JNI init again — it must reuse the running stream instead of opening a second
/// one (which would leak the first stream and double the DSP load).
pub fn boot(sample_rate: u32, channels: u32) -> Result<(u32, u32), String> {
    static SUPERVISOR_STARTED: AtomicBool = AtomicBool::new(false);

    let system = ensure_system(sample_rate, channels);

    // Reuse a healthy stream if one is already open.
    let existing = STREAM.load(Ordering::SeqCst);
    if !existing.is_null() {
        let state = unsafe { AAudioStream_getState(existing) };
        if state == AAUDIO_STREAM_STATE_STARTED
            || state == AAUDIO_STREAM_STATE_STARTING
            || state == AAUDIO_STREAM_STATE_OPEN
            || state == AAUDIO_STREAM_STATE_PAUSED
        {
            let rate = unsafe { AAudioStream_getSampleRate(existing) }.max(1) as u32;
            let burst = unsafe { AAudioStream_getFramesPerBurst(existing) }.max(1) as u32;
            log(&format!(
                "boot: reusing live stream (state={}, rate={}, burst={})",
                state, rate, burst
            ));
            return Ok((rate, burst));
        }
        // Disconnected / stopped / closed: drop it before opening a new one.
        close_stream();
    }

    let (rate, burst) = {
        let mut driver = system
            .driver
            .lock()
            .map_err(|_| "driver poisoned".to_string())?;
        driver.start_android_with()?
    };

    if !SUPERVISOR_STARTED.swap(true, Ordering::SeqCst) {
        std::thread::Builder::new()
            .name("sonora-stream-supervisor".into())
            .spawn(move || supervisor(Arc::clone(&system.driver)))
            .map_err(|e| e.to_string())?;
    }
    Ok((rate, burst))
}

impl crate::oboe_bridge::AudioDriver {
    /// Android-only: open the AAudio stream and adopt its reported parameters.
    fn start_android_with(&mut self) -> Result<(u32, u32), String> {
        self.set_state(crate::oboe_bridge::AudioStreamState::Starting);
        match open_output_stream(self.sample_rate, self.channel_count) {
            Ok((rate, burst)) => {
                self.device_sample_rate = rate;
                self.frames_per_burst = burst;
                self.adopt_device_rate(rate);
                self.set_state(crate::oboe_bridge::AudioStreamState::Running);
                Ok((rate, burst))
            }
            Err(e) => {
                self.set_state(crate::oboe_bridge::AudioStreamState::Error);
                Err(e)
            }
        }
    }
}
