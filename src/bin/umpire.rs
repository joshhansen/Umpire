//!
//! Umpire: Combat Quest of the Millennium
//!
#![forbid(unsafe_code)]
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::let_and_return)]
#![allow(clippy::too_many_arguments)]

use std::{
    collections::HashMap,
    convert::TryFrom,
    io::{BufRead,BufReader,Write,stdout},
    rc::Rc,
    sync::{
        RwLock,
    },
    thread,
    time::{Duration,SystemTime},
};

use clap::Arg;

use rsrl::{
    fa::{
        linear::{
            optim::SGD,
            LFA,
            VectorFunction,
        },
    },
};

use umpire::{
    cli,
    color::{palette16, palette256, palette24},
    conf,
    game::{
        Game,
        PlayerNum,
        PlayerType,
        ai::{
            RL_AI,
            RandomAI,
            rl::Basis,
        },
        player::{
            TurnTaker,
        },
    },
    name::{city_namer,unit_namer},
    ui::TermUI,
    util::{
        Dims,
        Wrap2d,
    }, log::LogTarget,
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
    let matches = cli::app(conf::APP_NAME, "fwWH")
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
        .arg(Arg::with_name("nosplash")
            .short("n")
            .long("nosplash")
            .help("Don't show the splash screen")
        )
        .arg(Arg::with_name("players")
            .short("p")
            .long("players")
            .takes_value(true)
            .required(true)
            .default_value("h1233")
            .help(
                format!("Player type specification string, {}", 
                    PlayerType::values().iter()
                    .map(|player_type| format!("'{}' for {}", player_type.spec_char(), player_type.desc()))
                    .collect::<Vec<String>>()
                    .join(", ")
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

    .get_matches();

    // let ai_model_path = matches.value_of("ai_model");
    let fog_of_war = matches.value_of("fog").unwrap() == "on";
    let player_types: Vec<PlayerType> = matches.value_of("players").unwrap()
        .chars()
        .map(|spec_char| {
            PlayerType::from_spec_char(spec_char)
                        .expect(format!("'{}' is not a valid player type", spec_char).as_str())
        })
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
    let wrapping = Wrap2d::try_from(matches.value_of("wrapping").unwrap().as_ref()).unwrap();

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
        Some(Rc::new(RwLock::new(unit_namer))),
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
                16 => palette16(num_players).expect(format!("Error loading 16-color palette").as_str()),
                256 => palette256(num_players).expect(format!("Error loading 256-color palette").as_str()),
                x => panic!("Unsupported color depth {}", x)
            }
        },
        24 => {
            palette24(num_players, fog_darkness)
            // match palette24(num_players, fog_darkness) {
            //     Ok(palette) => run_ui(game, use_alt_screen, palette, unicode, quiet, confirm_turn_end),
            //     Err(err) => eprintln!("Error loading truecolor palette: {}", err)
            // }
        },
        x => panic!("Unsupported color depth {}", x)
    };


    let map_dims = game.dims();

    {// Scope for the UI. When it goes out of scope it will clean up the terminal, threads, audio, etc.

        let mut ui = TermUI::new(
            map_dims,
            palette,
            unicode,
            confirm_turn_end,
            quiet,
            use_alt_screen,
        ).unwrap();

        // We can share one instance of RandomAI across players since it's stateless
        let mut random_ai = RandomAI::new(0);

        let mut ais: HashMap<usize,RL_AI<LFA<Basis,SGD,VectorFunction>>> = HashMap::new();

        for ptype in player_types.iter() {
            if let PlayerType::AI(level) = ptype {
                ais.insert(*level, match level {
                    1 => bincode::deserialize(include_bytes!("../../ai/10x10_e100_s100000_a__scorefix__turnpenalty.ai")).unwrap(),
                    2 => bincode::deserialize(include_bytes!("../../ai/20x20_e100_s100000_a__scorefix__turnpenalty.ai")).unwrap(),
                    3 => bincode::deserialize(include_bytes!("../../ai/10-30_e100_s100000_a__scorefix__turnpenalty.ai")).unwrap(),
                    4 => bincode::deserialize(include_bytes!("../../ai/10-40+full_e100_s100000_a.ai")).unwrap(),
                    level => unreachable!("Unsupported AI level: {}", level)
                });
            }
        }

        let mut game = game;

        'outer: loop {
            for (i, ptype) in player_types.iter().enumerate() {
                ui.log_message(format!("Player of type {:?}", ptype));

                if game.victor().is_some() {
                    break 'outer;
                }

                let next_player = player_types[(i+1) % player_types.len()];
                let clear_at_end_of_turn = match next_player {
                    PlayerType::Human => false,
                    _ => true,
                };

                match ptype {
                    PlayerType::Human => ui.take_turn(&mut game, clear_at_end_of_turn),
                    PlayerType::Random => random_ai.take_turn(&mut game, clear_at_end_of_turn),
                    PlayerType::AI(level) => ais.get_mut(level).unwrap().take_turn(&mut game, clear_at_end_of_turn),
                }
            }
        }

    }// UI drops here, deinitializing the user interface

    println!("\n\n\tHe rules a moment: Chaos umpire sits,
    \tAnd by decision more embroils the fray
    \tBy which he reigns: next him, high arbiter,
    \tChance governs all.

    \t\t\t\tParadise Lost (2.907-910)\n"
    );
}