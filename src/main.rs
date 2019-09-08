//!
//! Umpire: Combat Quest of the Millennium
//!

// FIXME Ask for production assignment immediately after city is conquered
// FIXME Resolve conflict between movement b and bomber b
// FIXME Don't let aircraft or naval vessels conquer cities
// FIXME Require production to be re-set when a city is conquered from another player
// FIXME Make it clear when a unit is inside a city
// FIXME Make Map::draw_tile faster, maybe by eliminating tile cloning
// FIXME Fix problems with small map sizes: 1) crash if viewport is larger than map 2) limitless wrapping where we should only wrap once
// TODO Implement transport functionality
// TODO Implement carrier functionality
// TODO Auto-explore mode for units
// TODO Fuel limits for aircraft
// TODO Long-range movement from examine mode
// TODO Show owner of city or unit in examine mode
// TODO Game save/load
// TODO Announce new unit creation
// TODO Color console text announcing turn start to correspond to player colors?
// TODO Show unit stats in examine mode
// TODO Show production cost / time when setting productions
// TODO Show possible orders / shortcuts in move unit mode
// TODO Zoomed-out map view?
// TODO AI
// TODO Unit names that better reflect current world naming patterns rather than just the US from 10/20 years ago.

#![allow(clippy::cognitive_complexity)]
#![allow(clippy::let_and_return)]


mod color;
pub mod conf;
pub mod game;
pub mod log;
#[macro_use]
mod macros;
pub mod name;
pub mod ui;
pub mod util;

extern crate clap;
extern crate csv;
extern crate flate2;
extern crate pastel;
extern crate rand;
extern crate termion;
extern crate unicode_segmentation;

// extern crate portaudio as pa;
// extern crate sample;

use std::io::{BufRead,BufReader,Write,stdout};

use clap::{Arg, App};
use termion::{
    color::{
        AnsiValue,
        Color,
    },
    terminal_size
};

use color::{Palette, palette16, palette256, palette24};
use game::{Game,PlayerNum};
use name::{city_namer,unit_namer};
use ui::DefaultUI;
use util::Dims;

fn print_loading_screen() {
    // let f = File::open("images/1945_Baseball_Umpire.txt").unwrap();
    // let file = BufReader::new(&f);
    let bytes: &[u8] = include_bytes!("../images/1945_Baseball_Umpire.txt");
    let r = BufReader::new(bytes);
    for line in r.lines() {
        let l = line.unwrap();
        println!("{}", l);
    }

    println!();

    println!("{}: Combat Quest of the Millennium", conf::APP_NAME);
    stdout().flush().unwrap();
}

fn main() {
    if let Ok((term_width,term_height)) = terminal_size() {
        let map_width_s: &str = &conf::MAP_WIDTH.to_string();
        let map_height_s: &str = &conf::MAP_HEIGHT.to_string();

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
            .arg(Arg::with_name("use_alt_screen")
                .short("a")
                .long("altscreen")
                .help("Use alternate screen")
                .takes_value(true)
                .default_value(conf::USE_ALTERNATE_SCREEN)
                .possible_values(&["on","off"])
            )
            .arg(Arg::with_name("map_width")
                .short("W")
                .long("width")
                .help("Map width")
                .takes_value(true)
                .default_value(map_width_s)
                .validator(|s| {
                    let width: Result<u16,_> = s.trim().parse();
                    width.map(|_n| ()).map_err(|_e| format!("Invalid map width '{}'", s))
                })
            )
            .arg(Arg::with_name("map_height")
                .short("H")
                .long("height")
                .help("Map height")
                .takes_value(true)
                .default_value(map_height_s)
                .validator(|s| {
                    let width: Result<u16,_> = s.trim().parse();
                    width.map(|_n| ()).map_err(|_e| format!("Invalid map height '{}'", s))
                })
            )
            .arg(Arg::with_name("colors")
                .short("c")
                .long("colors")
                .help("Colors supported. 16=16 colors, 256=256 colors, 24=24-bit color")
                .takes_value(true)
                .default_value("256")
                .possible_values(&["16","256","24"])
                .validator(|s| {
                    let width: Result<u16,_> = s.trim().parse();
                    width.map(|_n| ()).map_err(|_e| format!("Invalid colors '{}'", s))
                })
            )
            .arg(Arg::with_name("fog_darkness")
                .short("d")
                .long("fogdarkness")
                .help("Number between 0.0 and 1.0 indicating how dark the fog effect should be")
                .takes_value(true)
                .default_value("0.1")
                .validator(|s| {
                    let width: Result<f64,_> = s.trim().parse();
                    width.map(|_n| ()).map_err(|_e| format!("Invalid map height '{}'", s))
                })
            )
            .arg(Arg::with_name("unicode")
                .short("u")
                .long("unicode")
                .help("Enable Unicode support")
                
            )
        .get_matches();

        print_loading_screen();

        let fog_of_war = matches.value_of("fog").unwrap() == "on";
        let num_players: PlayerNum = matches.value_of("players").unwrap().parse().unwrap();
        let use_alt_screen = matches.value_of("use_alt_screen").unwrap() == "on";
        let map_width: u16 = matches.value_of("map_width").unwrap().parse().unwrap();
        let map_height: u16 = matches.value_of("map_height").unwrap().parse().unwrap();
        let color_depth: u16 = matches.value_of("colors").unwrap().parse().unwrap();
        let fog_darkness: f64 = matches.value_of("fog_darkness").unwrap().parse().unwrap();
        let unicode: bool = matches.is_present("unicode");

        let map_dims: Dims = Dims::new(map_width, map_height);

        let city_namer = city_namer();
        let unit_namer = unit_namer();


        let game = Game::new(map_dims, city_namer, num_players, fog_of_war, unit_namer, &mut DefaultUI);
        let dims = Dims{ width: term_width, height: term_height };

        match color_depth {
            16 | 256 => {
                let palette: Palette<AnsiValue> = match color_depth {
                    16 => palette16(),
                    256 => palette256(),
                    _ => unreachable!()
                };
                run_ui(game, dims, use_alt_screen, palette, unicode);

            },
            24 => {
                match palette24(num_players, fog_darkness) {
                    Ok(palette) => run_ui(game, dims, use_alt_screen, palette, unicode),
                    Err(err) => eprintln!("Error loading truecolor palette: {}", err)
                }
            },
            x => eprintln!("Unsupported color palette {}", x)
        }
        
    } else {
        eprintln!("Unable to get terminal size");
    }
}

fn run_ui<C:Color+Copy>(game: Game, dims: Dims, use_alt_screen: bool, palette: Palette<C>, unicode: bool) {
    if let Err(msg) = ui::run(game, dims, use_alt_screen, palette, unicode) {
        eprintln!("Error running UI: {}", msg);
    }
}