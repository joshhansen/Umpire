use std::{collections::HashMap, fmt, path::Path};

use serde::{Deserialize, Serialize};

use crate::{
    cli::Specified,
    game::{action::AiPlayerAction, alignment::AlignedMaybe},
    util::{Vec2d, POSSIBLE_DIRECTIONS},
};

use super::{
    map::dijkstra::Source,
    obs::Obs,
    unit::{UnitType, POSSIBLE_UNIT_TYPES},
    IGame, PlayerNum, PlayerSecret, PlayerType, UmpireResult,
};

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
pub const DEEP_WIDTH: usize = 11;
pub const DEEP_HEIGHT: usize = 11;
pub const DEEP_TILES: usize = DEEP_WIDTH * DEEP_HEIGHT;

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

/// Feature vector for use in AI training; the specified player's current state
///
/// Map of the output vector:
///
/// # 15: 1d features
/// * 1: current turn
/// * 1: current player city count
/// * 1: number of tiles observed by current player
/// * 1: percentage of tiles observed by current player
/// * 11: the type of unit being represented, where "city" is also a type of unit (one hot encoded)
/// * 10: number of units controlled by current player (infantry, armor, fighters, bombers, transports, destroyers
///                                                     submarines, cruisers, battleships, carriers)
/// # 363: 2d features, three layers
/// * 121: is_enemy_belligerent (11x11)
/// * 121: is_observed (11x11)
/// * 121: is_neutral (11x11)
///
pub async fn player_features_classic(
    game: &dyn IGame,
    player_secret: PlayerSecret,
) -> UmpireResult<Vec<fX>> {
    // For every tile we add these f64's:
    // is the tile observed or not?
    // which player controls the tile (one hot encoded)
    // is there a city or not?
    // what is the unit type? (one hot encoded, could be none---all zeros)
    // for each of the five potential carried units:
    //   what is the unit type? (one hot encoded, could be none---all zeros)
    //

    let unit_id = game
        .player_unit_orders_requests(player_secret)
        .await
        .unwrap()
        .iter()
        .cloned()
        .next();
    let city_loc = game
        .player_production_set_requests(player_secret)
        .await
        .unwrap()
        .iter()
        .cloned()
        .next();

    let unit_type = if let Some(unit_id) = unit_id {
        game.player_unit_by_id(player_secret, unit_id)
            .await
            .map(|maybe_unit| maybe_unit.map(|unit| unit.type_))
            .unwrap()
    } else {
        None
    };

    // We also add a context around the currently active unit (if any)
    let mut x = Vec::with_capacity(FEATS_LEN);

    // General statistics

    // NOTE Update dnn::ADDED_WIDE_FEATURES to reflect the number of generic features added here

    // - current turn
    x.push(game.turn().await as fX);

    // - number of cities player controls
    x.push(game.player_city_count(player_secret).await.unwrap() as fX);

    let observations = game.player_observations(player_secret).await.unwrap();

    // - number of tiles observed
    let num_observed: fX = observations.num_observed() as fX;
    x.push(num_observed);

    // - percentage of tiles observed
    let dims = game.dims().await;
    x.push(num_observed / dims.area() as fX);

    // - unit type writ large
    for unit_type_ in &UnitType::values() {
        x.push(if let Some(unit_type) = unit_type {
            if unit_type == *unit_type_ {
                1.0
            } else {
                0.0
            }
        } else {
            0.0
        });
    }
    // Also includes whether it's a city or not
    x.push(if city_loc.is_some() { 1.0 } else { 0.0 });

    // NOTE The unit counts are not included in dnn::ADDED_WIDE_FEATURES
    // - number of each type of unit controlled by player
    let empty_map = HashMap::new();
    let type_counts = game
        .player_unit_type_counts(player_secret)
        .await
        .unwrap_or(empty_map);
    let counts_vec: Vec<fX> = UnitType::values()
        .iter()
        .map(|type_| *type_counts.get(type_).unwrap_or(&0) as fX)
        .collect();

    x.extend(counts_vec);

    // Relatively positioned around next unit (if any) or city

    let loc = if let Some(unit_id) = unit_id {
        Some(
            match game.player_unit_loc(player_secret, unit_id).await.unwrap() {
                Some(loc) => loc,
                None => {
                    panic!("Unit was in orders requests but not in current player observations")
                }
            },
        )
    } else {
        city_loc
    };

    let mut is_enemy_belligerent = Vec::new();
    let mut is_observed = Vec::new();
    let mut is_neutral = Vec::new();
    let mut is_city = Vec::new();

    let player = game.current_player().await;

    // 2d features
    for inc_x in -5..=5 {
        for inc_y in -5..=5 {
            let inc: Vec2d<i32> = Vec2d::new(inc_x, inc_y);

            let obs = if let Some(origin) = loc {
                game.wrapping()
                    .await
                    .wrapped_add(dims, origin, inc)
                    .map_or(&Obs::Unobserved, |loc| observations.get(loc))
            } else {
                &Obs::Unobserved
            };

            // x.extend_from_slice(&obs_to_vec(&obs, self.num_players));
            // push_obs_to_vec(&mut x, &obs, self.num_players);

            let mut enemy = 0.0;
            let mut observed = 0.0;
            let mut neutral = 0.0;
            let mut city = 0.0;

            if let Obs::Observed { tile, .. } = obs {
                observed = 1.0;

                if tile.city.is_some() {
                    city = 1.0;
                }

                if let Some(alignment) = tile.alignment_maybe() {
                    if alignment.is_neutral() {
                        neutral = 1.0;
                    } else if alignment.is_belligerent() && alignment.is_enemy_of_player(player) {
                        enemy = 1.0;
                    }
                }
            }

            is_enemy_belligerent.push(enemy);
            is_observed.push(observed);
            is_neutral.push(neutral);
            is_city.push(city);
        }
    }

    x.extend(is_enemy_belligerent);
    x.extend(is_observed);
    x.extend(is_neutral);
    x.extend(is_city);

    Ok(x)
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
