pub mod metronome;

use metronome::Metronome;
use sonora_core::{ClipData, SongSession, TICKS_PER_BEAT};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportState {
    Stopped,
    Playing,
    Paused,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScheduledEvent {
    NoteOn {
        track_idx: usize,
        note: u8,
        velocity: u8,
    },
    NoteOff {
        track_idx: usize,
        note: u8,
    },
    DrumTrigger {
        track_idx: usize,
        channel: u8,
        velocity: u8,
    },
}

pub struct Sequencer {
    sample_rate: f32,
    pub state: TransportState,
    pub current_tick: u64,
    tick_accumulator: f64,
    pub metronome: Metronome,
    last_beat: u64,
}

impl Sequencer {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            state: TransportState::Stopped,
            current_tick: 0,
            tick_accumulator: 0.0,
            metronome: Metronome::new(sample_rate),
            last_beat: u64::MAX,
        }
    }

    /// Re-time the clock + metronome when the output device rate changes (Android stream reopen).
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        if sample_rate > 0.0 {
            self.sample_rate = sample_rate;
            self.metronome.set_sample_rate(sample_rate);
        }
    }

    pub fn play(&mut self) {
        self.state = TransportState::Playing;
    }

    pub fn pause(&mut self) {
        self.state = TransportState::Paused;
    }

    pub fn stop(&mut self) {
        self.state = TransportState::Stopped;
        self.current_tick = 0;
        self.tick_accumulator = 0.0;
        self.last_beat = u64::MAX;
    }

    pub fn seek_tick(&mut self, tick: u64) {
        self.current_tick = tick;
        self.tick_accumulator = 0.0;
        self.last_beat = u64::MAX;
    }

    pub fn seek_bar(&mut self, bar: u32, session: &SongSession) {
        let tick = bar as u64 * session.ticks_per_bar();
        self.seek_tick(tick);
    }

    /// Advance the sequencer by one sample frame and collect any events occurring on this frame
    #[inline(always)]
    pub fn advance_frame(&mut self, session: &SongSession, events: &mut Vec<ScheduledEvent>) {
        if self.state != TransportState::Playing {
            return;
        }

        let ticks_per_frame =
            (session.bpm as f64 * TICKS_PER_BEAT as f64) / (self.sample_rate as f64 * 60.0);
        self.tick_accumulator += ticks_per_frame;

        if self.tick_accumulator >= 1.0 {
            let ticks_to_advance = self.tick_accumulator.floor() as u64;
            self.tick_accumulator -= ticks_to_advance as f64;

            let prev_tick = self.current_tick;
            let mut next_tick = prev_tick + ticks_to_advance;

            let mut wrapped = false;

            // Handle Loop region
            if session.loop_enabled {
                let loop_start_tick = session.loop_start_bar as u64 * session.ticks_per_bar();
                let loop_end_tick = session.loop_end_bar as u64 * session.ticks_per_bar();
                if loop_end_tick > loop_start_tick && next_tick >= loop_end_tick {
                    let overshoot = next_tick - loop_end_tick;
                    // Fire everything between the previous tick and the wrap
                    // point, then re-fire the loop head the overshoot skipped
                    // (otherwise downbeat events at loop_start are dropped on
                    // every wrap).
                    self.collect_events_in_range(prev_tick, loop_end_tick, session, events);
                    next_tick = loop_start_tick + overshoot;
                    self.collect_events_in_range(loop_start_tick, next_tick, session, events);
                    wrapped = true;
                }
            }

            self.current_tick = next_tick;

            // Check metronome beat trigger
            let current_beat = self.current_tick / TICKS_PER_BEAT;
            if current_beat != self.last_beat {
                self.last_beat = current_beat;
                let is_downbeat =
                    current_beat.is_multiple_of(session.time_signature.numerator as u64);
                self.metronome.trigger_beat(is_downbeat);
            }

            // Collect notes and triggers occurring in [prev_tick, next_tick)
            if !wrapped {
                self.collect_events_in_range(prev_tick, next_tick, session, events);
            }
        }
    }

    fn collect_events_in_range(
        &self,
        start_tick: u64,
        end_tick: u64,
        session: &SongSession,
        events: &mut Vec<ScheduledEvent>,
    ) {
        for (track_idx, track) in session.tracks.iter().enumerate() {
            if track.muted {
                continue;
            }

            for clip in &track.clips {
                let clip_start = clip.start_tick;
                let clip_end = clip_start + clip.duration_ticks;

                // Check overlap with clip (inclusive of clip_end for note-offs)
                if start_tick > clip_end || end_tick <= clip_start {
                    continue;
                }

                match &clip.data {
                    ClipData::Midi(notes) => {
                        for note in notes {
                            let note_start = clip_start + note.start_tick;
                            let note_end = note_start + note.duration_ticks;

                            if start_tick <= note_start && note_start < end_tick {
                                events.push(ScheduledEvent::NoteOn {
                                    track_idx,
                                    note: note.note_number,
                                    velocity: note.velocity,
                                });
                            }

                            if start_tick <= note_end && note_end < end_tick {
                                events.push(ScheduledEvent::NoteOff {
                                    track_idx,
                                    note: note.note_number,
                                });
                            }
                        }
                    }
                    ClipData::DrumPattern(pat) => {
                        let step_duration_ticks = TICKS_PER_BEAT / 4; // 16th note steps = 240 ticks
                        for step in &pat.steps {
                            let step_tick = clip_start + (step.step as u64 * step_duration_ticks);
                            if start_tick <= step_tick && step_tick < end_tick {
                                events.push(ScheduledEvent::DrumTrigger {
                                    track_idx,
                                    channel: step.channel,
                                    velocity: step.velocity,
                                });
                            }
                        }
                    }
                    ClipData::Audio(_) => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sonora_core::{Clip, MidiNote};

    /// Minimal session (single piano track, no demo content) so event counts
    /// are exactly predictable.
    fn bare_session() -> SongSession {
        let mut session = SongSession::default();
        session.tracks.truncate(1); // piano only
        session.tracks[0].clips.clear(); // drop the demo arrangement
        session
    }

    #[test]
    fn test_sequencer_transport_and_events() {
        let mut session = bare_session();
        let notes = vec![
            MidiNote::new(60, 0, 480, 100),
            MidiNote::new(64, 480, 480, 110),
        ];
        session.tracks[0]
            .clips
            .push(Clip::new_midi("Test Clip", 0, 960, notes));

        let mut seq = Sequencer::new(48000.0);
        seq.play();

        let mut events = Vec::new();
        // Advance 48000 frames (1 second at 120 bpm = 2 beats = 1920 ticks)
        for _ in 0..48000 {
            seq.advance_frame(&session, &mut events);
        }

        assert!(events.len() >= 4); // NoteOn 60, NoteOff 60, NoteOn 64, NoteOff 64
        assert!(matches!(events[0], ScheduledEvent::NoteOn { note: 60, .. }));
    }

    /// Regression: an event sitting exactly on the loop start (the downbeat)
    /// must re-fire on every wrap, not just the first pass. The pre-fix
    /// sequencer skipped `[loop_start, next)` after wrapping, so the downbeat
    /// went silent from the second pass onwards.
    #[test]
    fn test_loop_wrap_fires_downbeat_events() {
        let mut session = bare_session();
        session.loop_enabled = true;
        session.loop_start_bar = 0;
        session.loop_end_bar = 1;
        session.tracks[0].clips.push(Clip::new_midi(
            "Loop Head",
            0,
            960,
            vec![MidiNote::new(60, 0, 480, 100)],
        ));

        let mut seq = Sequencer::new(48000.0);
        seq.play();

        // 240000 frames @ 48 kHz / 120 bpm = 9600 ticks = 2.5 passes of the
        // 1-bar loop → the tick-0 downbeat must have fired exactly 3 times.
        let mut events = Vec::new();
        for _ in 0..240_000 {
            seq.advance_frame(&session, &mut events);
        }

        let note_ons = events
            .iter()
            .filter(|e| matches!(e, ScheduledEvent::NoteOn { note: 60, .. }))
            .count();
        assert_eq!(
            note_ons, 3,
            "downbeat NoteOn must re-fire on every loop wrap (got {note_ons})"
        );
    }
}
