//! Reified player actions

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::util::{Direction, Location};

use super::{
    ai::POSSIBLE_ACTIONS,
    city::CityID,
    move_::Move,
    unit::{
        orders::{Orders, OrdersOutcome},
        Unit, UnitID, UnitType,
    },
    Game, GameError, TurnStart,
};

/// Bare-bones actions, reduced for machine learning purposes
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum AiPlayerAction {
    SetNextCityProduction { unit_type: UnitType },
    MoveNextUnit { direction: Direction },
    DisbandNextUnit,
    SkipNextUnit,
}

impl AiPlayerAction {
    pub fn legal_actions(game: &Game) -> HashSet<Self> {
        let mut a = HashSet::new();

        debug_assert!(!game.turn_is_done());

        //TODO Possibly consider actions for all cities instead of just the next one that isn't set yet
        if let Some(city_loc) = game.current_player_production_set_requests().next() {
            for unit_type in game.valid_productions_conservative(city_loc) {
                a.insert(AiPlayerAction::SetNextCityProduction { unit_type });
            }
        }

        //TODO Possibly consider actions for all units instead of just the next one that needs orders
        if let Some(unit_id) = game.unit_orders_requests().next() {
            for direction in game.current_player_unit_legal_directions(unit_id).unwrap() {
                a.insert(AiPlayerAction::MoveNextUnit { direction });
            }
            a.insert(AiPlayerAction::SkipNextUnit);
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
            a.push(AiPlayerAction::SetNextCityProduction { unit_type });
        }
        for direction in Direction::values().iter().cloned() {
            a.push(AiPlayerAction::MoveNextUnit { direction });
        }
        a.push(AiPlayerAction::SkipNextUnit);

        a
    }

    pub fn from_idx(mut idx: usize) -> Result<Self, ()> {
        let unit_types = UnitType::values();
        if unit_types.len() > idx {
            return Ok(AiPlayerAction::SetNextCityProduction {
                unit_type: unit_types[idx],
            });
        }

        idx -= unit_types.len();

        let dirs = Direction::values();
        if dirs.len() > idx {
            return Ok(AiPlayerAction::MoveNextUnit {
                direction: dirs[idx],
            });
        }

        idx -= dirs.len();

        if idx == 0 {
            return Ok(AiPlayerAction::SkipNextUnit);
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
            AiPlayerAction::SetNextCityProduction { unit_type } => {
                let city_loc = game
                    .current_player_production_set_requests()
                    .next()
                    .unwrap();
                game.set_production_by_loc(city_loc, unit_type).map(|_| ())
            }
            AiPlayerAction::MoveNextUnit { direction } => {
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
            AiPlayerAction::DisbandNextUnit => {
                let unit_id = game.unit_orders_requests().next().unwrap();
                game.disband_unit_by_id(unit_id).map(|_| ())
            }
            AiPlayerAction::SkipNextUnit => {
                let unit_id = game.unit_orders_requests().next().unwrap();
                game.order_unit_skip(unit_id).map(|_| ())
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub enum PlayerAction {
    EndTurn,
    SetCityProduction {
        city_id: CityID,
        production: UnitType,
    },
    MoveUnit {
        unit_id: UnitID,
        dest: Location,
    },
    MoveUnitInDirection {
        unit_id: UnitID,
        direction: Direction,
    },
    DisbandUnit {
        unit_id: UnitID,
    },
    OrderUnit {
        unit_id: UnitID,
        orders: Orders,
    },
}

#[derive(Debug, Deserialize, Serialize)]
pub enum PlayerActionOutcome {
    TurnEnded(TurnStart),
    SetCityProduction {
        city_id: CityID,
        production: UnitType,
        prior_production: Option<UnitType>,
    },
    MoveUnit {
        unit_id: UnitID,
        /// When moving by direction, this could be None
        dest: Option<Location>,
        move_: Move,
    },
    DisbandUnit {
        disbanded: Unit,
    },
    OrderUnit {
        unit_id: UnitID,
        orders: Orders,
        orders_outcome: OrdersOutcome,
    },
}

impl PlayerAction {
    pub fn take(self, game: &mut Game) -> Result<PlayerActionOutcome, GameError> {
        match self {
            PlayerAction::EndTurn => game
                .end_turn()
                .map(|turn_start| PlayerActionOutcome::TurnEnded(turn_start)),
            PlayerAction::SetCityProduction {
                city_id,
                production,
            } => game
                .set_production_by_id(city_id, production)
                .map(|prior_production| PlayerActionOutcome::SetCityProduction {
                    city_id,
                    production,
                    prior_production,
                }),
            PlayerAction::MoveUnit { unit_id, dest } => game
                .move_unit_by_id(unit_id, dest)
                .map(|move_| PlayerActionOutcome::MoveUnit {
                    unit_id,
                    dest: Some(dest),
                    move_,
                })
                .map_err(|move_err| GameError::MoveError(move_err)),
            PlayerAction::MoveUnitInDirection { unit_id, direction } => {
                let dest = game
                    .current_player_unit_by_id(unit_id)
                    .unwrap()
                    .loc
                    .shift_wrapped(direction, game.dims(), game.wrapping);
                game.move_unit_by_id_in_direction(unit_id, direction)
                    .map(|move_| PlayerActionOutcome::MoveUnit {
                        unit_id,
                        dest,
                        move_,
                    })
                    .map_err(|move_err| GameError::MoveError(move_err))
            }
            PlayerAction::DisbandUnit { unit_id } => game
                .disband_unit_by_id(unit_id)
                .map(|disbanded| PlayerActionOutcome::DisbandUnit { disbanded }),
            PlayerAction::OrderUnit { unit_id, orders } => game
                .set_and_follow_orders(unit_id, orders)
                .map(|orders_outcome| PlayerActionOutcome::OrderUnit {
                    unit_id,
                    orders,
                    orders_outcome,
                }),
        }
    }
}
