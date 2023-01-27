use std::{fmt, fs::File, io::Write, path::Path};

use common::game::{
    ai::{fX, AISpec, TrainingInstance, UmpireAction},
    Game,
};
use rand::{thread_rng, Rng};

use common::{game::player::TurnTaker, util::sparsify};

use rsrl::{
    fa::{EnumerableStateActionFunction, StateActionFunction},
    DerefVec,
};

impl DerefVec for Game {
    fn deref_vec(&self) -> Vec<fX> {
        self.features()
    }
}

pub trait Loadable: Sized {
    fn load<P: AsRef<Path>>(path: P) -> Result<Self, String>;
}

pub trait Storable {
    fn store(self, path: &Path) -> Result<(), String>;
}

// Sub-modules
pub mod dnn;
mod random;
pub mod rl;

use dnn::DNN;
use rl::LFA_;

pub enum AI {
    Random(RandomAI),
    LFA(LFA_),
    DNN(DNN),
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
            Self::DNN(fa) => fa.evaluate(state, action),
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
            Self::DNN(fa) => fa.update_with_error(
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
        UmpireAction::possible_actions().len()
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

        if path.as_ref().extension().map(|ext| ext.to_str()) == Some(Some("deep")) {
            DNN::load(path).map(Self::DNN)
        } else {
            let f = File::open(path).unwrap(); //NOTE unwrap on file open
            let result: Result<LFA_, String> =
                bincode::deserialize_from(f).map_err(|err| format!("{}", err));
            result.map(Self::LFA)
        }
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
            Self::DNN(fa) => fa.store(path)
        }
    }
}

impl AI {
    fn best_action(&self, game: &Game) -> Result<usize, String> {
        match self {
            Self::Random(_ai) => Err(String::from("Call RandomAI::take_turn etc. directly")),
            Self::LFA(fa) => Ok(find_legal_max(fa, game, true).0),
            Self::DNN(fa) => {
                let action = find_legal_max(fa, game, false).0;
                // println!("ACTION: {:?}", UmpireAction::from_idx(action));
                Ok(action)
            }
        }
    }

    fn _take_turn_unended(
        &mut self,
        game: &mut Game,
        generate_data: bool,
    ) -> Option<Vec<TrainingInstance>> {
        let mut training_instances = if generate_data {
            Some(Vec::new())
        } else {
            None
        };

        let player = game.current_player();

        while !game.turn_is_done() {
            // features: Vec<f64>,// the view on the game state
            // pre_score: f64,// the player's score prior to the action
            // action_idx: usize,// the action taken
            // post_score: f64,// the player's score after the action
            // outcome: TrainingOutcome,// how did things work out for the player?

            let (num_features, features, pre_score) = if generate_data {
                let features = game.features();
                let (num_features, features) = sparsify(features);

                (
                    Some(num_features),
                    Some(features),
                    Some(game.player_score(player).unwrap()),
                )
            } else {
                (None, None, None)
            };

            let action_idx = self.best_action(game).unwrap();
            let action = UmpireAction::from_idx(action_idx).unwrap();
            action.take(game).unwrap();

            if generate_data {
                let post_score = game.player_score(player).unwrap();
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

        training_instances
    }
}

impl TurnTaker for AI {
    fn take_turn_not_clearing(
        &mut self,
        game: &mut Game,
        generate_data: bool,
    ) -> Option<Vec<TrainingInstance>> {
        match self {
            Self::Random(ai) => ai.take_turn_not_clearing(game, generate_data),
            Self::LFA(_fa) => {
                let result = self._take_turn_unended(game, generate_data);

                game.end_turn().unwrap();

                result
            }
            Self::DNN(_fa) => {
                let result = self._take_turn_unended(game, generate_data);

                game.end_turn().unwrap();

                result
            }
        }
    }

    fn take_turn_clearing(
        &mut self,
        game: &mut Game,
        generate_data: bool,
    ) -> Option<Vec<TrainingInstance>> {
        match self {
            Self::Random(ai) => ai.take_turn_clearing(game, generate_data),
            Self::LFA(_fa) => {
                let result = self._take_turn_unended(game, generate_data);

                game.end_turn_clearing().unwrap();

                result
            }
            Self::DNN(_fa) => {
                let result = self._take_turn_unended(game, generate_data);

                game.end_turn_clearing().unwrap();

                result
            }
        }
    }
}

// Exports
pub use random::RandomAI;
pub use rl::find_legal_max;
