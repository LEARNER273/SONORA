//! Offline demo render: writes the shipped 8-bar demo session to a WAV file so
//! the engine content (instruments, sequencer, mixer, master chain) can be
//! auditioned on any audio player without an Android device.
//!
//! Run from the workspace root:
//!     cargo run --release -p sonora-renderer --example render_demo

use sonora_codecs::WavFormat;
use sonora_core::SongSession;
use sonora_renderer::SongRenderer;

fn main() {
    let session = SongSession::default();
    let total_ticks = session.ticks_per_bar() * session.loop_end_bar as u64;
    let seconds = total_ticks as f64 / 960.0 * 60.0 / f64::from(session.bpm);

    println!(
        "Rendering '{}' — {} bars, {:.1} s @ {:.0} BPM, {} tracks…",
        session.name,
        session.loop_end_bar,
        seconds,
        session.bpm,
        session.tracks.len()
    );

    let bytes = SongRenderer::render_song(&session, 48_000, WavFormat::Pcm16, |_| {})
        .expect("offline render failed");

    let out = "artifacts/demo_render.wav";
    std::fs::write(out, &bytes).expect("write wav");
    println!(
        "Wrote {out} ({} bytes) — open it with any audio player",
        bytes.len()
    );
}
