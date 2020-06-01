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
    collections::{
        HashMap,
    },
    convert::TryFrom,
    io::{
        stdout,
        Write,
    },
    rc::Rc,
    str::FromStr,
    path::Path,
};

use clap::{AppSettings, Arg, SubCommand};

use crossterm::{
    execute,
    cursor::MoveTo,
    terminal::{
        Clear,
        ClearType,
    },
};

use umpire::{
    cli::{
        self,
        parse_spec,
    },
    conf,
    game::{
        Game,
        ai::{
            AI,
            AISpec,
            Storable,
            rl::{
                trained_agent,

            },
        },
        player::{
            PlayerNum,
            TurnTaker,
        },
    },
    name::IntNamer,
    util::{
        Dims,
        Wrap2d,
    },
};

fn f32_validator(s: String) -> Result<(),String> {
    f64::from_str(s.as_str())
        .map(|_| ())
        .map_err(|err| format!("{}", err))
}

fn f64_validator(s: String) -> Result<(),String> {
    f64::from_str(s.as_str())
        .map(|_| ())
        .map_err(|err| format!("{}", err))
}

fn parse_ai_spec<S:AsRef<str>>(spec: S) -> Result<Vec<AISpec>,String> {
    parse_spec(spec, "AI")
}

fn parse_ai_specs(specs: &Vec<&str>) -> Result<Vec<AISpec>,String> {
    let mut ai_specs: Vec<AISpec> = Vec::new();
    for ai_spec_s in specs {
        let sub_specs = parse_ai_spec(ai_spec_s)?;
        ai_specs.extend(sub_specs);
    }
    Ok(ai_specs)
}

fn load_ais(ai_types: &Vec<AISpec>) -> Result<Vec<Rc<RefCell<AI>>>,String> {
    let mut ais: Vec<Rc<RefCell<AI>>> = Vec::with_capacity(ai_types.len());
    for ai_type in ai_types {
        let ai: AI = ai_type.clone().into();
        let ai = Rc::new(RefCell::new(ai));
        ais.push(ai);
    }
    Ok(ais)

}

static AI_MODEL_SPECS_HELP: &'static str = "AI model specifications, comma-separated. The models to be evaluated. 'r' or 'random' for the purely random AI, or a serialized AI model file path, or directory path for TensorFlow SavedModel format";

fn main() -> Result<(),String> {
    let matches = cli::app("Umpire AI Trainer", "fvwHW")
    .version(conf::APP_VERSION)
    .author("Josh Hansen <hansen.joshuaa@gmail.com>")
    .setting(AppSettings::SubcommandRequiredElseHelp)
    .arg(
        Arg::with_name("episodes")
        .short("e")
        .long("episodes")
        .takes_value(true)
        .default_value("100")
        .validator(|s| {
            let episodes: Result<usize,_> = s.trim().parse();
            episodes.map(|_n| ()).map_err(|_e| format!("Invalid episodes '{}'", s))
        })
    )
    
    .arg(
        Arg::with_name("steps")
        .short("s")
        .long("steps")
        .takes_value(true)
        .default_value("100000")
        .help("The number of steps to execute in each episode")
        .validator(|s| {
            let steps: Result<u64,_> = s.trim().parse();
            steps.map(|_n| ()).map_err(|_e| format!("Invalid steps '{}'", s))
        })
    )

    .arg(
        Arg::with_name("fix_output_loc")
        .short("F")
        .long("fix")
        .help("Fix the location of output. Makes the output seem animated.")
    )

    .subcommand(
        SubCommand::with_name("eval")
        .about(format!("Have a set of AIs duke it out to see who plays the game of {} best", conf::APP_NAME).as_str())
        .arg(
            Arg::with_name("ai_models")
                .help(AI_MODEL_SPECS_HELP)
                .multiple(true)
                .required(true)
        )
    )

    .subcommand(
        cli::app("train", "")
        .about(format!("Train an AI for the game of {}", conf::APP_NAME).as_str())
        .setting(AppSettings::ArgRequiredElseHelp)
        .arg(
            Arg::with_name("avoid_skip")
            .short("a")
            .long("avoid_skip")
            .help("Execute policies in a way that avoids the SkipNextUnit action when possible")
            .takes_value(false)
        )
        .arg(
            Arg::with_name("qlearning_alpha")
            .short("-A")
            .long("alpha")
            .help("The alpha parameter (learning rate) for the Q-Learning algorithm")
            .takes_value(true)
            .validator(f64_validator)
            .default_value("0.01")
        )
        .arg(
            Arg::with_name("qlearning_gamma")
            .short("-G")
            .long("gamma")
            .help("The gamma parameter (discount rate) for the Q-Learning algorithm")
            .takes_value(true)
            .validator(f64_validator)
            .default_value("0.9")
        )
        .arg(
            Arg::with_name("epsilon")
            .short("-E")
            .long("epsilon")
            .help("The epsilon of the epsilon-greedy training policy. The probability of taking a random action rather the policy action")
            .takes_value(true)
            .validator(f64_validator)
            .default_value("0.05")
        )
        .arg(
            Arg::with_name("dnn_learning_rate")
            .short("-D")
            .long("dnnlr")
            .help("The learning rate of the neural network (if any)")
            .takes_value(true)
            .validator(f32_validator)
            .default_value("10e-3")
        )
        .arg(
            Arg::with_name("deep")
            .short("d")
            .long("deep")
            .help("Indicates that a deep neural network should be trained rather than a linear function approximator")
            .takes_value(false)
        )
        .arg(
            Arg::with_name("initial_model_path")
                .short("i")
                .long("initial")
                .help("Serialized AI model file path for the initial model to use as a starting point for training")
                .takes_value(true)
                .validator(|s| {
                    if Path::new(&s).exists() {
                        Ok(())
                    } else {
                        Err(format!("Initial model path '{}' does not exist", s))
                    }
                })
        )
        .arg(
            Arg::with_name("out")
            .help("Output path to serialize the resultin AI model to")
            .multiple(false)
            .required(true)
        )
        .arg(
            Arg::with_name("opponent")
                .help(AI_MODEL_SPECS_HELP)
                .multiple(true)
                .required(true)
        )
        
    )

    .get_matches();

    // Arguments common across subcommands:
    let episodes: usize = matches.value_of("episodes").unwrap().parse().unwrap();
    let fix_output_loc: bool = matches.is_present("fix_output_loc");
    let fog_of_war = matches.value_of("fog").unwrap() == "on";

    let map_heights: Vec<u16> = matches.values_of("map_height").unwrap().map(|h| h.parse().unwrap()).collect();
    let map_widths: Vec<u16> = matches.values_of("map_width").unwrap().map(|w| w.parse().unwrap()).collect();

    if map_heights.len() != map_widths.len() {
        return Err(String::from("The same number of widths and heights must be specified"));
    }
    
    let dims: Vec<Dims> = map_heights.into_iter().enumerate().map(|(i,h)| Dims::new(map_widths[i], h)).collect();

    let steps: u64 = matches.value_of("steps").unwrap().parse().unwrap();
    let verbosity = matches.occurrences_of("verbose") as usize;
    let wrapping = Wrap2d::try_from(matches.value_of("wrapping").unwrap().as_ref()).unwrap();

    let (subcommand, sub_matches) = matches.subcommand();

    match subcommand {
        "eval" => println!("Evaluating {} AIs", conf::APP_NAME),
        "train" => println!("Training {} AI", conf::APP_NAME),
        c => unreachable!("Unrecognized subcommand {} should have been caught by the agument parser; there's a bug somehere", c)
    }

    let sub_matches = sub_matches.unwrap();

    let mut stdout = stdout();

    if fix_output_loc {
        execute!(stdout, Clear(ClearType::All)).unwrap();
    }

    println!("Dimensions: {}", dims.iter().map(|dims| format!("{}", dims)).collect::<Vec<String>>().join(", "));

    println!("Episodes: {}", episodes);

    println!("Steps: {}", steps);

    println!("Verbosity: {}", verbosity);

    if subcommand == "eval" {

        if dims.len() > 1 {
            return Err(String::from("Only one set of dimensions can be given for evaluation"));
        }

        let map_width = dims[0].width;
        let map_height = dims[0].height;

        let ai_specs_s: Vec<&str> = sub_matches.values_of("ai_models").unwrap().collect();
        let ai_specs: Vec<AISpec> = parse_ai_specs(&ai_specs_s)?;
        let mut ais: Vec<Rc<RefCell<AI>>> = load_ais(&ai_specs)?;

        let num_ais = ais.len();

        for i in 0..num_ais {
            // let spec1 = (*ais[i]).borrow().spec();
            let spec1 = ai_specs[i].clone();
            let ai1 = Rc::clone(ais.get_mut(i).unwrap());

            for j in 0..num_ais {
                if i < j {
                    // let spec2 = ai_specs[j];
                    let spec2 = ai_specs[j].clone();
                    let ai2 = ais.get_mut(j).unwrap();

                    if fix_output_loc {
                        execute!(stdout, MoveTo(0,0)).unwrap();
                    }

                    println!("{} vs. {}", spec1, spec2);

                    let mut victory_counts: HashMap<Option<PlayerNum>,usize> = HashMap::new();

                    for _ in 0..episodes {

                        let city_namer = IntNamer::new("city");

                        let mut game = Game::new(
                            Dims::new(map_width, map_height),
                            city_namer,
                            2,
                            fog_of_war,
                            None,
                            wrapping,
                        );

                        if verbosity > 1 {
                            if fix_output_loc {
                                execute!(stdout, MoveTo(0,1)).unwrap();
                            }
                            println!("{:?}", game);
                        }

                        for _ in 0..steps {
                            // if verbosity > 1 {
                            //     if fix_output_loc {
                            //         execute!(stdout, MoveTo(0,1)).unwrap();
                            //     }
                            //     println!("{:?}", game);
                            // }

                            if game.victor().is_some() {
                                break;
                            }

                            ai1.borrow_mut().take_turn_clearing(&mut game);

                            if verbosity > 1 {
                                if fix_output_loc {
                                    execute!(stdout, MoveTo(0,1)).unwrap();
                                }
                                println!("{:?}", game);
                            }

                            if game.victor().is_some() {
                                break;
                            }

                            ai2.borrow_mut().take_turn_clearing(&mut game);
                        }

                        * victory_counts.entry(game.victor()).or_insert(0) += 1;

                        if verbosity > 0 {
                            if let Some(victor) = game.victor() {
                                println!("Victory: {}", match victor {
                                    0 => &spec1,
                                    1 => &spec2,
                                    v => panic!("Unrecognized victor {}", v)
                                });
                            } else {
                                println!("Draw");
                            }
                        

                            if verbosity > 1 {
                                let scores = game.player_scores();
                                println!("{} score: {}", spec1, scores.get(0).unwrap());
                                println!("{} score: {}", spec2, scores.get(1).unwrap());
                                println!("Turn: {}", game.turn());
                                println!();
                            }
                        }

                    }

                    println!("{} wins: {}", spec1, victory_counts.get(&Some(0)).unwrap_or(&0));
                    println!("{} wins: {}", spec2, victory_counts.get(&Some(1)).unwrap_or(&0));
                    println!("Draws: {}", victory_counts.get(&None).unwrap_or(&0));

                }
            }
        }


    } else if subcommand == "train" {

        let mut opponent_specs_s: Vec<&str> = sub_matches.values_of("opponent").unwrap().collect();

        if opponent_specs_s.is_empty() {
            opponent_specs_s.push("random");
        }

        let opponent_specs: Vec<AISpec> = parse_ai_specs(&opponent_specs_s)?;

        let alpha: f64 = f64::from_str( sub_matches.value_of("qlearning_alpha").unwrap() ).unwrap();
        let gamma: f64 = f64::from_str( sub_matches.value_of("qlearning_gamma").unwrap() ).unwrap();
        let epsilon: f64 = f64::from_str( sub_matches.value_of("epsilon").unwrap() ).unwrap();
        let dnn_learning_rate: f32 = f32::from_str( sub_matches.value_of("dnn_learning_rate").unwrap() ).unwrap();

        let avoid_skip = sub_matches.is_present("avoid_skip");
        let deep = sub_matches.is_present("deep");
        let initial_model_path = sub_matches.value_of("initial_model_path").map(String::from);
        let output_path = sub_matches.value_of("out").unwrap();
    
        let initialize_from_spec_s = initial_model_path.unwrap_or(String::from("random"));

        println!("Initialize From: {}", initialize_from_spec_s);
        
        println!("Opponents: {}", opponent_specs_s.join(", "));
    
        println!("Output path: {}", output_path);

        // let initialize_from_spec_s = initial_model_path.unwrap_or("random");
        let initialize_from_spec = AISpec::try_from(initialize_from_spec_s)
            .map_err(|err| format!("{}", err))?;

        let initialize_from: AI = initialize_from_spec.into();

        let qf = {
            // let domain_builder = Box::new(move || UmpireDomain::new_from_path(Dims::new(map_width, map_height), ai_model_path.as_ref(), verbose));
    
            let agent = trained_agent(initialize_from, deep, opponent_specs, dims, episodes, steps, alpha, gamma, epsilon, dnn_learning_rate, avoid_skip, fix_output_loc, fog_of_war, verbosity)?;
    
            agent.q.q_func.0
        };
    
        // Pry the q function loose
        let qfd = Rc::try_unwrap(qf)
            .map_err(|err| format!("Error unwrapping trained AI: {:?}", err))
        ?.into_inner();

        let ai: AI = Rc::try_unwrap(qfd.0)
            .map_err(|err| format!("Error unwrapping trained AI: {:?}", err))
        ?.into_inner();

        let path = Path::new(output_path);
        ai.store(path)
          .map_err(|err| format!("Error storing model at path {}: {}", path.to_string_lossy(), err))?;

    } else {
        println!("{}", matches.usage());
    }

    Ok(())
}