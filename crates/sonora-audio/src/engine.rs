use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use crate::command::AudioEngineCommand;
use sonora_core::{SongSession, TrackType};
use sonora_dsp::{
    BiquadFilter, BrickwallLimiter, Distortion, FastFourierTransform, FilterType, LevelMeter,
    Reverb, StereoChorus, StereoCompressor, StereoDelay,
};
use sonora_instruments::{BassEngine, DrumEngine, GuitarEngine, PianoEngine, SynthEngine};
use sonora_sequencer::{ScheduledEvent, Sequencer};

pub struct AudioEngine {
    pub sample_rate: f32,
    pub session: SongSession,
    pub sequencer: Sequencer,

    // Instrument engines
    pub piano: PianoEngine,
    pub guitar: GuitarEngine,
    pub bass: BassEngine,
    pub synth: SynthEngine,
    pub drums: DrumEngine,

    // Master DSP chain
    pub master_chorus: StereoChorus,
    pub master_distortion: Distortion,
    pub master_filter: BiquadFilter,
    pub master_delay: StereoDelay,
    pub master_reverb: Reverb,
    pub master_compressor: StereoCompressor,
    pub master_limiter: BrickwallLimiter,
    pub master_meter: LevelMeter,

    // Telemetry & Visualizer buffers
    pub fft: FastFourierTransform,
    pub waveform_ring: Vec<f32>,
    pub waveform_write_idx: usize,
    pub fft_mags: Vec<f32>,

    // Event scratch buffer for zero allocation inside audio loop
    event_scratch: Vec<ScheduledEvent>,

    // Lock-free command queue
    command_queue: Arc<Mutex<VecDeque<AudioEngineCommand>>>,
}

impl AudioEngine {
    pub fn new(sample_rate: f32) -> Self {
        let session = SongSession::default();
        let sequencer = Sequencer::new(sample_rate);

        let piano = PianoEngine::new(sample_rate);
        let guitar = GuitarEngine::new(sample_rate);
        let bass = BassEngine::new(sample_rate);
        let synth = SynthEngine::new(sample_rate);
        let drums = DrumEngine::new(sample_rate);

        let master_chorus = StereoChorus::new(sample_rate);
        let master_distortion = Distortion::new();
        let master_filter = BiquadFilter::new(sample_rate, FilterType::Lowpass, 20000.0, 0.707);
        let master_delay = StereoDelay::new(sample_rate, 2.0);
        let master_reverb = Reverb::new(sample_rate);
        let master_compressor = StereoCompressor::new(sample_rate);
        let master_limiter = BrickwallLimiter::new(sample_rate, -0.3);
        let master_meter = LevelMeter::new(sample_rate);

        let fft = FastFourierTransform::new(256);
        let waveform_ring = vec![0.0f32; 1024];
        let fft_mags = vec![0.0f32; 128];

        Self {
            sample_rate,
            session,
            sequencer,
            piano,
            guitar,
            bass,
            synth,
            drums,
            master_chorus,
            master_distortion,
            master_filter,
            master_delay,
            master_reverb,
            master_compressor,
            master_limiter,
            master_meter,
            fft,
            waveform_ring,
            waveform_write_idx: 0,
            fft_mags,
            event_scratch: Vec::with_capacity(32),
            command_queue: Arc::new(Mutex::new(VecDeque::with_capacity(64))),
        }
    }

    pub fn get_command_sender(&self) -> Arc<Mutex<VecDeque<AudioEngineCommand>>> {
        Arc::clone(&self.command_queue)
    }

    pub fn send_command(&self, cmd: AudioEngineCommand) {
        if let Ok(mut q) = self.command_queue.lock() {
            if q.len() < 256 {
                q.push_back(cmd);
            }
        }
    }

    fn drain_commands(&mut self) {
        let mut cmds = Vec::new();
        if let Ok(mut q) = self.command_queue.try_lock() {
            while let Some(cmd) = q.pop_front() {
                cmds.push(cmd);
            }
        }
        for cmd in cmds {
            self.handle_command(cmd);
        }
    }

    fn handle_command(&mut self, cmd: AudioEngineCommand) {
        match cmd {
            AudioEngineCommand::Play => self.sequencer.play(),
            AudioEngineCommand::Pause => self.sequencer.pause(),
            AudioEngineCommand::Stop => {
                self.sequencer.stop();
                self.all_notes_off();
            }
            AudioEngineCommand::SeekTick(tick) => {
                self.sequencer.seek_tick(tick);
                self.all_notes_off();
            }
            AudioEngineCommand::LoadSession(session) => {
                self.sequencer.stop();
                self.all_notes_off();
                self.session = session;
                self.update_presets_from_session();
            }
            AudioEngineCommand::NoteOn {
                track_idx,
                note,
                velocity,
            } => {
                self.dispatch_note_on(track_idx, note, velocity);
            }
            AudioEngineCommand::NoteOff { track_idx, note } => {
                self.dispatch_note_off(track_idx, note);
            }
            AudioEngineCommand::TriggerDrum { channel, velocity } => {
                self.drums.trigger_channel(channel, velocity);
            }
            AudioEngineCommand::SetTrackVolume {
                track_idx,
                volume_db,
            } => {
                if let Some(t) = self.session.tracks.get_mut(track_idx) {
                    t.volume_db = volume_db;
                }
            }
            AudioEngineCommand::SetTrackPan { track_idx, pan } => {
                if let Some(t) = self.session.tracks.get_mut(track_idx) {
                    t.pan = pan;
                }
            }
            AudioEngineCommand::SetTrackMute { track_idx, muted } => {
                if let Some(t) = self.session.tracks.get_mut(track_idx) {
                    t.muted = muted;
                }
            }
            AudioEngineCommand::SetTrackSolo { track_idx, soloed } => {
                if let Some(t) = self.session.tracks.get_mut(track_idx) {
                    t.soloed = soloed;
                }
            }
            AudioEngineCommand::SetMasterVolume(vol_db) => {
                self.session.master_volume_db = vol_db;
            }
            AudioEngineCommand::SetMetronomeEnabled(en) => {
                self.sequencer.metronome.enabled = en;
            }
            AudioEngineCommand::SetMetronomeVolume(vol) => {
                self.sequencer.metronome.volume_db = vol;
            }
            AudioEngineCommand::SetSustainPedal(sus) => {
                self.piano.set_sustain(sus);
            }
            AudioEngineCommand::SetBpm(bpm) => {
                self.session.bpm = bpm.clamp(30.0, 300.0);
            }
            AudioEngineCommand::SetLoop {
                enabled,
                start_bar,
                end_bar,
            } => {
                self.session.loop_enabled = enabled;
                self.session.loop_start_bar = start_bar;
                self.session.loop_end_bar = end_bar;
            }
            AudioEngineCommand::SetDrumStep {
                clip_idx,
                channel,
                step,
                active,
                velocity,
            } => {
                // Locate the drum track (first Drums-type track) and toggle a step
                if let Some(track) = self
                    .session
                    .tracks
                    .iter_mut()
                    .find(|t| t.track_type == TrackType::Drums)
                {
                    if let Some(clip) = track.clips.get_mut(clip_idx) {
                        if let sonora_core::ClipData::DrumPattern(pat) = &mut clip.data {
                            if active {
                                if !pat
                                    .steps
                                    .iter()
                                    .any(|s| s.channel == channel && s.step == step)
                                {
                                    pat.steps.push(sonora_core::DrumStep {
                                        channel,
                                        step,
                                        velocity: velocity.clamp(1, 127),
                                    });
                                    pat.steps.sort_by_key(|s| (s.channel, s.step));
                                }
                            } else {
                                pat.steps
                                    .retain(|s| !(s.channel == channel && s.step == step));
                            }
                        }
                    }
                }
            }
            AudioEngineCommand::AddMidiNotes { track_idx, notes } => {
                // Append notes into a MIDI clip on the given track (creating one if absent)
                if let Some(track) = self.session.tracks.get_mut(track_idx) {
                    if let Some(clip) = track
                        .clips
                        .iter_mut()
                        .find(|c| matches!(c.data, sonora_core::ClipData::Midi(_)))
                    {
                        if let sonora_core::ClipData::Midi(existing) = &mut clip.data {
                            for n in notes {
                                let end = n.start_tick + n.duration_ticks;
                                if end > clip.duration_ticks {
                                    clip.duration_ticks = end;
                                }
                                existing.push(n);
                            }
                            existing.sort_by_key(|n| n.start_tick);
                        }
                    } else {
                        track
                            .clips
                            .push(sonora_core::Clip::new_midi("Recorded", 0, 3840, notes));
                    }
                }
            }
            AudioEngineCommand::SetPreset { instrument, preset } => {
                // Keep the session's preset_name in sync so save/load round-trips.
                let track_type = match instrument {
                    0 => Some(TrackType::Piano),
                    1 => Some(TrackType::Guitar),
                    2 => Some(TrackType::Bass),
                    3 => Some(TrackType::Synth),
                    4 => Some(TrackType::Drums),
                    _ => None,
                };
                if let Some(tt) = track_type {
                    if let Some(track) = self.session.tracks.iter_mut().find(|t| t.track_type == tt)
                    {
                        track.preset_name = preset.clone();
                    }
                }
                match instrument {
                    0 => self.piano.set_preset(&preset),
                    1 => self.guitar.set_preset(&preset),
                    2 => self.bass.set_preset(&preset),
                    3 => self.synth.set_preset(&preset),
                    4 => self.drums.set_kit(&preset),
                    _ => {}
                }
            }
            AudioEngineCommand::SetSynthLfo { rate_hz, depth } => {
                self.synth.lfo_rate_hz = rate_hz.clamp(0.05, 20.0);
                self.synth.lfo_depth = depth.clamp(0.0, 1.0);
            }
        }
    }

    fn update_presets_from_session(&mut self) {
        for track in &self.session.tracks {
            match track.track_type {
                TrackType::Piano => self.piano.set_preset(&track.preset_name),
                TrackType::Guitar => self.guitar.set_preset(&track.preset_name),
                TrackType::Bass => self.bass.set_preset(&track.preset_name),
                TrackType::Synth => self.synth.set_preset(&track.preset_name),
                TrackType::Drums => self.drums.set_kit(&track.preset_name),
                TrackType::Audio => {}
            }
        }
    }

    pub fn dispatch_note_on(&mut self, track_idx: usize, note: u8, velocity: u8) {
        if let Some(track) = self.session.tracks.get(track_idx) {
            match track.track_type {
                TrackType::Piano => self.piano.note_on(note, velocity),
                TrackType::Guitar => self.guitar.note_on(note, velocity),
                TrackType::Bass => self.bass.note_on(note, velocity),
                TrackType::Synth => self.synth.note_on(note, velocity),
                TrackType::Drums => self.drums.trigger_channel(note % 8, velocity),
                TrackType::Audio => {}
            }
        }
    }

    pub fn dispatch_note_off(&mut self, track_idx: usize, note: u8) {
        if let Some(track) = self.session.tracks.get(track_idx) {
            match track.track_type {
                TrackType::Piano => self.piano.note_off(note),
                TrackType::Guitar => self.guitar.note_off(note),
                TrackType::Bass => self.bass.note_off(note),
                TrackType::Synth => self.synth.note_off(note),
                TrackType::Drums => {}
                TrackType::Audio => {}
            }
        }
    }

    pub fn all_notes_off(&mut self) {
        self.piano.all_notes_off();
        self.guitar.all_notes_off();
        self.bass.all_notes_off();
        self.synth.all_notes_off();
        self.drums.all_notes_off();
    }

    /// Primary Real-Time Block Render Callback
    #[inline(always)]
    pub fn process_block(&mut self, out_left: &mut [f32], out_right: &mut [f32]) {
        assert_eq!(out_left.len(), out_right.len());
        self.drain_commands();

        let num_frames = out_left.len();
        let any_solo = self.session.tracks.iter().any(|t| t.soloed);
        let master_gain = 10.0f32.powf(self.session.master_volume_db / 20.0);

        for frame in 0..num_frames {
            // Sequencer frame clock advance
            self.event_scratch.clear();
            self.sequencer
                .advance_frame(&self.session, &mut self.event_scratch);

            // Execute scheduled sequencer events
            let count = self.event_scratch.len();
            for i in 0..count {
                let event = self.event_scratch[i].clone();
                match event {
                    ScheduledEvent::NoteOn {
                        track_idx,
                        note,
                        velocity,
                    } => {
                        self.dispatch_note_on(track_idx, note, velocity);
                    }
                    ScheduledEvent::NoteOff { track_idx, note } => {
                        self.dispatch_note_off(track_idx, note);
                    }
                    ScheduledEvent::DrumTrigger {
                        channel, velocity, ..
                    } => {
                        self.drums.trigger_channel(channel, velocity);
                    }
                }
            }

            // Synthesize each instrument
            let (piano_l, piano_r) = self.piano.process_sample();
            let (guitar_l, guitar_r) = self.guitar.process_sample();
            let (bass_l, bass_r) = self.bass.process_sample();
            let (synth_l, synth_r) = self.synth.process_sample();
            let (drums_l, drums_r) = self.drums.process_sample();

            let mut bus_l = 0.0f32;
            let mut bus_r = 0.0f32;

            // Route instruments through track mixer channels
            for track in self.session.tracks.iter() {
                if track.muted || (any_solo && !track.soloed) {
                    continue;
                }

                let (src_l, src_r) = match track.track_type {
                    TrackType::Piano => (piano_l, piano_r),
                    TrackType::Guitar => (guitar_l, guitar_r),
                    TrackType::Bass => (bass_l, bass_r),
                    TrackType::Synth => (synth_l, synth_r),
                    TrackType::Drums => (drums_l, drums_r),
                    TrackType::Audio => (0.0, 0.0),
                };

                let track_vol = 10.0f32.powf(track.volume_db / 20.0);
                // Equal-power 3dB stereo panning
                let pan = track.pan.clamp(-1.0, 1.0);
                let gain_l = ((1.0 - pan) * 0.5).sqrt() * track_vol;
                let gain_r = ((1.0 + pan) * 0.5).sqrt() * track_vol;

                bus_l += src_l * gain_l;
                bus_r += src_r * gain_r;
            }

            // Metronome addition
            let metro = self.sequencer.metronome.process_sample();
            bus_l += metro;
            bus_r += metro;

            // Apply Master Bus Effects
            let (chorus_l, chorus_r) = self.master_chorus.process(bus_l, bus_r);
            let dist_l = self.master_distortion.process_sample(chorus_l);
            let dist_r = self.master_distortion.process_sample(chorus_r);
            let filt_l = self.master_filter.process_sample(dist_l);
            let filt_r = self.master_filter.process_sample(dist_r);
            let (delay_l, delay_r) = self.master_delay.process(filt_l, filt_r);
            let (rev_l, rev_r) = self.master_reverb.process(delay_l, delay_r);
            let (comp_l, comp_r) = self.master_compressor.process(rev_l, rev_r);

            // Master Gain & Brickwall Peak Limiter
            let scaled_l = comp_l * master_gain;
            let scaled_r = comp_r * master_gain;
            let (final_l, final_r) = self.master_limiter.process(scaled_l, scaled_r);

            out_left[frame] = final_l;
            out_right[frame] = final_r;

            // Telemetry updates
            self.master_meter.process_sample(final_l, final_r);
            let mono_mix = (final_l + final_r) * 0.5;
            self.waveform_ring[self.waveform_write_idx] = mono_mix;
            self.waveform_write_idx = (self.waveform_write_idx + 1) % self.waveform_ring.len();
        }

        // Compute FFT magnitudes periodically
        self.fft
            .compute_magnitudes(&self.waveform_ring, &mut self.fft_mags);
    }
}
