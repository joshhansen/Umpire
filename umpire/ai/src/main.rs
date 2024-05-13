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
    collections::HashMap,
    fs::File,
    io::stdout,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};

use burn::{
    backend::wgpu::WgpuDevice,
    data::{dataloader::DataLoaderBuilder, dataset::Dataset},
    optim::SgdConfig,
    prelude::*,
    record::CompactRecorder,
    tensor::backend::AutodiffBackend,
};
use burn_autodiff::Autodiff;
use burn_train::{metric::LossMetric, LearnerBuilder};
use burn_wgpu::Wgpu;

use clap::{value_parser, Arg, ArgAction};

use crossterm::{
    cursor::{MoveTo, Show},
    execute,
    terminal::{size, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};

use umpire_ai::{
    agz::AgzActionModelConfig,
    data::{AgzBatcher, AgzData, AgzDatum},
    AiBackend, Storable,
};

use common::{
    game::ai::{fX, POSSIBLE_ACTIONS},
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
use umpire_tui::{color::palette16, map::Map, Draw};

fn parse_ai_specs(specs: &Vec<String>) -> Result<Vec<AISpec>, String> {
    let mut ai_specs: Vec<AISpec> = Vec::new();
    for ai_spec_s in specs {
        let sub_specs = parse_ai_spec(ai_spec_s)?;
        ai_specs.extend(sub_specs);
    }
    Ok(ai_specs)
}

fn load_ais<B: Backend>(ai_types: &Vec<AISpec>) -> Result<Vec<Rc<RefCell<AI<B>>>>, String> {
    let mut unique_ais: HashMap<AISpec, Rc<RefCell<AI<B>>>> = HashMap::new();

    for ai_type in ai_types {
        println!("Loading AI type {}", ai_type);
        unique_ais.entry(ai_type.clone()).or_insert_with(|| {
            let ai: AI<B> = ai_type.clone().into();
            Rc::new(RefCell::new(ai))
        });
    }

    let mut ais: Vec<Rc<RefCell<AI<B>>>> = Vec::with_capacity(ai_types.len());
    for ai_type in ai_types {
        let ai: Rc<RefCell<AI<B>>> = Rc::clone(&unique_ais[ai_type]);
        ais.push(ai);
    }
    Ok(ais)
}

static AI_MODEL_SPECS_HELP: &str = "AI model specifications, comma-separated. The models to be evaluated. 'r' or 'random' for the purely random AI, or a serialized AI model file path, or directory path for TensorFlow SavedModel format";

static SUBCMD_AGZTRAIN: &str = "agztrain";

static SUBCMD_EVAL: &str = "eval";

#[tokio::main]
async fn main() -> Result<(), String> {
    let matches = cli::app("Umpire AI Trainer", "fvwHW")
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
        Arg::new("steps")
        .short('s')
        .long("steps")
        .default_value("100000")
        .help("The number of steps to execute in each episode")
        .value_parser(value_parser!(u64))
    )

    .arg(
        Arg::new("fix_output_loc")
        .short('F')
        .long("fix")
        .help("Fix the location of output. Makes the output seem animated.")
        .action(ArgAction::SetTrue)
    )

    // .subcommand(
    //     SubCommand::new("datagen")
    //     .about("Generate data for direct modeling of state-action values")
    //     .arg(
    //         Arg::new("out")
    //         .help("Output path for CSV formatted data")
    //         .multiple(false)
    //         .required(true)
    //     )
    // )

    .subcommand(
        cli::app(SUBCMD_EVAL, "S")
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
            Arg::new("datagenprob")
            .short('p')
            .long("datagenprob")
            .help("The probability that a training instance will be included in the serialized output. Since training instances in the same episode are highly correlated it can be helpful to use only a sample.")
            .value_parser(value_parser!(f64))
            .default_value("1.0")
        )
    )

    .subcommand(
        cli::app(SUBCMD_AGZTRAIN, "D")
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
            Arg::new("testprob")
                .short('t')
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
                .default_value("512")
        )
        .arg(
            Arg::new("gpu")
                .short('g')
                .long("gpu")
                .help("Index of the GPU to use; falls back to CPU if none exists")
                .value_parser(value_parser!(usize))
                .default_value("0")
        )
        .arg(
            Arg::new("input")
                .help("Input files containing TrainingInstances")
                .action(ArgAction::Append)
                .required(true)
        )
    )// subcommand agztrain

    .get_matches();

    let (_term_width, term_height) =
        size().map_err(|kind| format!("Could not get terminal size: {}", kind))?;

    // Arguments common across subcommands:
    let episodes = *matches.get_one::<usize>("episodes").unwrap();
    let fix_output_loc = *matches.get_one::<bool>("fix_output_loc").unwrap();
    let fog_of_war = *matches.get_one::<bool>("fog").unwrap();

    let map_heights: Vec<u16> = matches
        .get_many::<u16>("map_height")
        .unwrap()
        .cloned()
        .collect();
    let map_widths: Vec<u16> = matches
        .get_many::<u16>("map_width")
        .unwrap()
        .cloned()
        .collect();

    if map_heights.len() != map_widths.len() {
        return Err(String::from(
            "The same number of widths and heights must be specified",
        ));
    }

    let dims: Vec<Dims> = map_heights
        .into_iter()
        .enumerate()
        .map(|(i, h)| Dims::new(map_widths[i], h))
        .collect();

    let steps = *matches.get_one::<u64>("steps").unwrap();
    let verbosity = matches.get_count("verbose");
    let wrappings: Vec<Wrap2d> = matches
        .get_many::<Wrap2d>("wrapping")
        .unwrap()
        .cloned()
        .collect();

    let (subcommand, sub_matches) = matches.subcommand().unwrap();

    match subcommand {
        "eval" => println!("Evaluating {} AIs", conf::APP_NAME),
        "qtrain" => println!("Training {} AI - Q-Learning", conf::APP_NAME),
        "agztrain" => println!("Training {} AI - a la AlphaGo Zero", conf::APP_NAME),
        c => unreachable!("Unrecognized subcommand {} should have been caught by the agument parser; there's a bug somehere", c)
    }

    let mut stdout = stdout();

    if fix_output_loc {
        execute!(stdout, EnterAlternateScreen).unwrap();
        execute!(stdout, Clear(ClearType::All)).unwrap();
        execute!(stdout, MoveTo(0, term_height - 7)).unwrap();
    }

    println!(
        "Dimensions: {}",
        dims.iter()
            .map(|dims| format!("{}", dims))
            .collect::<Vec<String>>()
            .join(", ")
    );

    println!("Episodes: {}", episodes);

    println!("Steps: {}", steps);

    println!("Verbosity: {}", verbosity);

    if subcommand == SUBCMD_EVAL {
        // if dims.len() > 1 {
        //     return Err(String::from("Only one set of dimensions can be given for evaluation"));
        // }

        let ai_specs_s: Vec<String> = sub_matches
            .get_many::<String>("ai_models")
            .unwrap()
            .cloned()
            .collect();
        let ai_specs: Vec<AISpec> = parse_ai_specs(&ai_specs_s)?;
        let mut ais: Vec<Rc<RefCell<AI<AiBackend>>>> = load_ais(&ai_specs)?;

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

        let generate_data = datagenpath.is_some();

        let datagen_prob = if generate_data {
            Some(sub_matches.get_one::<f64>("datagenprob").cloned().unwrap())
        } else {
            None
        };

        let mut data_outfile = datagenpath.map(|datagenpath| File::create(datagenpath).unwrap());

        let num_ais = ais.len();

        let palette = palette16(num_ais).unwrap();

        let print_results = |victory_counts: &HashMap<Option<PlayerNum>, usize>| {
            for (i, spec) in ai_specs.iter().map(|s| s.spec()).enumerate() {
                println!(
                    "{} wins: {}",
                    spec,
                    victory_counts.get(&Some(i)).unwrap_or(&0)
                );
            }
            println!("Draws: {}", victory_counts.get(&None).unwrap_or(&0));
        };

        let seed = matches.get_one::<u64>("random_seed").cloned();
        let mut rng = init_rng(seed);

        let mut victory_counts: HashMap<Option<PlayerNum>, usize> = HashMap::new();
        for _ in 0..episodes {
            let city_namer = IntNamer::new("city");

            let map_dims = dims.choose(&mut rng).cloned().unwrap();

            let map_width = map_dims.width;
            let map_height = map_dims.height;

            let mut map = if fix_output_loc {
                let mut map = Map::new(Rect::new(0, 2, map_width, map_height), map_dims, false);
                map.set_viewport_offset(Vec2d::new(0, 0));
                Some(map)
            } else {
                None
            };

            let wrapping = wrappings.choose(&mut rng).cloned().unwrap();

            let (game, secrets) = Game::new(
                Some(init_rng(seed)), // instantiate another rng for Game to own
                map_dims,
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

            println!("Evaluating: {:?} {:?} {}", ai_specs_s, wrapping, map_dims);

            let mut player_partial_data: Option<HashMap<PlayerNum, Vec<TrainingInstance>>> =
                datagenpath.map(|_| HashMap::new());

            'steps: for s in 0..steps {
                for (player, ctrl) in ctrls.iter_mut().enumerate() {
                    if ctrl.victor().await.is_some() {
                        break 'steps;
                    }

                    let draw = s % 200 / 100 == player as u64;

                    let ai = ais.get_mut(player).unwrap();

                    let mut turn = ctrl.turn_ctrl(true).await;

                    let turn_outcome = ai
                        .borrow_mut()
                        .take_turn(&mut rng, &mut turn, datagen_prob)
                        .await;

                    if let Some(player_partial_data) = player_partial_data.as_mut() {
                        let partial_data =
                            player_partial_data.entry(player).or_insert_with(Vec::new);

                        partial_data.extend(turn_outcome.training_instances.unwrap().into_iter());
                    }

                    if verbosity > 1 && fix_output_loc && draw {
                        map.as_mut()
                            .unwrap()
                            .draw(&turn, &mut stdout, &palette)
                            .await
                            .unwrap();
                        execute!(stdout, MoveTo(0, map_height + 2)).unwrap();
                    }

                    turn.force_end_turn().await.unwrap();
                }
            }

            if let Some(mut player_partial_data) = player_partial_data {
                // Mark the training instances (if we've been tracking them) with the game's outcome

                let players: Vec<usize> = player_partial_data.keys().cloned().collect();
                if let Some(victor) = game.read().await.victor().await {
                    for player in players {
                        let data = player_partial_data.get_mut(&player).unwrap();

                        for instance in data {
                            if player == victor {
                                instance.victory();
                            } else {
                                instance.defeat();
                            }
                        }
                    }
                } else {
                    for player in players {
                        let data = player_partial_data.get_mut(&player).unwrap();
                        for instance in data {
                            instance.inconclusive();
                        }
                    }
                }

                if generate_data {
                    // Write the training instances
                    let mut w = data_outfile.as_mut().unwrap();

                    for instance in player_partial_data
                        .into_values()
                        .flat_map(|values| values.into_iter())
                    {
                        debug_assert!(instance.outcome.is_some());
                        bincode::serialize_into(&mut w, &instance).unwrap();
                    }
                }
            }

            *victory_counts
                .entry(game.read().await.victor().await)
                .or_insert(0) += 1;

            println!();
            print_results(&victory_counts);
        }

        execute!(stdout, LeaveAlternateScreen).unwrap();

        print_results(&victory_counts);
    } else if subcommand == SUBCMD_AGZTRAIN {
        let batch_size = sub_matches.get_one::<usize>("batchsize").cloned().unwrap();
        let learning_rate = *sub_matches.get_one::<f64>("dnn_learning_rate").unwrap();
        let gpu = sub_matches.get_one::<usize>("gpu").cloned().unwrap();

        println!("Learning rate: {}", learning_rate);

        {
            let input_paths: Vec<String> = sub_matches
                .get_many::<String>("input")
                .unwrap()
                .cloned()
                .collect();

            let output_path = sub_matches.get_one::<String>("out").unwrap().clone();
            let output_path = Path::new(&output_path).to_owned();

            let device = WgpuDevice::DiscreteGpu(gpu);

            let sample_prob: f64 = sub_matches.get_one("sampleprob").cloned().unwrap();

            let model_config = AgzActionModelConfig::new(POSSIBLE_ACTIONS);

            let test_prob: f64 = sub_matches.get_one("testprob").cloned().unwrap();

            println!("Test portion: {}", test_prob);

            let mut train_data: Vec<AgzDatum> = Vec::new();

            let mut valid_data: Vec<AgzDatum> = Vec::new();

            let seed = sub_matches.get_one::<u64>("random_seed").cloned();
            let mut rng = init_rng(seed);

            for input_path in input_paths {
                if verbosity > 0 {
                    println!("Loading {}", input_path);
                }

                let data = {
                    let mut r = File::open(input_path).unwrap();

                    let mut data: Vec<TrainingInstance> = Vec::new();

                    loop {
                        let maybe_instance: bincode::Result<TrainingInstance> =
                            bincode::deserialize_from(&mut r);

                        if let Ok(instance) = maybe_instance {
                            data.push(instance);
                        } else {
                            break;
                        }
                    }
                    data
                };

                for datum in data {
                    if rng.gen::<f64>() > sample_prob {
                        let features: Vec<fX> = densify(datum.num_features, &datum.features);

                        let datum = AgzDatum {
                            features,
                            action: datum.action.into(),
                            outcome: datum.outcome.unwrap(),
                        };

                        if rng.gen::<f64>() <= test_prob {
                            valid_data.push(datum);
                        } else {
                            train_data.push(datum);
                        }
                    }
                }
            }

            let train_data: AgzData = AgzData::new(train_data);
            let valid_data: AgzData = AgzData::new(valid_data);

            println!("Train size: {}", train_data.len());
            println!("Valid size: {}", valid_data.len());

            // let adam_config = AdamConfig::new();
            let opt_config = SgdConfig::new();

            let mut train_config = TrainingConfig::new(model_config, opt_config);
            train_config.batch_size = batch_size;
            train_config.learning_rate = learning_rate;
            train_config.num_epochs = episodes;

            train::<Autodiff<Wgpu>, PathBuf>(
                &output_path,
                train_config,
                device,
                train_data,
                valid_data,
            );
        }
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

    #[config(default = 256)]
    pub batch_size: usize,

    #[config(default = 4)]
    pub num_workers: usize,

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
        .num_workers(config.num_workers)
        .build(train);

    let dataloader_valid = DataLoaderBuilder::new(batcher_valid)
        .batch_size(config.batch_size)
        .shuffle(config.seed)
        .num_workers(config.num_workers)
        .build(valid);

    let learner = LearnerBuilder::new(artifact_dir_s)
        .metric_train_numeric(LossMetric::new())
        .metric_valid_numeric(LossMetric::new())
        .with_file_checkpointer(CompactRecorder::new())
        .devices(vec![device.clone()])
        .num_epochs(config.num_epochs)
        .summary()
        .build(
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
