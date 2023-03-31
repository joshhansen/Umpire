//!
//! Umpire: Combat Quest of the Millennium
//!
#![forbid(unsafe_code)]
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::let_and_return)]
#![allow(clippy::too_many_arguments)]

use std::{
    cell::RefCell,
    collections::HashMap,
    io::{stdout, BufRead, BufReader, Write},
    net::{IpAddr, Ipv6Addr},
    rc::Rc,
    sync::{Arc, RwLock},
    thread,
    time::{Duration, SystemTime},
};

use clap::{builder::BoolishValueParser, Arg, ArgAction};

use tarpc::{client, context, tokio_serde::formats::Bincode};

use self::ui::TermUI;

use umpire_ai::AI;

use umpire_tui::color::{palette16, palette24, palette256};

use common::{
    cli::{self, players_arg},
    conf,
    game::{ai::AISpec, player::TurnTaker, Game, IGame, PlayerNum, PlayerSecret, PlayerType},
    log::LogTarget,
    name::{city_namer, unit_namer},
    rpc::{RpcGame, UmpireRpcClient},
    util::{Dims, Wrap2d},
};

pub mod ui;

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

#[tokio::main]
async fn main() -> Result<(), String> {
    let matches = cli::app(conf::APP_NAME, "fwHW")
        .version(conf::APP_VERSION)
        .author("Josh Hansen <hansen.joshuaa@gmail.com>")
        .about(conf::APP_SUBTITLE)
        .arg(
            Arg::new("use_alt_screen")
                .short('a')
                .long("altscreen")
                .help("Use alternate screen")
                .default_value("on")
                .value_parser(BoolishValueParser::new()),
        )
        .arg(
            Arg::new("colors")
                .short('c')
                .long("colors")
                .help("Colors supported. 16=16 colors, 256=256 colors, 24=24-bit color")
                .default_value("256")
                .value_parser(["16", "256", "24"]),
        )
        .arg(
            Arg::new("fog_darkness")
                .short('F')
                .long("fogdarkness")
                .help("Number between 0.0 and 1.0 indicating how dark the fog effect should be")
                .default_value("0.1")
                .value_parser(|s: &str| {
                    let width: Result<f64, _> = s.trim().parse();
                    width.map_err(|_e| format!("Invalid map height '{}'", s))
                }),
        )
        .arg(
            Arg::new("nosplash")
                .short('n')
                .long("nosplash")
                .help("Don't show the splash screen")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("quiet")
                .short('q')
                .long("quiet")
                .help("Don't produce sound"),
        )
        .arg(
            Arg::new("unicode")
                .short('u')
                .long("unicode")
                .help("Enable Unicode support"),
        )
        .arg(
            Arg::new("confirm_turn_end")
                .short('C')
                .long("confirm")
                .help("Wait for explicit confirmation of turn end."),
        )
        .arg(players_arg().required_unless_present("server"))
        .arg(Arg::new("server").required_unless_present("players"))
        .get_matches();

    // let ai_model_path = matches.value_of("ai_model");
    // let fog_of_war = matches.value_of("fog").unwrap() == "on";
    // let player_types: Vec<PlayerType> = matches.value_of("players").unwrap()
    //     .chars()
    //     .map(|spec_char| {
    //         PlayerType::from_spec_char(spec_char)
    //                     .expect(format!("'{}' is not a valid player type", spec_char).as_str())
    //     })
    //     .collect()
    // ;

    let nosplash = matches.contains_id("nosplash");

    let start_time = SystemTime::now();
    if !nosplash {
        print_loading_screen();
    }

    if !nosplash {
        let elapsed_time = SystemTime::now().duration_since(start_time).unwrap();
        if elapsed_time < MIN_LOAD_SCREEN_DISPLAY_TIME {
            let remaining = MIN_LOAD_SCREEN_DISPLAY_TIME - elapsed_time;
            thread::sleep(remaining);
        }
    }

    let use_alt_screen = matches.get_one::<bool>("use_alt_screen").cloned().unwrap();
    let color_depth: u16 = matches
        .get_one::<String>("colors")
        .unwrap()
        .parse()
        .unwrap();
    let fog_darkness = matches.get_one::<f64>("fog_darkness").unwrap().clone();
    let unicode = matches.contains_id("unicode");
    let quiet = matches.contains_id("quiet");
    let confirm_turn_end = matches.contains_id("confirm_turn_end");

    let local_server = matches.contains_id("players");

    let (mut game, secrets, num_players, dims, player_types) = if local_server {
        let player_types = matches.get_one::<Vec<PlayerType>>("players").unwrap(); //FIXME take from server

        let num_players: PlayerNum = player_types.len(); //FIXME take from server
        let map_width = matches.get_one::<u16>("map_width").unwrap().clone(); //FIXME take from server
        let map_height = matches.get_one::<u16>("map_height").unwrap().clone(); //FIXME take from server

        let wrapping = matches.get_one::<Wrap2d>("wrapping").unwrap().clone(); //FIXME take from server
        let fog_of_war = matches.get_one::<bool>("fog").unwrap().clone(); //FIXME take from server

        let map_dims: Dims = Dims::new(map_width, map_height);
        if (map_dims.area() as PlayerNum) < num_players {
            return Err(format!("Map dimensions of {} give an area of {} which is not enough room for {} players; area of {} or greater required.",
                map_dims, map_dims.area(), num_players, num_players));
        }

        let city_namer = city_namer();
        let unit_namer = unit_namer();
        let (game, secrets) = Game::new(
            map_dims,
            city_namer,
            player_types.len(),
            fog_of_war,
            Some(Arc::new(RwLock::new(unit_namer))),
            wrapping,
        );
        (
            Box::new(game) as Box<dyn IGame>,
            secrets
                .iter()
                .cloned()
                .map(Some)
                .collect::<Vec<Option<PlayerSecret>>>(),
            num_players,
            map_dims,
            player_types.clone(),
        )
    } else {
        let server_hostname = matches.get_one::<String>("server").unwrap();
        let server_addr = (IpAddr::V6(Ipv6Addr::LOCALHOST), 21131);

        let transport = tarpc::serde_transport::tcp::connect(&server_addr, Bincode::default)
            .await
            .map_err(|err| format!("Error connecting to server {}: {}", server_hostname, err))?;

        // let (client_transport, server_transport) = tarpc::transport::channel::unbounded();

        let client = UmpireRpcClient::new(client::Config::default(), transport).spawn();

        let secrets = client
            .player_secrets_known(context::current())
            .await
            .map_err(|err| {
                format!(
                    "Error fetching player secrets from {}: {}",
                    server_hostname, err
                )
            })?;

        let player_types = client.player_types(context::current()).await.unwrap();

        let game = Box::new(RpcGame::new(client)) as Box<dyn IGame>;

        let num_players = game.num_players().await;

        let dims = game.dims().await;

        (game, secrets, num_players, dims, player_types)
    };

    let palette = match color_depth {
        16 | 256 => match color_depth {
            16 => palette16(num_players).expect(format!("Error loading 16-color palette").as_str()),
            256 => {
                palette256(num_players).expect(format!("Error loading 256-color palette").as_str())
            }
            x => panic!("Unsupported color depth {}", x),
        },
        24 => {
            palette24(num_players, fog_darkness)
            // match palette24(num_players, fog_darkness) {
            //     Ok(palette) => run_ui(game, use_alt_screen, palette, unicode, quiet, confirm_turn_end),
            //     Err(err) => eprintln!("Error loading truecolor palette: {}", err)
            // }
        }
        x => panic!("Unsupported color depth {}", x),
    };

    {
        // Scope for the UI. When it goes out of scope it will clean up the terminal, threads, audio, etc.

        let mut ui = TermUI::new(
            dims,
            palette,
            unicode,
            confirm_turn_end,
            quiet,
            use_alt_screen,
        )
        .unwrap();

        // We can share one instance of RandomAI across players since it's stateless
        // let mut random_ai = RandomAI::new(0);

        // AIs indexed by spec
        // let mut ais: HashMap<String,RL_AI<LFA<Basis,SGD,VectorFunction>>> = HashMap::new();
        let mut ais: HashMap<AISpec, Rc<RefCell<AI>>> = HashMap::new();

        if local_server {
            for ptype in player_types.iter() {
                if let PlayerType::AI(ai_type) = ptype {
                    let ai: AI = ai_type.clone().into();
                    let ai = Rc::new(RefCell::new(ai));
                    // let player: Rc<RefCell<dyn TurnTaker>> = ai_type.clone().into();
                    ais.insert(ai_type.clone(), ai);
                }
            }
        }

        'outer: loop {
            if game.victor().await.is_some() {
                break 'outer;
            }

            let player = game.current_player().await;

            // Only take the turn locally if we have the corresponding player's secret
            if let Some(secret) = secrets[player] {
                ui.log_message(format!("Player {}'s turn", player));
                match &player_types[player] {
                    PlayerType::Human => {
                        let training_instances = ui
                            .take_turn_not_clearing(game.as_mut(), player, secret, false)
                            .await;
                        assert!(training_instances.is_none());
                    }
                    PlayerType::AI(ai_type) => {
                        let training_instances = ais
                            .get_mut(ai_type)
                            .unwrap()
                            .borrow_mut()
                            .take_turn_clearing(game.as_mut(), player, secret, false)
                            .await;
                        assert!(training_instances.is_none());
                    }
                }
            } else {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }

            // for (i, ptype) in player_types.iter().enumerate() {
            //     ui.log_message(format!("Player of type {:?}", ptype));

            //     if game.victor().await.is_some() {
            //         break 'outer;
            //     }

            //     let next_player = &player_types[(i + 1) % player_types.len()];
            //     let clear_at_end_of_turn = match next_player {
            //         PlayerType::Human => false,
            //         _ => true,
            //     };

            //     // Only take the turn locally if we have the corresponding player's secret
            //     if let Some(secret) = secrets[i] {
            //         match ptype {
            //             PlayerType::Human => {
            //                 let training_instances = ui
            //                     .take_turn(game.as_mut(), i, secret, clear_at_end_of_turn, false)
            //                     .await;
            //                 assert!(training_instances.is_none());
            //             }
            //             PlayerType::AI(ai_type) => {
            //                 let training_instances = ais
            //                     .get_mut(ai_type)
            //                     .unwrap()
            //                     .borrow_mut()
            //                     .take_turn(game.as_mut(), i, secret, clear_at_end_of_turn, false)
            //                     .await;
            //                 assert!(training_instances.is_none());
            //             }
            //         }
            //     }
            // }
        }
    } // UI drops here, deinitializing the user interface

    println!(
        "\n\n\tHe rules a moment: Chaos umpire sits,
    \tAnd by decision more embroils the fray
    \tBy which he reigns: next him, high arbiter,
    \tChance governs all.

    \t\t\t\tParadise Lost (2.907-910)\n"
    );

    Ok(())
}
