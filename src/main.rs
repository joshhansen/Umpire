//!
//! Umpire: a game of world conquest
//!

//Wishlist:
// Copy is implemented for Rgb, Bg, Fg

mod conf;
mod game;
mod map;
mod ui;
mod unit;
mod util;

extern crate rand;
extern crate terminal_size;
extern crate termion;

use std::io::stdout;

use terminal_size::{Width, Height, terminal_size};
use termion::raw::IntoRawMode;

use util::Dims;

// Derived configuration
const MAP_DIMS: Dims = Dims { width: conf::MAP_WIDTH, height: conf::MAP_HEIGHT };



fn main() {

    // let home:Result<String,()> = conf::get("HOME");
    // println!("{}", home.unwrap());
    // let shlvl:Result<u16,()> = conf::get("SHLVL");
    // println!("{}", shlvl.unwrap());

    let stdout_0 : std::io::Stdout = stdout();
    let stdout_1 = stdout_0.lock().into_raw_mode().unwrap();
    if let Some((Width(term_width), Height(term_height))) = terminal_size() {
        let mut ui = ui::UI::new(
            game::Game::new(MAP_DIMS),
            stdout_1,
            Dims{ width: term_width, height: term_height },
            conf::HEADER_HEIGHT, conf::FOOTER_HEIGHT
        );

        ui.run();
    } else {
        println!("Unable to get terminal size");
    }
}
