//! AAudio FFI — pure-Rust bindings to Android's AAudio C API (libaaudio.so, API 26+).
//!
//! The DSP itself stays in Rust (`AudioEngine::process_block`). This module only
//! opens/controls the low-latency output stream and pumps the callback into the engine.

#![allow(non_camel_case_types, dead_code)]

use std::os::raw::{c_int, c_void};

// ---------------------------------------------------------------------------
// Raw AAudio C API bindings (AAudio.h)
// ---------------------------------------------------------------------------

pub type aaudio_result_t = i32;
pub type aaudio_stream_state_t = i32;
pub type aaudio_direction_t = i32;
pub type aaudio_format_t = i32;
pub type aaudio_sharing_mode_t = i32;
pub type aaudio_performance_mode_t = i32;

pub const AAUDIO_OK: aaudio_result_t = 0;

pub const AAUDIO_DIRECTION_OUTPUT: aaudio_direction_t = 0;
pub const AAUDIO_DIRECTION_INPUT: aaudio_direction_t = 1;

// Formats: INVALID=-1, UNSPECIFIED=0, PCM_I16=1, PCM_FLOAT=2, PCM_I24_PACKED=3.
// (Values verified against the NDK r26b AAudio.h; we stream float32.)
pub const AAUDIO_FORMAT_PCM_FLOAT: aaudio_format_t = 2;

pub const AAUDIO_SHARING_MODE_EXCLUSIVE: aaudio_sharing_mode_t = 0;
pub const AAUDIO_SHARING_MODE_SHARED: aaudio_sharing_mode_t = 1;

// Performance modes are 10/11/12 in AAudio (unlike Oboe's 0/1/2) — using the
// Oboe numbering makes openStream() fail with AAUDIO_ERROR_ILLEGAL_ARGUMENT.
pub const AAUDIO_PERFORMANCE_MODE_NONE: aaudio_performance_mode_t = 10;
pub const AAUDIO_PERFORMANCE_MODE_POWER_SAVING: aaudio_performance_mode_t = 11;
pub const AAUDIO_PERFORMANCE_MODE_LOW_LATENCY: aaudio_performance_mode_t = 12;

pub const AAUDIO_STREAM_STATE_OPEN: aaudio_stream_state_t = 2;
pub const AAUDIO_STREAM_STATE_STARTING: aaudio_stream_state_t = 3;
pub const AAUDIO_STREAM_STATE_STARTED: aaudio_stream_state_t = 4;
pub const AAUDIO_STREAM_STATE_PAUSING: aaudio_stream_state_t = 5;
pub const AAUDIO_STREAM_STATE_PAUSED: aaudio_stream_state_t = 6;
pub const AAUDIO_STREAM_STATE_FLUSHED: aaudio_stream_state_t = 8;
pub const AAUDIO_STREAM_STATE_STOPPING: aaudio_stream_state_t = 10;
pub const AAUDIO_STREAM_STATE_STOPPED: aaudio_stream_state_t = 11;
pub const AAUDIO_STREAM_STATE_DISCONNECTED: aaudio_stream_state_t = 13;

pub type AAudioStream = c_void;
pub type AAudioStreamBuilder = c_void;

pub type AAudioStream_dataCallback = Option<
    extern "C" fn(
        stream: *mut AAudioStream,
        user_data: *mut c_void,
        audio_data: *mut c_void,
        num_frames: c_int,
    ) -> aaudio_result_t,
>;
pub type AAudioStream_errorCallback = Option<
    extern "C" fn(stream: *mut AAudioStream, user_data: *mut c_void, error: aaudio_result_t),
>;

extern "C" {
    pub fn AAudio_createStreamBuilder(builder: *mut *mut AAudioStreamBuilder) -> aaudio_result_t;
    pub fn AAudioStreamBuilder_delete(builder: *mut AAudioStreamBuilder);

    pub fn AAudioStreamBuilder_setDirection(
        builder: *mut AAudioStreamBuilder,
        direction: aaudio_direction_t,
    );
    pub fn AAudioStreamBuilder_setSharingMode(
        builder: *mut AAudioStreamBuilder,
        mode: aaudio_sharing_mode_t,
    );
    pub fn AAudioStreamBuilder_setPerformanceMode(
        builder: *mut AAudioStreamBuilder,
        mode: aaudio_performance_mode_t,
    );
    pub fn AAudioStreamBuilder_setSampleRate(builder: *mut AAudioStreamBuilder, rate: c_int);
    pub fn AAudioStreamBuilder_setChannelCount(builder: *mut AAudioStreamBuilder, channels: c_int);
    pub fn AAudioStreamBuilder_setFormat(
        builder: *mut AAudioStreamBuilder,
        format: aaudio_format_t,
    );
    pub fn AAudioStreamBuilder_setDataCallback(
        builder: *mut AAudioStreamBuilder,
        cb: AAudioStream_dataCallback,
    );
    pub fn AAudioStreamBuilder_setErrorCallback(
        builder: *mut AAudioStreamBuilder,
        cb: AAudioStream_errorCallback,
    );

    pub fn AAudioStreamBuilder_openStream(
        builder: *mut AAudioStreamBuilder,
        stream: *mut *mut AAudioStream,
    ) -> aaudio_result_t;

    pub fn AAudioStream_requestStart(stream: *mut AAudioStream) -> aaudio_result_t;
    pub fn AAudioStream_requestPause(stream: *mut AAudioStream) -> aaudio_result_t;
    pub fn AAudioStream_requestStop(stream: *mut AAudioStream) -> aaudio_result_t;
    pub fn AAudioStream_requestFlush(stream: *mut AAudioStream) -> aaudio_result_t;
    pub fn AAudioStream_close(stream: *mut AAudioStream) -> aaudio_result_t;

    pub fn AAudioStream_getSampleRate(stream: *mut AAudioStream) -> c_int;
    pub fn AAudioStream_getFramesPerBurst(stream: *mut AAudioStream) -> c_int;
    pub fn AAudioStream_getState(stream: *mut AAudioStream) -> aaudio_stream_state_t;

    pub fn AAudioStream_getSharingMode(stream: *mut AAudioStream) -> c_int;
    pub fn AAudioStream_getPerformanceMode(stream: *mut AAudioStream) -> c_int;
    pub fn AAudioStream_setBufferSizeInFrames(stream: *mut AAudioStream, frames: c_int) -> c_int;

    pub fn AAudio_convertResultToText(result: aaudio_result_t) -> *const std::os::raw::c_char;
}

/// Maximum scratch block the callback may hand us; larger requests are chunked.
pub const MAX_SCRATCH_FRAMES: usize = 2048;
