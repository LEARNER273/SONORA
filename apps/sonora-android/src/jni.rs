//! JNI bindings — the WebView UI reaches the Rust engine through the
//! `com.sonora.app.SonoraNative` Java interface (loaded from `libsonora.so`).
//!
//! String-returning calls use JSON envelopes: `{"ok": bool, ...}`.
//! Export calls are CPU-heavy: the Java side must invoke them off the UI thread.

#![cfg(target_os = "android")]

use jni::objects::{JClass, JString};
use jni::sys::{jboolean, jfloat, jint, jstring};
use jni::JNIEnv;

// `crate` == the sonora-android lib, giving direct access to the plain C ABI fns.
use crate as native;

// Transport ----------------------------------------------------------------

#[no_mangle]
pub extern "system" fn Java_com_sonora_app_SonoraNative_init<'a>(
    _env: JNIEnv<'a>,
    _class: JClass<'a>,
    sample_rate: jint,
) -> jboolean {
    (native::sonora_init_engine(sample_rate) != 0) as jboolean
}

#[no_mangle]
pub extern "system" fn Java_com_sonora_app_SonoraNative_play<'a>(
    _env: JNIEnv<'a>,
    _class: JClass<'a>,
) {
    native::sonora_resume_stream();
    native::sonora_play();
}

#[no_mangle]
pub extern "system" fn Java_com_sonora_app_SonoraNative_pause<'a>(
    _env: JNIEnv<'a>,
    _class: JClass<'a>,
) {
    native::sonora_pause();
}

#[no_mangle]
pub extern "system" fn Java_com_sonora_app_SonoraNative_stop<'a>(
    _env: JNIEnv<'a>,
    _class: JClass<'a>,
) {
    native::sonora_stop();
}

#[no_mangle]
pub extern "system" fn Java_com_sonora_app_SonoraNative_serviceAudio<'a>(
    _env: JNIEnv<'a>,
    _class: JClass<'a>,
) {
    native::sonora_service_audio();
}

// Instruments ---------------------------------------------------------------

#[no_mangle]
pub extern "system" fn Java_com_sonora_app_SonoraNative_noteOn<'a>(
    _env: JNIEnv<'a>,
    _class: JClass<'a>,
    track: jint,
    note: jint,
    velocity: jint,
) {
    native::sonora_note_on(track, note, velocity);
}

#[no_mangle]
pub extern "system" fn Java_com_sonora_app_SonoraNative_noteOff<'a>(
    _env: JNIEnv<'a>,
    _class: JClass<'a>,
    track: jint,
    note: jint,
) {
    native::sonora_note_off(track, note);
}

#[no_mangle]
pub extern "system" fn Java_com_sonora_app_SonoraNative_triggerDrum<'a>(
    _env: JNIEnv<'a>,
    _class: JClass<'a>,
    channel: jint,
    velocity: jint,
) {
    native::sonora_trigger_drum(channel, velocity);
}

#[no_mangle]
pub extern "system" fn Java_com_sonora_app_SonoraNative_setSustain<'a>(
    _env: JNIEnv<'a>,
    _class: JClass<'a>,
    on: jboolean,
) {
    native::sonora_set_sustain(on as i32);
}

#[no_mangle]
pub extern "system" fn Java_com_sonora_app_SonoraNative_setPreset<'a>(
    mut env: JNIEnv<'a>,
    _class: JClass<'a>,
    instrument: jint,
    preset: JString<'a>,
) {
    let c = match jstring_to_cstring(&mut env, &preset) {
        Some(c) => c,
        None => return,
    };
    unsafe { native::sonora_set_preset(instrument, c.as_ptr()) };
}

#[no_mangle]
pub extern "system" fn Java_com_sonora_app_SonoraNative_setSynthLfo<'a>(
    _env: JNIEnv<'a>,
    _class: JClass<'a>,
    rate_hz: jfloat,
    depth: jfloat,
) {
    native::sonora_set_synth_lfo(rate_hz, depth);
}

#[no_mangle]
pub extern "system" fn Java_com_sonora_app_SonoraNative_getSampleRate<'a>(
    _env: JNIEnv<'a>,
    _class: JClass<'a>,
) -> jint {
    native::sonora_get_sample_rate()
}

#[no_mangle]
pub extern "system" fn Java_com_sonora_app_SonoraNative_setDrumStep<'a>(
    _env: JNIEnv<'a>,
    _class: JClass<'a>,
    clip: jint,
    channel: jint,
    step: jint,
    active: jboolean,
    velocity: jint,
) {
    native::sonora_set_drum_step(clip, channel, step, active as i32, velocity);
}

// Mixer / transport params ---------------------------------------------------

#[no_mangle]
pub extern "system" fn Java_com_sonora_app_SonoraNative_setBpm<'a>(
    _env: JNIEnv<'a>,
    _class: JClass<'a>,
    bpm: jfloat,
) {
    native::sonora_set_bpm(bpm);
}

#[no_mangle]
pub extern "system" fn Java_com_sonora_app_SonoraNative_setLoop<'a>(
    _env: JNIEnv<'a>,
    _class: JClass<'a>,
    enabled: jboolean,
    start_bar: jint,
    end_bar: jint,
) {
    native::sonora_set_loop(enabled as i32, start_bar, end_bar);
}

#[no_mangle]
pub extern "system" fn Java_com_sonora_app_SonoraNative_setTrackVolume<'a>(
    _env: JNIEnv<'a>,
    _class: JClass<'a>,
    track: jint,
    volume_db: jfloat,
) {
    native::sonora_set_track_volume(track, volume_db);
}

#[no_mangle]
pub extern "system" fn Java_com_sonora_app_SonoraNative_setTrackPan<'a>(
    _env: JNIEnv<'a>,
    _class: JClass<'a>,
    track: jint,
    pan: jfloat,
) {
    native::sonora_set_track_pan(track, pan);
}

#[no_mangle]
pub extern "system" fn Java_com_sonora_app_SonoraNative_setTrackMute<'a>(
    _env: JNIEnv<'a>,
    _class: JClass<'a>,
    track: jint,
    muted: jboolean,
) {
    native::sonora_set_track_mute(track, muted as i32);
}

#[no_mangle]
pub extern "system" fn Java_com_sonora_app_SonoraNative_setTrackSolo<'a>(
    _env: JNIEnv<'a>,
    _class: JClass<'a>,
    track: jint,
    soloed: jboolean,
) {
    native::sonora_set_track_solo(track, soloed as i32);
}

#[no_mangle]
pub extern "system" fn Java_com_sonora_app_SonoraNative_setMasterVolume<'a>(
    _env: JNIEnv<'a>,
    _class: JClass<'a>,
    volume_db: jfloat,
) {
    native::sonora_set_master_volume(volume_db);
}

#[no_mangle]
pub extern "system" fn Java_com_sonora_app_SonoraNative_setMetronome<'a>(
    _env: JNIEnv<'a>,
    _class: JClass<'a>,
    enabled: jboolean,
) {
    native::sonora_set_metronome(enabled as i32);
}

// Session & projects ----------------------------------------------------------

#[no_mangle]
pub extern "system" fn Java_com_sonora_app_SonoraNative_loadSession<'a>(
    mut env: JNIEnv<'a>,
    _class: JClass<'a>,
    json: JString<'a>,
) -> jboolean {
    let s = match env.get_string(&json) {
        Ok(s) => s,
        Err(_) => return 0,
    };
    let text = String::from(s);
    let c = match std::ffi::CString::new(text) {
        Ok(c) => c,
        Err(_) => return 0,
    };
    unsafe { (native::sonora_load_session_json(c.as_ptr()) != 0) as jboolean }
}

#[no_mangle]
pub extern "system" fn Java_com_sonora_app_SonoraNative_getSessionJson<'a>(
    env: JNIEnv<'a>,
    _class: JClass<'a>,
) -> JString<'a> {
    let mut buf = vec![0u8; 1 << 20];
    let written = unsafe {
        native::sonora_get_session_json(
            buf.as_mut_ptr() as *mut std::os::raw::c_char,
            buf.len() as i32,
        )
    };
    let text = if written > 0 {
        String::from_utf8_lossy(&buf[..(written - 1) as usize]).into_owned()
    } else {
        "{\"ok\":false,\"error\":\"session too large\"}".to_string()
    };
    jni_new_string(&env, &text)
}

#[no_mangle]
pub extern "system" fn Java_com_sonora_app_SonoraNative_setStorageRoot<'a>(
    mut env: JNIEnv<'a>,
    _class: JClass<'a>,
    path: JString<'a>,
) {
    let s = match env.get_string(&path) {
        Ok(s) => s,
        Err(_) => return,
    };
    let text = String::from(s);
    let c = match std::ffi::CString::new(text) {
        Ok(c) => c,
        Err(_) => return,
    };
    unsafe { native::sonora_set_storage_root(c.as_ptr()) };
}

/// Run a `sonora_*` C-ABI call that writes a JSON response into a caller buffer,
/// and hand the response back to Java as a JString.
fn call_with_out_string<'a>(
    env: &JNIEnv<'a>,
    f: impl FnOnce(*mut std::os::raw::c_char, i32) -> i32,
) -> JString<'a> {
    let mut buf = vec![0u8; 1 << 20];
    let written = f(
        buf.as_mut_ptr() as *mut std::os::raw::c_char,
        buf.len() as i32,
    );
    let text = if written > 0 {
        String::from_utf8_lossy(&buf[..(written - 1) as usize]).into_owned()
    } else if written < 0 {
        format!(
            "{{\"ok\":false,\"error\":\"required buffer: {}\"}}",
            -written
        )
    } else {
        "{\"ok\":false,\"error\":\"call failed\"}".to_string()
    };
    jni_new_string(env, &text)
}

fn jni_new_string<'a>(env: &JNIEnv<'a>, text: &str) -> JString<'a> {
    env.new_string(text)
        .unwrap_or_else(|_| env.new_string("{\"ok\":false}").expect("json"))
}

#[no_mangle]
pub extern "system" fn Java_com_sonora_app_SonoraNative_createProject<'a>(
    mut env: JNIEnv<'a>,
    _class: JClass<'a>,
    name: JString<'a>,
) -> JString<'a> {
    let c = match jstring_to_cstring(&mut env, &name) {
        Some(c) => c,
        None => return call_with_out_string(&env, |_, _| 0),
    };
    let ptr = c.as_ptr();
    call_with_out_string(&env, |out, len| unsafe {
        native::sonora_project_create(ptr, out, len)
    })
}

#[no_mangle]
pub extern "system" fn Java_com_sonora_app_SonoraNative_listProjects<'a>(
    env: JNIEnv<'a>,
    _class: JClass<'a>,
) -> JString<'a> {
    call_with_out_string(&env, |out, len| unsafe {
        native::sonora_project_list(out, len)
    })
}

#[no_mangle]
pub extern "system" fn Java_com_sonora_app_SonoraNative_openProject<'a>(
    mut env: JNIEnv<'a>,
    _class: JClass<'a>,
    path: JString<'a>,
) -> JString<'a> {
    let c = match jstring_to_cstring(&mut env, &path) {
        Some(c) => c,
        None => return call_with_out_string(&env, |_, _| 0),
    };
    let ptr = c.as_ptr();
    call_with_out_string(&env, |out, len| unsafe {
        native::sonora_project_open(ptr, out, len)
    })
}

#[no_mangle]
pub extern "system" fn Java_com_sonora_app_SonoraNative_saveProject<'a>(
    env: JNIEnv<'a>,
    _class: JClass<'a>,
) -> JString<'a> {
    call_with_out_string(&env, |out, len| unsafe {
        native::sonora_project_save_current(out, len)
    })
}

#[no_mangle]
pub extern "system" fn Java_com_sonora_app_SonoraNative_autosave<'a>(
    env: JNIEnv<'a>,
    _class: JClass<'a>,
) -> JString<'a> {
    call_with_out_string(&env, |out, len| unsafe {
        native::sonora_project_autosave(out, len)
    })
}

// Exports (must be called off the UI thread) ---------------------------------

#[no_mangle]
pub extern "system" fn Java_com_sonora_app_SonoraNative_exportWav<'a>(
    env: JNIEnv<'a>,
    _class: JClass<'a>,
    format: jint,
) -> JString<'a> {
    call_with_out_string(&env, |out, len| unsafe {
        native::sonora_export_wav(format, out, len)
    })
}

#[no_mangle]
pub extern "system" fn Java_com_sonora_app_SonoraNative_exportMidi<'a>(
    env: JNIEnv<'a>,
    _class: JClass<'a>,
) -> JString<'a> {
    call_with_out_string(&env, |out, len| unsafe {
        native::sonora_export_midi(out, len)
    })
}

#[no_mangle]
pub extern "system" fn Java_com_sonora_app_SonoraNative_exportStems<'a>(
    env: JNIEnv<'a>,
    _class: JClass<'a>,
) -> JString<'a> {
    call_with_out_string(&env, |out, len| unsafe {
        native::sonora_export_stems(out, len)
    })
}

// Telemetry -------------------------------------------------------------------
//
// Returned as JSON strings: the WebView's @JavascriptInterface cannot marshal
// Java arrays (they arrive as `undefined` in JS), only primitives/Strings.

fn floats_json(values: &[f32]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(values.len() * 8 + 2);
    s.push('[');
    for (i, v) in values.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        if v.is_finite() {
            let _ = write!(s, "{:.5}", v);
        } else {
            s.push('0');
        }
    }
    s.push(']');
    s
}

fn jstring_or_null(env: &JNIEnv, value: &str) -> jstring {
    match env.new_string(value) {
        Ok(js) => js.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "system" fn Java_com_sonora_app_SonoraNative_getTelemetryJson<'a>(
    env: JNIEnv<'a>,
    _class: JClass<'a>,
) -> jstring {
    let mut peak_l: f32 = 0.0;
    let mut peak_r: f32 = 0.0;
    let mut rms_l: f32 = 0.0;
    let mut rms_r: f32 = 0.0;
    let mut tick: u64 = 0;
    unsafe {
        native::sonora_get_telemetry(
            &mut peak_l as *mut f32,
            &mut peak_r as *mut f32,
            &mut rms_l as *mut f32,
            &mut rms_r as *mut f32,
            &mut tick as *mut u64,
        );
    }
    let json = format!(
        "[{:.6},{:.6},{:.6},{:.6},{}]",
        peak_l, peak_r, rms_l, rms_r, tick
    );
    jstring_or_null(&env, &json)
}

#[no_mangle]
pub extern "system" fn Java_com_sonora_app_SonoraNative_getWaveformJson<'a>(
    env: JNIEnv<'a>,
    _class: JClass<'a>,
) -> jstring {
    let mut buf = vec![0.0f32; 256];
    unsafe {
        native::sonora_get_waveform(buf.as_mut_ptr(), buf.len() as i32);
    }
    jstring_or_null(&env, &floats_json(&buf))
}

#[no_mangle]
pub extern "system" fn Java_com_sonora_app_SonoraNative_getFftJson<'a>(
    env: JNIEnv<'a>,
    _class: JClass<'a>,
) -> jstring {
    let mut buf = vec![0.0f32; 128];
    unsafe {
        native::sonora_get_fft(buf.as_mut_ptr(), buf.len() as i32);
    }
    jstring_or_null(&env, &floats_json(&buf))
}

/// Helper: copy a Java string into an owned CString (NUL-terminated for the C ABI).
fn jstring_to_cstring<'a>(env: &mut JNIEnv<'a>, js: &JString<'a>) -> Option<std::ffi::CString> {
    let s = env.get_string(js).ok()?;
    std::ffi::CString::new(String::from(s)).ok()
}
