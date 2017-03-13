//!
//! Umpire: a game of world conquest
//!

//Wishlist:
// Copy is implemented for Rgb, Bg, Fg

// Name ideas:
// * ocracy
// * emp
// * umpire
// * pire
// * perium
// * shmempire
// * hegemon
// * vegemon
// * perator
// * metropole

#![feature(try_from)]

mod conf;
mod game;
#[macro_use]
mod macros;
mod map;
mod ui;
mod unit;
mod util;

extern crate clap;
extern crate csv;
extern crate rand;
extern crate terminal_size;
extern crate termion;

extern crate portaudio as pa;
extern crate sample;

use std::io::stdout;

use clap::{Arg, App};
use terminal_size::{Width, Height, terminal_size};
use termion::raw::IntoRawMode;

use util::Dims;
use game::Game;
use unit::PlayerNum;

// Derived configuration
const MAP_DIMS: Dims = Dims { width: conf::MAP_WIDTH, height: conf::MAP_HEIGHT };


fn main() {
    if let Some((Width(term_width), Height(term_height))) = terminal_size() {
        let stdout_0 : std::io::Stdout = stdout();
        let stdout_1 = stdout_0.lock().into_raw_mode().unwrap();

        let matches = App::new(conf::APP_NAME)
            .version("0.1")
            .author("Josh Hansen <hansen.joshuaa@gmail.com>")
            .about("Combat Quest of the Millennium")
            .arg(Arg::with_name("fog")
              .short("f")
              .long("fog")
              .help("Enable or disable fog of war")
              .takes_value(true)
              .default_value(conf::FOG_OF_WAR)
              .possible_values(&["on","off"])
            )
            .arg(Arg::with_name("players")
                .short("p")
                .long("players")
                .help("Number of players")
                .takes_value(true)
                .required(true)
                .default_value(conf::NUM_PLAYERS)
                .validator(|s| {
                    let players: Result<PlayerNum,_> = s.trim().parse();
                    players.map(|_n| ()).map_err(|_e| String::from("Couldn't parse number of players"))
                })
            )
            // .arg(Arg::with_name("clear")
            //     .short("c")
            //     .long("clear")
            //     .help("Reset the terminal")
            //     .takes_value(false)
            //     .required(false)
            // )
        .get_matches();

        let fog_of_war = matches.value_of("fog").unwrap() == "on";
        let num_players: PlayerNum = matches.value_of("players").unwrap().parse().unwrap();


        let mut log_listener = |msg:String| {
            println!("{}", msg);
        };

        let mut game = Game::new(MAP_DIMS, num_players, fog_of_war, &mut log_listener);

        let mut ui = ui::UI::new(
            &game.map_dims,
            Dims{ width: term_width, height: term_height },
            stdout_1,
        );

        let mut mode = ui::mode::Mode::TurnStart;
        while mode.run(&mut game, &mut ui) {
            ui.log_message(format!("Mode: {:?}", mode));
        }

        println!("Thanks for playing {}!\n", conf::APP_NAME);
    } else {
        println!("Unable to get terminal size");
    }
}
