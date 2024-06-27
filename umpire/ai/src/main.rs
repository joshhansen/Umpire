//! Tool for working with Umpire's AIs
//!
//! Strategy:
//! First we bootstrap the AI by having it play against a random baseline.
//! Then we train it against itself.
//! These initial games should have small maps and only two players.
//!
//! Once we have a simple AI, incorporate it into the UI.

#![forbid(unsafe_code)]
use std::{
    cell::RefCell,
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::stdout,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};

use burn::{
    data::{dataloader::DataLoaderBuilder, dataset::Dataset},
    nn::DropoutConfig,
    optim::SgdConfig,
    prelude::*,
    record::{BinFileRecorder, FullPrecisionSettings},
    tensor::backend::AutodiffBackend,
};
use burn_autodiff::Autodiff;
use burn_train::{metric::LossMetric, LearnerBuilder};
use burn_wgpu::{Wgpu, WgpuDevice};

use clap::{builder::BoolishValueParser, value_parser, Arg, ArgAction};

use crossterm::{
    cursor::{MoveTo, Show},
    execute,
    terminal::{size, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};

use flate2::{read::GzDecoder, write::GzEncoder, Compression};

use umpire_ai::{
    agz::AgzActionModelConfig,
    data::{AgzBatcher, AgzData, AgzDatum},
    Storable,
};

use common::{
    game::{
        ai::{AiBackend, AiDevice, TrainingOutcome, POSSIBLE_ACTIONS, P_DROPOUT},
        map::gen::MapType,
        ActionNum, TurnNum,
    },
    util::{densify, init_rng},
};

use rand::prelude::SliceRandom;
use rand::Rng;

use tokio::sync::RwLock as RwLockTokio;

use common::{
    cli::{self, parse_ai_spec, Specified},
    conf,
    game::{
        ai::{AISpec, TrainingInstance},
        player::{PlayerControl, PlayerNum},
        turn_async::TurnTaker,
        Game, IGame,
    },
    name::IntNamer,
    util::{Dims, Rect, Vec2d, Wrap2d},
};

use umpire_ai::AI;
use umpire_tui::{color::palette16, map::Map, Component, Draw};

const SEED_INTERVAL: u64 = 924898;

fn parse_ai_specs(specs: &Vec<String>) -> Result<Vec<AISpec>, String> {
    let mut ai_specs: Vec<AISpec> = Vec::new();
    for ai_spec_s in specs {
        let sub_specs = parse_ai_spec(ai_spec_s)?;
        ai_specs.extend(sub_specs);
    }
    Ok(ai_specs)
}

fn load_ais(ai_types: &Vec<AISpec>) -> Result<Vec<Rc<RefCell<AI<Wgpu>>>>, String> {
    let mut unique_ais: BTreeMap<AISpec, Rc<RefCell<AI<Wgpu>>>> = BTreeMap::new();

    for ai_type in ai_types {
        println!("Loading AI type {}", ai_type);
        unique_ais.entry(ai_type.clone()).or_insert_with(|| {
            let ai: AI<Wgpu> = ai_type.clone().into();
            Rc::new(RefCell::new(ai))
        });
    }

    let mut ais: Vec<Rc<RefCell<AI<Wgpu>>>> = Vec::with_capacity(ai_types.len());
    for ai_type in ai_types {
        let ai: Rc<RefCell<AI<Wgpu>>> = Rc::clone(&unique_ais[ai_type]);
        ais.push(ai);
    }
    Ok(ais)
}

static AI_MODEL_SPECS_HELP: &str = "AI model specifications, comma-separated. The models to be evaluated. 'r' or 'random' for the purely random AI, or a serialized AI model file path, or directory path for TensorFlow SavedModel format";

static SUBCMD_AGZTRAIN: &str = "agztrain";

static SUBCMD_EVAL: &str = "eval";

#[tokio::main]
async fn main() -> Result<(), String> {
    let matches = cli::app("Umpire AI Trainer", "v")
    .version(conf::APP_VERSION)
    .author("Josh Hansen <umpire@joshhansen.tech>")
    .subcommand_required(true)
    .arg(
        Arg::new("episodes")
        .short('e')
        .long("episodes")
        .default_value("100")
        .value_parser(value_parser!(usize))
    )

    .arg(
        Arg::new("fix_output_loc")
        .short('F')
        .long("fix")
        .help("Fix the location of output. Makes the output seem animated.")
        .action(ArgAction::SetTrue)
    )
    .subcommand(
        cli::app(SUBCMD_EVAL, "MSwHWfgu")
        .about(format!("Have a set of AIs duke it out to see who plays the game of {} best", conf::APP_NAME))
        .arg(
            Arg::new("ai_models")
                .help(AI_MODEL_SPECS_HELP)
                .action(ArgAction::Append)
                .required(true)
        )
        .arg(
            Arg::new("datagenpath")
            .short('P')
            .long("datagenpath")
            .help("Generate state-action value function training data based on the eval output, serializing to this path")
        )
        .arg(
            Arg::new("datagenqty")
            .short('Q')
            .long("datagenqty")
            .help("The # of samples taken per class; really min(n,q) where n is samples available and q is qty desired")
            .value_parser(value_parser!(usize))
            .default_value("10")
        )
        .arg(
            Arg::new("datagenqty_eq")
            .long("eq")
            .help("Ensure the quantities of victories and defeats recorded are equal; use the min if less than the datagen qty (-Q)")
            .value_parser(BoolishValueParser::new())
            .default_value("true")
        )
        .arg(
            Arg::new("detsec")
            .long("detsec")
            .help("Generate secrets from the random seed if any; only use for benchmarking/profiling")
            .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new("steps")
            .short('s')
            .long("steps")
            .default_value("100000")
            .help("The number of steps to execute in each episode")
            .value_parser(value_parser!(usize))
        )
        .arg(
            Arg::new("ignored_outcomes")
            .short('i')
            .long("ignore")
            .help("Outcomes to ignore")
            .action(ArgAction::Append)
        )
    )
    .subcommand(
        cli::app(SUBCMD_AGZTRAIN, "DSgu")
        .about(format!("Train an AlphaGo Zero-inspired neural network AI for the game of {}", conf::APP_NAME))
        .arg_required_else_help(true)
        // .arg(
        //     Arg::new("initial_model_path")
        //         .short('i')
        //         .long("initial")
        //         .help("Serialized AI model file path for the initial model to use as a starting point for training")
        //         .value_parser(|s: &str| {
        //             if Path::new(&s).exists() {
        //                 Ok(String::from(s))
        //             } else {
        //                 Err(format!("Initial model path '{}' does not exist", s))
        //             }
        //         })
        // )
        .arg(
            Arg::new("out")
            .help("Output path to serialize the resulting AI model to")
            .short('o')
            .required(true)
        )
        .arg(
            Arg::new("sampleprob")
                .short('s')
                .help("Probability of an available instance being included in training")
                .value_parser(value_parser!(f64))
                .default_value("1.0")
        )
        .arg(
            Arg::new("validprob")
                .short('V')
                .help("Probability of an instance being included in the test set")
                .value_parser(value_parser!(f64))
                .default_value("0.05")
        )
        .arg(
            Arg::new("batchsize")
                .short('B')
                .long("batchsize")
                .help("Size of training batches")
                .value_parser(value_parser!(usize))
                .default_value("2048")
        )
        .arg(
            Arg::new("resume_epoch")
                .short('R')
                .long("resume")
                .help("Epoch of checkpoint to resume from")
                .value_parser(value_parser!(usize))
        )
        .arg(
            Arg::new("dataload_threads")
                .short('J')
                .long("dataload_threads")
                .help("Number of threads for the dataload process")
                .value_parser(value_parser!(usize))
                .default_value("8")
        )
        .arg(
            Arg::new("input")
                .help("Input files containing TrainingInstances")
                .action(ArgAction::Append)
                .required(true)
        )
    )// subcommand agztrain

    .get_matches();

    let (term_width, term_height) =
        size().map_err(|kind| format!("Could not get terminal size: {}", kind))?;

    // Arguments common across subcommands:
    let episodes = *matches.get_one::<usize>("episodes").unwrap();
    let fix_output_loc = *matches.get_one::<bool>("fix_output_loc").unwrap();

    let verbosity = matches.get_count("verbose");
    let (subcommand, sub_matches) = matches.subcommand().unwrap();

    match subcommand {
        "eval" => println!("Evaluating {} AIs", conf::APP_NAME),
        "agztrain" => println!("Training {} AI - a la AlphaGo Zero", conf::APP_NAME),
        c => unreachable!("Unrecognized subcommand {} should have been caught by the agument parser; there's a bug somehere", c)
    }

    let mut stdout = stdout();

    if fix_output_loc {
        execute!(stdout, EnterAlternateScreen).unwrap();
        execute!(stdout, Clear(ClearType::All)).unwrap();
        execute!(stdout, MoveTo(0, term_height - 7)).unwrap();
    }

    println!("Episodes: {}", episodes);

    println!("Verbosity: {}", verbosity);

    if subcommand == SUBCMD_EVAL {
        let steps: usize = sub_matches.get_one("steps").copied().unwrap();
        println!("Steps: {}", steps);

        let map_heights: Vec<u16> = sub_matches
            .get_many::<u16>("map_height")
            .unwrap()
            .cloned()
            .collect();
        let map_widths: Vec<u16> = sub_matches
            .get_many::<u16>("map_width")
            .unwrap()
            .cloned()
            .collect();

        let map_types: Vec<MapType> = sub_matches
            .get_many::<MapType>("map_type")
            .unwrap()
            .copied()
            .collect();
        let wrappings: Vec<Wrap2d> = sub_matches
            .get_many::<Wrap2d>("wrapping")
            .unwrap()
            .cloned()
            .collect();
        let fog_of_war = sub_matches.get_one::<bool>("fog").copied().unwrap();

        let ai_specs_s: Vec<String> = sub_matches
            .get_many::<String>("ai_models")
            .unwrap()
            .cloned()
            .collect();

        let gpu = sub_matches.get_one::<usize>("gpu").copied();
        let device = gpu.map_or_else(Default::default, AiDevice::DiscreteGpu);

        // Load up the AI specifications, respecting --gpu if present
        let ai_specs: Vec<AISpec> = {
            let mut ai_specs = parse_ai_specs(&ai_specs_s)?;

            for ai_spec in ai_specs.iter_mut() {
                match ai_spec {
                    AISpec::FromPath {
                        device: device_, ..
                    } => {
                        *device_ = device;
                    }
                    AISpec::FromLevel {
                        device: device_, ..
                    } => {
                        *device_ = device;
                    }
                    _ => {
                        // do nothing
                    }
                }
            }
            ai_specs
        };

        let mut ais: Vec<Rc<RefCell<AI<AiBackend>>>> = load_ais(&ai_specs)?;
        let num_ais = ais.len();

        let ignored_outcomes: BTreeSet<TrainingOutcome> = sub_matches
            .get_many::<String>("ignored_outcomes")
            .unwrap_or_default()
            .cloned()
            .filter_map(|s| TrainingOutcome::try_from(s).ok())
            .collect();

        let min_unit_choices: usize = sub_matches.get_one("min_unit_choices").copied().unwrap();
        println!("Min unit choices: {}", min_unit_choices);

        let datagenpath = sub_matches.get_one::<String>("datagenpath").map(Path::new);
        if let Some(datagenpath) = datagenpath {
            println!("Generating data to path: {}", datagenpath.display());

            if datagenpath.exists() {
                eprintln!(
                    "Warning: datagen path {} already exists; will overwrite",
                    datagenpath.display()
                )
            }
        }

        let datagen_qty: Option<usize> =
            datagenpath.map(|_| sub_matches.get_one("datagenqty").copied().unwrap());

        let datagen_qty_eq: bool = sub_matches.get_one("datagenqty_eq").copied().unwrap();

        if let Some(datagen_qty) = datagen_qty {
            println!("Datagen qty: {}", datagen_qty);
        }

        let mut data_outfile = datagenpath.map(|datagenpath| {
            let w = File::create(datagenpath).unwrap();
            GzEncoder::new(w, Compression::default())
        });

        let palette = palette16(num_ais).unwrap();

        let print_results = |victory_counts: &BTreeMap<Option<PlayerNum>, usize>,
                             game_lengths: &BTreeMap<TurnNum, usize>| {
            for (i, spec) in ai_specs.iter().map(|s| s.spec()).enumerate() {
                println!(
                    "{} wins: {}",
                    spec,
                    victory_counts.get(&Some(i)).unwrap_or(&0)
                );
            }
            println!("Draws: {}", victory_counts.get(&None).unwrap_or(&0));
            let mut total_games: usize = 0;
            let mut total_turns: TurnNum = 0;
            for (turn, freq) in game_lengths {
                total_games += *freq;
                total_turns += *turn * *freq as TurnNum;
            }
            let mean_game_length = total_turns as f64 / total_games as f64;
            println!("Average game length: {}", mean_game_length);
        };

        let mut seed = sub_matches.get_one::<u64>("random_seed").cloned();
        if let Some(seed) = seed.as_ref() {
            println!("Random seed: {:?}", seed);
        }
        let mut rng = init_rng(seed);
        let deterministic_secrets = sub_matches.get_one::<bool>("detsec").copied().unwrap();
        if deterministic_secrets {
            println!("***WARNING*** Secret generation may be deterministic");
        }

        let mut total_training_instances_written = 0usize;

        let mut victory_counts: BTreeMap<Option<PlayerNum>, usize> = BTreeMap::new();
        let mut game_lengths: BTreeMap<TurnNum, usize> = BTreeMap::new();
        for e in 0..episodes {
            let city_namer = IntNamer::new("city");

            let map_width = map_widths.choose(&mut rng).copied().unwrap();
            let map_height = map_heights.choose(&mut rng).copied().unwrap();
            let map_dims = Dims::new(map_width, map_height);
            let map_type = map_types.choose(&mut rng).copied().unwrap();
            let wrapping = wrappings.choose(&mut rng).cloned().unwrap();

            let mut maps: Vec<Map> = if fix_output_loc {
                // If they fit, put one map per player, side-by-side
                if map_width * num_ais as u16 <= term_width {
                    (0..num_ais)
                        .map(|player| {
                            let rect =
                                Rect::new(map_width * player as u16, 2, map_width, map_height);
                            let mut map = Map::new(rect, map_dims, false);
                            map.set_viewport_offset(Vec2d::new(0, 0));
                            map
                        })
                        .collect()
                } else {
                    // Otherwise, just make one map which we'll multiplex
                    let mut map = Map::new(Rect::new(0, 2, map_width, map_height), map_dims, false);
                    map.set_viewport_offset(Vec2d::new(0, 0));
                    vec![map]
                }
            } else {
                Vec::new()
            };

            let game_rng = init_rng(seed);
            let (game, secrets) = Game::new(
                Some(game_rng),
                deterministic_secrets,
                map_dims,
                map_type,
                city_namer,
                num_ais,
                fog_of_war,
                None,
                wrapping,
            );

            let game = Arc::new(RwLockTokio::new(game)) as Arc<RwLockTokio<dyn IGame>>;

            let mut ctrls: Vec<PlayerControl> = Vec::with_capacity(num_ais);
            for (player, secret) in secrets.iter().cloned().enumerate() {
                ctrls.push(PlayerControl::new(Arc::clone(&game), player, secret).await);
            }

            if fix_output_loc {
                execute!(stdout, MoveTo(0, 0)).unwrap();
            }

            println!(
                "Evaluating: {:?} {:?} {} {:?}",
                ai_specs_s, wrapping, map_dims, map_types
            );

            let mut player_partial_data: Option<BTreeMap<PlayerNum, Vec<TrainingInstance>>> =
                datagenpath.map(|_| BTreeMap::new());

            let mut last_turn: TurnNum = 0;
            'steps: for s in 0..steps {
                last_turn = s as TurnNum;
                for (player, ctrl) in ctrls.iter_mut().enumerate() {
                    if ctrl.victor().await.is_some() {
                        break 'steps;
                    }

                    let ai = ais.get_mut(player).unwrap();

                    let mut turn = ctrl.turn_ctrl(true).await;

                    let turn_outcome = ai
                        .borrow_mut()
                        .take_turn(&mut turn, Some(1.0), device)
                        .await;

                    if let Some(player_partial_data) = player_partial_data.as_mut() {
                        let partial_data =
                            player_partial_data.entry(player).or_insert_with(Vec::new);

                        partial_data.extend(
                            turn_outcome
                                .training_instances
                                .unwrap()
                                .into_iter()
                                // Only include instances where units had enough actions to choose from
                                .filter(|i| {
                                    (i.action.unit_action()
                                        && i.legal_actions.len() >= min_unit_choices)
                                        || i.action.city_action()
                                }),
                        );
                    }

                    if verbosity > 1 && fix_output_loc {
                        if maps.len() == 1 {
                            // Only one map would fit, so we take turns using it
                            let draw = s % 200 / 100 == player;
                            if draw {
                                maps.get_mut(0)
                                    .unwrap()
                                    .draw(&turn, &mut stdout, &palette)
                                    .await
                                    .unwrap();
                            }
                        } else {
                            debug_assert!(maps.len() > 1);
                            maps.get_mut(player)
                                .unwrap()
                                .draw(&turn, &mut stdout, &palette)
                                .await
                                .unwrap();
                        }

                        execute!(stdout, MoveTo(0, term_height - 10 - num_ais as u16)).unwrap();
                        println!("Game {} / {}", e, episodes);
                        println!("Step {} / {}", s, steps);
                    }

                    turn.force_end_turn().await.unwrap();
                }
            }

            *game_lengths.entry(last_turn).or_default() += 1;

            let mut data_by_outcome: BTreeMap<TrainingOutcome, Vec<TrainingInstance>> =
                BTreeMap::new();
            for t in TrainingOutcome::values() {
                data_by_outcome.insert(t, Vec::new());
            }
            if let Some(player_partial_data) = player_partial_data {
                // Mark the training instances (if we've been tracking them) with the game's outcome

                if let Some(victor) = game.read().await.victor().await {
                    for (player, partial_data) in player_partial_data.into_iter() {
                        for mut instance in partial_data {
                            if player == victor {
                                if !ignored_outcomes.contains(&TrainingOutcome::Victory) {
                                    instance.victory(last_turn);
                                    data_by_outcome
                                        .get_mut(&TrainingOutcome::Victory)
                                        .unwrap()
                                        .push(instance);
                                }
                            } else if !ignored_outcomes.contains(&TrainingOutcome::Defeat) {
                                instance.defeat(last_turn);
                                data_by_outcome
                                    .get_mut(&TrainingOutcome::Defeat)
                                    .unwrap()
                                    .push(instance);
                            }
                        }
                    }
                } else if !ignored_outcomes.contains(&TrainingOutcome::Inconclusive) {
                    for partial_data in player_partial_data.into_values() {
                        for mut instance in partial_data {
                            instance.inconclusive(last_turn);
                            data_by_outcome
                                .get_mut(&TrainingOutcome::Inconclusive)
                                .unwrap()
                                .push(instance);
                        }
                    }
                }

                debug_assert!(datagenpath.is_some());

                // Shuffle and truncate per-class
                for data in data_by_outcome.values_mut() {
                    data.shuffle(&mut rng);
                    data.truncate(datagen_qty.unwrap());
                }

                if datagen_qty_eq {
                    // Ensure that victories and defeats are recorded in equal quantity
                    // This likely means truncating further
                    // TODO Only truncate once per outcome
                    let victory_qty = data_by_outcome[&TrainingOutcome::Victory].len();
                    let defeat_qty = data_by_outcome[&TrainingOutcome::Defeat].len();
                    match victory_qty.cmp(&defeat_qty) {
                        Ordering::Greater => {
                            data_by_outcome
                                .get_mut(&TrainingOutcome::Victory)
                                .unwrap()
                                .truncate(defeat_qty);
                        }
                        Ordering::Less => {
                            data_by_outcome
                                .get_mut(&TrainingOutcome::Defeat)
                                .unwrap()
                                .truncate(victory_qty);
                        }
                        Ordering::Equal => {
                            // do nothing
                        }
                    }
                }

                // Write the training instances
                let mut w = data_outfile.as_mut().unwrap();
                let mut training_instances_written = 0usize;

                for instance in data_by_outcome
                    .into_values()
                    .flat_map(|values| values.into_iter())
                {
                    debug_assert!(instance.outcome.is_some());
                    bincode::serialize_into(&mut w, &instance).unwrap();

                    training_instances_written += 1;
                    total_training_instances_written += 1;
                }

                if fix_output_loc {
                    execute!(stdout, MoveTo(0, term_height - 1),).unwrap();
                }
                println!(
                    "Wrote {} ({} total)",
                    training_instances_written, total_training_instances_written
                );
            }

            *victory_counts
                .entry(game.read().await.victor().await)
                .or_default() += 1;

            println!();
            print_results(&victory_counts, &game_lengths);

            if let Some(seed) = seed.as_mut() {
                *seed += SEED_INTERVAL;
            }

            if fix_output_loc {
                for map in maps.iter_mut() {
                    map.clear(&mut stdout);
                }
            }
        } // end for each episode

        execute!(stdout, LeaveAlternateScreen).unwrap();

        print_results(&victory_counts, &game_lengths);

        println!(
            "Total training instances written: {}",
            total_training_instances_written,
        );
    } else if subcommand == SUBCMD_AGZTRAIN {
        let batch_size: usize = sub_matches.get_one("batchsize").copied().unwrap();
        let learning_rate = sub_matches
            .get_one::<f64>("dnn_learning_rate")
            .copied()
            .unwrap();
        let gpu = sub_matches.get_one::<usize>("gpu").copied();

        println!("Batch size: {}", batch_size);
        println!("Learning rate: {}", learning_rate);
        if let Some(gpu) = gpu {
            println!("GPU: {}", gpu);
        }

        let dataload_threads: usize = sub_matches.get_one("dataload_threads").copied().unwrap();
        println!("Dataload threads: {}", dataload_threads);

        let input_paths: Vec<String> = sub_matches.get_many("input").unwrap().cloned().collect();

        let output_path: String = sub_matches.get_one("out").cloned().unwrap();
        let output_path = Path::new(&output_path).to_owned();

        let device = gpu.map_or_else(Default::default, WgpuDevice::DiscreteGpu);

        let dropout_config = DropoutConfig::new(P_DROPOUT);
        let model_config = AgzActionModelConfig::new(POSSIBLE_ACTIONS, dropout_config);

        let sample_prob: f64 = sub_matches.get_one("sampleprob").copied().unwrap();
        let valid_prob: f64 = sub_matches.get_one("validprob").copied().unwrap();

        println!("Sample prob: {}", sample_prob);
        println!("Validation prob: {}", valid_prob);

        let resume_epoch: Option<usize> = sub_matches.get_one("resume_epoch").copied();

        let min_unit_choices: usize = sub_matches.get_one("min_unit_choices").copied().unwrap();
        println!("Min unit choices: {}", min_unit_choices);

        let mut data: Vec<AgzDatum> = Vec::new();

        let seed = sub_matches.get_one::<u64>("random_seed").copied();
        if let Some(seed) = seed.as_ref() {
            println!("Random seed: {:?}", seed);
        }
        let mut rng = init_rng(seed);

        for input_path in input_paths {
            if verbosity > 0 {
                println!("Loading {}", input_path);
            }

            let r = File::open(input_path).unwrap();
            let mut r = GzDecoder::new(r);

            let mut count = 0usize;

            loop {
                let maybe_instance: bincode::Result<TrainingInstance> =
                    bincode::deserialize_from(&mut r);

                if let Ok(instance) = maybe_instance {
                    // If it was a unit action, make sure it chose between at least min_unit_choices options
                    if (instance.action.unit_action()
                        && instance.legal_actions.len() >= min_unit_choices)
                        || instance.action.city_action()
                    {
                        let outcome = instance.outcome.unwrap();
                        count += 1;

                        if rng.gen_bool(sample_prob) {
                            data.push(AgzDatum {
                                features: densify(instance.num_features, &instance.features),
                                turns_until_outcome: instance.last_turn.unwrap() - instance.turn,
                                action: instance.action.into(),
                                outcome,
                            });
                        }
                    }
                } else {
                    break;
                }
            }

            if verbosity > 0 {
                println!("\tLoaded {}", count);
            }
        }

        let mut class_balance: BTreeMap<TrainingOutcome, usize> = BTreeMap::new();
        for datum in &data {
            *class_balance.entry(datum.outcome).or_default() += 1;
        }

        println!("Class balance: {:?}", class_balance);

        let mut train_data: Vec<AgzDatum> = Vec::new();
        let mut valid_data: Vec<AgzDatum> = Vec::new();

        for datum in data.into_iter() {
            if rng.gen_bool(valid_prob) {
                valid_data.push(datum);
            } else {
                train_data.push(datum);
            }
        }

        let train_data: AgzData = AgzData::new(train_data);
        let valid_data: AgzData = AgzData::new(valid_data);

        println!("Train size: {}", train_data.len());
        println!("Valid size: {}", valid_data.len());

        // let adam_config = AdamConfig::new();
        let opt_config = SgdConfig::new();

        let mut train_config =
            TrainingConfig::new(model_config, opt_config, batch_size, dataload_threads);
        train_config.batch_size = batch_size;
        train_config.learning_rate = learning_rate;
        train_config.num_epochs = episodes;

        train::<Autodiff<Wgpu>, PathBuf>(
            &output_path,
            train_config,
            device,
            train_data,
            valid_data,
            resume_epoch,
        );
    } else {
        return Err(String::from("A subcommand must be given"));
    }

    if fix_output_loc {
        execute!(stdout, LeaveAlternateScreen).unwrap();
        execute!(stdout, Show).unwrap();
    }

    Ok(())
}

#[derive(Config)]
pub struct TrainingConfig {
    pub model: AgzActionModelConfig,

    pub optimizer: SgdConfig,

    #[config(default = 10)]
    pub num_epochs: usize,

    pub batch_size: usize,

    pub dataload_threads: usize,

    #[config(default = 42)]
    pub seed: u64,

    #[config(default = 1.0e-4)]
    pub learning_rate: f64,
}

fn create_artifact_dir<P: AsRef<Path>>(artifact_dir: &P) {
    // Remove existing artifacts before to get an accurate learner summary
    std::fs::remove_dir_all(artifact_dir).ok();
    std::fs::create_dir_all(artifact_dir).ok();
}

pub fn train<B: AutodiffBackend, P: AsRef<Path>>(
    artifact_dir: &P,
    config: TrainingConfig,
    device: B::Device,
    train: AgzData,
    valid: AgzData,
    resume_epoch: Option<usize>,
) {
    let artifact_dir_s: &str = artifact_dir.as_ref().to_str().unwrap();
    create_artifact_dir(artifact_dir);

    let config_path = {
        let mut p = artifact_dir.as_ref().to_path_buf();
        p.push("config.json");
        p.as_path().to_owned()
    };

    config
        .save(config_path)
        .expect("Config should be saved successfully");

    B::seed(config.seed);

    let batcher_train = AgzBatcher::<B>::new(device.clone());
    let batcher_valid = AgzBatcher::<B::InnerBackend>::new(device.clone());

    let dataloader_train = DataLoaderBuilder::new(batcher_train)
        .batch_size(config.batch_size)
        .shuffle(config.seed)
        .num_workers(config.dataload_threads)
        .build(train);

    let dataloader_valid = DataLoaderBuilder::new(batcher_valid)
        .batch_size(config.batch_size)
        .shuffle(config.seed)
        .num_workers(config.dataload_threads)
        .build(valid);

    let mut learner_builder = LearnerBuilder::new(artifact_dir_s)
        .metric_train_numeric(LossMetric::new())
        .metric_valid_numeric(LossMetric::new())
        .with_file_checkpointer(BinFileRecorder::<FullPrecisionSettings>::new())
        .devices(vec![device.clone()])
        .num_epochs(config.num_epochs)
        .summary();

    if let Some(resume_epoch) = resume_epoch {
        learner_builder = learner_builder.checkpoint(resume_epoch);
    }

    let learner = learner_builder.build(
        config.model.init::<B>(device),
        config.optimizer.init(),
        config.learning_rate,
    );

    let model_path = {
        let mut p = artifact_dir.as_ref().to_path_buf();
        p.push("model");
        p.as_path().to_owned()
    };

    let model_trained = learner.fit(dataloader_train, dataloader_valid);
    model_trained.store(&model_path).unwrap();
}
