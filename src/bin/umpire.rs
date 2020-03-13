//!
//! Umpire: Combat Quest of the Millennium
//!

#![allow(clippy::cognitive_complexity)]
#![allow(clippy::let_and_return)]
#![allow(clippy::too_many_arguments)]

use core::cell::RefCell;

use std::{
    io::{BufRead,BufReader,Write,stdout},
    sync::{
        Arc,
        RwLock,
    },
    thread,
    time::{Duration,SystemTime},
};

use clap::{Arg, App};

use umpire::{
    color::{Palette, palette16, palette256, palette24},
    conf,
    game::{
        Game,
        PlayerNum,
        PlayerType,
        ai::RandomAI,
        player::Player,
    },
    name::{city_namer,unit_namer},
    ui,
    util::{
        Dims,
        Wrap,
        Wrap2d,
    },
};

const MIN_LOAD_SCREEN_DISPLAY_TIME: Duration = Duration::from_secs(3);

fn print_loading_screen() {
    let bytes: &[u8] = include_bytes!("../../images/1945_Baseball_Umpire.txt");
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
        // .arg(Arg::with_name("players")
        //     .short("p")
        //     .long("players")
        //     .help("Number of human players")
        //     .takes_value(true)
        //     .required(true)
        //     .default_value(conf::NUM_PLAYERS)
        //     .validator(|s| {
        //         let players: Result<PlayerNum,_> = s.trim().parse();
        //         players.map(|_n| ()).map_err(|_e| String::from("Couldn't parse number of players"))
        //     })
        // )
        .arg(Arg::with_name("players")
            .short("p")
            .long("players")
            .takes_value(true)
            .required(true)
            .default_value("hhrr")
            .help(
                format!("Player type specification string e.g. 'hhrr', {}", 
                    PlayerType::values().iter()
                    .map(|player_type| format!("'{}' for {}", player_type.spec_char(), player_type.desc()))
                    .collect::<Vec<String>>()
                    .join("")
                ).as_str()
            )
            .validator(|s| {
                for spec_char in s.chars() {
                    PlayerType::from_spec_char(spec_char)
                    .map(|_| ())
                    .map_err(|_| format!("'{}' is not a valid player type", spec_char))?;
                }
                Ok(())
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
        .arg(Arg::with_name("wrapping")
            .short("w")
            .long("wrapping")
            .help("Whether to wrap horizontally ('h'), vertically ('v'), both ('b'), or neither ('n')")
            .takes_value(true)
            .default_value("b")
            .validator(|s| {
                match s.as_ref() {
                    "h" | "v" | "b" | "n" => Ok(()),
                    x => Err(format!("{} is not a supported wrapping type", x))
                }
            })
        )
    .get_matches();

    let fog_of_war = matches.value_of("fog").unwrap() == "on";
    let player_types: Vec<PlayerType> = matches.value_of("ptypes").unwrap()
        .chars()
        .map(|spec_char| PlayerType::from_spec_char(spec_char).expect(format!("'{}' is not a valid player type", spec_char).as_str()))
        .collect()
    ;
    let num_players: PlayerNum = player_types.len();
    let use_alt_screen = matches.value_of("use_alt_screen").unwrap() == "on";
    let map_width: u16 = matches.value_of("map_width").unwrap().parse().unwrap();
    let map_height: u16 = matches.value_of("map_height").unwrap().parse().unwrap();
    let color_depth: u16 = matches.value_of("colors").unwrap().parse().unwrap();
    let fog_darkness: f64 = matches.value_of("fog_darkness").unwrap().parse().unwrap();
    let unicode: bool = matches.is_present("unicode");
    let quiet: bool = matches.is_present("quiet");
    let nosplash: bool = matches.is_present("nosplash");
    let confirm_turn_end: bool = matches.is_present("confirm_turn_end");
    let wrapping: Wrap2d = match matches.value_of("wrapping").unwrap().as_ref() {
        "h" => Wrap2d{horiz: Wrap::Wrapping, vert: Wrap::NonWrapping},
        "v" => Wrap2d{horiz: Wrap::NonWrapping, vert: Wrap::Wrapping},
        "b" => Wrap2d{horiz: Wrap::Wrapping, vert: Wrap::Wrapping},
        "n" => Wrap2d{horiz: Wrap::NonWrapping, vert: Wrap::NonWrapping},
        _ => unreachable!(),
    };

    let map_dims: Dims = Dims::new(map_width, map_height);
    if (map_dims.area() as PlayerNum) < num_players {
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

    let game = Game::new(
        map_dims,
        city_namer,
        player_types.len(),
        fog_of_war,
        Arc::new(RefCell::new(unit_namer)),
        wrapping,
    );

    if !nosplash {
        let elapsed_time = SystemTime::now().duration_since(start_time).unwrap();
        if elapsed_time < MIN_LOAD_SCREEN_DISPLAY_TIME {
            let remaining = MIN_LOAD_SCREEN_DISPLAY_TIME - elapsed_time;
            thread::sleep(remaining);
        }
    }

    let palette = match color_depth {
        16 | 256 => {
            match color_depth {
                16 => palette16(),
                256 => palette256(),
                x => panic!("Unsupported color depth {}", x)
            }
        },
        24 => {
            palette24(num_players, fog_darkness).expect(format!("Error loading truecolor palette").as_str())
            // match palette24(num_players, fog_darkness) {
            //     Ok(palette) => run_ui(game, use_alt_screen, palette, unicode, quiet, confirm_turn_end),
            //     Err(err) => eprintln!("Error loading truecolor palette: {}", err)
            // }
        },
        x => panic!("Unsupported color depth {}", x)
    };

    // Initialize players and AI

    let mut human_players = Vec::new();
    let mut ai_players = Vec::new();
    let mut ais: Vec<Box<RefCell<dyn Player>>> = Vec::new();

    for (pnum, ptype) in player_types.iter().enumerate() {
        match ptype {
            PlayerType::Human => {
                human_players.push(pnum);
            },
            PlayerType::Random => {
                ai_players.push(pnum);
                ais.push(Box::new(RefCell::new(RandomAI::new())));

            },
        }
    }

    let game = Arc::new(RwLock::new(game));

    let ai_thread_handle = thread::Builder::new().name("AI".to_string()).spawn(move || {
        'outer: loop {
            for idx in 0..ai_players.len() {
                let player = ai_players[idx];
                if {
                    let game = game.read().unwrap()
                    game.victor().is_some()
                } {
                    break 'outer;
                }

                let ai = ais[idx];

                let game = game.write().unwrap();
                let ctrl = game.player_turn_control(player);
                ai.borrow().play(ctrl);
            }
        }
    }).unwrap();



    // let ui_thread_handle = thread::Builder::new().name("UI".to_string()).spawn(move || {
    if let Err(msg) = ui::run(game, human_players, use_alt_screen, palette, unicode, quiet, confirm_turn_end) {
        eprintln!("Error running UI: {}", msg);
    }

    ai_thread_handle.join();

    // });

    // run_ui(game, use_alt_screen, palette, unicode, quiet, confirm_turn_end);

    // pub fn play(mut game: Game, players: Vec<Box<RefCell<dyn Player>>>) {
    // play(game, players);
}

// fn run_ui(game: Game, use_alt_screen: bool, palette: Palette, unicode: bool, quiet: bool,
//     confirm_turn_end: bool) {
//     if let Err(msg) = ui::run(game, use_alt_screen, palette, unicode, quiet, confirm_turn_end) {
//         eprintln!("Error running UI: {}", msg);
//     }
// }