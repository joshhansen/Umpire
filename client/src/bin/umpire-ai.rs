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
    io::{stdout, Write},
    path::Path,
    rc::Rc,
};

use clap::{value_parser, Arg, ArgAction, Command};

use crossterm::{
    cursor::MoveTo,
    execute,
    terminal::{size, Clear, ClearType},
};

use common::{
    cli::{self, parse_spec},
    conf,
    game::{
        ai::{AISpec, TrainingInstance},
        player::{PlayerNum, TurnTaker},
        Game,
    },
    name::IntNamer,
    util::{Dims, Rect, Vec2d, Wrap2d},
};
use rand::{prelude::SliceRandom, thread_rng};

use umpire_client::{
    color::palette16,
    game::ai::{rl::trained_agent, Storable, AI},
    ui::{Draw, Map},
};

fn parse_ai_spec<S: AsRef<str>>(spec: S) -> Result<Vec<AISpec>, String> {
    parse_spec(spec, "AI")
}

fn parse_ai_specs(specs: &Vec<&str>) -> Result<Vec<AISpec>, String> {
    let mut ai_specs: Vec<AISpec> = Vec::new();
    for ai_spec_s in specs {
        let sub_specs = parse_ai_spec(ai_spec_s)?;
        ai_specs.extend(sub_specs);
    }
    Ok(ai_specs)
}

fn load_ais(ai_types: &Vec<AISpec>) -> Result<Vec<Rc<RefCell<AI>>>, String> {
    let mut ais: Vec<Rc<RefCell<AI>>> = Vec::with_capacity(ai_types.len());
    for ai_type in ai_types {
        let ai: AI = ai_type.clone().into();
        let ai = Rc::new(RefCell::new(ai));
        ais.push(ai);
    }
    Ok(ais)
}

static AI_MODEL_SPECS_HELP: &'static str = "AI model specifications, comma-separated. The models to be evaluated. 'r' or 'random' for the purely random AI, or a serialized AI model file path, or directory path for TensorFlow SavedModel format";

fn main() -> Result<(), String> {
    let matches = cli::app("Umpire AI Trainer", "fvwHW")
    .version(conf::APP_VERSION)
    .author("Josh Hansen <hansen.joshuaa@gmail.com>")
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
        Command::new("eval")
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
    )

    .subcommand(
        cli::app("train", "")
        .about(format!("Train an AI for the game of {}", conf::APP_NAME))
        .arg_required_else_help(true)
        .arg(
            Arg::new("avoid_skip")
            .short('a')
            .long("avoid_skip")
            .help("Execute policies in a way that avoids the SkipNextUnit action when possible")
            .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new("qlearning_alpha")
            .short('A')
            .long("alpha")
            .help("The alpha parameter (learning rate) for the Q-Learning algorithm")
            .value_parser(value_parser!(f64))
            .default_value("0.01")
        )
        .arg(
            Arg::new("qlearning_gamma")
            .short('G')
            .long("gamma")
            .help("The gamma parameter (discount rate) for the Q-Learning algorithm")
            .value_parser(value_parser!(f64))
            .default_value("0.9")
        )
        .arg(
            Arg::new("epsilon")
            .short('E')
            .long("epsilon")
            .help("The epsilon of the epsilon-greedy training policy. The probability of taking a random action rather the policy action")
            .value_parser(value_parser!(f64))
            .default_value("0.05")
        )
        .arg(
            Arg::new("epsilon_decay")
            .long("epsilon-decay")
            .help("A factor to multiply the epsilon by with some probability.")
            .value_parser(value_parser!(f64))
            .default_value("1.0")
        )
        .arg(
            Arg::new("epsilon_decay_prob")
            .long("epsilon-decay-prob")
            .help("The probability, when sampling from the epsilon-greedy policy, of decaying the epsilon")
            .value_parser(value_parser!(f64))
            .default_value("10e-4")
        )
        .arg(
            Arg::new("min_epsilon")
            .long("min-epsilon")
            .help("The lowest value that epsilon should decay to")
            .value_parser(value_parser!(f64))
            .default_value("0.0")
        )
        .arg(
            Arg::new("dnn_learning_rate")
            .short('D')
            .long("dnnlr")
            .help("The learning rate of the neural network (if any)")
            .value_parser(value_parser!(f32))
            .default_value("10e-3")
        )
        .arg(
            Arg::new("deep")
            .short('d')
            .long("deep")
            .help("Indicates that a deep neural network should be trained rather than a linear function approximator")
        )
        .arg(
            Arg::new("initial_model_path")
                .short('i')
                .long("initial")
                .help("Serialized AI model file path for the initial model to use as a starting point for training")
                .value_parser(|s: &str| {
                    if Path::new(&s).exists() {
                        Ok(String::from(s))
                    } else {
                        Err(format!("Initial model path '{}' does not exist", s))
                    }
                })
        )
        .arg(
            Arg::new("out")
            .help("Output path to serialize the resulting AI model to")
            .required(true)
        )
        // .arg(
        //     Arg::new("opponent")
        //         .help(AI_MODEL_SPECS_HELP)
        //         .multiple(true)
        //         .required(true)
        // )
    )

    .get_matches();

    let (_term_width, term_height) =
        size().map_err(|kind| format!("Could not get terminal size: {}", kind))?;

    // Arguments common across subcommands:
    let episodes = matches.get_one::<usize>("episodes").unwrap().clone();
    let fix_output_loc = matches.contains_id("fix_output_loc");
    let fog_of_war = matches.get_one::<bool>("fog").unwrap().clone();

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

    let steps = matches.get_one::<u64>("steps").unwrap().clone();
    let verbosity = matches.get_count("verbose");
    let wrappings: Vec<Wrap2d> = matches
        .get_many::<Wrap2d>("wrapping")
        .unwrap()
        .cloned()
        .collect();

    let (subcommand, sub_matches) = matches.subcommand().unwrap();

    match subcommand {
        "eval" => println!("Evaluating {} AIs", conf::APP_NAME),
        "train" => println!("Training {} AI", conf::APP_NAME),
        c => unreachable!("Unrecognized subcommand {} should have been caught by the agument parser; there's a bug somehere", c)
    }

    let mut stdout = stdout();

    if fix_output_loc {
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

    if subcommand == "eval" {
        // if dims.len() > 1 {
        //     return Err(String::from("Only one set of dimensions can be given for evaluation"));
        // }

        let map_width = dims[0].width;
        let map_height = dims[0].height;

        let ai_specs_s: Vec<&str> = sub_matches
            .get_many::<&str>("ai_models")
            .unwrap()
            .cloned()
            .collect();
        let ai_specs: Vec<AISpec> = parse_ai_specs(&ai_specs_s)?;
        let mut ais: Vec<Rc<RefCell<AI>>> = load_ais(&ai_specs)?;

        let datagenpath = sub_matches.get_one::<&str>("datagenpath").map(Path::new);
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

        let mut data_outfile = datagenpath.map(|datagenpath| File::create(datagenpath).unwrap());

        // let data = bincode::serialize(&fa).unwrap();

        // let display = path.display();

        // let mut file = File::create(&path).map_err(|err| {
        //     format!("couldn't create {}: {}", display, err)
        // })?;

        // file.write_all(&data).map_err(|err| format!("Couldn't write to {}: {}", display, err))

        let num_ais = ais.len();

        let mut map = if fix_output_loc {
            let mut map = Map::new(
                Rect::new(0, 2, map_width, map_height),
                Dims::new(map_width, map_height),
                false,
            );
            map.set_viewport_offset(Vec2d::new(0, 0));
            Some(map)
        } else {
            None
        };

        let palette = palette16(num_ais).unwrap();

        let mut victory_counts: HashMap<Option<PlayerNum>, usize> = HashMap::new();
        for _ in 0..episodes {
            let city_namer = IntNamer::new("city");

            let mut rng = thread_rng();

            let map_dims = dims.choose(&mut rng).cloned().unwrap();
            let wrapping = wrappings.choose(&mut rng).cloned().unwrap();

            let mut game = Game::new(map_dims, city_namer, num_ais, fog_of_war, None, wrapping);

            if fix_output_loc {
                execute!(stdout, MoveTo(0, 0)).unwrap();
            }

            println!("Evaluating: {:?} {:?} {:?}", ai_specs_s, wrapping, dims);

            if verbosity > 1 {
                if fix_output_loc {
                    let ctrl = game.player_turn_control_nonending(0);

                    map.as_mut().unwrap().draw(&ctrl, &mut stdout, &palette);
                } else {
                    println!("{:?}", game);
                }
            }

            let mut player_partial_data: Option<HashMap<PlayerNum, Vec<TrainingInstance>>> =
                datagenpath.map(|_| HashMap::new());

            'steps: for _ in 0..steps {
                for i in 0..num_ais {
                    if game.victor().is_some() {
                        break 'steps;
                    }

                    let player = game.current_player();

                    let ai = ais.get_mut(i).unwrap();
                    let mut maybe_training_instances =
                        ai.borrow_mut().take_turn_clearing(&mut game, generate_data);

                    if let Some(player_partial_data) = player_partial_data.as_mut() {
                        let partial_data =
                            player_partial_data.entry(player).or_insert_with(Vec::new);
                        partial_data.append(maybe_training_instances.as_mut().unwrap());
                    }

                    //TODO write the instance somewhere specific to the player so we can annotate it with
                    //     victory/defeat/inconclusive after the game runs the specified episodes

                    if verbosity > 1 {
                        if fix_output_loc {
                            let ctrl = game.player_turn_control_nonending(i);

                            map.as_mut().unwrap().draw(&ctrl, &mut stdout, &palette);
                            execute!(stdout, MoveTo(0, map_height + 2)).unwrap();
                        } else {
                            println!("{:?}", game);
                        }
                        println!("Turn: {}", game.turn());
                    }
                }
            }

            if let Some(player_partial_data) = player_partial_data.as_mut() {
                // Mark the training instances (if we've been tracking them) with the game's outcome

                let players: Vec<usize> = player_partial_data.keys().cloned().collect();
                if let Some(victor) = game.victor() {
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

                // Now write the resolved instances
                for instances in player_partial_data.values() {
                    for instance in instances {
                        let data = bincode::serialize(instance).unwrap();
                        data_outfile.as_mut().unwrap().write_all(&data).unwrap();
                    }
                }
            }

            *victory_counts.entry(game.victor()).or_insert(0) += 1;

            println!();
            for i in 0..num_ais {
                let spec = ai_specs_s[i];
                println!(
                    "{} wins: {}",
                    spec,
                    victory_counts.get(&Some(i)).unwrap_or(&0)
                );
            }
            println!("Draws: {}", victory_counts.get(&None).unwrap_or(&0));
        }
    } else if subcommand == "train" {
        // let mut opponent_specs_s: Vec<&str> = sub_matches.values_of("opponent").unwrap().collect();

        // if opponent_specs_s.is_empty() {
        //     opponent_specs_s.push("random");
        // }

        // let opponent_specs: Vec<AISpec> = parse_ai_specs(&opponent_specs_s)?;

        let alpha = sub_matches
            .get_one::<f64>("qlearning_alpha")
            .unwrap()
            .clone();
        let gamma = sub_matches
            .get_one::<f64>("qlearning_gamma")
            .unwrap()
            .clone();
        let epsilon = sub_matches.get_one::<f64>("epsilon").unwrap().clone();
        let epsilon_decay = sub_matches.get_one::<f64>("epsilon_decay").unwrap().clone();
        let decay_prob = sub_matches
            .get_one::<f64>("epsilon_decay_prob")
            .unwrap()
            .clone();
        let min_epsilon = sub_matches.get_one::<f64>("min_epsilon").unwrap().clone();
        let dnn_learning_rate = sub_matches
            .get_one::<f32>("dnn_learning_rate")
            .unwrap()
            .clone();

        let avoid_skip = sub_matches.contains_id("avoid_skip");
        let deep = sub_matches.contains_id("deep");
        let initial_model_path = sub_matches.get_one::<String>("initial_model_path").cloned();
        let output_path = sub_matches.get_one::<&str>("out").unwrap().clone();

        let initialize_from_spec_s = initial_model_path.unwrap_or(String::from("random"));

        println!("Initialize From: {}", initialize_from_spec_s);

        // println!("Opponents: {}", opponent_specs_s.join(", "));

        println!("Output path: {}", output_path);
        println!(
            "alpha: {} gamma: {} epsilon: {} lr: {} avoid_skip: {} deep: {}",
            alpha, gamma, epsilon, dnn_learning_rate, avoid_skip, deep
        );

        // let initialize_from_spec_s = initial_model_path.unwrap_or("random");
        let initialize_from_spec =
            AISpec::try_from(initialize_from_spec_s).map_err(|err| format!("{}", err))?;

        let initialize_from: AI = initialize_from_spec.into();

        let qf = {
            // let domain_builder = Box::new(move || UmpireDomain::new_from_path(Dims::new(map_width, map_height), ai_model_path.as_ref(), verbose));

            let agent = trained_agent(
                initialize_from,
                deep,
                2,
                dims,
                wrappings,
                episodes,
                steps,
                alpha,
                gamma,
                epsilon,
                epsilon_decay,
                decay_prob,
                min_epsilon,
                dnn_learning_rate,
                avoid_skip,
                fix_output_loc,
                fog_of_war,
                verbosity,
                Some(Path::new("./memory.dat")),
                0.01,
            )?;

            agent.q.q_func.0
        };

        // Pry the q function loose
        let qfd = Rc::try_unwrap(qf)
            .map_err(|err| format!("Error unwrapping trained AI: {:?}", err))?
            .into_inner();

        let ai: AI = Rc::try_unwrap(qfd.0)
            .map_err(|err| format!("Error unwrapping trained AI: {:?}", err))?
            .into_inner();

        let path = Path::new(output_path);
        ai.store(path).map_err(|err| {
            format!(
                "Error storing model at path {}: {}",
                path.to_string_lossy(),
                err
            )
        })?;
    } else {
        return Err(String::from("A subcommand must be given"));
    }

    Ok(())
}
