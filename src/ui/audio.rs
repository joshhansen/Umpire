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

// Currently supports i8, i32, f32.
pub type AudioSample = f32;
pub type Input = AudioSample;
pub type Output = AudioSample;

const CHANNELS: i32 = 2;
// const FRAMES: u32 = 64;
const SAMPLE_HZ: f64 = 44_100.0;

use std::sync::mpsc::Receiver;



// use portaudio as pa;
use pitch_calc::{Letter, LetterOctave};

use crate::{
    game::unit::UnitType,
};


pub enum Sounds {
    Silence,
    Unit(UnitType),
    Intro,

    /// Quit playing sounds
    Exit,
}

pub fn play_sounds(rx: Receiver<Sounds>, initial_sound: Sounds) -> Result<(), failure::Error> {
    // // Construct our fancy Synth!
    // let mut synth = {
        

    //     // The following envelopes should create a downward pitching sine wave that gradually quietens.
    //     // Try messing around with the points and adding some of your own!
    //     let amp_env = Envelope::from(vec!(
    //         //         Time ,  Amp ,  Curve
    //         Point::new(0.0  ,  0.0 ,  0.0),
    //         Point::new(0.01 ,  1.0 ,  0.0),
    //         Point::new(0.45 ,  1.0 ,  0.0),
    //         Point::new(0.81 ,  0.8 ,  0.0),
    //         Point::new(1.0  ,  0.0 ,  0.0),
    //     ));
    //     let freq_env = Envelope::from(vec!(
    //         //         Time    , Freq   , Curve
    //         Point::new(0.0     , 0.0    , 0.0),
    //         Point::new(0.00136 , 1.0    , 0.0),
    //         Point::new(0.015   , 0.02   , 0.0),
    //         Point::new(0.045   , 0.005  , 0.0),
    //         Point::new(0.1     , 0.0022 , 0.0),
    //         Point::new(0.35    , 0.0011 , 0.0),
    //         Point::new(1.0     , 0.0    , 0.0),
    //     ));

    //     // Now we can create our oscillator from our envelopes.
    //     // There are also Sine, Noise, NoiseWalk, SawExp and Square waveforms.
    //     let oscillator = Oscillator::new(oscillator::waveform::Square, amp_env, freq_env, ());

    //     // Here we construct our Synth from our oscillator.
    //     Synth::retrigger(())
    //         .oscillator(oscillator) // Add as many different oscillators as desired.
    //         .duration(6000.0) // Milliseconds.
    //         .base_pitch(LetterOctave(Letter::C, 1).hz()) // Hz.
    //         .loop_points(0.49, 0.51) // Loop start and end points.
    //         .fade(500.0, 500.0) // Attack and Release in milliseconds.
    //         .num_voices(16) // By default Synth is monophonic but this gives it `n` voice polyphony.
    //         .volume(0.2)
    //         .detune(0.5)
    //         .spread(1.0)

    //     // Other methods include:
    //         // .loop_start(0.0)
    //         // .loop_end(1.0)
    //         // .attack(ms)
    //         // .release(ms)
    //         // .note_freq_generator(nfg)
    //         // .oscillators([oscA, oscB, oscC])
    //         // .volume(1.0)
    // };

    // // Construct a note for the synth to perform. Have a play around with the pitch and duration!
    // let note = LetterOctave(Letter::C, 1);
    // let note_velocity = 1.0;
    // synth.note_on(note, note_velocity);

    // // We'll call this to release the note after 4 seconds.
    // let note_duration = 4.0;
    // let mut is_note_off = false;

    // // We'll use this to keep track of time and break from the loop after 6 seconds.
    // let mut timer: f64 = 0.0;

    // // This will be used to determine the delta time between calls to the callback.
    // let mut prev_time = None;

    // // The callback we'll use to pass to the Stream.
    // let callback = move |pa::OutputStreamCallbackArgs { buffer, time, .. }| {
    //     let buffer: &mut [[f32; CHANNELS as usize]] = sample::slice::to_frame_slice_mut(buffer).unwrap();
    //     sample::slice::equilibrium(buffer);

    //     synth.fill_slice(buffer, SAMPLE_HZ as f64);

    //     // println!("{}", timer);
    //     if timer < 6.0 {

    //         let last_time = prev_time.unwrap_or(time.current);
    //         let dt = time.current - last_time;
    //         timer += dt;
    //         prev_time = Some(time.current);

    //         // Once the timer exceeds our note duration, send the note_off.
    //         if timer > note_duration && !is_note_off {
    //             synth.note_off(note);
    //             is_note_off = true;
    //         }
    //         pa::Continue
    //     } else {
    //         pa::Complete
    //     }
    // };

    // // Construct PortAudio and the stream.
    // let pa = pa::PortAudio::new()?;
    // let settings = pa.default_output_stream_settings::<f32>(CHANNELS, SAMPLE_HZ, FRAMES)?;
    // let mut stream = pa.open_non_blocking_stream(settings, callback)?;
    // stream.start()?;

    // // Loop while the stream is active.
    // while let Ok(true) = stream.is_active() {
    //     // std::thread::sleep(std::time::Duration::from_millis(16));
    // }

    // Ok(())



    // Construct our fancy Synth!
    let mut synth = {
        
        use synth::{Point, Oscillator, oscillator, Envelope};

        // The following envelopes should create a downward pitching sine wave that gradually quietens.
        // Try messing around with the points and adding some of your own!
        let amp_env = Envelope::from(vec!(
            //         Time ,  Amp ,  Curve
            Point::new(0.0  ,  0.0 ,  0.0),
            Point::new(0.01 ,  1.0 ,  0.0),
            Point::new(0.45 ,  1.0 ,  0.0),
            Point::new(0.81 ,  0.8 ,  0.0),
            Point::new(1.0  ,  0.0 ,  0.0),
        ));
        let freq_env = Envelope::from(vec!(
            //         Time    , Freq   , Curve
            Point::new(0.0     , 0.0    , 0.0),
            Point::new(0.00136 , 1.0    , 0.0),
            Point::new(0.015   , 0.02   , 0.0),
            Point::new(0.045   , 0.005  , 0.0),
            Point::new(0.1     , 0.0022 , 0.0),
            Point::new(0.35    , 0.0011 , 0.0),
            Point::new(1.0     , 0.0    , 0.0),
        ));

        // Now we can create our oscillator from our envelopes.
        // There are also Sine, Noise, NoiseWalk, SawExp and Square waveforms.
        let oscillator = Oscillator::new(oscillator::waveform::Square, amp_env, freq_env, ());

        // Here we construct our Synth from our oscillator.
        Synth::retrigger(())
            .oscillator(oscillator) // Add as many different oscillators as desired.
            .duration(6000.0) // Milliseconds.
            .base_pitch(LetterOctave(Letter::C, 1).hz()) // Hz.
            .loop_points(0.49, 0.51) // Loop start and end points.
            .fade(500.0, 500.0) // Attack and Release in milliseconds.
            .num_voices(16) // By default Synth is monophonic but this gives it `n` voice polyphony.
            .volume(0.2)
            .detune(0.5)
            .spread(1.0)

        // Other methods include:
            // .loop_start(0.0)
            // .loop_end(1.0)
            // .attack(ms)
            // .release(ms)
            // .note_freq_generator(nfg)
            // .oscillators([oscA, oscB, oscC])
            // .volume(1.0)
    };

    // Construct a note for the synth to perform. Have a play around with the pitch and duration!
    let note = LetterOctave(Letter::C, 1);
    let note_velocity = 1.0;
    synth.note_on(note, note_velocity);


    let host = cpal::default_host();
    let device = host.default_output_device().expect("failed to find a default output device");
    let format = device.default_output_format()?;



    // The callback we'll use to pass to the Stream.
    let mut callback = |data: &mut cpal::StreamData| {


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
    };

    
    
    let event_loop = host.event_loop();
    let stream_id = event_loop.build_output_stream(&device, &format)?;
    event_loop.play_stream(stream_id.clone())?;

    event_loop.run(move |_id, result| {
        let mut data = result.unwrap();

        callback(&mut data);
    });

    Ok(())
}





// extern crate cpal;
// extern crate failure;
// extern crate pitch_calc as pitch;
// extern crate sample;
// extern crate synth;


// use cpal::traits::{DeviceTrait, EventLoopTrait, HostTrait};
// // use portaudio as pa;
// use pitch::{Letter, LetterOctave};
// use synth::Synth;

// // Currently supports i8, i32, f32.
// pub type AudioSample = f32;
// pub type Input = AudioSample;
// pub type Output = AudioSample;

// const CHANNELS: i32 = 2;
// // const FRAMES: u32 = 64;
// const SAMPLE_HZ: f64 = 44_100.0;

// fn main() {
//     run().unwrap()
// }


// fn run() -> Result<(), failure::Error> {

//     // Construct our fancy Synth!
//     let mut synth = {
        
//         use synth::{Point, Oscillator, oscillator, Envelope};

//         // The following envelopes should create a downward pitching sine wave that gradually quietens.
//         // Try messing around with the points and adding some of your own!
//         let amp_env = Envelope::from(vec!(
//             //         Time ,  Amp ,  Curve
//             Point::new(0.0  ,  0.0 ,  0.0),
//             Point::new(0.01 ,  1.0 ,  0.0),
//             Point::new(0.45 ,  1.0 ,  0.0),
//             Point::new(0.81 ,  0.8 ,  0.0),
//             Point::new(1.0  ,  0.0 ,  0.0),
//         ));
//         let freq_env = Envelope::from(vec!(
//             //         Time    , Freq   , Curve
//             Point::new(0.0     , 0.0    , 0.0),
//             Point::new(0.00136 , 1.0    , 0.0),
//             Point::new(0.015   , 0.02   , 0.0),
//             Point::new(0.045   , 0.005  , 0.0),
//             Point::new(0.1     , 0.0022 , 0.0),
//             Point::new(0.35    , 0.0011 , 0.0),
//             Point::new(1.0     , 0.0    , 0.0),
//         ));

//         // Now we can create our oscillator from our envelopes.
//         // There are also Sine, Noise, NoiseWalk, SawExp and Square waveforms.
//         let oscillator = Oscillator::new(oscillator::waveform::Square, amp_env, freq_env, ());

//         // Here we construct our Synth from our oscillator.
//         Synth::retrigger(())
//             .oscillator(oscillator) // Add as many different oscillators as desired.
//             .duration(6000.0) // Milliseconds.
//             .base_pitch(LetterOctave(Letter::C, 1).hz()) // Hz.
//             .loop_points(0.49, 0.51) // Loop start and end points.
//             .fade(500.0, 500.0) // Attack and Release in milliseconds.
//             .num_voices(16) // By default Synth is monophonic but this gives it `n` voice polyphony.
//             .volume(0.2)
//             .detune(0.5)
//             .spread(1.0)

//         // Other methods include:
//             // .loop_start(0.0)
//             // .loop_end(1.0)
//             // .attack(ms)
//             // .release(ms)
//             // .note_freq_generator(nfg)
//             // .oscillators([oscA, oscB, oscC])
//             // .volume(1.0)
//     };

//     // Construct a note for the synth to perform. Have a play around with the pitch and duration!
//     let note = LetterOctave(Letter::C, 1);
//     let note_velocity = 1.0;
//     synth.note_on(note, note_velocity);


//     let host = cpal::default_host();
//     let device = host.default_output_device().expect("failed to find a default output device");
//     let format = device.default_output_format()?;



//     // The callback we'll use to pass to the Stream.
//     let mut callback = |data: &mut cpal::StreamData| {


//         match data {
//             cpal::StreamData::Output { buffer: cpal::UnknownTypeOutputBuffer::U16(ref mut buffer) } => {
//                 let buffer: &mut [u16] = &mut *buffer;

//                 let buffer: &mut [[u16; CHANNELS as usize]] = sample::slice::to_frame_slice_mut(buffer).unwrap();

//                 sample::slice::equilibrium(buffer);
//                 synth.fill_slice(buffer, SAMPLE_HZ as f64);
//             },
//             cpal::StreamData::Output { buffer: cpal::UnknownTypeOutputBuffer::I16(ref mut buffer) } => {
//                 let buffer: &mut [i16] = &mut *buffer;

//                 let buffer: &mut [[i16; CHANNELS as usize]] = sample::slice::to_frame_slice_mut(buffer).unwrap();

//                 sample::slice::equilibrium(buffer);
//                 synth.fill_slice(buffer, SAMPLE_HZ as f64);
//             },
//             cpal::StreamData::Output { buffer: cpal::UnknownTypeOutputBuffer::F32(ref mut buffer) } => {
//                 let buffer: &mut [f32] = &mut *buffer;

//                 let buffer: &mut [[f32; CHANNELS as usize]] = sample::slice::to_frame_slice_mut(buffer).unwrap();

//                 sample::slice::equilibrium(buffer);
//                 synth.fill_slice(buffer, SAMPLE_HZ as f64);
//             },
//             _ => {
//                 eprintln!("Unsupported output stream format");
//             },
//         }
//     };

    
    
//     let event_loop = host.event_loop();
//     let stream_id = event_loop.build_output_stream(&device, &format)?;
//     event_loop.play_stream(stream_id.clone())?;

//     event_loop.run(move |_id, result| {
//         let mut data = result.unwrap();

//         callback(&mut data);
//     });
// }