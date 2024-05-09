use std::{collections::HashMap, fmt, path::Path};

use serde::{Deserialize, Serialize};

use crate::{cli::Specified, game::action::AiPlayerAction, util::POSSIBLE_DIRECTIONS};

use super::{unit::POSSIBLE_UNIT_TYPES, PlayerNum, PlayerType};

#[allow(non_camel_case_types)]
pub type fX = f32;

pub const POSSIBLE_CITY_ACTIONS: usize = POSSIBLE_UNIT_TYPES; // all possible productions

pub const POSSIBLE_UNIT_ACTIONS: usize = POSSIBLE_DIRECTIONS + 2; // plus skip and disband

pub const POSSIBLE_ACTIONS: usize = POSSIBLE_CITY_ACTIONS + POSSIBLE_UNIT_ACTIONS;

pub const ADDED_WIDE_FEATURES: usize = 4;
pub const UNIT_TYPE_WRIT_LARGE_LEN: usize = POSSIBLE_UNIT_TYPES + 1; // what sort of unit is being considered, including
                                                                     // "city" as a unit type (thus the +1)

/// Number of 1d (wide) features
/// Includes `POSSIBLE_UNIT_TYPES` twice: once for the unit type one-hot-encoded, once for the overall unit counts, plus one for city
pub const WIDE_LEN: usize = UNIT_TYPE_WRIT_LARGE_LEN + POSSIBLE_UNIT_TYPES + ADDED_WIDE_FEATURES;
pub const DEEP_WIDTH: usize = 15;
pub const DEEP_HEIGHT: usize = 15;
pub const DEEP_TILES: usize = DEEP_WIDTH * DEEP_HEIGHT;

pub const DEEP_WIDTH_REL_MIN: i32 = DEEP_WIDTH as i32 / -2;
pub const DEEP_WIDTH_REL_MAX: i32 = DEEP_WIDTH as i32 / 2;
pub const DEEP_HEIGHT_REL_MIN: i32 = DEEP_HEIGHT as i32 / -2;
pub const DEEP_HEIGHT_REL_MAX: i32 = DEEP_HEIGHT as i32 / 2;

/// Number of "channels" in convolution output
pub const BASE_CONV_FEATS: usize = 16;

pub const DEEP_LEN: usize = DEEP_TILES * BASE_CONV_FEATS;

/// Total length of convolution output after reducing to 3x3
pub const DEEP_OUT_LEN: usize = 9 * BASE_CONV_FEATS;

/// Total length of the feature vectors that are input to the dnn
pub const FEATS_LEN: usize = WIDE_LEN + DEEP_LEN;

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

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum TrainingOutcome {
    Victory,
    Defeat,
    Inconclusive,
}

impl TrainingOutcome {
    pub fn to_training_target(self) -> fX {
        match self {
            Self::Victory => 1.0,
            Self::Inconclusive => 0.25, // punish draws, but not as harshly as defeats
            Self::Defeat => 0.0,
        }
    }
}

/// An instance in which an action was taken in a game state and a reward was achieved; annotated with whether the
/// player later went on to victory, defeat, or an inconclusive outcome
#[derive(Serialize, Deserialize)]
pub struct TrainingInstance {
    pub player: PlayerNum, // the player that took the action
    pub num_features: usize,
    pub features: HashMap<usize, fX>,
    pub pre_score: f64,         // the player's score prior to the action
    pub action: AiPlayerAction, // the action taken
    pub post_score: f64,        // the player's score after the action
    pub outcome: Option<TrainingOutcome>, // how did things work out for the player?
                                // set as None until the outcome is determined
}
impl TrainingInstance {
    pub fn undetermined(
        player: PlayerNum,
        num_features: usize,
        features: HashMap<usize, fX>,
        pre_score: f64,
        action: AiPlayerAction,
        post_score: f64,
    ) -> Self {
        Self {
            player,
            num_features,
            features,
            pre_score,
            action,
            post_score,
            outcome: None,
        }
    }

    pub fn determine(&mut self, outcome: TrainingOutcome) {
        self.outcome = Some(outcome);
    }

    pub fn victory(&mut self) {
        self.determine(TrainingOutcome::Victory);
    }

    pub fn defeat(&mut self) {
        self.determine(TrainingOutcome::Defeat);
    }

    pub fn inconclusive(&mut self) {
        self.determine(TrainingOutcome::Inconclusive);
    }
}

/// A user specification of an AI
///
/// Used as a lightweight description of an AI to be passed around. Also to validate AIs given at the command line.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
pub enum AISpec {
    /// A horrible AI that makes decisions randomly
    Random,

    /// AI loaded from a path.
    ///
    /// See the Loadable impl for `AI` for more information.
    FromPath(String),

    /// AI loaded from a preset AI level, beginning at 1
    FromLevel(usize),
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
            "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" => Ok(Self::FromLevel(
                value.chars().next().unwrap().to_digit(10).unwrap() as usize,
            )),
            s => {
                if Path::new(s).exists() {
                    Ok(Self::FromPath(value))
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
