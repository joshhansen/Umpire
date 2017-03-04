//! Play a sine wave for several seconds.
//!
//! A rusty adaptation of the official PortAudio C "paex_sine.c" example by Phil Burk and Ross
//! Bencina.

use std;

use pa;
use sample::{Frame, Sample, Signal, ToFrameSliceMut};
use sample::signal;

use unit::{Unit,UnitType};

const FRAMES_PER_BUFFER: u32 = 512;
const NUM_CHANNELS: i32 = 1;
const SAMPLE_RATE: f64 = 44_100.0;

fn main() {
    run().unwrap();
}

pub fn run() -> Result<(), pa::Error> {

    // Create a signal chain to play back 1 second of each oscillator at A4.
    let hz = signal::rate(SAMPLE_RATE).const_hz(15.0);
    let one_sec = SAMPLE_RATE as usize;
    let mut signal = hz.clone().sine().take(one_sec)
    .map(|f| f.map(|s| s.to_sample::<f32>()))
    .scale_amp(0.2);

    // let mut signal: () = hz.clone().sine().take(one_sec)
    //     .chain(hz.clone().saw().take(one_sec))
    //     .chain(hz.clone().square().take(one_sec))
    //     .chain(hz.clone().noise_simplex().take(one_sec))
    //     .chain(signal::noise(0).take(one_sec))
    //     .map(|f| f.map(|s| s.to_sample::<f32>()))
    //     .scale_amp(0.2);

    // Initialise PortAudio.
    let pa = try!(pa::PortAudio::new());
    let settings = try!(pa.default_output_stream_settings::<f32>(NUM_CHANNELS,
                                                                 SAMPLE_RATE,
                                                                 FRAMES_PER_BUFFER));

    // Define the callback which provides PortAudio the audio.
    let callback = move |pa::OutputStreamCallbackArgs { buffer, .. }| {
        let buffer: &mut [[f32; 1]] = buffer.to_frame_slice_mut().unwrap();
        for out_frame in buffer {
            match signal.next() {
                Some(frame) => *out_frame = frame,
                None => return pa::Complete,
            }
        }
        pa::Continue
    };

    let mut stream: pa::Stream<pa::NonBlocking,pa::Output<f32>> = try!(pa.open_non_blocking_stream(settings, callback));
    try!(stream.start());

    while let Ok(true) = stream.is_active() {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    try!(stream.stop());
    try!(stream.close());

    Ok(())
}

pub fn make_noise(amp: f32, freq: f64) -> Result<pa::Stream<pa::NonBlocking,pa::Output<f32>>,pa::Error> {
    let hz = signal::rate(SAMPLE_RATE).const_hz(freq);
    let one_sec = 10 * SAMPLE_RATE as usize;
    let mut signal = hz.clone().sine().take(one_sec)
    .map(|f| f.map(|s| s.to_sample::<f32>()))
    .scale_amp(amp);

    // Initialise PortAudio.
    let pa = try!(pa::PortAudio::new());
    let settings = try!(pa.default_output_stream_settings::<f32>(NUM_CHANNELS,
                                                                 SAMPLE_RATE,
                                                                 FRAMES_PER_BUFFER));

    // Define the callback which provides PortAudio the audio.
    let callback = move |pa::OutputStreamCallbackArgs { buffer, .. }| {
        let buffer: &mut [[f32; 1]] = buffer.to_frame_slice_mut().unwrap();
        for out_frame in buffer {
            match signal.next() {
                Some(frame) => *out_frame = frame,
                None => return pa::Complete,
            }
        }
        pa::Continue
    };

    let mut stream: pa::Stream<pa::NonBlocking,pa::Output<f32>> = try!(pa.open_non_blocking_stream(settings, callback));
    try!(stream.start());

    Ok(stream)
}

pub trait Noisy {
    fn amp(&self) -> f32;
    fn freq(&self) -> f64;
    fn make_noise(&self) -> Result<pa::Stream<pa::NonBlocking,pa::Output<f32>>,pa::Error> {
        make_noise(self.amp(), self.freq())

        // while let Ok(true) = stream.is_active() {
        //     std::thread::sleep(std::time::Duration::from_millis(100));
        // }
        //
        // try!(stream.stop());
        // try!(stream.close());
        //
        // Ok(())
    }


}

impl Noisy for Unit {
    fn amp(&self) -> f32 {
        match self.type_ {
            UnitType::INFANTRY => 0.2,
            UnitType::ARMOR => 0.3,
            UnitType::FIGHTER => 0.2,
            UnitType::BOMBER => 0.3,
            UnitType::TRANSPORT => 0.4,
            UnitType::DESTROYER => 0.35,
            UnitType::SUBMARINE => 0.1,
            UnitType::CRUISER => 0.5,
            UnitType::BATTLESHIP => 0.6,
            UnitType::CARRIER => 0.55
        }
    }

    fn freq(&self) -> f64 {
        match self.type_ {
            UnitType::INFANTRY => 20.0,
            UnitType::ARMOR => 16.0,
            UnitType::FIGHTER => 32.0,
            UnitType::BOMBER => 26.0,
            UnitType::TRANSPORT => 14.0,
            UnitType::DESTROYER => 12.0,
            UnitType::SUBMARINE => 12.0,
            UnitType::CRUISER => 10.0,
            UnitType::BATTLESHIP => 8.0,
            UnitType::CARRIER => 8.0
        }
    }
}
