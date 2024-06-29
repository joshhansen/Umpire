//! Reified player actions

use std::{collections::BTreeSet, fmt::Display};

use serde::{Deserialize, Serialize};

use crate::util::{Direction, Location};

use super::{
    ai::POSSIBLE_ACTIONS,
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
    City(NextCityAction),
    Unit(NextUnitAction),
    //NOTE When adding new action types, make sure to add them to `possible_actions` in agz.rs
}

impl AiPlayerAction {
    pub fn city_action(&self) -> bool {
        match self {
            Self::City(_) => true,
            Self::Unit(_) => false,
        }
    }
    pub fn unit_action(&self) -> bool {
        !self.city_action()
    }

    /// All actions possible in general---not specific to any particular game state
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
    pub const POSSIBLE: [Self; POSSIBLE_ACTIONS] = [
        AiPlayerAction::City(NextCityAction::SetProduction {
            unit_type: UnitType::Infantry,
        }),
        AiPlayerAction::City(NextCityAction::SetProduction {
            unit_type: UnitType::Armor,
        }),
        AiPlayerAction::City(NextCityAction::SetProduction {
            unit_type: UnitType::Fighter,
        }),
        AiPlayerAction::City(NextCityAction::SetProduction {
            unit_type: UnitType::Bomber,
        }),
        AiPlayerAction::City(NextCityAction::SetProduction {
            unit_type: UnitType::Transport,
        }),
        AiPlayerAction::City(NextCityAction::SetProduction {
            unit_type: UnitType::Destroyer,
        }),
        AiPlayerAction::City(NextCityAction::SetProduction {
            unit_type: UnitType::Submarine,
        }),
        AiPlayerAction::City(NextCityAction::SetProduction {
            unit_type: UnitType::Cruiser,
        }),
        AiPlayerAction::City(NextCityAction::SetProduction {
            unit_type: UnitType::Battleship,
        }),
        AiPlayerAction::City(NextCityAction::SetProduction {
            unit_type: UnitType::Carrier,
        }),
        AiPlayerAction::Unit(NextUnitAction::Move {
            direction: Direction::Up,
        }),
        AiPlayerAction::Unit(NextUnitAction::Move {
            direction: Direction::Down,
        }),
        AiPlayerAction::Unit(NextUnitAction::Move {
            direction: Direction::Left,
        }),
        AiPlayerAction::Unit(NextUnitAction::Move {
            direction: Direction::Right,
        }),
        AiPlayerAction::Unit(NextUnitAction::Move {
            direction: Direction::UpLeft,
        }),
        AiPlayerAction::Unit(NextUnitAction::Move {
            direction: Direction::UpRight,
        }),
        AiPlayerAction::Unit(NextUnitAction::Move {
            direction: Direction::DownLeft,
        }),
        AiPlayerAction::Unit(NextUnitAction::Move {
            direction: Direction::DownRight,
        }),
        AiPlayerAction::Unit(NextUnitAction::Disband),
        AiPlayerAction::Unit(NextUnitAction::Skip),
    ];
}

impl Display for AiPlayerAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::City(a) => match a {
                NextCityAction::SetProduction { unit_type } => {
                    f.write_str(unit_type.key().to_string().as_str())
                }
            },
            Self::Unit(a) => match a {
                NextUnitAction::Move { direction } => {
                    f.write_str(direction.sym().to_string().as_str())
                }
                NextUnitAction::Disband => f.write_str("D"),
                NextUnitAction::Skip => f.write_str("S"),
            },
        }
    }
}

impl From<usize> for AiPlayerAction {
    fn from(idx: usize) -> Self {
        Self::POSSIBLE[idx]
    }
}

impl From<AiPlayerAction> for usize {
    fn from(a: AiPlayerAction) -> Self {
        AiPlayerAction::POSSIBLE
            .into_iter()
            .position(|b| a == b)
            .unwrap()
    }
}

impl Actionable for AiPlayerAction {
    fn to_action(&self, game: &mut Game, secret: PlayerSecret) -> UmpireResult<PlayerAction> {
        Ok(match self {
            Self::City(city_action) => match city_action {
                NextCityAction::SetProduction { unit_type } => {
                    let city_loc = game.player_production_set_requests(secret)?.next().unwrap();

                    let city_id = game.player_city_by_loc(secret, city_loc)?.unwrap().id;

                    PlayerAction::SetCityProduction {
                        city_id,
                        production: *unit_type,
                    }
                }
            },
            Self::Unit(unit_action) => match unit_action {
                NextUnitAction::Move { direction } => {
                    let unit_id = game.player_unit_orders_requests(secret)?.next().unwrap();

                    debug_assert!({
                        let legal: BTreeSet<Direction> = game
                            .player_unit_legal_directions(secret, unit_id)?
                            .collect();

                        legal.contains(direction)
                    });

                    PlayerAction::MoveUnitInDirection {
                        unit_id,
                        direction: *direction,
                    }
                }
                NextUnitAction::Disband => {
                    let unit_id = game.player_unit_orders_requests(secret)?.next().unwrap();
                    PlayerAction::DisbandUnit { unit_id }
                }
                NextUnitAction::Skip => {
                    let unit_id = game.player_unit_orders_requests(secret)?.next().unwrap();
                    PlayerAction::SkipUnit { unit_id }
                }
            },
        })
    }
}

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, Deserialize, Serialize)]
pub enum NextCityAction {
    SetProduction { unit_type: UnitType },
}

impl NextCityAction {
    /// Currently possible actions
    pub async fn legal(turn: &PlayerTurn<'_>) -> Vec<Self> {
        if let Some(city_loc) = turn.player_production_set_requests().await.first() {
            turn.valid_productions_conservative(*city_loc)
                .await
                .into_iter()
                .map(|unit_type| Self::SetProduction { unit_type })
                .collect()
        } else {
            Vec::with_capacity(0) // no legal actions because there's no next city
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
        AiPlayerAction::City(self)
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
            AiPlayerAction::City(city_action) => Ok(city_action),
            _ => Err(()),
        }
    }
}

impl Ord for NextCityAction {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self {
            Self::SetProduction {
                unit_type: unit_type1,
            } => match other {
                Self::SetProduction {
                    unit_type: unit_type2,
                } => unit_type1.cmp(unit_type2),
            },
        }
    }
}

impl PartialOrd for NextCityAction {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(<Self as Ord>::cmp(self, other))
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize, Serialize)]
pub enum NextUnitAction {
    Move { direction: Direction },
    Disband,
    Skip,
}

impl NextUnitAction {
    /// Currently possible actions
    pub async fn legal(turn: &PlayerTurn<'_>) -> Vec<Self> {
        if let Some(unit_id) = turn.player_unit_orders_requests().await.first() {
            // disband, skip, then any move actions
            [Self::Disband, Self::Skip]
                .into_iter()
                .chain(
                    turn.player_unit_legal_directions(*unit_id)
                        .await
                        .unwrap()
                        .into_iter()
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
                    let legal: BTreeSet<Direction> = game
                        .player_unit_legal_directions(secret, unit_id)?
                        .collect();

                    // println!("legal moves: {}", legal.len());

                    legal.contains(direction)
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
        AiPlayerAction::Unit(self)
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
            AiPlayerAction::Unit(unit_action) => Ok(unit_action),
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
                .map(PlayerActionOutcome::TurnStarted),
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
