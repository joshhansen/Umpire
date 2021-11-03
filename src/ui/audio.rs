use cpal::{
    traits::{DeviceTrait, EventLoopTrait, HostTrait}
};

use synth::{
    Envelope,
    Oscillator,
    Point,
    Synth,
    oscillator
};

use dasp;

const CHANNELS: i32 = 2;
const SAMPLE_HZ: f64 = 44_100.0;

use std::sync::mpsc::Receiver;


use pitch_calc::{Letter, LetterOctave};

use crate::{
    game::unit::UnitType,
};


pub(in crate::ui) enum Sounds {
    Silence,
    Unit(UnitType),
    // Intro,
}

pub(in crate::ui) trait Noisy {
    fn freqs(&self) -> Vec<f32>;
    fn volume(&self) -> f32;
}

impl Noisy for UnitType {
    fn freqs(&self) -> Vec<f32> {
        match self {
            UnitType::Infantry => Vec::new(),
            UnitType::Armor => vec![LetterOctave(Letter::C, 3).hz()],
            UnitType::Fighter => vec![LetterOctave(Letter::F, 3).hz()],
            UnitType::Bomber => vec![LetterOctave(Letter::D, 3).hz()],
            UnitType::Transport => vec![LetterOctave(Letter::C, 2).hz(), LetterOctave(Letter::G, 2).hz()],
            UnitType::Destroyer => vec![LetterOctave(Letter::D, 2).hz()],
            UnitType::Submarine => vec![LetterOctave(Letter::C, 2).hz()],
            UnitType::Cruiser => vec![LetterOctave(Letter::A, 1).hz(), LetterOctave(Letter::C, 1).hz()],
            UnitType::Battleship => vec![LetterOctave(Letter::C, 1).hz(), LetterOctave(Letter::Db, 1).hz()],
            UnitType::Carrier => vec![LetterOctave(Letter::C, 1).hz()],
        }
    }

    fn volume(&self) -> f32 {
        match self {
            UnitType::Infantry => 0.0,
            UnitType::Armor => 0.05,
            UnitType::Fighter => 0.15,
            UnitType::Bomber => 0.15,
            UnitType::Transport => 0.05,
            UnitType::Destroyer => 0.15,
            UnitType::Submarine => 0.15,
            UnitType::Cruiser => 0.15,
            UnitType::Battleship => 0.15,
            UnitType::Carrier => 0.15,
        }
    }
}

fn synth_for_sound(sound: Sounds) -> Synth<synth::instrument::mode::Poly, (), synth::oscillator::waveform::Square, Envelope, Envelope, ()> {
    match sound {
        Sounds::Silence => Synth::poly(()),
        Sounds::Unit(unit_type) => {
            let freqs = unit_type.freqs();
            if !freqs.is_empty() {

                let volume = unit_type.volume();

                // The following envelopes should create a downward pitching sine wave that gradually quietens.
                // Try messing around with the points and adding some of your own!
                let amp_env = Envelope::from(vec!(
                    //         Time ,  Amp ,  Curve
                    // Point::new(0.0  ,  0.6  ,  0.0),
                    // Point::new(0.5  ,  0.7  ,  0.0),
                    // Point::new(1.0  ,  0.6  ,  0.0),
                    Point::new(0.0, volume.into(), 0.0),
                    Point::new(1.0, volume.into(), 0.0),
                ));
                let freq_env = Envelope::from(vec!(
                    //         Time    , Freq   , Curve
                    Point::new(0.0     , 0.0    , 0.0),
                    Point::new(3000.0     , 0.01    , 0.0),
                    Point::new(6000.0     , 0.0    , 0.0),
                ));

                // Now we can create our oscillator from our envelopes.
                // There are also Sine, Noise, NoiseWalk, SawExp and Square waveforms.
                let oscillator = Oscillator::new(oscillator::waveform::Square, amp_env, freq_env, ());
                // let oscillator = Oscillator::new(oscillator::waveform::Sine, amp_env, freq_env, ());

                // Here we construct our Synth from our oscillator.
                let mut synth = Synth::poly(())
                    .oscillator(oscillator) // Add as many different oscillators as desired.
                    .duration(6000.0) // Milliseconds.
                    // .base_pitch(100.0) // Hz.
                    .loop_points(0.0, 1.0) // Loop start and end points.
                    // .fade(500.0, 500.0) // Attack and Release in milliseconds.
                    // .num_voices(16) // By default Synth is monophonic but this gives it `n` voice polyphony.
                    .num_voices(16)
                    .volume(volume)
                    // .detune(0.5)
                    // .spread(1.0)
                ;

                for freq in freqs {
                    synth.note_on(freq, 1.0);
                }

                // synth.note_on(freq, 1.0);
                // synth.note_on(freq+50.0, 1.0);
                // Other methods include:
                    // .loop_start(0.0)
                    // .loop_end(1.0)
                    // .attack(ms)
                    // .release(ms)
                    // .note_freq_generator(nfg)
                    // .oscillators([oscA, oscB, oscC])
                    // .volume(1.0)

                synth
            } else {
                Synth::poly(())
            }
        },
        // Sounds::Intro => {
        //     unimplemented!();
        // }

    }
}


pub(in crate::ui) fn play_sounds(rx: Receiver<Sounds>, sound: Sounds) -> Result<(), failure::Error> {

    let mut synth = synth_for_sound(sound);

    let host = cpal::default_host();
    let device = host.default_output_device().expect("failed to find a default output device");
    let format = device.default_output_format()?;
    
    
    let event_loop = host.event_loop();
    let stream_id = event_loop.build_output_stream(&device, &format)?;
    event_loop.play_stream(stream_id.clone())?;

    event_loop.run(move |_id, result| {
        let mut data = result.unwrap();

        // Check if we got a new assignment
        if let Ok(sound_) = rx.try_recv() {
            synth.stop();
            synth = synth_for_sound(sound_);
        }

        //FIXME deduplicate this duplicated code which varies only by a type known at runtime but not compile time
        match data {
            cpal::StreamData::Output { buffer: cpal::UnknownTypeOutputBuffer::U16(ref mut buffer) } => {
                let buffer: &mut [u16] = &mut *buffer;

                let buffer: &mut [[u16; CHANNELS as usize]] = dasp::slice::to_frame_slice_mut(buffer).unwrap();

                dasp::slice::equilibrium(buffer);
                synth.fill_slice(buffer, SAMPLE_HZ as f64);
            },
            cpal::StreamData::Output { buffer: cpal::UnknownTypeOutputBuffer::I16(ref mut buffer) } => {
                let buffer: &mut [i16] = &mut *buffer;

                let buffer: &mut [[i16; CHANNELS as usize]] = dasp::slice::to_frame_slice_mut(buffer).unwrap();

                dasp::slice::equilibrium(buffer);
                synth.fill_slice(buffer, SAMPLE_HZ as f64);
            },
            cpal::StreamData::Output { buffer: cpal::UnknownTypeOutputBuffer::F32(ref mut buffer) } => {
                let buffer: &mut [f32] = &mut *buffer;

                let buffer: &mut [[f32; CHANNELS as usize]] = dasp::slice::to_frame_slice_mut(buffer).unwrap();

                dasp::slice::equilibrium(buffer);
                synth.fill_slice(buffer, SAMPLE_HZ as f64);
            },
            _ => {
                eprintln!("Unsupported output stream format");
            },
        }
    });
}
