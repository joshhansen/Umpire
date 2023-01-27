use std::{
    collections::{HashMap, HashSet},
    fmt,
    path::Path,
};

use serde::{Deserialize, Serialize};

use crate::{
    cli::Specified,
    game::GameError,
    util::{Direction, Vec2d},
};

use super::{
    map::dijkstra::Source, obs::Obs, unit::UnitType, AlignedMaybe, Game, PlayerNum, PlayerType,
};

pub type fX = f64;

//FIXME Someday compute this at compile time
pub const POSSIBLE_ACTIONS: usize = UnitType::values().len() + Direction::values().len() + 2;

pub const ADDED_WIDE_FEATURES: i64 = 4;
pub const UNIT_TYPE_WRIT_LARGE_LEN: i64 = UnitType::values().len() as i64 + 1; // what sort of unit is being considered, including
                                                                               // "city" as a unit type (thus the +1)

pub const WIDE_LEN: i64 =
    UNIT_TYPE_WRIT_LARGE_LEN + UnitType::values().len() as i64 + ADDED_WIDE_FEATURES;
pub const DEEP_WIDTH: i64 = 11;
pub const DEEP_HEIGHT: i64 = 11;
pub const DEEP_LEN: i64 = DEEP_WIDTH * DEEP_HEIGHT;
pub const DEEP_FEATS: i64 = 4;
pub const FEATS_LEN: i64 = WIDE_LEN + DEEP_FEATS * DEEP_LEN;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum UmpireAction {
    SetNextCityProduction { unit_type: UnitType },
    MoveNextUnit { direction: Direction },
    DisbandNextUnit,
    SkipNextUnit,
}

impl UmpireAction {
    pub fn legal_actions(game: &Game) -> HashSet<Self> {
        let mut a = HashSet::new();

        debug_assert!(!game.turn_is_done());

        //TODO Possibly consider actions for all cities instead of just the next one that isn't set yet
        if let Some(city_loc) = game.production_set_requests().next() {
            for unit_type in game.valid_productions_conservative(city_loc) {
                a.insert(UmpireAction::SetNextCityProduction { unit_type });
            }
        }

        //TODO Possibly consider actions for all units instead of just the next one that needs orders
        if let Some(unit_id) = game.unit_orders_requests().next() {
            for direction in game.current_player_unit_legal_directions(unit_id).unwrap() {
                a.insert(UmpireAction::MoveNextUnit { direction });
            }
            a.insert(UmpireAction::SkipNextUnit);
        }

        debug_assert!(!a.is_empty());

        a
    }

    /// All actions possible in general---not specific to any particular game state
    /// TODO: Make this an array?
    // UnitType::Infantry,    0
    // UnitType::Armor,       1
    // UnitType::Fighter,     2
    // UnitType::Bomber,      3
    // UnitType::Transport,   4
    // UnitType::Destroyer,   5
    // UnitType::Submarine,   6
    // UnitType::Cruiser,     7
    // UnitType::Battleship,  8
    // UnitType::Carrier      9
    // Direction::Up,         10
    // Direction::Down,       11
    // Direction::Left,       12
    // Direction::Right,      13
    // Direction::UpLeft,     14
    // Direction::UpRight,    15
    // Direction::DownLeft,   16
    // Direction::DownRight,  17
    // SkipNextTurn           18
    pub fn possible_actions() -> Vec<Self> {
        let mut a = Vec::with_capacity(POSSIBLE_ACTIONS);
        for unit_type in UnitType::values().iter().cloned() {
            a.push(UmpireAction::SetNextCityProduction { unit_type });
        }
        for direction in Direction::values().iter().cloned() {
            a.push(UmpireAction::MoveNextUnit { direction });
        }
        a.push(UmpireAction::SkipNextUnit);

        a
    }

    pub fn from_idx(mut idx: usize) -> Result<Self, ()> {
        let unit_types = UnitType::values();
        if unit_types.len() > idx {
            return Ok(UmpireAction::SetNextCityProduction {
                unit_type: unit_types[idx],
            });
        }

        idx -= unit_types.len();

        let dirs = Direction::values();
        if dirs.len() > idx {
            return Ok(UmpireAction::MoveNextUnit {
                direction: dirs[idx],
            });
        }

        idx -= dirs.len();

        if idx == 0 {
            return Ok(UmpireAction::SkipNextUnit);
        }

        Err(())
    }

    pub fn to_idx(self) -> usize {
        Self::possible_actions()
            .into_iter()
            .position(|a| self == a)
            .unwrap()
    }

    pub fn take(self, game: &mut Game) -> Result<(), GameError> {
        match self {
            UmpireAction::SetNextCityProduction { unit_type } => {
                let city_loc = game.production_set_requests().next().unwrap();
                game.set_production_by_loc(city_loc, unit_type).map(|_| ())
            }
            UmpireAction::MoveNextUnit { direction } => {
                let unit_id = game.unit_orders_requests().next().unwrap();
                debug_assert!({
                    let legal: HashSet<Direction> = game
                        .current_player_unit_legal_directions(unit_id)
                        .unwrap()
                        .collect();

                    // println!("legal moves: {}", legal.len());

                    legal.contains(&direction)
                });

                game.move_unit_by_id_in_direction(unit_id, direction)
                    .map(|_| ())
                    .map_err(GameError::MoveError)
            }
            UmpireAction::DisbandNextUnit => {
                let unit_id = game.unit_orders_requests().next().unwrap();
                game.disband_unit_by_id(unit_id).map(|_| ())
            }
            UmpireAction::SkipNextUnit => {
                let unit_id = game.unit_orders_requests().next().unwrap();
                game.order_unit_skip(unit_id).map(|_| ())
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
pub enum TrainingOutcome {
    Victory,
    Defeat,
    Inconclusive,
}

/// An instance in which an action was taken in a game state and a reward was achieved; annotated with whether the
/// player later went on to victory, defeat, or an inconclusive outcome
#[derive(Serialize, Deserialize)]
pub struct TrainingInstance {
    player: PlayerNum, // the player that took the action
    num_features: usize,
    features: HashMap<usize, f64>,
    pre_score: f64,       // the player's score prior to the action
    action: UmpireAction, // the action taken
    post_score: f64,      // the player's score after the action
    outcome: Option<TrainingOutcome>, // how did things work out for the player?
                          // set as None until the outcome is determined
}
impl TrainingInstance {
    pub fn undetermined(
        player: PlayerNum,
        num_features: usize,
        features: HashMap<usize, f64>,
        pre_score: f64,
        action: UmpireAction,
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
pub fn player_features(game: &Game, player: PlayerNum) -> Vec<fX> {
    // For every tile we add these f64's:
    // is the tile observed or not?
    // which player controls the tile (one hot encoded)
    // is there a city or not?
    // what is the unit type? (one hot encoded, could be none---all zeros)
    // for each of the five potential carried units:
    //   what is the unit type? (one hot encoded, could be none---all zeros)
    //

    let unit_id = game.player_unit_orders_requests(player).next();
    let city_loc = game.player_production_set_requests(player).next();

    let unit_type = unit_id.and_then(|unit_id| {
        game.player_unit_by_id(player, unit_id)
            .map(|unit| unit.type_)
    });

    // We also add a context around the currently active unit (if any)
    let mut x = Vec::with_capacity(FEATS_LEN as usize);

    // General statistics

    // NOTE Update dnn::ADDED_WIDE_FEATURES to reflect the number of generic features added here

    // - current turn
    x.push(game.turn as fX);

    // - number of cities player controls
    x.push(game.player_city_count(player) as fX);

    let observations = game.player_observations(player);

    // - number of tiles observed
    let num_observed: fX = observations.num_observed() as fX;
    x.push(num_observed);

    // - percentage of tiles observed
    x.push(num_observed / game.dims().area() as fX);

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
    let type_counts = game.player_unit_type_counts(player).unwrap_or(&empty_map);
    let counts_vec: Vec<fX> = UnitType::values()
        .iter()
        .map(|type_| *type_counts.get(type_).unwrap_or(&0) as fX)
        .collect();

    x.extend(counts_vec);

    // Relatively positioned around next unit (if any) or city

    let loc = unit_id
        .map(|unit_id| match game.player_unit_loc(player, unit_id) {
            Some(loc) => loc,
            None => {
                panic!("Unit was in orders requests but not in current player observations")
            }
        })
        .or(city_loc);

    let mut is_enemy_belligerent = Vec::new();
    let mut is_observed = Vec::new();
    let mut is_neutral = Vec::new();
    let mut is_city = Vec::new();

    // 2d features
    for inc_x in -5..=5 {
        for inc_y in -5..=5 {
            let inc: Vec2d<i32> = Vec2d::new(inc_x, inc_y);

            let obs = if let Some(origin) = loc {
                game.wrapping
                    .wrapped_add(game.dims(), origin, inc)
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
                    } else if alignment.is_belligerent()
                        && alignment.is_enemy_of_player(game.current_player)
                    {
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

    x
}

/// A user specification of an AI
///
/// Used as a lightweight description of an AI to be passed around. Also to validate AIs given at the command line.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum AISpec {
    /// A horrible AI that makes decisions randomly
    Random,

    /// AI loaded from a path. If it's a file, deserialize the usual `rsrl` `LFA`-based model. If it's a directory,
    /// load it as a TensorFlow SavedModel.
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

impl Into<PlayerType> for AISpec {
    fn into(self) -> PlayerType {
        PlayerType::AI(self)
    }
}

impl Into<String> for AISpec {
    fn into(self) -> String {
        String::from(self.spec())
    }
}
