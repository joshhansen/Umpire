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
    rc::Rc,
    sync::{Arc, RwLock},
    thread,
    time::{Duration, SystemTime},
};

use burn_wgpu::Wgpu;

use clap::{builder::BoolishValueParser, Arg, ArgAction};

use tarpc::{client, context, tokio_serde::formats::Bincode};
use tokio::{net::lookup_host, sync::RwLock as RwLockTokio};

use self::ui::TermUI;

use umpire_ai::AI;

use umpire_tui::color::{palette16, palette24, palette256};

use common::{
    cli::{self, players_arg},
    conf,
    game::{
        ai::AISpec, player::PlayerControl, turn_async::TurnTaker, Game, IGame, PlayerNum,
        PlayerSecret, PlayerType,
    },
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
                .help("Don't produce sound")
                .action(ArgAction::SetTrue),
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
    let fog_darkness = *matches.get_one::<f64>("fog_darkness").unwrap();
    let unicode = matches.contains_id("unicode");
    let quiet = matches.contains_id("quiet");
    let confirm_turn_end = matches.contains_id("confirm_turn_end");

    let local_server = matches.contains_id("players");

    let (game, secrets, num_players, dims, player_types) = if local_server {
        let player_types = matches.get_one::<Vec<PlayerType>>("players").unwrap();

        let num_players: PlayerNum = player_types.len();
        let map_width = *matches.get_one::<u16>("map_width").unwrap();
        let map_height = *matches.get_one::<u16>("map_height").unwrap();

        let wrapping = *matches.get_one::<Wrap2d>("wrapping").unwrap();
        let fog_of_war = *matches.get_one::<bool>("fog").unwrap();

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
            Arc::new(RwLockTokio::new(game)) as Arc<RwLockTokio<dyn IGame>>,
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

        let server_addr = lookup_host(format!("{}:{}", server_hostname, conf::PORT))
            .await
            .map_err(|err| format!("Server DNS lookup error: {}", err))?
            .find(|addr| addr.is_ipv4())
            .ok_or(String::from(
                "No address returned looking up server domain name",
            ))?;

        let transport = tarpc::serde_transport::tcp::connect(server_addr, Bincode::default)
            .await
            .map_err(|err| {
                format!(
                    "Error connecting to server {} at address {}: {}",
                    server_hostname, server_addr, err
                )
            })?;

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

        let game = Arc::new(RwLockTokio::new(RpcGame::new(client))) as Arc<RwLockTokio<dyn IGame>>;

        let num_players = game.read().await.num_players().await;

        let dims = game.read().await.dims().await;

        (game, secrets, num_players, dims, player_types)
    };

    let palette = match color_depth {
        16 | 256 => match color_depth {
            16 => palette16(num_players).expect("Error loading 16-color palette"),
            256 => palette256(num_players).expect("Error loading 256-color palette"),
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

    // Make PlayerControl's for all players we have secrets for
    let mut ctrls: Vec<Option<PlayerControl>> = Vec::with_capacity(num_players);
    for player in 0..num_players {
        ctrls.push(if let Some(secret) = secrets[player] {
            Some(PlayerControl::new(Arc::clone(&game), player, secret).await)
        } else {
            None
        });
    }

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
        let mut ais: HashMap<AISpec, Rc<RefCell<AI<Wgpu>>>> = HashMap::new();

        if local_server {
            for ptype in player_types.iter() {
                if let PlayerType::AI(ai_type) = ptype {
                    let ai: AI<Wgpu> = ai_type.clone().into();
                    let ai = Rc::new(RefCell::new(ai));
                    // let player: Rc<RefCell<dyn TurnTaker>> = ai_type.clone().into();
                    ais.insert(ai_type.clone(), ai);
                }
            }
        }

        'outer: loop {
            if game.read().await.victor().await.is_some() {
                break 'outer;
            }

            let player = game.read().await.current_player().await;

            // Only take the turn locally if we have the corresponding player's secret
            if let Some(_secret) = secrets[player] {
                ui.log_message(format!("Player {}'s turn", player));

                let ctrl = ctrls.get_mut(player).unwrap().as_mut().unwrap();

                let is_ai = if let PlayerType::AI(_) = player_types[player] {
                    true
                } else {
                    false
                };

                // Automatically clear productions for AIs, but not for humans
                let mut turn = ctrl.turn_ctrl(is_ai).await;

                match &player_types[player] {
                    PlayerType::Human => {
                        let turn_outcome = ui.take_turn(&mut turn, None).await;
                        assert!(turn_outcome.training_instances.is_none());

                        if turn_outcome.quit {
                            turn.force_end_turn().await.unwrap();
                            break;
                        }
                    }
                    PlayerType::AI(ai_type) => {
                        let turn_outcome = ais
                            .get_mut(ai_type)
                            .unwrap()
                            .borrow_mut()
                            .take_turn(&mut turn, None)
                            .await;
                        assert!(turn_outcome.training_instances.is_none());

                        // I guess maybe someday a robot might throw in the towel?
                        if turn_outcome.quit {
                            turn.force_end_turn().await.unwrap();
                            break;
                        }
                    }
                }

                turn.force_end_turn().await.unwrap();
                debug_assert!(turn.ended());
            } else {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
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
