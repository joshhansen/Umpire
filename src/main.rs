//!
//! Umpire: Combat Quest of the Millennium
//!

// FIXME Ask for production assignment immediately after city is conquered
// FIXME Resolve conflict between movement b and bomber b
// FIXME Don't let aircraft or naval vessels conquer cities
// FIXME Require production to be re-set when a city is conquered from another player
// FIXME Make it clear when a unit is inside a city
// FIXME Don't let units have land or water color
// FIXME Make Map::draw_tile faster, maybe by eliminating tile cloning
// TODO Implement transport functionality
// TODO Implement carrier functionality
// TODO Allow sentry
// TODO Allow skipping a turn for a particular unit
// TODO Auto-explore mode for units
// TODO Fuel limits for aircraft
// TODO Long-range movement from examine mode
// TODO Allow activation of a unit from examine mode
// TODO Show owner of city or unit in examine mode
// TODO Game save/load
// TODO Announce new unit creation
// TODO Color console text announcing turn start to correspond to player colors
// TODO Show unit stats in examine mode
// TODO Show production cost / time when setting productions
// TODO Show possible orders / shortcuts in move unit mode
// TODO Zoomed-out map view?

#![feature(conservative_impl_trait)]
#![feature(try_from)]

pub mod conf;
pub mod game;
pub mod log;
#[macro_use]
mod macros;
pub mod map;
pub mod name;
pub mod ui;
pub mod unit;
pub mod util;

extern crate clap;
extern crate csv;
extern crate rand;
extern crate termion;
extern crate unicode_segmentation;

// extern crate portaudio as pa;
// extern crate sample;

use std::fs::File;
use std::io::{BufRead,BufReader,Write,stdout};

use clap::{Arg, App};
use termion::terminal_size;

use game::Game;
use name::{city_namer,unit_namer};
use ui::DefaultUI;
use unit::PlayerNum;
use util::Dims;

// Derived configuration
const MAP_DIMS: Dims = Dims { width: conf::MAP_WIDTH, height: conf::MAP_HEIGHT };

fn print_loading_screen() {
    let f = File::open("images/1945_Baseball_Umpire.txt").unwrap();
    let file = BufReader::new(&f);
    for line in file.lines() {
        let l = line.unwrap();
        println!("{}", l);
    }

    println!();

    println!("{}: Combat Quest of the Millennium", conf::APP_NAME);
    stdout().flush().unwrap();
}

fn main() {
    if let Ok((term_width,term_height)) = terminal_size() {
        let matches = App::new(conf::APP_NAME)
            .version("0.1")
            .author("Josh Hansen <hansen.joshuaa@gmail.com>")
            .about(conf::APP_SUBTITLE)
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

        print_loading_screen();

        let fog_of_war = matches.value_of("fog").unwrap() == "on";
        let num_players: PlayerNum = matches.value_of("players").unwrap().parse().unwrap();

        match city_namer() {
            Ok(city_namer) => {
                match unit_namer() {
                    Ok(unit_namer) => {

                        let game = Game::new(MAP_DIMS, city_namer, num_players, fog_of_war, unit_namer, &mut DefaultUI);

                        if let Err(msg) = ui::run(game, Dims{ width: term_width, height: term_height }) {
                            println!("Error running UI: {}", msg);
                        }
                    },
                    Err(err) => {
                        println!("Error loading unit namer: {}", err);
                    }
                }
            },
            Err(msg) => {
                println!("Error loading city names: {}", msg);
            }
        }
    } else {
        println!("Unable to get terminal size");
    }
}
