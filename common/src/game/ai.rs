use std::{cmp::Ordering, collections::BTreeMap, fmt, path::Path};

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::{cli::Specified, game::action::AiPlayerAction, util::POSSIBLE_DIRECTIONS};

use super::{
    unit::{POSSIBLE_UNIT_TYPES, POSSIBLE_UNIT_TYPES_WRIT_LARGE},
    PlayerNum, PlayerType, TurnNum,
};

#[allow(non_camel_case_types)]
pub type fX = f32;

pub const P_DROPOUT: f64 = 0.4;

pub const POSSIBLE_CITY_ACTIONS: usize = POSSIBLE_UNIT_TYPES; // all possible productions

pub const POSSIBLE_UNIT_ACTIONS: usize = POSSIBLE_DIRECTIONS + 2; // plus skip and disband

pub const POSSIBLE_ACTIONS: usize = POSSIBLE_CITY_ACTIONS + POSSIBLE_UNIT_ACTIONS;

pub const ADDED_WIDE_FEATURES: usize = 12;

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
pub const BASE_CONV_FEATS: usize = 16;

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
    /// Ranges from 0 to 1; exactly 0 for defeat on the first turn, exactly 1 for victory on the first turn
    /// closer to 0.5 for later defeats and victories (rewarding survival; punishing delay)
    ///
    /// Draws are punished like a defeat on turn 990
    pub fn to_training_target(self, turn: TurnNum) -> fX {
        match self {
            Self::Victory => 0.5 + 0.5 / (10.0 as fX + turn as fX).log10(),
            Self::Inconclusive => 0.33333334,
            Self::Defeat => 0.5 - 0.5 / (10.0 as fX + turn as fX).log10(),
        }
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
    pub features: BTreeMap<usize, fX>,
    pub turn: TurnNum,
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
        features: BTreeMap<usize, fX>,
        turn: TurnNum,
        pre_score: f64,
        action: AiPlayerAction,
        post_score: f64,
    ) -> Self {
        Self {
            player,
            num_features,
            features,
            turn,
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

lazy_static! {
    static ref RANDOM_RGX: Regex = Regex::new(r"^r(?:and(?:om)?)?(?:(?P<seed>\d+))?$").unwrap();
}

/// A user specification of an AI
///
/// Used as a lightweight description of an AI to be passed around. Also to validate AIs given at the command line.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
pub enum AISpec {
    /// A horrible AI that makes decisions randomly
    Random { seed: Option<u64> },

    /// AI loaded from a path.
    ///
    /// See the Loadable impl for `AI` for more information.
    FromPath(String),

    /// AI loaded from a preset AI level, beginning at 1
    FromLevel(usize),
}

impl PartialOrd for AISpec {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

// random > path > level
impl Ord for AISpec {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self {
            Self::Random { seed } => match other {
                Self::Random { seed: other_seed } => seed.cmp(other_seed),
                _ => Ordering::Greater,
            },
            Self::FromPath(path) => match other {
                Self::Random { seed: _ } => Ordering::Less,
                Self::FromPath(other_path) => path.cmp(other_path),
                Self::FromLevel(_) => Ordering::Greater,
            },
            Self::FromLevel(level) => match other {
                Self::FromLevel(other_level) => level.cmp(other_level),
                _ => Ordering::Less,
            },
        }
    }
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

        match value.as_str() {
            "0" => Ok(Self::FromLevel(
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
            Self::Random { seed } => {
                let mut s = String::from("random");
                if let Some(seed) = seed {
                    s.push_str(seed.to_string().as_str());
                }
                s
            }
            Self::FromPath(path) => format!("AI from path {}", path),
            Self::FromLevel(level) => format!("level {} AI", level),
        }
    }

    /// A canonicalized string representation of the item
    fn spec(&self) -> String {
        match self {
            Self::Random { seed } => {
                let mut s = String::from("r");
                if let Some(seed) = seed {
                    s.push_str(seed.to_string().as_str());
                }
                s
            }
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
