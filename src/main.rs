//!
//! Umpire: a game of world conquest
//!

#![feature(conservative_impl_trait)]
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
extern crate termion;
extern crate unicode_segmentation;

extern crate portaudio as pa;
extern crate sample;

use std::io::stdout;

use clap::{Arg, App};
use termion::color::Rgb;
use termion::raw::IntoRawMode;
use termion::screen::AlternateScreen;
use termion::terminal_size;

use ui::log::{Message,MessageSource};
use util::Dims;
use game::Game;
use unit::PlayerNum;

// Derived configuration
const MAP_DIMS: Dims = Dims { width: conf::MAP_WIDTH, height: conf::MAP_HEIGHT };


fn main() {
    if let Ok((term_width,term_height)) = terminal_size() {
        {//This is here so screen drops completely when the game ends. That lets us print a farewell message to a clean console.
            let screen = AlternateScreen::from(stdout().into_raw_mode().unwrap());

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
            .get_matches();

            let fog_of_war = matches.value_of("fog").unwrap() == "on";
            let num_players: PlayerNum = matches.value_of("players").unwrap().parse().unwrap();


            let mut log_listener = |msg:String| {
                println!("{}", msg);
            };

            let mut game = Game::new(MAP_DIMS, num_players, fog_of_war, &mut log_listener);

            let mut ui = ui::UI::new(
                &game.map_dims(),
                Dims{ width: term_width, height: term_height },
                screen,
            );

            let mut mode = ui::mode::Mode::TurnStart;
            while mode.run(&mut game, &mut ui) {
                if let ui::mode::Mode::Examine{cursor_viewport_loc:_, first:_} = mode {
                    // don't bother
                } else {
                    ui.log_message(Message {
                        text: format!("Mode: {:?}", mode),
                        mark: None,
                        fg_color: Some(Rgb(255,140,0)),
                        bg_color: None,
                        source: Some(MessageSource::Main)
                    });
                }
            }
        }

        println!("Thanks for playing {}!\n", conf::APP_NAME);
    } else {
        println!("Unable to get terminal size");
    }
}
