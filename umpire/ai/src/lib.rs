use std::{fmt, path::Path};

use async_trait::async_trait;

use burn::{prelude::*, tensor::ElementComparison};

use burn_autodiff::Autodiff;
use burn_wgpu::{Wgpu, WgpuDevice};
use futures::lock::Mutex as MutexAsync;

use common::{
    game::{
        action::AiPlayerAction,
        ai::{AISpec, TrainingFocus, TrainingInstance},
        player::PlayerTurn,
        turn::TurnOutcome,
        turn_async::TurnTaker as TurnTakerAsync,
        Game,
    },
    util::sparsify,
};

pub type AiBackend = Wgpu;
pub type AiBackendTrain = Autodiff<AiBackend>;
pub const fn default_device() -> <AiBackend as Backend>::Device {
    WgpuDevice::BestAvailable
}

pub trait Loadable<B: Backend>: Sized {
    fn load<P: AsRef<Path>>(path: P, device: B::Device) -> Result<Self, String>;
}

pub trait Storable {
    fn store(self, path: &Path) -> Result<(), String>;
}

pub trait StorableAsBytes {
    fn store_as_bytes(self) -> Result<Vec<u8>, String>;
}

pub trait LoadableFromBytes: Sized {
    fn load_from_bytes<S: std::io::Read>(bytes: S) -> Result<Self, String>;
}

// Sub-modules
pub mod agz;
pub mod data;

mod random;

use agz::AgzActionModel;

pub enum AI<B: Backend> {
    Random(RandomAI),

    /// AlphaGo Zero style action model
    AGZ(MutexAsync<AgzActionModel<B>>),
}

impl<B: Backend> AI<B> {
    pub fn random(verbosity: usize, fix_output_loc: bool) -> Self {
        Self::Random(RandomAI::new(verbosity, fix_output_loc))
    }
}

impl<B: Backend> fmt::Debug for AI<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Random(_) => "random",
                Self::AGZ(_) => "agz",
            }
        )
    }
}

// impl StateActionFunction<Game, usize> for AI {
//     type Output = f64;

//     fn evaluate(&self, state: &Game, action: &usize) -> Self::Output {
//         match self {
//             Self::Random(_) => {
//                 let mut rng = thread_rng();
//                 rng.gen()
//             }
//             Self::LFA(fa) => fa.evaluate(state, action),
//             #[cfg(feature = "pytorch")]
//             Self::DNN(fa) => fa.lock().unwrap().evaluate(state, action),
//             #[cfg(feature = "pytorch")]
//             Self::AGZ(_agz) => {
//                 unimplemented!("We haven't implemented the RSRL traits for AgzActionModel yet")
//             }
//         }
//     }

//     fn update_with_error(
//         &mut self,
//         state: &Game,
//         action: &usize,
//         value: Self::Output,
//         estimate: Self::Output,
//         error: Self::Output,
//         raw_error: Self::Output,
//         learning_rate: f64,
//     ) {
//         match self {
//             Self::Random(_) => { /* do nothing */ }
//             Self::LFA(fa) => fa.update_with_error(
//                 state,
//                 action,
//                 value,
//                 estimate,
//                 error,
//                 raw_error,
//                 learning_rate,
//             ),
//             #[cfg(feature = "pytorch")]
//             Self::DNN(fa) => fa.lock().unwrap().update_with_error(
//                 state,
//                 action,
//                 value,
//                 estimate,
//                 error,
//                 raw_error,
//                 learning_rate,
//             ),
//             #[cfg(feature = "pytorch")]
//             Self::AGZ(_agz) => {
//                 unimplemented!("We haven't implemented the RSRL traits for AgzActionModel yet")
//             }
//         }
//     }
// }

// impl EnumerableStateActionFunction<Game> for AI {
//     fn n_actions(&self) -> usize {
//         AiPlayerAction::possible_actions().len()
//     }

//     fn evaluate_all(&self, state: &Game) -> Vec<f64> {
//         (0..self.n_actions())
//             .map(|action| self.evaluate(state, &action))
//             .collect()
//     }

//     fn update_all_with_errors(
//         &mut self,
//         state: &Game,
//         values: Vec<f64>,
//         estimates: Vec<f64>,
//         errors: Vec<f64>,
//         raw_errors: Vec<f64>,
//         learning_rate: f64,
//     ) {
//         for (i, value) in values.iter().enumerate() {
//             self.update_with_error(
//                 state,
//                 &i,
//                 *value,
//                 estimates[i],
//                 errors[i],
//                 raw_errors[i],
//                 learning_rate,
//             );
//         }
//     }
// }

impl<B: Backend> From<AISpec> for AI<B> {
    fn from(ai_type: AISpec) -> Self {
        match ai_type {
            AISpec::Random => Self::Random(RandomAI::new(0, false)), //NOTE Assuming 0 verbosity
            AISpec::FromPath(path) => {
                let device: B::Device = Default::default();
                Self::load(Path::new(path.as_str()), device).unwrap()
            }
            AISpec::FromLevel(level) => {
                // let lfa: LFA_ = match level {
                //     1 => bincode::deserialize(include_bytes!(
                //         "../../../ai/lfa/10x10_e100_s100000_a__scorefix__turnpenalty.ai"
                //     ))
                //     .unwrap(),
                //     2 => bincode::deserialize(include_bytes!(
                //         "../../../ai/lfa/20x20_e100_s100000_a__scorefix__turnpenalty.ai"
                //     ))
                //     .unwrap(),
                //     3 => bincode::deserialize(include_bytes!(
                //         "../../../ai/lfa/10-30_e100_s100000_a__scorefix__turnpenalty.ai"
                //     ))
                //     .unwrap(),
                //     4 => bincode::deserialize(include_bytes!(
                //         "../../../ai/lfa/10-40+full_e100_s100000_a.ai"
                //     ))
                //     .unwrap(),
                //     level => unreachable!("Unsupported AI level: {}", level),
                // };
                // Self::LFA(lfa)
                panic!()
            }
        }
    }
}

impl<B: Backend> Loadable<B> for AI<B> {
    /// Loads the actual AI instance from a file.
    ///
    /// With feature "pytorch" enabled, files ending with .agz will be deserialized as AlphaGo Zero
    /// style action models (`AI::AGZ`).
    ///
    /// With feature "pytorch" enabled, files ending with .deep will be deserialized as an `rsrl`
    /// Q-learning model with DNN action model (`AI::DNN`).
    ///
    /// Everything else will be loaded as an `rsrl` Q-learning model with a linear action model (`AI::LFA`).
    fn load<P: AsRef<Path>>(path: P, device: B::Device) -> Result<Self, String> {
        if !path.as_ref().exists() {
            return Err(format!(
                "Could not load AI from path '{:?}' because it doesn't exist",
                path.as_ref()
            ));
        }

        if path.as_ref().to_string_lossy().contains(".agz") {
            return AgzActionModel::load(path, device).map(|agz| Self::AGZ(MutexAsync::new(agz)));
        }

        panic!("Could not load AI from path {}", path.as_ref().display());

        // #[cfg(feature = "pytorch")]
        // if path.as_ref().extension().map(|ext| ext.to_str()) == Some(Some("deep")) {
        //     return DNN::load(path).map(|dnn| Self::DNN(Mutex::new(dnn)));
        // }

        // let f = File::open(path).unwrap(); //NOTE unwrap on file open
        // let result: Result<LFA_, String> =
        //     bincode::deserialize_from(f).map_err(|err| format!("{}", err));
        // result.map(Self::LFA)
    }
}

impl<B: Backend> Storable for AI<B> {
    fn store(self, path: &Path) -> Result<(), String> {
        match self {
            Self::Random(_) => Err(String::from("Cannot store random AI; load explicitly using the appropriate specification (r/rand/random)")),
            Self::AGZ(agz) => agz.into_inner().store(path),
        }
    }
}

impl<B: Backend> AI<B> {
    fn best_action(&self, game: &Game) -> Result<usize, String> {
        match self {
            Self::Random(_ai) => Err(String::from("Call RandomAI::take_turn etc. directly")),
            Self::AGZ(_agz) => Err(String::from("Call AgzActionModel::take_turn etc. directly")),
        }
    }

    async fn _take_turn_unended(
        &mut self,
        game: &mut PlayerTurn<'_>,
        datagen_prob: Option<f64>,
    ) -> TurnOutcome {
        let mut training_instances = if datagen_prob.is_some() {
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

            let (num_features, features, pre_score) = if datagen_prob.is_some() {
                let features = game
                    .player_features(TrainingFocus::UnitIfExistsElseCity)
                    .await;
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

            if let Some(datagen_prob) = datagen_prob {
                if rand::random::<f64>() <= datagen_prob {
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
        }

        TurnOutcome {
            training_instances,
            quit: false, //Robots don't quit!
        }
    }
}

#[async_trait]
impl<B: Backend> TurnTakerAsync for AI<B> {
    async fn take_turn(&mut self, turn: &mut PlayerTurn, datagen_prob: Option<f64>) -> TurnOutcome {
        match self {
            Self::Random(ai) => ai.take_turn(turn, datagen_prob).await,
            Self::AGZ(agz) => agz.lock().await.take_turn(turn, datagen_prob).await,
        }
    }
}

// Exports
pub use random::RandomAI;
