//! SONORA Android — native application library (`libsonora.so`).
//!
//! Two binding layers:
//! 1. Plain C ABI (`sonora_*`) — host-testable, used by tests and possible native glue.
//! 2. JNI bindings (`Java_com_sonora_app_SonoraNative_*`) — called by the WebView UI
//!    through the `SonoraNative` Java interface (see `platform/java/`).

use std::path::PathBuf;
#[cfg(target_os = "android")]
use std::sync::{Arc, Mutex};
#[cfg(not(target_os = "android"))]
use std::sync::{Arc, Mutex, OnceLock};

use sonora_audio::{AudioDriver, AudioEngineCommand};
use sonora_core::SongSession;
use sonora_midi::MidiFile;
use sonora_project::{PackageManager, ProjectDatabase};
use sonora_renderer::SongRenderer;

#[cfg(target_os = "android")]
use sonora_audio::aaudio_android;

#[cfg(target_os = "android")]
mod jni;

// ---------------------------------------------------------------------------
// Process-wide state
// ---------------------------------------------------------------------------

#[cfg(not(target_os = "android"))]
static HOST_DRIVER: OnceLock<Arc<Mutex<AudioDriver>>> = OnceLock::new();

static STORAGE_ROOT: Mutex<Option<PathBuf>> = Mutex::new(None);
static CURRENT_PROJECT: Mutex<Option<PathBuf>> = Mutex::new(None);

pub fn get_or_init_driver(sample_rate: u32) -> Arc<Mutex<AudioDriver>> {
    #[cfg(target_os = "android")]
    {
        aaudio_android::ensure_system(sample_rate, 2).driver.clone()
    }
    #[cfg(not(target_os = "android"))]
    {
        HOST_DRIVER
            .get_or_init(|| Arc::new(Mutex::new(AudioDriver::new(sample_rate, 2))))
            .clone()
    }
}

fn with_engine<T>(f: impl FnOnce(&mut sonora_audio::AudioEngine) -> T) -> Option<T> {
    let driver = get_or_init_driver(48_000);
    let engine_arc = driver.lock().ok()?.engine();
    let mut eng = engine_arc.lock().ok()?;
    Some(f(&mut eng))
}

fn send_engine_command(cmd: AudioEngineCommand) {
    with_engine(|eng| eng.send_command(cmd));
}

fn storage_root() -> PathBuf {
    STORAGE_ROOT
        .lock()
        .ok()
        .and_then(|g| g.clone())
        .unwrap_or_else(|| PathBuf::from("/data/data/com.sonora.app/files"))
}

fn set_storage_root(path: &str) {
    let p = PathBuf::from(path);
    let _ = std::fs::create_dir_all(p.join("projects"));
    let _ = std::fs::create_dir_all(p.join("exports"));
    *STORAGE_ROOT.lock().unwrap_or_else(|e| e.into_inner()) = Some(p);
}

fn current_project_dir() -> Option<PathBuf> {
    CURRENT_PROJECT.lock().ok().and_then(|g| g.clone())
}

fn set_current_project(dir: PathBuf) {
    *CURRENT_PROJECT.lock().unwrap_or_else(|e| e.into_inner()) = Some(dir);
}

fn project_db() -> Option<ProjectDatabase> {
    let path = storage_root().join("sonora_catalog.db");
    ProjectDatabase::open(&path.to_string_lossy()).ok()
}

fn exports_dir() -> PathBuf {
    let d = storage_root().join("exports");
    let _ = std::fs::create_dir_all(&d);
    d
}

// ---------------------------------------------------------------------------
// Plain C ABI (host-testable core)
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn sonora_init_engine(sample_rate: i32) -> i32 {
    let rate = if sample_rate > 8_000 {
        sample_rate as u32
    } else {
        48_000
    };
    #[cfg(target_os = "android")]
    {
        match aaudio_android::boot(rate, 2) {
            Ok((device_rate, _burst)) => {
                let _ = get_or_init_driver(device_rate);
                1
            }
            Err(_) => 0,
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = get_or_init_driver(rate);
        1
    }
}

#[no_mangle]
pub extern "C" fn sonora_play() {
    #[cfg(target_os = "android")]
    sonora_audio::android_log("SonoraAudio: transport play requested");
    send_engine_command(AudioEngineCommand::Play);
}

#[no_mangle]
pub extern "C" fn sonora_pause() {
    send_engine_command(AudioEngineCommand::Pause);
}

#[no_mangle]
pub extern "C" fn sonora_stop() {
    // Stop the transport only. The device stream stays running so the on-screen
    // keyboards/drums still preview while stopped (otherwise the HAL is paused
    // and note-on produces no sound).
    send_engine_command(AudioEngineCommand::Stop);
}

/// Resume the device stream after stop/pause (Android).
#[no_mangle]
pub extern "C" fn sonora_resume_stream() -> i32 {
    #[cfg(target_os = "android")]
    {
        let driver = get_or_init_driver(48_000);
        let ok = aaudio_android::request_resume().is_ok();
        if let Ok(mut d) = driver.lock() {
            if ok {
                d.set_state(sonora_audio::AudioStreamState::Running);
            }
        }
        if ok {
            1
        } else {
            0
        }
    }
    #[cfg(not(target_os = "android"))]
    1
}

/// Repair/reopen the device stream after a disconnect (idempotent).
#[no_mangle]
pub extern "C" fn sonora_service_audio() {
    #[cfg(target_os = "android")]
    aaudio_android::service();
}

#[no_mangle]
pub extern "C" fn sonora_note_on(track_idx: i32, note: i32, velocity: i32) {
    send_engine_command(AudioEngineCommand::NoteOn {
        track_idx: track_idx.max(0) as usize,
        note: note.clamp(0, 127) as u8,
        velocity: velocity.clamp(1, 127) as u8,
    });
}

#[no_mangle]
pub extern "C" fn sonora_note_off(track_idx: i32, note: i32) {
    send_engine_command(AudioEngineCommand::NoteOff {
        track_idx: track_idx.max(0) as usize,
        note: note.clamp(0, 127) as u8,
    });
}

#[no_mangle]
pub extern "C" fn sonora_trigger_drum(channel: i32, velocity: i32) {
    send_engine_command(AudioEngineCommand::TriggerDrum {
        channel: channel.clamp(0, 7) as u8,
        velocity: velocity.clamp(1, 127) as u8,
    });
}

#[no_mangle]
pub extern "C" fn sonora_set_bpm(bpm: f32) {
    send_engine_command(AudioEngineCommand::SetBpm(bpm));
}

#[no_mangle]
pub extern "C" fn sonora_set_loop(enabled: i32, start_bar: i32, end_bar: i32) {
    send_engine_command(AudioEngineCommand::SetLoop {
        enabled: enabled != 0,
        start_bar: start_bar.max(0) as u32,
        end_bar: end_bar.max(start_bar + 1) as u32,
    });
}

#[no_mangle]
pub extern "C" fn sonora_set_track_volume(track_idx: i32, volume_db: f32) {
    send_engine_command(AudioEngineCommand::SetTrackVolume {
        track_idx: track_idx.max(0) as usize,
        volume_db,
    });
}

#[no_mangle]
pub extern "C" fn sonora_set_track_pan(track_idx: i32, pan: f32) {
    send_engine_command(AudioEngineCommand::SetTrackPan {
        track_idx: track_idx.max(0) as usize,
        pan: pan.clamp(-1.0, 1.0),
    });
}

#[no_mangle]
pub extern "C" fn sonora_set_track_mute(track_idx: i32, muted: i32) {
    send_engine_command(AudioEngineCommand::SetTrackMute {
        track_idx: track_idx.max(0) as usize,
        muted: muted != 0,
    });
}

#[no_mangle]
pub extern "C" fn sonora_set_track_solo(track_idx: i32, soloed: i32) {
    send_engine_command(AudioEngineCommand::SetTrackSolo {
        track_idx: track_idx.max(0) as usize,
        soloed: soloed != 0,
    });
}

#[no_mangle]
pub extern "C" fn sonora_set_master_volume(volume_db: f32) {
    send_engine_command(AudioEngineCommand::SetMasterVolume(volume_db));
}

#[no_mangle]
pub extern "C" fn sonora_set_metronome(enabled: i32) {
    send_engine_command(AudioEngineCommand::SetMetronomeEnabled(enabled != 0));
}

#[no_mangle]
pub extern "C" fn sonora_set_sustain(enabled: i32) {
    send_engine_command(AudioEngineCommand::SetSustainPedal(enabled != 0));
}

/// Select an instrument preset/kit. `instrument`: 0=piano, 1=guitar, 2=bass, 3=synth, 4=drums.
///
/// # Safety
///
/// `preset` must point to a valid NUL-terminated UTF-8 string, or be null.
#[no_mangle]
pub unsafe extern "C" fn sonora_set_preset(instrument: i32, preset: *const std::os::raw::c_char) {
    if preset.is_null() {
        return;
    }
    let name = std::ffi::CStr::from_ptr(preset)
        .to_string_lossy()
        .into_owned();
    send_engine_command(AudioEngineCommand::SetPreset {
        instrument: instrument.clamp(0, 4) as u8,
        preset: name,
    });
}

#[no_mangle]
pub extern "C" fn sonora_set_synth_lfo(rate_hz: f32, depth: f32) {
    send_engine_command(AudioEngineCommand::SetSynthLfo { rate_hz, depth });
}

/// Device sample rate actually in use by the output stream (0 = unknown/not running).
#[no_mangle]
pub extern "C" fn sonora_get_sample_rate() -> i32 {
    let driver = get_or_init_driver(48_000);
    let guard = match driver.lock() {
        Ok(g) => g,
        Err(_) => return 0,
    };
    guard.device_sample_rate as i32
}

#[no_mangle]
pub extern "C" fn sonora_set_drum_step(
    clip_idx: i32,
    channel: i32,
    step: i32,
    active: i32,
    velocity: i32,
) {
    send_engine_command(AudioEngineCommand::SetDrumStep {
        clip_idx: clip_idx.max(0) as usize,
        channel: channel.clamp(0, 7) as u8,
        step: step.max(0) as u32,
        active: active != 0,
        velocity: velocity.clamp(1, 127) as u8,
    });
}

/// Replace the live session from a JSON string (`json` must outlive the call).
///
/// # Safety
///
/// `json` must point to a valid NUL-terminated UTF-8 string, or be null.
#[no_mangle]
pub unsafe extern "C" fn sonora_load_session_json(json: *const std::os::raw::c_char) -> i32 {
    if json.is_null() {
        return 0;
    }
    let cstr = std::ffi::CStr::from_ptr(json);
    match serde_json::from_str::<SongSession>(&cstr.to_string_lossy()) {
        Ok(session) => {
            send_engine_command(AudioEngineCommand::LoadSession(session));
            1
        }
        Err(_) => 0,
    }
}

/// Serialize the live session JSON into `out_buf` (NUL-terminated).
/// Returns bytes written (including NUL), or 0/-1 on failure.
///
/// # Safety
///
/// `out_buf` must be valid for writes of `buf_len` bytes (`buf_len >= 2`).
#[no_mangle]
pub unsafe extern "C" fn sonora_get_session_json(
    out_buf: *mut std::os::raw::c_char,
    buf_len: i32,
) -> i32 {
    if out_buf.is_null() || buf_len < 2 {
        return -1;
    }
    let json = match with_engine(|eng| serde_json::to_string(&eng.session)) {
        Some(Ok(json)) => json,
        _ => return 0,
    };
    let bytes = json.as_bytes();
    let cap = buf_len as usize - 1;
    if bytes.len() > cap {
        return -(bytes.len() as i32); // negative = required size
    }
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_buf as *mut u8, bytes.len());
    *out_buf.add(bytes.len()) = 0;
    bytes.len() as i32 + 1
}

/// Telemetry: peaks + transport tick, written to caller arrays (may be null).
/// Telemetry: peaks/RMS + transport tick, written to caller arrays (may be null).
///
/// # Safety
///
/// Each out pointer must be valid for a write of one `f32`/`u64`, or be null.
#[no_mangle]
pub unsafe extern "C" fn sonora_get_telemetry(
    out_peak_l: *mut f32,
    out_peak_r: *mut f32,
    out_rms_l: *mut f32,
    out_rms_r: *mut f32,
    out_tick: *mut u64,
) {
    let read = with_engine(|eng| {
        let (peak_l, peak_r, rms_l, rms_r) = eng.master_meter.read_and_reset();
        (peak_l, peak_r, rms_l, rms_r, eng.sequencer.current_tick)
    });
    if let Some((peak_l, peak_r, rms_l, rms_r, tick)) = read {
        if !out_peak_l.is_null() {
            *out_peak_l = peak_l;
        }
        if !out_peak_r.is_null() {
            *out_peak_r = peak_r;
        }
        if !out_rms_l.is_null() {
            *out_rms_l = rms_l;
        }
        if !out_rms_r.is_null() {
            *out_rms_r = rms_r;
        }
        if !out_tick.is_null() {
            *out_tick = tick;
        }
    }
}

/// Decimated waveform into `out` (len = out_len, clamped to available).
///
/// # Safety
///
/// `out` must be valid for writes of `out_len` `f32` values (`out_len >= 1`).
#[no_mangle]
pub unsafe extern "C" fn sonora_get_waveform(out: *mut f32, out_len: i32) -> i32 {
    if out.is_null() || out_len <= 0 {
        return 0;
    }
    let len = out_len as usize;
    let driver = get_or_init_driver(48_000);
    let driver_guard = match driver.lock() {
        Ok(g) => g,
        Err(_) => return 0,
    };
    let engine_arc = driver_guard.engine();
    if let Ok(eng) = engine_arc.lock() {
        let out = std::slice::from_raw_parts_mut(out, len);
        let ring_len = eng.waveform_ring.len();
        let stride = (ring_len / len).max(1);
        let mut written = 0usize;
        let mut idx = eng.waveform_write_idx;
        while written < len {
            out[written] = eng.waveform_ring[idx % ring_len];
            idx = idx.wrapping_add(stride);
            written += 1;
        }
        return written as i32;
    }
    0
}

/// FFT magnitude spectrum (0..~1) into `out`.
///
/// # Safety
///
/// `out` must be valid for writes of `out_len` `f32` values (`out_len >= 1`).
#[no_mangle]
pub unsafe extern "C" fn sonora_get_fft(out: *mut f32, out_len: i32) -> i32 {
    if out.is_null() || out_len <= 0 {
        return 0;
    }
    let len = out_len as usize;
    let driver = get_or_init_driver(48_000);
    let driver_guard = match driver.lock() {
        Ok(g) => g,
        Err(_) => return 0,
    };
    let engine_arc = driver_guard.engine();
    if let Ok(eng) = engine_arc.lock() {
        let out = std::slice::from_raw_parts_mut(out, len);
        let n = eng.fft_mags.len().min(len);
        for (i, mag) in eng.fft_mags[..n].iter().enumerate() {
            out[i] = *mag;
        }
        return n as i32;
    }
    0
}

// ---------------------------------------------------------------------------
// Project management (plain C ABI, string out-buffer convention)
// ---------------------------------------------------------------------------

fn write_cstr(out: *mut std::os::raw::c_char, buf_len: i32, value: &str) -> i32 {
    if out.is_null() || buf_len < 2 {
        return -1;
    }
    let bytes = value.as_bytes();
    let cap = buf_len as usize - 1;
    if bytes.len() > cap {
        return -(bytes.len() as i32);
    }
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), out as *mut u8, bytes.len());
        *out.add(bytes.len()) = 0;
    }
    bytes.len() as i32 + 1
}

///
/// # Safety
///
/// `path` must point to a valid NUL-terminated UTF-8 string, or be null.
#[no_mangle]
pub unsafe extern "C" fn sonora_set_storage_root(path: *const std::os::raw::c_char) {
    if path.is_null() {
        return;
    }
    let p = std::ffi::CStr::from_ptr(path)
        .to_string_lossy()
        .into_owned();
    set_storage_root(&p);
}

///
/// # Safety
///
/// `name` must point to a valid NUL-terminated UTF-8 string, or be null; `out` must be valid for writes of `buf_len` bytes.
#[no_mangle]
pub unsafe extern "C" fn sonora_project_create(
    name: *const std::os::raw::c_char,
    out: *mut std::os::raw::c_char,
    buf_len: i32,
) -> i32 {
    if name.is_null() {
        return -1;
    }
    let name_str = std::ffi::CStr::from_ptr(name)
        .to_string_lossy()
        .into_owned();
    let db = project_db();
    let response = match PackageManager::create_project(
        &name_str,
        &storage_root().join("projects"),
        db.as_ref(),
    ) {
        Ok((session, dir)) => {
            set_current_project(dir.clone());
            send_engine_command(AudioEngineCommand::LoadSession(session));
            serde_json::json!({ "ok": true, "path": dir.to_string_lossy() })
        }
        Err(e) => serde_json::json!({ "ok": false, "error": e }),
    };
    write_cstr(out, buf_len, &response.to_string())
}

///
/// # Safety
///
/// `out` must be valid for writes of `buf_len` bytes.
#[no_mangle]
pub unsafe extern "C" fn sonora_project_list(out: *mut std::os::raw::c_char, buf_len: i32) -> i32 {
    let response = match project_db() {
        Some(db) => match db.list_all() {
            Ok(records) => {
                let projects: Vec<serde_json::Value> = records
                    .iter()
                    .map(|r| {
                        serde_json::json!({
                            "id": r.id,
                            "name": r.name,
                            "path": r.path,
                            "bpm": r.bpm,
                            "trackCount": r.track_count,
                            "createdAt": r.created_at,
                            "updatedAt": r.updated_at,
                        })
                    })
                    .collect();
                serde_json::json!({ "ok": true, "projects": projects })
            }
            Err(e) => serde_json::json!({ "ok": false, "error": e.to_string() }),
        },
        None => serde_json::json!({ "ok": false, "error": "storage root not initialized" }),
    };
    write_cstr(out, buf_len, &response.to_string())
}

///
/// # Safety
///
/// `path` must point to a valid NUL-terminated UTF-8 string, or be null; `out` must be valid for writes of `buf_len` bytes.
#[no_mangle]
pub unsafe extern "C" fn sonora_project_open(
    path: *const std::os::raw::c_char,
    out: *mut std::os::raw::c_char,
    buf_len: i32,
) -> i32 {
    if path.is_null() {
        return -1;
    }
    let path_str = std::ffi::CStr::from_ptr(path)
        .to_string_lossy()
        .into_owned();
    let dir = PathBuf::from(&path_str);
    let response = match PackageManager::load_project(&dir) {
        Ok(session) => {
            set_current_project(dir.clone());
            send_engine_command(AudioEngineCommand::LoadSession(session));
            serde_json::json!({ "ok": true, "path": dir.to_string_lossy() })
        }
        Err(e) => serde_json::json!({ "ok": false, "error": e }),
    };
    write_cstr(out, buf_len, &response.to_string())
}

///
/// # Safety
///
/// `out` must be valid for writes of `buf_len` bytes.
#[no_mangle]
pub unsafe extern "C" fn sonora_project_save_current(
    out: *mut std::os::raw::c_char,
    buf_len: i32,
) -> i32 {
    let response = match current_project_dir() {
        Some(dir) => {
            let session = with_engine(|eng| eng.session.clone());
            match session {
                Some(session) => {
                    match PackageManager::save_project(&session, &dir, project_db().as_ref()) {
                        Ok(()) => serde_json::json!({ "ok": true, "path": dir.to_string_lossy() }),
                        Err(e) => serde_json::json!({ "ok": false, "error": e }),
                    }
                }
                None => serde_json::json!({ "ok": false, "error": "engine unavailable" }),
            }
        }
        None => serde_json::json!({ "ok": false, "error": "no project open" }),
    };
    write_cstr(out, buf_len, &response.to_string())
}

///
/// # Safety
///
/// `out` must be valid for writes of `buf_len` bytes.
#[no_mangle]
pub unsafe extern "C" fn sonora_project_autosave(
    out: *mut std::os::raw::c_char,
    buf_len: i32,
) -> i32 {
    let response = match current_project_dir() {
        Some(dir) => {
            let session = with_engine(|eng| eng.session.clone());
            match session {
                Some(session) => match PackageManager::autosave(&session, &dir) {
                    Ok(path) => serde_json::json!({ "ok": true, "path": path.to_string_lossy() }),
                    Err(e) => serde_json::json!({ "ok": false, "error": e }),
                },
                None => serde_json::json!({ "ok": false, "error": "engine unavailable" }),
            }
        }
        None => serde_json::json!({ "ok": false, "error": "no project open" }),
    };
    write_cstr(out, buf_len, &response.to_string())
}

// ---------------------------------------------------------------------------
// Export (call from a background thread — these are CPU-heavy)
// ---------------------------------------------------------------------------

/// Offline-render the session to a WAV file. `format`: 0 = 16-bit PCM,
/// 1 = 24-bit PCM, 2 = 32-bit float. Call from a background thread.
///
/// # Safety
///
/// `out` must be valid for writes of `buf_len` bytes.
#[no_mangle]
pub unsafe extern "C" fn sonora_export_wav(
    format: i32,
    out: *mut std::os::raw::c_char,
    buf_len: i32,
) -> i32 {
    let session = with_engine(|eng| eng.session.clone());
    let response = match session {
        Some(session) => {
            let format = match format {
                1 => sonora_codecs::WavFormat::Pcm24,
                2 => sonora_codecs::WavFormat::Float32,
                _ => sonora_codecs::WavFormat::Pcm16,
            };
            match SongRenderer::render_song(&session, 48_000, format, |_| {}) {
                Ok(bytes) => {
                    let safe = session
                        .name
                        .replace(|c: char| !c.is_alphanumeric() && c != '_' && c != '-', "_");
                    let ts = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    let path = exports_dir().join(format!("{}_{}.wav", safe, ts));
                    match std::fs::write(&path, &bytes) {
                        Ok(()) => {
                            serde_json::json!({ "ok": true, "path": path.to_string_lossy(), "bytes": bytes.len() })
                        }
                        Err(e) => serde_json::json!({ "ok": false, "error": e.to_string() }),
                    }
                }
                Err(e) => serde_json::json!({ "ok": false, "error": e }),
            }
        }
        None => serde_json::json!({ "ok": false, "error": "engine unavailable" }),
    };
    write_cstr(out, buf_len, &response.to_string())
}

///
/// # Safety
///
/// `out` must be valid for writes of `buf_len` bytes.
#[no_mangle]
pub unsafe extern "C" fn sonora_export_midi(out: *mut std::os::raw::c_char, buf_len: i32) -> i32 {
    let session = with_engine(|eng| eng.session.clone());
    let response = match session {
        Some(session) => {
            let bytes = MidiFile::export_smf(&session);
            let safe = session
                .name
                .replace(|c: char| !c.is_alphanumeric() && c != '_' && c != '-', "_");
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let path = exports_dir().join(format!("{}_{}.mid", safe, ts));
            match std::fs::write(&path, &bytes) {
                Ok(()) => {
                    serde_json::json!({ "ok": true, "path": path.to_string_lossy(), "bytes": bytes.len() })
                }
                Err(e) => serde_json::json!({ "ok": false, "error": e.to_string() }),
            }
        }
        None => serde_json::json!({ "ok": false, "error": "engine unavailable" }),
    };
    write_cstr(out, buf_len, &response.to_string())
}

///
/// # Safety
///
/// `out` must be valid for writes of `buf_len` bytes.
#[no_mangle]
pub unsafe extern "C" fn sonora_export_stems(out: *mut std::os::raw::c_char, buf_len: i32) -> i32 {
    let session = with_engine(|eng| eng.session.clone());
    let response = match session {
        Some(session) => {
            match SongRenderer::render_stems(&session, 48_000, sonora_codecs::WavFormat::Pcm16) {
                Ok(stems) => {
                    let ts = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    let mut written = Vec::new();
                    for (name, bytes) in stems {
                        let safe = name
                            .replace(|c: char| !c.is_alphanumeric() && c != '_' && c != '-', "_");
                        let path = exports_dir().join(format!("stem_{}_{}.wav", safe, ts));
                        if std::fs::write(&path, &bytes).is_ok() {
                            written.push(path.to_string_lossy().into_owned());
                        }
                    }
                    serde_json::json!({ "ok": true, "count": written.len(), "paths": written })
                }
                Err(e) => serde_json::json!({ "ok": false, "error": e }),
            }
        }
        None => serde_json::json!({ "ok": false, "error": "engine unavailable" }),
    };
    write_cstr(out, buf_len, &response.to_string())
}

// ---------------------------------------------------------------------------
// Host tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// These tests share the process-wide driver + storage statics, so they
    /// must not run concurrently with each other (other crates' tests still
    /// run in parallel — this only serializes within this module).
    static TEST_SERIAL: Mutex<()> = Mutex::new(());

    #[test]
    fn test_c_bridge_interleaved_rendering() {
        let _serial = TEST_SERIAL.lock().unwrap_or_else(|e| e.into_inner());

        assert_eq!(sonora_init_engine(48000), 1);
        sonora_note_on(0, 60, 100);

        // Drive the host driver directly through its interleaved RT path.
        let mut buffer = vec![0.0f32; 1024];
        let driver = get_or_init_driver(48000);
        let mut guard = driver.lock().unwrap();
        guard.start().unwrap();
        guard.render_interleaved_rt(&mut buffer, 512);
        drop(guard);

        let max_val = buffer.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        // A fresh host driver silences until the engine drains its limiter lookahead;
        // render again after the note to confirm output.
        let mut buffer2 = vec![0.0f32; 1024];
        let mut guard = driver.lock().unwrap();
        guard.render_interleaved_rt(&mut buffer2, 512);
        drop(guard);
        let max2 = buffer2.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(
            max_val > 0.0 || max2 > 0.0,
            "Expected audio output after lookahead buffer"
        );

        // Leave the shared engine quiet for the next test.
        sonora_note_off(0, 60);
    }

    #[test]
    fn test_session_json_roundtrip() {
        let _serial = TEST_SERIAL.lock().unwrap_or_else(|e| e.into_inner());

        let mut buf = vec![0i8; 16384];
        let written = unsafe {
            sonora_get_session_json(
                buf.as_mut_ptr() as *mut std::os::raw::c_char,
                buf.len() as i32,
            )
        };
        assert!(written > 0, "session JSON should serialize");
        let json = unsafe {
            std::ffi::CStr::from_ptr(buf.as_ptr() as *const std::os::raw::c_char)
                .to_string_lossy()
                .into_owned()
        };
        let parsed: SongSession = serde_json::from_str(&json).expect("valid session JSON");
        assert_eq!(parsed.tracks.len(), 5);
    }

    #[test]
    fn test_project_lifecycle() {
        let _serial = TEST_SERIAL.lock().unwrap_or_else(|e| e.into_inner());

        let root = std::env::temp_dir().join(format!("sonora_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        set_storage_root(root.to_string_lossy().as_ref());

        let mut out = vec![0i8; 8192];
        let name = std::ffi::CString::new("TestSong").unwrap();
        let n = unsafe {
            sonora_project_create(
                name.as_ptr(),
                out.as_mut_ptr() as *mut std::os::raw::c_char,
                out.len() as i32,
            )
        };
        assert!(n > 0);
        let created: serde_json::Value = serde_json::from_str(
            unsafe { std::ffi::CStr::from_ptr(out.as_ptr() as *const std::os::raw::c_char) }
                .to_string_lossy()
                .as_ref(),
        )
        .unwrap();
        assert_eq!(created["ok"], true);

        // list
        let n = unsafe {
            sonora_project_list(
                out.as_mut_ptr() as *mut std::os::raw::c_char,
                out.len() as i32,
            )
        };
        assert!(n > 0);
        let listed: serde_json::Value = serde_json::from_str(
            unsafe { std::ffi::CStr::from_ptr(out.as_ptr() as *const std::os::raw::c_char) }
                .to_string_lossy()
                .as_ref(),
        )
        .unwrap();
        assert_eq!(listed["projects"].as_array().map(|a| a.len()), Some(1));

        // save current
        let n = unsafe {
            sonora_project_save_current(
                out.as_mut_ptr() as *mut std::os::raw::c_char,
                out.len() as i32,
            )
        };
        assert!(n > 0);
        let saved: serde_json::Value = serde_json::from_str(
            unsafe { std::ffi::CStr::from_ptr(out.as_ptr() as *const std::os::raw::c_char) }
                .to_string_lossy()
                .as_ref(),
        )
        .unwrap();
        assert_eq!(saved["ok"], true);

        // Restore global state so later tests don't inherit a deleted storage root.
        *STORAGE_ROOT.lock().unwrap_or_else(|e| e.into_inner()) = None;
        *CURRENT_PROJECT.lock().unwrap_or_else(|e| e.into_inner()) = None;
        let _ = std::fs::remove_dir_all(&root);
    }
}
