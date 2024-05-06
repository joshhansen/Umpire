//! Reified player actions

use std::{collections::HashSet, ops::Deref};

use serde::{Deserialize, Serialize};

use crate::{
    game::TurnPhase,
    util::{Direction, Location},
};

use super::{
    ai::POSSIBLE_ACTIONS_USIZE,
    city::CityID,
    move_::Move,
    player::PlayerTurn,
    unit::{
        orders::{Orders, OrdersOutcome},
        UnitID, UnitType,
    },
    Game, GameError, OrdersSet, PlayerSecret, ProductionSet, TurnStart, UmpireResult,
    UnitDisbanded,
};

/// Something that can be converted into a PlayerAction
/// Like Into<PlayerAction> but with extra context
pub trait Actionable {
    fn to_action(&self, game: &mut Game, secret: PlayerSecret) -> UmpireResult<PlayerAction>;
}

/// Bare-bones actions, reduced for machine learning purposes
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum AiPlayerAction {
    SetNextCityProduction { unit_type: UnitType },
    MoveNextUnit { direction: Direction },
    DisbandNextUnit,
    SkipNextUnit,
    //NOTE When adding new action types, make sure to add them to `possible_actions`
}

impl AiPlayerAction {
    pub fn legal_actions<G: Deref<Target = Game>>(game: G) -> HashSet<Self> {
        let mut a = HashSet::new();

        debug_assert!(!game.current_turn_is_done());
        debug_assert_eq!(game.turn_phase(), TurnPhase::Main);

        //TODO Possibly consider actions for all cities instead of just the next one that isn't set yet
        if let Some(city_loc) = game.current_player_production_set_requests().next() {
            for unit_type in game.current_player_valid_productions_conservative(city_loc) {
                a.insert(AiPlayerAction::SetNextCityProduction { unit_type });
            }
        }

        //TODO Possibly consider actions for all units instead of just the next one that needs orders
        if let Some(unit_id) = game.current_player_unit_orders_requests().next() {
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
        let mut a = Vec::with_capacity(POSSIBLE_ACTIONS_USIZE);
        for unit_type in UnitType::values() {
            a.push(AiPlayerAction::SetNextCityProduction { unit_type });
        }
        for direction in Direction::values() {
            a.push(AiPlayerAction::MoveNextUnit { direction });
        }
        a.push(AiPlayerAction::DisbandNextUnit);
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
}

impl Actionable for AiPlayerAction {
    fn to_action(&self, game: &mut Game, secret: PlayerSecret) -> UmpireResult<PlayerAction> {
        Ok(match self {
            AiPlayerAction::SetNextCityProduction { unit_type } => {
                let city_loc = game.player_production_set_requests(secret)?.next().unwrap();

                let city_id = game.player_city_by_loc(secret, city_loc)?.unwrap().id;

                PlayerAction::SetCityProduction {
                    city_id,
                    production: *unit_type,
                }
            }
            AiPlayerAction::MoveNextUnit { direction } => {
                let unit_id = game.player_unit_orders_requests(secret)?.next().unwrap();

                debug_assert!({
                    let legal: HashSet<Direction> = game
                        .player_unit_legal_directions(secret, unit_id)?
                        .collect();

                    legal.contains(&direction)
                });

                PlayerAction::MoveUnitInDirection {
                    unit_id,
                    direction: *direction,
                }
            }
            AiPlayerAction::DisbandNextUnit => {
                let unit_id = game.player_unit_orders_requests(secret)?.next().unwrap();
                PlayerAction::DisbandUnit { unit_id }
            }
            AiPlayerAction::SkipNextUnit => {
                let unit_id = game.player_unit_orders_requests(secret)?.next().unwrap();
                PlayerAction::SkipUnit { unit_id }
            }
        })
    }
}

#[derive(Copy, Clone, Debug, Deserialize, Serialize)]
pub enum NextCityAction {
    SetProduction { unit_type: UnitType },
}

impl NextCityAction {
    /// Currently possible actions
    pub async fn legal(turn: &PlayerTurn<'_>) -> Vec<Self> {
        if let Some(city_loc) = turn.player_production_set_requests().await.iter().next() {
            turn.valid_productions_conservative(*city_loc)
                .await
                .iter()
                .copied()
                .map(|unit_type| Self::SetProduction { unit_type })
                .collect()
        } else {
            Vec::new() // no legal actions because there's no next city
        }
    }

    /// The number of possible city actions overall, regardless of current circumstances
    pub const fn possible() -> usize {
        UnitType::values().len()
    }
}

impl Actionable for NextCityAction {
    fn to_action(&self, game: &mut Game, secret: PlayerSecret) -> UmpireResult<PlayerAction> {
        let next_city_loc = game.player_production_set_requests(secret)?.next().unwrap();
        let next_city_id = game.player_city_by_loc(secret, next_city_loc)?.unwrap().id;
        Ok(match self {
            Self::SetProduction { unit_type } => PlayerAction::SetCityProduction {
                city_id: next_city_id,
                production: *unit_type,
            },
        })
    }
}

impl Into<AiPlayerAction> for NextCityAction {
    fn into(self) -> AiPlayerAction {
        match self {
            Self::SetProduction { unit_type } => {
                AiPlayerAction::SetNextCityProduction { unit_type }
            }
        }
    }
}

impl From<usize> for NextCityAction {
    fn from(idx: usize) -> Self {
        Self::SetProduction {
            unit_type: UnitType::values()[idx],
        }
    }
}

impl Into<usize> for NextCityAction {
    fn into(self) -> usize {
        match self {
            Self::SetProduction { unit_type } => UnitType::values()
                .iter()
                .position(|ut| *ut == unit_type)
                .unwrap(),
        }
    }
}

impl TryFrom<AiPlayerAction> for NextCityAction {
    type Error = ();
    fn try_from(action: AiPlayerAction) -> Result<Self, Self::Error> {
        match action {
            AiPlayerAction::SetNextCityProduction { unit_type } => {
                Ok(NextCityAction::SetProduction { unit_type })
            }
            _ => Err(()),
        }
    }
}

#[derive(Copy, Clone, Debug, Deserialize, Serialize)]
pub enum NextUnitAction {
    Move { direction: Direction },
    Disband,
    Skip,
}

impl NextUnitAction {
    /// Currently possible actions
    pub async fn legal(turn: &PlayerTurn<'_>) -> Vec<Self> {
        if let Some(unit_id) = turn.player_unit_orders_requests().await.iter().next() {
            // disband, skip, then any move actions
            [Self::Disband, Self::Skip]
                .iter()
                .copied()
                .chain(
                    turn.player_unit_legal_directions(*unit_id)
                        .await
                        .unwrap()
                        .iter()
                        .copied()
                        .map(|direction| Self::Move { direction }),
                )
                .collect()
        } else {
            Vec::new() // no legal actions because there's no next unit
        }
    }

    pub const fn possible() -> usize {
        Direction::values().len() + 2
    }
}

impl Actionable for NextUnitAction {
    fn to_action(&self, game: &mut Game, secret: PlayerSecret) -> UmpireResult<PlayerAction> {
        Ok(match self {
            Self::Move { direction } => {
                let unit_id = game.player_unit_orders_requests(secret)?.next().unwrap();
                debug_assert!({
                    let legal: HashSet<Direction> = game
                        .player_unit_legal_directions(secret, unit_id)?
                        .collect();

                    // println!("legal moves: {}", legal.len());

                    legal.contains(&direction)
                });

                PlayerAction::MoveUnitInDirection {
                    unit_id,
                    direction: *direction,
                }
            }
            Self::Disband => {
                let unit_id = game.player_unit_orders_requests(secret)?.next().unwrap();
                PlayerAction::DisbandUnit { unit_id }
            }
            Self::Skip => {
                let unit_id = game.player_unit_orders_requests(secret)?.next().unwrap();
                PlayerAction::OrderUnit {
                    unit_id,
                    orders: Orders::Skip,
                }
            }
        })
    }
}

impl Into<AiPlayerAction> for NextUnitAction {
    fn into(self) -> AiPlayerAction {
        match self {
            Self::Move { direction } => AiPlayerAction::MoveNextUnit { direction },
            Self::Disband => AiPlayerAction::DisbandNextUnit,
            Self::Skip => AiPlayerAction::SkipNextUnit,
        }
    }
}

impl From<usize> for NextUnitAction {
    fn from(idx: usize) -> Self {
        match idx {
            0 => Self::Disband,
            1 => Self::Skip,
            x => Self::Move {
                direction: Direction::values()[x - 2],
            },
        }
    }
}

impl TryFrom<AiPlayerAction> for NextUnitAction {
    type Error = ();
    fn try_from(action: AiPlayerAction) -> Result<Self, ()> {
        match action {
            AiPlayerAction::MoveNextUnit { direction } => Ok(NextUnitAction::Move { direction }),
            AiPlayerAction::DisbandNextUnit => Ok(NextUnitAction::Disband),
            AiPlayerAction::SkipNextUnit => Ok(NextUnitAction::Skip),
            _ => Err(()),
        }
    }
}

impl Into<usize> for NextUnitAction {
    fn into(self) -> usize {
        match self {
            Self::Disband => 0,
            Self::Skip => 1,
            Self::Move { direction } => {
                Direction::values()
                    .iter()
                    .position(|d| *d == direction)
                    .unwrap()
                    + 2
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub enum PlayerAction {
    BeginTurn {
        clear_after_unit_production: bool,
    },
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
    SkipUnit {
        unit_id: UnitID,
    },
}

impl Actionable for PlayerAction {
    fn to_action(&self, _game: &mut Game, _secret: PlayerSecret) -> UmpireResult<PlayerAction> {
        Ok(*self)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub enum PlayerActionOutcome {
    TurnStarted(TurnStart),
    TurnEnded,
    ProductionSet(ProductionSet),
    MoveUnit {
        unit_id: UnitID,
        /// When moving by direction, this could be None
        dest: Option<Location>,
        move_: Move,
    },
    OrderUnit {
        unit_id: UnitID,
        orders: Orders,
        orders_outcome: OrdersOutcome,
    },
    UnitDisbanded(UnitDisbanded),
    UnitSkipped {
        unit_id: UnitID,
        orders_outcome: OrdersSet,
    },
}

impl PlayerAction {
    pub fn take(
        self,
        game: &mut Game,
        player_secret: PlayerSecret,
    ) -> Result<PlayerActionOutcome, GameError> {
        match self {
            Self::BeginTurn {
                clear_after_unit_production,
            } => game
                .begin_turn(player_secret, clear_after_unit_production)
                .map(|turn_start| PlayerActionOutcome::TurnStarted(turn_start)),
            Self::EndTurn => game
                .end_turn(player_secret)
                .map(|_| PlayerActionOutcome::TurnEnded),
            Self::SetCityProduction {
                city_id,
                production,
            } => game
                .set_production_by_id(player_secret, city_id, production)
                .map(PlayerActionOutcome::ProductionSet),
            Self::MoveUnit { unit_id, dest } => game
                .move_unit_by_id(player_secret, unit_id, dest)
                .map(|move_| PlayerActionOutcome::MoveUnit {
                    unit_id,
                    dest: Some(dest),
                    move_,
                }),
            Self::MoveUnitInDirection { unit_id, direction } => {
                let dest = game
                    .current_player_unit_by_id(unit_id)
                    .unwrap()
                    .loc
                    .shift_wrapped(direction, game.dims(), game.wrapping);
                game.move_unit_by_id_in_direction(player_secret, unit_id, direction)
                    .map(|move_| PlayerActionOutcome::MoveUnit {
                        unit_id,
                        dest,
                        move_,
                    })
            }
            Self::DisbandUnit { unit_id } => game
                .disband_unit_by_id(player_secret, unit_id)
                .map(PlayerActionOutcome::UnitDisbanded),
            Self::OrderUnit { unit_id, orders } => game
                .set_and_follow_orders(player_secret, unit_id, orders)
                .map(|orders_outcome| PlayerActionOutcome::OrderUnit {
                    unit_id,
                    orders,
                    orders_outcome,
                }),
            Self::SkipUnit { unit_id } => {
                game.order_unit_skip(player_secret, unit_id)
                    .map(|orders_outcome| PlayerActionOutcome::UnitSkipped {
                        unit_id,
                        orders_outcome,
                    })
            }
        }
    }
}
