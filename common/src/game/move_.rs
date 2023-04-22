use serde::{Deserialize, Serialize};

use thiserror::Error;

use crate::{
    game::{
        city::City,
        combat::CombatOutcome,
        obs::LocatedObs,
        unit::{Unit, UnitID},
    },
    util::Location,
};

pub type MoveResult = Result<Move, MoveError>;

/// A move.
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Move {
    /// The unit as it will be at the end of the proposed move
    pub unit: Unit,

    /// The unit's starting location in the proposed move
    pub starting_loc: Location,

    /// The components of the proposed move
    pub components: Vec<MoveComponent>,
}
impl Move {
    /// unit represents the unit _after_ the move is completed
    pub fn new(unit: Unit, starting_loc: Location, components: Vec<MoveComponent>) -> MoveResult {
        if components.is_empty() {
            Err(MoveError::ZeroLengthMove)
        } else {
            Ok(Self {
                unit,
                starting_loc,
                components,
            })
        }
    }

    /// Did the unit survive the move?
    pub fn moved_successfully(&self) -> bool {
        self.components
            .iter()
            .map(MoveComponent::moved_successfully)
            .all(|success| success)
    }

    /// The city conquered at the end of this move, if any
    pub fn conquered_city(&self) -> Option<&City> {
        if let Some(move_) = self.components.last() {
            if let Some(city_combat) = move_.city_combat.as_ref() {
                if city_combat.victorious() {
                    return Some(city_combat.defender());
                }
            }
        }

        None
    }

    /// If the unit survived to the end of the move, its destination
    pub fn ending_loc(&self) -> Option<Location> {
        if self.moved_successfully() {
            self.components.last().map(|move_| move_.loc)
            // Some(self.moves.last().unwrap().loc)
        } else {
            None
        }
    }

    /// If the unit survived to the end of the move, which (if any) unit ended up carrying it?
    pub fn ending_carrier(&self) -> Option<UnitID> {
        if self.moved_successfully() {
            if let Some(move_) = self.components.last() {
                move_.carrier
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.components.len()
    }

    pub fn distance_moved(&self) -> usize {
        self.components.iter().map(|mc| mc.distance_moved()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }

    /// All observations made during the move
    ///
    /// Earliest first, latest last
    ///
    /// Could include locations observed more than once, you'll want to dedupe
    pub fn observations<'a>(&'a self) -> impl Iterator<Item = &LocatedObs> + 'a {
        self.components
            .iter()
            .flat_map(|c| c.observations_after_move.iter())
    }

    /// Did the move end with the unit running out of fuel?
    pub fn fuel_ran_out(&self) -> bool {
        self.components
            .last()
            .map(|move_| move_.fuel_ran_out)
            .unwrap_or(false)
    }
}

//FIXME The name is a misnomer---UnitAction or something would be more accurate
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct MoveComponent {
    pub prev_loc: Location,
    pub loc: Location,
    /// Was the unit carried by another unit? If so, which one?
    pub carrier: Option<UnitID>,
    pub unit_combat: Option<CombatOutcome<Unit, Unit>>,
    pub city_combat: Option<CombatOutcome<Unit, City>>,
    pub observations_after_move: Vec<LocatedObs>,

    /// Flag to mark after the fact whether fuel ran out in this move
    pub fuel_ran_out: bool,
}
impl MoveComponent {
    pub fn new(prev_loc: Location, loc: Location) -> Self {
        MoveComponent {
            prev_loc,
            loc,
            carrier: None,
            unit_combat: None,
            city_combat: None,
            observations_after_move: Vec::with_capacity(0),
            fuel_ran_out: false,
        }
    }

    /// Did the unit survive the move and combat represented by this component?
    ///
    /// Running out of fuel after moving counts as _not_ surviving.
    pub fn moved_successfully(&self) -> bool {
        if let Some(ref combat) = self.unit_combat {
            if combat.destroyed() {
                return false;
            }
        }
        if let Some(ref combat) = self.city_combat {
            if combat.destroyed() {
                return false;
            }
        }

        if self.fuel_ran_out {
            return false;
        }

        true
    }

    /// How far did the unit move? Either 0 or 1---0 if it stayed in the same place,
    /// 1 otherwise.
    ///
    /// NOTE: Assumes that all moves are of distance 1 or less
    pub fn distance_moved(&self) -> usize {
        if self.prev_loc == self.loc {
            0
        } else {
            1
        }
    }
}

#[derive(Debug, Deserialize, Error, PartialEq, Serialize)]
pub enum MoveError {
    #[error("Cannot execute a move of length zero")]
    ZeroLengthMove,

    #[error("Ordered move of unit spans a distance ({intended_distance}) greater than the number of moves remaining ({moves_remaining})")]
    RemainingMovesExceeded {
        intended_distance: u16,
        moves_remaining: u16,
    },

    #[error("Cannot move unit at source location {src} because there is no unit there")]
    SourceUnitNotAtLocation { src: Location },

    #[error("Cannot move unit with ID {id:?} because none exists")]
    SourceUnitDoesNotExist { id: UnitID },

    #[error(
        "Cannot move unit at source location {src} with ID {id:?} becuase no such unit exists"
    )]
    SourceUnitWithIdNotAtLocation { id: UnitID, src: Location },

    #[error("No route from {src} to {dest} for unit with ID {id:?}")]
    NoRoute {
        id: UnitID,
        src: Location,
        dest: Location,
    },

    #[error("Destination out of bounds")]
    DestinationOutOfBounds,

    #[error("Insufficient fuel")]
    InsufficientFuel,
}
