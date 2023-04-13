use std::{fmt, fs::File, io::Write, path::Path};

#[cfg(feature = "pytorch")]
use std::sync::Mutex;

#[cfg(feature = "pytorch")]
use std::ops::Deref;

use async_trait::async_trait;

use rand::{thread_rng, Rng};

use rsrl::fa::{EnumerableStateActionFunction, StateActionFunction};

use common::{
    game::{
        action::AiPlayerAction,
        ai::{AISpec, TrainingInstance},
        player::PlayerTurn,
        turn::TurnOutcome,
        turn_async::TurnTaker as TurnTakerAsync,
        Game,
    },
    util::sparsify,
};

pub trait Loadable: Sized {
    fn load<P: AsRef<Path>>(path: P) -> Result<Self, String>;
}

pub trait Storable {
    fn store(self, path: &Path) -> Result<(), String>;
}

pub trait StorableAsBytes {
    fn store_as_bytes(self) -> Result<Vec<u8>, String>;
}

pub trait LoadableFromBytes: Sized {
    fn load_from_bytes<S: std::io::Read + std::io::Seek>(bytes: S) -> Result<Self, String>;
}

// Sub-modules
#[cfg(feature = "pytorch")]
pub mod agz;

#[cfg(feature = "pytorch")]
pub mod dnn;
mod random;
pub mod rl;

#[cfg(feature = "pytorch")]
use dnn::DNN;

use rl::LFA_;

pub enum AI {
    Random(RandomAI),
    LFA(LFA_),
    #[cfg(feature = "pytorch")]
    DNN(Mutex<DNN>),
}

impl AI {
    pub fn random(verbosity: usize, fix_output_loc: bool) -> Self {
        Self::Random(RandomAI::new(verbosity, fix_output_loc))
    }
}

impl fmt::Debug for AI {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Random(_) => "random",
                Self::LFA(_) => "lfa",
                #[cfg(feature = "pytorch")]
                Self::DNN(_) => "dnn",
            }
        )
    }
}

impl StateActionFunction<Game, usize> for AI {
    type Output = f64;

    fn evaluate(&self, state: &Game, action: &usize) -> Self::Output {
        match self {
            Self::Random(_) => {
                let mut rng = thread_rng();
                rng.gen()
            }
            Self::LFA(fa) => fa.evaluate(state, action),
            #[cfg(feature = "pytorch")]
            Self::DNN(fa) => fa.lock().unwrap().evaluate(state, action),
        }
    }

    fn update_with_error(
        &mut self,
        state: &Game,
        action: &usize,
        value: Self::Output,
        estimate: Self::Output,
        error: Self::Output,
        raw_error: Self::Output,
        learning_rate: f64,
    ) {
        match self {
            Self::Random(_) => { /* do nothing */ }
            Self::LFA(fa) => fa.update_with_error(
                state,
                action,
                value,
                estimate,
                error,
                raw_error,
                learning_rate,
            ),
            #[cfg(feature = "pytorch")]
            Self::DNN(fa) => fa.lock().unwrap().update_with_error(
                state,
                action,
                value,
                estimate,
                error,
                raw_error,
                learning_rate,
            ),
        }
    }
}

impl EnumerableStateActionFunction<Game> for AI {
    fn n_actions(&self) -> usize {
        AiPlayerAction::possible_actions().len()
    }

    fn evaluate_all(&self, state: &Game) -> Vec<f64> {
        (0..self.n_actions())
            .map(|action| self.evaluate(state, &action))
            .collect()
    }

    fn update_all_with_errors(
        &mut self,
        state: &Game,
        values: Vec<f64>,
        estimates: Vec<f64>,
        errors: Vec<f64>,
        raw_errors: Vec<f64>,
        learning_rate: f64,
    ) {
        for (i, value) in values.iter().enumerate() {
            self.update_with_error(
                state,
                &i,
                *value,
                estimates[i],
                errors[i],
                raw_errors[i],
                learning_rate,
            );
        }
    }
}

impl From<AISpec> for AI {
    fn from(ai_type: AISpec) -> Self {
        match ai_type {
            AISpec::Random => Self::Random(RandomAI::new(0, false)), //NOTE Assuming 0 verbosity
            AISpec::FromPath(path) => Self::load(Path::new(path.as_str())).unwrap(),
            AISpec::FromLevel(level) => {
                let lfa: LFA_ = match level {
                    1 => bincode::deserialize(include_bytes!(
                        "../../../ai/10x10_e100_s100000_a__scorefix__turnpenalty.ai"
                    ))
                    .unwrap(),
                    2 => bincode::deserialize(include_bytes!(
                        "../../../ai/20x20_e100_s100000_a__scorefix__turnpenalty.ai"
                    ))
                    .unwrap(),
                    3 => bincode::deserialize(include_bytes!(
                        "../../../ai/10-30_e100_s100000_a__scorefix__turnpenalty.ai"
                    ))
                    .unwrap(),
                    4 => bincode::deserialize(include_bytes!(
                        "../../../ai/10-40+full_e100_s100000_a.ai"
                    ))
                    .unwrap(),
                    level => unreachable!("Unsupported AI level: {}", level),
                };
                Self::LFA(lfa)
            }
        }
    }
}

impl Loadable for AI {
    fn load<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        if !path.as_ref().exists() {
            return Err(format!(
                "Could not load AI from path '{:?}' because it doesn't exist",
                path.as_ref()
            ));
        }

        #[cfg(feature = "pytorch")]
        if path.as_ref().extension().map(|ext| ext.to_str()) == Some(Some("deep")) {
            return DNN::load(path).map(|dnn| Self::DNN(Mutex::new(dnn)));
        }

        let f = File::open(path).unwrap(); //NOTE unwrap on file open
        let result: Result<LFA_, String> =
            bincode::deserialize_from(f).map_err(|err| format!("{}", err));
        result.map(Self::LFA)
    }
}

impl Storable for AI {
    fn store(self, path: &Path) -> Result<(), String> {
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
            #[cfg(feature = "pytorch")]
            Self::DNN(fa) => fa.into_inner().unwrap().store(path)
        }
    }
}

impl AI {
    fn best_action(&self, game: &Game) -> Result<usize, String> {
        match self {
            Self::Random(_ai) => Err(String::from("Call RandomAI::take_turn etc. directly")),
            Self::LFA(fa) => Ok(find_legal_max(fa, game, true).0),
            #[cfg(feature = "pytorch")]
            Self::DNN(fa) => {
                let action = find_legal_max(fa.lock().unwrap().deref(), game, false).0;
                // println!("ACTION: {:?}", UmpireAction::from_idx(action));
                Ok(action)
            }
        }
    }

    async fn _take_turn_unended(
        &mut self,
        game: &mut PlayerTurn<'_>,
        generate_data: bool,
    ) -> TurnOutcome {
        let mut training_instances = if generate_data {
            Some(Vec::new())
        } else {
            None
        };

        let player = game.current_player().await;

        while !game.current_turn_is_done().await {
            // features: Vec<f64>,// the view on the game state
            // pre_score: f64,// the player's score prior to the action
            // action_idx: usize,// the action taken
            // post_score: f64,// the player's score after the action
            // outcome: TrainingOutcome,// how did things work out for the player?

            let (num_features, features, pre_score) = if generate_data {
                let features = game.player_features().await;
                let (num_features, features) = sparsify(features);

                (
                    Some(num_features),
                    Some(features),
                    Some(game.player_score().await.unwrap()),
                )
            } else {
                (None, None, None)
            };

            let action_idx = self
                .best_action(&game.clone_underlying_game_state().await.unwrap())
                .unwrap();
            let action = AiPlayerAction::from_idx(action_idx).unwrap();

            game.take_simple_action(action).await.unwrap();

            if generate_data {
                let post_score = game.player_score().await.unwrap();
                training_instances.as_mut().map(|v| {
                    v.push(TrainingInstance::undetermined(
                        player,
                        num_features.unwrap(),
                        features.unwrap(),
                        pre_score.unwrap(),
                        action,
                        post_score,
                    ));
                });
            }
        }

        TurnOutcome {
            training_instances,
            quit: false, //Robots don't quit!
        }
    }
}

#[async_trait]
impl TurnTakerAsync for AI {
    async fn take_turn(&mut self, turn: &mut PlayerTurn, generate_data: bool) -> TurnOutcome {
        match self {
            Self::Random(ai) => ai.take_turn(turn, generate_data).await,
            Self::LFA(_fa) => self._take_turn_unended(turn, generate_data).await,
            #[cfg(feature = "pytorch")]
            Self::DNN(_fa) => self._take_turn_unended(turn, generate_data).await,
        }
    }
}

// Exports
pub use random::RandomAI;
pub use rl::find_legal_max;
