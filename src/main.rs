//!
//! Umpire: Combat Quest of the Millennium
//!

// 0.4 Milestones
// FIXME Auto-explore mode for units is broken
//       - the search for the next goal is somtimes really slow
//       - it happens that the auto-explore will request a move that exceeds a unit's moves remaining
// FIXME Disallow selecting units on cities where such units make no sense
// FIXME Don't let aircraft or naval vessels conquer cities
// TODO Allow examine mode go-to to unobserved tiles, on a best-effort basis
// TODO Deploy to crates.io
// TODO Refresh README.md
// FIXME Fix unit production log messages

// 0.5 Milestones
// TODO Fuel limits for aircraft
// TODO Wake up units with auto-explore and go-to orders when they encounter something interesting
// TODO Wake up sentried units when an enemy comes within their sight.
// TODO Opening theme music
// TODO Allow map specification at command-line
// FIXME Make it clear when a unit is inside a city

// 0.6 Milestones
// TODO Make splash screen respect color palette
// FIXME Make splash screen fit the terminal size
// FIXME Fix problems with small map sizes: 1) crash if viewport is larger than map 2) limitless wrapping where we should only wrap once
// TODO Zoomed-out map view?
// TODO Color console text announcing turn start to correspond to player colors?

// 0.7 Milestones
// TODO Travis infrastructure
// TODO Remove all git-based dependencies
// TODO Decruftification
// TODO API cleanup
// TODO Improved test coverage
// TODO? Move `log` into `ui` and make `Game` fully abstract?
// TODO Profile and optimize

// 0.8 Milestones
// TODO Windows support
// TODO OSX support
// TODO Game save/load

// 0.9 Milestones
// TODO Unit names that better reflect current world naming patterns rather than just the US from 10/20 years ago.

// 1.0 Milestones
// TODO AI

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

use std::{
    io::{BufRead,BufReader,Write,stdout},
    thread,
    time::{Duration,SystemTime},
};

use clap::{Arg, App};

use crate::{
    color::{Palette, palette16, palette256, palette24},
    game::{Game,PlayerNum},
    name::{city_namer,unit_namer},
    util::Dims,
};

const MIN_LOAD_SCREEN_DISPLAY_TIME: Duration = Duration::from_secs(3);

fn print_loading_screen() {
    let bytes: &[u8] = include_bytes!("../images/1945_Baseball_Umpire.txt");
    let r = BufReader::new(bytes);
    for line in r.lines() {
        let l = line.unwrap();
        println!("{}", l);
    }

    println!();

    println!("{}: {}", conf::APP_NAME, conf::APP_SUBTITLE);
    stdout().flush().unwrap();
}

fn main() {
    let map_width_s: &str = &conf::MAP_WIDTH.to_string();
    let map_height_s: &str = &conf::MAP_HEIGHT.to_string();

    let matches = App::new(conf::APP_NAME)
        .version(conf::APP_VERSION)
        .author("Josh Hansen <hansen.joshuaa@gmail.com>")
        .about(conf::APP_SUBTITLE)

        .arg(Arg::with_name("use_alt_screen")
            .short("a")
            .long("altscreen")
            .help("Use alternate screen")
            .takes_value(true)
            .default_value(conf::USE_ALTERNATE_SCREEN)
            .possible_values(&["on","off"])
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
        .arg(Arg::with_name("fog")
            .short("f")
            .long("fog")
            .help("Enable or disable fog of war")
            .takes_value(true)
            .default_value(conf::FOG_OF_WAR)
            .possible_values(&["on","off"])
        )
        .arg(Arg::with_name("nosplash")
            .short("n")
            .long("nosplash")
            .help("Don't show the splash screen")
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
        .arg(Arg::with_name("quiet")
            .short("q")
            .long("quiet")
            .help("Don't produce sound")
        )
        .arg(Arg::with_name("unicode")
            .short("u")
            .long("unicode")
            .help("Enable Unicode support")
        )
        .arg(Arg::with_name("confirm_turn_end")
            .short("C")
            .long("confirm")
            .help("Wait for explicit confirmation of turn end.")
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
    .get_matches();

    let fog_of_war = matches.value_of("fog").unwrap() == "on";
    let num_players: PlayerNum = matches.value_of("players").unwrap().parse().unwrap();
    let use_alt_screen = matches.value_of("use_alt_screen").unwrap() == "on";
    let map_width: u16 = matches.value_of("map_width").unwrap().parse().unwrap();
    let map_height: u16 = matches.value_of("map_height").unwrap().parse().unwrap();
    let color_depth: u16 = matches.value_of("colors").unwrap().parse().unwrap();
    let fog_darkness: f64 = matches.value_of("fog_darkness").unwrap().parse().unwrap();
    let unicode: bool = matches.is_present("unicode");
    let quiet: bool = matches.is_present("quiet");
    let nosplash: bool = matches.is_present("nosplash");
    let confirm_turn_end: bool = matches.is_present("confirm_turn_end");

    let map_dims: Dims = Dims::new(map_width, map_height);
    if map_dims.area() < u32::from(num_players) {
        eprintln!("Map dimensions of {} give an area of {} which is not enough room for {} players; area of {} or greater required.",
            map_dims, map_dims.area(), num_players, num_players);
        return;
    }

    let start_time = SystemTime::now();
    if !nosplash {
        print_loading_screen();
    }

    let city_namer = city_namer();
    let unit_namer = unit_namer();

    let game = Game::new(map_dims, city_namer, num_players, fog_of_war, unit_namer);

    if !nosplash {
        let elapsed_time = SystemTime::now().duration_since(start_time).unwrap();
        if elapsed_time < MIN_LOAD_SCREEN_DISPLAY_TIME {
            let remaining = MIN_LOAD_SCREEN_DISPLAY_TIME - elapsed_time;
            thread::sleep(remaining);
        }
    }

    match color_depth {
        16 | 256 => {
            let palette: Palette = match color_depth {
                16 => palette16(),
                256 => palette256(),
                _ => unreachable!()
            };
            run_ui(game, use_alt_screen, palette, unicode, quiet, confirm_turn_end);

        },
        24 => {
            match palette24(num_players, fog_darkness) {
                Ok(palette) => run_ui(game, use_alt_screen, palette, unicode, quiet, confirm_turn_end),
                Err(err) => eprintln!("Error loading truecolor palette: {}", err)
            }
        },
        x => eprintln!("Unsupported color palette {}", x)
    }
}

fn run_ui(game: Game, use_alt_screen: bool, palette: Palette, unicode: bool, quiet: bool,
    confirm_turn_end: bool) {
    if let Err(msg) = ui::run(game, use_alt_screen, palette, unicode, quiet, confirm_turn_end) {
        eprintln!("Error running UI: {}", msg);
    }
}