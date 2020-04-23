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
    fs::File,
    io::Write,
    rc::Rc,
    path::Path,
};

use clap::{AppSettings, Arg, SubCommand};

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
    conf,
    game::{
        Game,
        ai::{
            RandomAI,
            rl::{
                Basis,
                trained_agent,

            }, RL_AI,
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

fn main() {
    let matches = cli::app("Umpire AI Trainer", "fvwHW")
    .version(conf::APP_VERSION)
    .author("Josh Hansen <hansen.joshuaa@gmail.com>")
    .setting(AppSettings::SubcommandRequiredElseHelp)
    .arg(
        Arg::with_name("episodes")
        .short("e")
        .long("episodes")
        .takes_value(true)
        .default_value("1000")
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
        .default_value("5000")
        .help("The number of steps to execute in each episode")
        .validator(|s| {
            let steps: Result<u64,_> = s.trim().parse();
            steps.map(|_n| ()).map_err(|_e| format!("Invalid steps '{}'", s))
        })
    )

    .subcommand(
        SubCommand::with_name("eval")
        .about(format!("Have a set of AIs duke it out to see who plays the game of {} best", conf::APP_NAME).as_str())
        .arg(
            Arg::with_name("ai_models")
                // .short("M")
                // .long("models")
                .help("AI model specifications, comma-separated. The models to be evaluated. 'r' or 'random' for the purely random AI, or a serialized AI model file path")
                // .takes_value(true)
                .number_of_values(1)
                .multiple(true)
        )
    )

    .subcommand(
        cli::app("train", "m")
        .about(format!("Train an AI for the game of {}", conf::APP_NAME).as_str())
        .setting(AppSettings::ArgRequiredElseHelp)
        .arg(
            Arg::with_name("avoid_skip")
            .short("a")
            .long("avoid_skip")
            .help("Execute policies in a way that avoids the SkipNextUnit action when possible")
        )
        // .arg(
        //     Arg::with_name("initial_model_path")
        //         .short("i")
        //         .long("initial")
        //         .help("Serialized AI model file path for the initial model to use as a starting point for training")
        //         .takes_value(true)
        // )
        .arg(
            Arg::with_name("out")
            .help("Output path to serialize the resultin AI model to")
            .required(true)
        )
    )

    .get_matches();

    
    // Arguments common across subcommands:
    let episodes: usize = matches.value_of("episodes").unwrap().parse().unwrap();
    let fog_of_war = matches.value_of("fog").unwrap() == "on";
    // let map_height: u16 = matches.value_of("map_height").unwrap().parse().unwrap();
    // let map_width: u16 = matches.value_of("map_width").unwrap().parse().unwrap();

    let map_heights: Vec<u16> = matches.values_of("map_height").unwrap().map(|h| h.parse().unwrap()).collect();
    let map_widths: Vec<u16> = matches.values_of("map_width").unwrap().map(|w| w.parse().unwrap()).collect();

    if map_heights.len() != map_widths.len() {
        eprintln!("The same number of widths and heights must be specified");
        return;
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

    println!("Dimensions: {}", dims.iter().map(|dims| format!("{}", dims)).collect::<Vec<String>>().join(", "));

    println!("Episodes: {}", episodes);

    println!("Steps: {}", steps);

    println!("Verbosity: {}", verbosity);

    if subcommand == "eval" {

        if dims.len() > 1 {
            eprintln!("Only one set of dimensions can be given for evaluation");
            return;
        }

        let map_width = dims[0].width;
        let map_height = dims[0].height;

        let ai_specs: Vec<&str> = sub_matches.values_of("ai_models").unwrap().collect();

        let ai_results: Vec<Result<Rc<RefCell<dyn TurnTaker>>,String>> = ai_specs.iter().map(|ai_model| {
            let b: Rc<RefCell<dyn TurnTaker>> =
            if *ai_model == "r" || *ai_model == "random" {

                Rc::new(RefCell::new(RandomAI::new(verbosity)))

            } else {

                let f = File::open(Path::new(ai_model))
                        .map_err(|e| format!("Could not open model file {}: {}", ai_model, e))?;

                let ai: RL_AI<LFA<Basis,SGD,VectorFunction>> = bincode::deserialize_from(f)
                        .map_err(|e| format!("Could not deserialize model file {}: {}", ai_model, e.as_ref()))?;

                Rc::new(RefCell::new(ai))
            };
            Ok(b)
        }).collect();

        let mut ais: Vec<Rc<RefCell<dyn TurnTaker>>> = Vec::with_capacity(ai_results.len());

        for ai_result in ai_results {
            match ai_result {
                Err(e) => {
                    eprintln!("{}", e);
                    return;
                },
                Ok(ai) => ais.push(ai)
            }
        }

        let num_ais = ais.len();

        for i in 0..num_ais {
            let spec1 = ai_specs[i];
            let ai1 = Rc::clone(ais.get_mut(i).unwrap());

            for j in 0..num_ais {
                if i < j {
                    let spec2 = ai_specs[j];
                    let ai2 = ais.get_mut(j).unwrap();
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

                        for _ in 0..steps {
                            if verbosity > 1 {
                                println!("{:?}", game);
                            }

                            if game.victor().is_some() {
                                break;
                            }

                            ai1.borrow_mut().take_turn_clearing(&mut game);

                            if verbosity > 1 {
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
                                    0 => spec1,
                                    1 => spec2,
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

        let ai_model_path = sub_matches.value_of("ai_model").map(String::from);
        let avoid_skip = sub_matches.is_present("avoid_skip");
        // let initial_model_path = sub_matches.value_of("initial_model_path").map(String::from);
        let output_path = sub_matches.value_of("out").unwrap();
    
        // println!("Initial AI: {}", initial_model_path.as_ref().unwrap_or(&String::from("none")));
        
        println!("Opponent: {}", ai_model_path.as_ref().unwrap_or(&String::from("Random")));
    
        println!("Output path: {}", output_path);
    
        let qf = {
            // let domain_builder = Box::new(move || UmpireDomain::new_from_path(Dims::new(map_width, map_height), ai_model_path.as_ref(), verbose));
    
            let agent = trained_agent(ai_model_path, dims, episodes, steps, avoid_skip, verbosity);
    
            agent.q.q_func.0
        };
    
        // Pry the q function loose
        let qfd = Rc::try_unwrap(qf).unwrap().into_inner();
        let qfdd = Rc::try_unwrap(qfd.0).unwrap().into_inner();
    
        let rl_ai = RL_AI::new(qfdd, avoid_skip);
    
        // let data = bincode::serialize(&rl_ai).unwrap();
    
        // let path = Path::new(output_path);
        // let display = path.display();
    
        // let mut file = match File::create(&path) {
        //     Err(why) => panic!("couldn't create {}: {}", display, why),
        //     Ok(file) => file,
        // };
    
        // match file.write_all(&data) {
        //     Err(why) => panic!("couldn't write to {}: {}", display, why),
        //     Ok(_) => println!("successfully wrote to {}", display),
        // }
    } else {
        println!("{}", matches.usage());
    }
}