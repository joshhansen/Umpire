

use std::{
    cell::RefCell,
    convert::{
        TryFrom,
        TryInto,
    },
    fmt,
    fs::File,
    io::Write,
    path::Path,
    rc::Rc,
};

use rand::{
    thread_rng,
    Rng,
};

use crate::{
    game::{
        player::TurnTaker,
    }, cli::Specified,
};

use rsrl::fa::{
    EnumerableStateActionFunction,
    StateActionFunction,
};

pub trait Loadable: Sized {
    fn load<P: AsRef<Path>>(path: P) ->  Result<Self,String>;
}

pub trait Storable {
    fn store(self, path: &Path) -> Result<(),String>;
}

// Sub-modules
pub mod dnn;
mod random;
pub mod rl;

use dnn::DNN;
use rl::LFA_;

/// A user specification of an AI
///
/// Used as a lightweight description of an AI to be passed around. Also to validate AIs given at the command line.
#[derive(Clone,Debug,Eq,Hash,PartialEq)]
pub enum AISpec {
    /// A horrible AI that makes decisions randomly
    Random,

    /// AI loaded from a path. If it's a file, deserialize the usual `rsrl` `LFA`-based model. If it's a directory,
    /// load it as a TensorFlow SavedModel.
    FromPath(String),

    /// AI loaded from a preset AI level, beginning at 1
    FromLevel(usize),
}

impl AISpec {
    // /// The text description of this type of AI
    // pub fn desc(&self) -> String {
    //     match self {
    //         Self::Random => String::from("random"),
    //         Self::FromPath(path) => format!("AI from path {}", path),
    //         Self::FromLevel(level) => format!("level {} AI", level),
    //     }
    // }

    // pub fn spec(&self) -> String {
    //     match self {
    //         Self::Random => "r".to_string(),
    //         Self::FromPath(path) => path.clone(),
    //         Self::FromLevel(level) => format!("{}", level),
    //     }
    // }

    // pub fn as_fa(self) -> Result<FunctionApproximator,String> {
    //     Ok(match self {
    //         Self::Random => FunctionApproximator::Random,
    //         Self::FromPath(path) => {
    //             let path = Path::new(path.as_str());
    //             if path.is_file() {
    //                 let f = File::open(path).unwrap();//NOTE unwrap on file open
    //                 let lfa: LFA_ = bincode::deserialize_from(f).unwrap();
    //                 FunctionApproximator::LFA(lfa)
    //             } else {
    //                 let dnn_fa: DNN = DNN::load(path).unwrap();
    //                 FunctionApproximator::DNN(dnn_fa)
    //             }
    //         },
    //         Self::FromLevel(level) => {
    //             let lfa: LFA_ = match level {
    //                 1 => bincode::deserialize(include_bytes!("../../ai/10x10_e100_s100000_a__scorefix__turnpenalty.ai")).unwrap(),
    //                 2 => bincode::deserialize(include_bytes!("../../ai/20x20_e100_s100000_a__scorefix__turnpenalty.ai")).unwrap(),
    //                 3 => bincode::deserialize(include_bytes!("../../ai/10-30_e100_s100000_a__scorefix__turnpenalty.ai")).unwrap(),
    //                 4 => bincode::deserialize(include_bytes!("../../ai/10-40+full_e100_s100000_a.ai")).unwrap(),
    //                 level => unreachable!("Unsupported AI level: {}", level)
    //             };
    //             FunctionApproximator::LFA(lfa)
    //         }
    //     })
    // }

    // pub fn as_rl_ai(self) -> Result<RL_AI,String> {
    //     let fa = self.as_fa()?;
    //     Ok(RL_AI::new(fa, true))
    // }
}

impl fmt::Display for AISpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.desc().fmt(f)
    }
}

impl TryFrom<String> for AISpec {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {

        match value.as_str() {
            "r" | "rand" | "random" => Ok(Self::Random),
            "1"|"2"|"3"|"4"|"5"|"6"|"7"|"8"|"9" => Ok(Self::FromLevel(value.chars().next().unwrap().to_digit(10).unwrap() as usize)),
            s => {
                if Path::new(s).exists() {
                    Ok(Self::FromPath(value))
                } else {
                    Err(format!("Unrecognized AI specification '{}'", s))
                }
            }
        }

        // if value.len() == 1 {
        //     let c: char = value.chars().next().unwrap();
        //     return TryFrom::try_from(c);
        // }


        // if value == "rand" || value == "random" {
        //     return Ok(Self::Random);
        // }



        // if Path::new(&value).exists() {
        //     Ok(Self::FromPath(value))
        // } else {
        //     Err(format!("AI can't be loaded from path {} because it doesn't exist", value))
        // }
    }
}

/// An item specified by a string on the command line
impl Specified for AISpec {

    /// A description to show up in the command line help
    fn desc(&self) -> String {
        match self {
            Self::Random => String::from("random"),
            Self::FromPath(path) => format!("AI from path {}", path),
            Self::FromLevel(level) => format!("level {} AI", level),
        }
    }

    /// A canonicalized string representation of the item
    fn spec(&self) -> String {
        match self {
            Self::Random => String::from("r"),
            Self::FromPath(path) => path.clone(),
            Self::FromLevel(level) => format!("{}", level),
        }
    }
}

impl TryFrom<Option<&String>> for AISpec {
    type Error = String;

    fn try_from(value: Option<&String>) -> Result<Self, Self::Error> {
        if let Some(value) = value {
            AISpec::try_from(value.clone())
        } else {
            Ok(Self::Random)
        }
    }
}

impl Into<PlayerType> for AISpec {
    fn into(self) -> PlayerType {
        PlayerType::AI(self)
    }
}

// impl Into<Rc<RefCell<dyn TurnTaker>>> for AIType {
//     fn into(self) -> Rc<RefCell<dyn TurnTaker>> {
//         // if let Self::Random = &self {
//         //     return Rc::new(RefCell::new( RandomAI::new(0) ));
//         // }

//         // //Unwrap should be OK since the conversion only fails on AIType::Random which we've already handled
//         // let fa: Rc<RefCell<dyn EnumerableStateActionFunction<Game>>> = self.try_into().unwrap();
//         // let refcell = Rc::try_unwrap(fa).unwrap().into_inner();

//         // Rc::new(RefCell::new())


//         match self {
//             Self::Random => Rc::new(RefCell::new(RandomAI::new(0))),
//             Self::FromPath(path) => {
//                 let path = Path::new(path.as_str());
//                 if path.is_file() {
//                     let f = File::open(path).unwrap();//NOTE unwrap on file open
//                     let lfa: LFA_ = bincode::deserialize_from(f).unwrap();
//                     Rc::new(RefCell::new(RL_AI::new(lfa, true)))
//                 } else {
//                     let rl_dnn_ai: DNN = DNN::load(path).unwrap();
//                     Rc::new(RefCell::new(RL_AI::new(rl_dnn_ai, true)))

//                 }
//             },
//             Self::FromLevel(level) => {
//                 let lfa: LFA_ = match level {
//                     1 => bincode::deserialize(include_bytes!("../../ai/10x10_e100_s100000_a__scorefix__turnpenalty.ai")).unwrap(),
//                     2 => bincode::deserialize(include_bytes!("../../ai/20x20_e100_s100000_a__scorefix__turnpenalty.ai")).unwrap(),
//                     3 => bincode::deserialize(include_bytes!("../../ai/10-30_e100_s100000_a__scorefix__turnpenalty.ai")).unwrap(),
//                     4 => bincode::deserialize(include_bytes!("../../ai/10-40+full_e100_s100000_a.ai")).unwrap(),
//                     level => unreachable!("Unsupported AI level: {}", level)
//                 };
//                 Rc::new(RefCell::new(RL_AI::new(lfa, true)))
//             }
//         }
//     }
// }

// impl TryInto<Rc<RefCell<dyn EnumerableStateActionFunction<Game>>>> for AIType {
//     type Error = String;

//     fn try_into(self) -> Result<Rc<RefCell<dyn EnumerableStateActionFunction<Game>>>,String> {
//         Ok(match self {
//             Self::Random => return Err(String::from("Cannot get state action function for random AI")),
//             Self::FromPath(path) => {
//                 let path = Path::new(path.as_str());
//                 if path.is_file() {
//                     let f = File::open(path).unwrap();//NOTE unwrap on file open
//                     let lfa: LFA_ = bincode::deserialize_from(f).unwrap();
//                     Rc::new(RefCell::new(lfa))
//                 } else {
//                     let dnn_fa: DNN = DNN::load(path).unwrap();
//                     Rc::new(RefCell::new(dnn_fa))
//                 }
//             },
//             Self::FromLevel(level) => {
//                 let lfa: LFA_ = match level {
//                     1 => bincode::deserialize(include_bytes!("../../ai/10x10_e100_s100000_a__scorefix__turnpenalty.ai")).unwrap(),
//                     2 => bincode::deserialize(include_bytes!("../../ai/20x20_e100_s100000_a__scorefix__turnpenalty.ai")).unwrap(),
//                     3 => bincode::deserialize(include_bytes!("../../ai/10-30_e100_s100000_a__scorefix__turnpenalty.ai")).unwrap(),
//                     4 => bincode::deserialize(include_bytes!("../../ai/10-40+full_e100_s100000_a.ai")).unwrap(),
//                     level => unreachable!("Unsupported AI level: {}", level)
//                 };
//                 Rc::new(RefCell::new(lfa))
//             }
//         })
//     }
// }

impl Into<String> for AISpec {
    fn into(self) -> String {
        String::from(self.spec())
        // match self {
        //     Self::Random => "r".to_string(),
        //     Self::FromPath(path) => path,
        //     Self::FromLevel(level) => format!("{}", level),
        // }
    }
}


pub enum AI {
    Random(RandomAI),
    LFA(LFA_),
    DNN(DNN)
}

impl AI {
    pub fn random(verbosity: usize, fix_output_loc: bool) -> Self {
        Self::Random(RandomAI::new(verbosity, fix_output_loc))
    }
}

impl fmt::Debug for AI {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", match self {
            Self::Random(_) => "random",
            Self::LFA(_) => "lfa",
            Self::DNN(_) => "dnn",
        })
    }
}

impl StateActionFunction<Game, usize> for AI {
    type Output = f64;

    fn evaluate(&self, state: &Game, action: &usize) -> Self::Output {
        match self {
            Self::Random(_) => {
                let mut rng = thread_rng();
                rng.gen()
            },
            Self::LFA(fa) => {
                fa.evaluate(state, action)
            },
            Self::DNN(fa) => {
                fa.evaluate(state, action)
            }
        }
    }

    fn update_with_error(&mut self, state: &Game, action: &usize, value: Self::Output, estimate: Self::Output,
            error: Self::Output, raw_error: Self::Output, learning_rate: f64) {

        match self {
            Self::Random(_) => { /* do nothing */ },
            Self::LFA(fa) => fa.update_with_error(state, action, value, estimate, error, raw_error, learning_rate),
            Self::DNN(fa) => fa.update_with_error(state, action, value, estimate, error, raw_error, learning_rate)
        }
    }
}

impl EnumerableStateActionFunction<Game> for AI {
    fn n_actions(&self) -> usize {
        UmpireAction::possible_actions().len()
    }

    fn evaluate_all(&self, state: &Game) -> Vec<f64> {
        (0..self.n_actions()).map(|action| self.evaluate(state, &action))
        .collect()
    }

    fn update_all_with_errors(&mut self, state: &Game, values: Vec<f64>, estimates: Vec<f64>, errors: Vec<f64>,
            raw_errors: Vec<f64>, learning_rate: f64) {

        for (i, value) in values.iter().enumerate() {
            self.update_with_error(state, &i, *value, estimates[i], errors[i], raw_errors[i], learning_rate);
        }
    }
}

impl From<AISpec> for AI {
    fn from(ai_type: AISpec) -> Self {
        match ai_type {
            AISpec::Random => Self::Random(RandomAI::new(0, false)),//NOTE Assuming 0 verbosity
            AISpec::FromPath(path) => Self::load(Path::new(path.as_str())).unwrap(),
            AISpec::FromLevel(level) => {
                let lfa: LFA_ = match level {
                    1 => bincode::deserialize(include_bytes!("../../ai/10x10_e100_s100000_a__scorefix__turnpenalty.ai")).unwrap(),
                    2 => bincode::deserialize(include_bytes!("../../ai/20x20_e100_s100000_a__scorefix__turnpenalty.ai")).unwrap(),
                    3 => bincode::deserialize(include_bytes!("../../ai/10-30_e100_s100000_a__scorefix__turnpenalty.ai")).unwrap(),
                    4 => bincode::deserialize(include_bytes!("../../ai/10-40+full_e100_s100000_a.ai")).unwrap(),
                    level => unreachable!("Unsupported AI level: {}", level)
                };
                Self::LFA(lfa)
            },
        }
    }
}

impl Loadable for AI {
    fn load<P: AsRef<Path>>(path: P) ->  Result<Self,String> {
        if !path.as_ref().exists() {
            return Err(format!("Could not load AI from path '{:?}' because it doesn't exist", path.as_ref()));
        }

        if path.as_ref().extension().map(|ext| ext.to_str()) == Some(Some("deep")) {
            DNN::load(path).map(Self::DNN)
        } else {
            let f = File::open(path).unwrap();//NOTE unwrap on file open
            let result: Result<LFA_,String> = bincode::deserialize_from(f)
                                                   .map_err(|err| format!("{}", err));
            result.map(Self::LFA)
        }
    }
}

impl Storable for AI {
    fn store(self, path: &Path) -> Result<(),String> {
        match self {
            Self::Random(_) => Err(String::from("Cannot store random AI; load explicitly using the appropriate specification (r/rand/random)")),
            Self::LFA(fa) => {
                let data = bincode::serialize(&fa).unwrap();

                let display = path.display();
            
                let mut file = File::create(&path).map_err(|err| {
                    format!("couldn't create {}: {}", display, err)
                })?;

                file.write_all(&data).map_err(|err| format!("Couldn't write to {}: {}", display, err))
            },
            Self::DNN(fa) => fa.store(path)
        }
    }
}

impl AI {
    fn best_action(&self, game: &Game) -> Result<usize,String> {
        match self {
            Self::Random(_ai) => {
                Err(String::from("Call RandomAI::take_turn etc. directly"))
            },
            Self::LFA(fa) => {
                Ok(find_legal_max(fa, game, true).0)
            },
            Self::DNN(fa) => {
                let action = find_legal_max(fa, game, false).0;
                // println!("ACTION: {:?}", UmpireAction::from_idx(action));
                Ok(action)
            },
        }
    }

    fn _take_turn_unended(&mut self, game: &mut Game) {
        while !game.turn_is_done() {
            let action_idx = self.best_action(game).unwrap();
            let action = UmpireAction::from_idx(action_idx).unwrap();
            action.take(game);
        }
    }
}

impl TurnTaker for AI {
    fn take_turn_not_clearing(&mut self, game: &mut Game) {
        match self {
            Self::Random(ai) => {
                ai.take_turn_not_clearing(game);
            },
            Self::LFA(_fa) => {
                self._take_turn_unended(game);

                game.end_turn().unwrap();
            },
            Self::DNN(_fa) => {
                self._take_turn_unended(game);

                game.end_turn().unwrap();
            }
        }
    }

    fn take_turn_clearing(&mut self, game: &mut Game) {
        match self {
            Self::Random(ai) => {
                ai.take_turn_clearing(game);
            },
            Self::LFA(_fa) => {
                self._take_turn_unended(game);

                game.end_turn_clearing().unwrap();
            },
            Self::DNN(_fa) => {
                self._take_turn_unended(game);

                game.end_turn_clearing().unwrap();
            }
        }
    }
}

// impl TryFrom<char> for AIType {
//     type Error = String;

//     fn try_from(value: char) -> Result<Self, Self::Error> {
//         match value {
//             'r' => Ok(Self::Random),
//             '1'|'2'|'3'|'4'|'5'|'6'|'7'|'8'|'9' => Ok(Self::FromLevel(value.to_digit(10).unwrap() as usize)),
//             c => Err(format!("Unrecognized AI specification '{}'", c))
//         }
//     }
// }

// impl TryInto<char> for AIType {
//     type Error = String;

//     fn try_into(self) -> Result<char,Self::Error> {
//         match self {
//             Self::Random => Ok('r'),
//             Self::FromPath => Err("")
//         }
//     }
// }




// Exports
pub use random::RandomAI;

pub use rl::{
    UmpireAction, find_legal_max,
};
use super::{Game, PlayerType};
