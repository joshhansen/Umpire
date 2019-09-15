extern crate cpal;
extern crate failure;
extern crate pitch_calc;
extern crate sample;
extern crate synth;


use cpal::traits::{DeviceTrait, EventLoopTrait, HostTrait};
// use portaudio as pa;
use synth::{
    Envelope,
    Oscillator,
    Point,
    Synth,
    oscillator
};

const CHANNELS: i32 = 2;
// const FRAMES: u32 = 64;
const SAMPLE_HZ: f64 = 44_100.0;

use std::sync::mpsc::Receiver;


use pitch_calc::{Letter, LetterOctave};

use crate::{
    game::unit::UnitType,
};


pub enum Sounds {
    Silence,
    Unit(UnitType),
    Intro,
}

pub trait Noisy {
    fn freq(&self) -> Option<f32>;
}

impl Noisy for UnitType {
    fn freq(&self) -> Option<f32> {
        match self {
            UnitType::Infantry => None,
            UnitType::Armor => Some(LetterOctave(Letter::C, 3).hz()),
            UnitType::Fighter => Some(LetterOctave(Letter::C, 2).hz()),
            UnitType::Bomber => Some(LetterOctave(Letter::F, 2).hz()),
            UnitType::Transport => Some(LetterOctave(Letter::C, 3).hz()),
            UnitType::Destroyer => Some(LetterOctave(Letter::C, 3).hz()),
            UnitType::Submarine => Some(LetterOctave(Letter::C, 3).hz()),
            UnitType::Cruiser => Some(LetterOctave(Letter::C, 3).hz()),
            UnitType::Battleship => Some(LetterOctave(Letter::C, 3).hz()),
            UnitType::Carrier => Some(LetterOctave(Letter::C, 3).hz()),
        }
    }
}

fn synth_for_sound(sound: Sounds) -> Synth<synth::instrument::mode::Mono, (), synth::oscillator::waveform::Square, Envelope, Envelope, ()> {
    match sound {
        Sounds::Silence => Synth::retrigger(()),
        Sounds::Unit(unit_type) => {

            if let Some(freq) = unit_type.freq() {

                // The following envelopes should create a downward pitching sine wave that gradually quietens.
                // Try messing around with the points and adding some of your own!
                let amp_env = Envelope::from(vec!(
                    //         Time ,  Amp ,  Curve
                    Point::new(0.0  ,  0.6  ,  0.0),
                    Point::new(0.5  ,  0.7  ,  0.0),
                    Point::new(1.0  ,  0.6  ,  0.0),
                ));
                let freq_env = Envelope::from(vec!(
                    //         Time    , Freq   , Curve
                    Point::new(0.0     , 0.4999999    , 0.0),
                    Point::new(0.5     , 0.5     , 0.0),
                    Point::new(1.0     , 0.4999999    , 0.0),
                ));

                // Now we can create our oscillator from our envelopes.
                // There are also Sine, Noise, NoiseWalk, SawExp and Square waveforms.
                let oscillator = Oscillator::new(oscillator::waveform::Square, amp_env, freq_env, ());

                // Here we construct our Synth from our oscillator.
                Synth::retrigger(())
                    .oscillator(oscillator) // Add as many different oscillators as desired.
                    .duration(6000.0) // Milliseconds.
                    .base_pitch(freq) // Hz.
                    .loop_points(0.0, 1.0) // Loop start and end points.
                    .fade(500.0, 500.0) // Attack and Release in milliseconds.
                    .num_voices(16) // By default Synth is monophonic but this gives it `n` voice polyphony.
                    .volume(0.2)
                    // .detune(0.5)
                    // .spread(1.0)

                // Other methods include:
                    // .loop_start(0.0)
                    // .loop_end(1.0)
                    // .attack(ms)
                    // .release(ms)
                    // .note_freq_generator(nfg)
                    // .oscillators([oscA, oscB, oscC])
                    // .volume(1.0)
            } else {
                Synth::retrigger(())
            }
        },
        Sounds::Intro => {
            unimplemented!();
        }

    }
}

pub fn play_sounds(rx: Receiver<Sounds>, sound: Sounds) -> Result<(), failure::Error> {

    let mut synth = synth_for_sound(sound);

    // Construct a note for the synth to perform. Have a play around with the pitch and duration!
    let note = LetterOctave(Letter::C, 1);
    let note_velocity = 1.0;
    synth.note_on(note, note_velocity);


    let host = cpal::default_host();
    let device = host.default_output_device().expect("failed to find a default output device");
    let format = device.default_output_format()?;
    
    
    let event_loop = host.event_loop();
    let stream_id = event_loop.build_output_stream(&device, &format)?;
    event_loop.play_stream(stream_id.clone())?;

    event_loop.run(move |_id, result| {
        let mut data = result.unwrap();

        match rx.try_recv() {
            Ok(sound_) => {
                synth.stop();
                synth = synth_for_sound(sound_);
                synth.note_on(note, note_velocity);
            },
            Err(_) => {
                // do nothing
            },
        }


        match data {
            cpal::StreamData::Output { buffer: cpal::UnknownTypeOutputBuffer::U16(ref mut buffer) } => {
                let buffer: &mut [u16] = &mut *buffer;

                let buffer: &mut [[u16; CHANNELS as usize]] = sample::slice::to_frame_slice_mut(buffer).unwrap();

                sample::slice::equilibrium(buffer);
                synth.fill_slice(buffer, SAMPLE_HZ as f64);
            },
            cpal::StreamData::Output { buffer: cpal::UnknownTypeOutputBuffer::I16(ref mut buffer) } => {
                let buffer: &mut [i16] = &mut *buffer;

                let buffer: &mut [[i16; CHANNELS as usize]] = sample::slice::to_frame_slice_mut(buffer).unwrap();

                sample::slice::equilibrium(buffer);
                synth.fill_slice(buffer, SAMPLE_HZ as f64);
            },
            cpal::StreamData::Output { buffer: cpal::UnknownTypeOutputBuffer::F32(ref mut buffer) } => {
                let buffer: &mut [f32] = &mut *buffer;

                let buffer: &mut [[f32; CHANNELS as usize]] = sample::slice::to_frame_slice_mut(buffer).unwrap();

                sample::slice::equilibrium(buffer);
                synth.fill_slice(buffer, SAMPLE_HZ as f64);
            },
            _ => {
                eprintln!("Unsupported output stream format");
            },
        }
    });

    Ok(())
}
