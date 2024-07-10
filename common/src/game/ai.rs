use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::{self, Display},
    path::Path,
};

use burn::{
    backend::{wgpu::WgpuDevice, Autodiff, Wgpu},
    tensor::backend::Backend,
};
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::{cli::Specified, game::action::AiPlayerAction, util::POSSIBLE_DIRECTIONS};

use super::{
    unit::{POSSIBLE_UNIT_TYPES, POSSIBLE_UNIT_TYPES_WRIT_LARGE},
    ActionNum, PlayerNum, PlayerType, TurnNum,
};

pub type AiBackend = Wgpu;
pub type AiBackendTrain = Autodiff<AiBackend>;
pub type AiBackendDevice = <AiBackend as Backend>::Device;

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize,
)]
pub enum AiDevice {
    #[default]
    Best,
    Cpu,
    DiscreteGpu(usize),
}
impl From<AiDevice> for WgpuDevice {
    fn from(value: AiDevice) -> Self {
        match value {
            AiDevice::Best => Self::BestAvailable,
            AiDevice::Cpu => Self::Cpu,
            AiDevice::DiscreteGpu(x) => Self::DiscreteGpu(x),
        }
    }
}

#[allow(non_camel_case_types)]
pub type fX = f32;

pub const P_DROPOUT: f64 = 0.4;

pub const POSSIBLE_CITY_ACTIONS: usize = POSSIBLE_UNIT_TYPES; // all possible productions

pub const POSSIBLE_UNIT_ACTIONS: usize = POSSIBLE_DIRECTIONS + 2; // plus skip and disband

pub const POSSIBLE_ACTIONS: usize = POSSIBLE_CITY_ACTIONS + POSSIBLE_UNIT_ACTIONS;

pub const ADDED_WIDE_FEATURES: usize = 13;

/// Number of 1d (wide) features
/// Includes `POSSIBLE_UNIT_TYPES` twice: once for the unit type one-hot-encoded, once for the overall unit counts, plus one for city
pub const WIDE_LEN: usize =
    POSSIBLE_UNIT_TYPES_WRIT_LARGE + POSSIBLE_UNIT_TYPES + ADDED_WIDE_FEATURES;
pub const DEEP_WIDTH: usize = 15;
pub const DEEP_HEIGHT: usize = 15;
pub const DEEP_TILES: usize = DEEP_WIDTH * DEEP_HEIGHT;

pub const DEEP_OUT_WIDTH: usize = 3;
pub const DEEP_OUT_HEIGHT: usize = 3;
pub const DEEP_OUT_TILES: usize = DEEP_OUT_WIDTH * DEEP_OUT_HEIGHT;

pub const DEEP_WIDTH_REL_MIN: i32 = DEEP_WIDTH as i32 / -2;
pub const DEEP_WIDTH_REL_MAX: i32 = DEEP_WIDTH as i32 / 2;
pub const DEEP_HEIGHT_REL_MIN: i32 = DEEP_HEIGHT as i32 / -2;
pub const DEEP_HEIGHT_REL_MAX: i32 = DEEP_HEIGHT as i32 / 2;

/// Number of "channels" in convolution output
pub const BASE_CONV_FEATS: usize = 20;

pub const DEEP_IN_LEN: usize = DEEP_TILES * BASE_CONV_FEATS;

pub const PER_ACTION_CHANNELS: usize = 1;

/// Total length of convolution output after reducing
pub const DEEP_OUT_LEN: usize = DEEP_OUT_TILES * POSSIBLE_ACTIONS * PER_ACTION_CHANNELS;

/// Total length of the feature vectors that are input to the dnn
pub const FEATS_LEN: usize = WIDE_LEN + DEEP_IN_LEN;

/// We customize the feature vector depending on if we're training a model for city actions or unit actions
/// This just lets us specify which.
///
/// UnitIfExistsElseCity is for compatibility as that was the old behavior
#[derive(Debug, Serialize, Deserialize)]
pub enum TrainingFocus {
    City,
    Unit,
    UnitIfExistsElseCity,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Ord, PartialOrd, Eq, PartialEq)]
pub enum TrainingOutcome {
    Victory,
    Defeat,
    Inconclusive,
}

impl TrainingOutcome {
    pub const fn values() -> [Self; 3] {
        [Self::Victory, Self::Defeat, Self::Inconclusive]
    }
    /// Ranges from 0 to 1; exactly 0 for immediate defeat, exactly 1 for immediate victory
    /// closer to 0.5 for later defeats and victories (rewarding survival; punishing delay)
    ///
    /// Draws are punished like a defeat in 990 turns
    pub fn to_training_target(self, turns_until_outcome: TurnNum) -> fX {
        match self {
            Self::Victory => 0.5 + 0.5 / (10.0 as fX + turns_until_outcome as fX).log10(),
            Self::Inconclusive => 0.33333334,
            Self::Defeat => 0.5 - 0.5 / (10.0 as fX + turns_until_outcome as fX).log10(),
        }
    }
}
impl Display for TrainingOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(<Self as Specified>::desc(self).as_str())
    }
}
impl Specified for TrainingOutcome {
    /// A description to show up in the command line help
    fn desc(&self) -> String {
        match self {
            Self::Victory => "victory".to_string(),
            Self::Defeat => "defeat".to_string(),
            Self::Inconclusive => "inconclusive".to_string(),
        }
    }

    /// A canonicalized string representation of the item
    fn spec(&self) -> String {
        match self {
            Self::Victory => "v".to_string(),
            Self::Defeat => "d".to_string(),
            Self::Inconclusive => "i".to_string(),
        }
    }
}
impl TryFrom<String> for TrainingOutcome {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "v" => Ok(Self::Victory),
            "d" => Ok(Self::Defeat),
            "i" => Ok(Self::Inconclusive),
            x => Err(format!("Unrecognized training outcome '{}'", x)),
        }
    }
}

/// An instance in which an action was taken in a game state and a reward was achieved; annotated with whether the
/// player later went on to victory, defeat, or an inconclusive outcome
#[derive(Serialize, Deserialize)]
pub struct TrainingInstance {
    pub player: PlayerNum, // the player that took the action
    pub num_features: usize,

    /// The actions among which the player selected
    pub legal_actions: BTreeSet<AiPlayerAction>,

    pub features: BTreeMap<usize, fX>,
    pub turn: TurnNum,

    /// The number of actions taken by the player prior to this one
    pub action_count: ActionNum,

    pub pre_score: f64,         // the player's score prior to the action
    pub action: AiPlayerAction, // the action taken
    pub post_score: f64,        // the player's score after the action

    /// How did things work out for the player?
    ///
    /// Set as None until the outcome is determined
    pub outcome: Option<TrainingOutcome>,

    /// The turn on which the game ended, or the last played on draws
    ///
    /// Set as None until the outcome is determined
    pub last_turn: Option<TurnNum>,
}
impl TrainingInstance {
    pub fn undetermined(
        player: PlayerNum,
        num_features: usize,
        legal_actions: BTreeSet<AiPlayerAction>,
        features: BTreeMap<usize, fX>,
        turn: TurnNum,
        action_count: ActionNum,
        pre_score: f64,
        action: AiPlayerAction,
        post_score: f64,
    ) -> Self {
        Self {
            player,
            num_features,
            legal_actions,
            features,
            turn,
            action_count,
            pre_score,
            action,
            post_score,
            outcome: None,
            last_turn: None,
        }
    }

    pub fn determine(&mut self, outcome: TrainingOutcome, last_turn: TurnNum) {
        self.outcome = Some(outcome);
        self.last_turn = Some(last_turn);
    }

    pub fn victory(&mut self, last_turn: TurnNum) {
        self.determine(TrainingOutcome::Victory, last_turn);
    }

    pub fn defeat(&mut self, last_turn: TurnNum) {
        self.determine(TrainingOutcome::Defeat, last_turn);
    }

    pub fn inconclusive(&mut self, last_turn: TurnNum) {
        self.determine(TrainingOutcome::Inconclusive, last_turn);
    }
}

lazy_static! {
    static ref RANDOM_RGX: Regex = Regex::new(r"^r(?:and(?:om)?)?(?:(?P<seed>\d+))?$").unwrap();
    static ref RANDOM_PLUS_RGX: Regex =
        Regex::new(r"^R(?:and(?:om)?)?(?:(?P<seed>\d+))?$").unwrap();
}

/// A user specification of an AI
///
/// Used as a lightweight description of an AI to be passed around. Also to validate AIs given at the command line.
#[derive(Clone, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Deserialize, Serialize)]
pub enum AISpec {
    /// A horrible AI that makes decisions randomly
    Random { seed: Option<u64> },

    /// A less-horrible AI that makes decisions randomly, but skips and disbands less
    RandomPlus { seed: Option<u64> },

    /// An even worse AI that only ever skips unit actions; first possible production for cities
    Skip,

    /// AI loaded from a path.
    ///
    /// See the Loadable impl for `AI` for more information.
    FromPath { path: String, device: AiDevice },

    /// AI loaded from a preset AI level, beginning at 1
    FromLevel { level: usize, device: AiDevice },
}

impl fmt::Display for AISpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.desc().fmt(f)
    }
}

impl TryFrom<String> for AISpec {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if let Some(m) = RANDOM_RGX.captures(value.as_str()) {
            let seed: Option<u64> = m.name("seed").map(|seed| seed.as_str().parse().unwrap());
            return Ok(Self::Random { seed });
        }

        if let Some(m) = RANDOM_PLUS_RGX.captures(value.as_str()) {
            let seed: Option<u64> = m.name("seed").map(|seed| seed.as_str().parse().unwrap());
            return Ok(Self::RandomPlus { seed });
        }

        match value.as_str() {
            "s" => Ok(Self::Skip),
            "0" | "1" => Ok(Self::FromLevel {
                level: value.chars().next().unwrap().to_digit(10).unwrap() as usize,
                device: Default::default(),
            }),
            s => {
                if Path::new(s).exists() {
                    Ok(Self::FromPath {
                        path: value,
                        device: Default::default(),
                    })
                } else {
                    Err(format!("Unrecognized AI specification '{}'", s))
                }
            }
        }
    }
}

/// An item specified by a string on the command line
impl Specified for AISpec {
    /// A description to show up in the command line help
    fn desc(&self) -> String {
        match self {
            Self::Random { seed } => {
                let mut s = String::from("random");
                if let Some(seed) = seed {
                    s.push('(');
                    s.push_str(seed.to_string().as_str());
                    s.push(')');
                }
                s
            }
            Self::RandomPlus { seed } => {
                let mut s = String::from("random+");
                if let Some(seed) = seed {
                    s.push('(');
                    s.push_str(seed.to_string().as_str());
                    s.push(')');
                }
                s
            }
            Self::Skip => String::from("skip"),
            Self::FromPath { path, .. } => format!("AI from path {}", path),
            Self::FromLevel { level, .. } => format!("level {} AI", level),
        }
    }

    /// A canonicalized string representation of the item
    fn spec(&self) -> String {
        match self {
            Self::Random { seed } => {
                let mut s = String::from("r");
                if let Some(seed) = seed {
                    s.push('(');
                    s.push_str(seed.to_string().as_str());
                    s.push(')');
                }
                s
            }
            Self::RandomPlus { seed } => {
                let mut s = String::from("R");
                if let Some(seed) = seed {
                    s.push('(');
                    s.push_str(seed.to_string().as_str());
                    s.push(')');
                }
                s
            }
            Self::Skip => String::from("s"),
            Self::FromPath { path, .. } => path.clone(),
            Self::FromLevel { level, .. } => format!("{}", level),
        }
    }
}

impl TryFrom<Option<&String>> for AISpec {
    type Error = String;

    fn try_from(value: Option<&String>) -> Result<Self, Self::Error> {
        if let Some(value) = value {
            AISpec::try_from(value.clone())
        } else {
            Ok(Self::Random { seed: None })
        }
    }
}

impl TryFrom<PlayerType> for AISpec {
    type Error = String;
    fn try_from(t: PlayerType) -> Result<Self, Self::Error> {
        match t {
            PlayerType::Human => Err("Human is not an AI".to_owned()),
            PlayerType::AI(s) => Ok(s),
        }
    }
}

impl From<AISpec> for String {
    fn from(s: AISpec) -> Self {
        s.spec()
    }
}
