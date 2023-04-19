use failure::Fail;

use serde::{Deserialize, Serialize};

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
        }
    }

    /// Did the unit survive the move and combat represented by this component?
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

#[derive(Debug, Deserialize, Fail, PartialEq, Serialize)]
pub enum MoveError {
    #[fail(display = "Cannot execute a move of length zero")]
    ZeroLengthMove,

    #[fail(
        display = "Ordered move of unit with ID {:?} from {} to {} spans a distance ({}) greater than the number of moves remaining ({})",
        id, src, dest, intended_distance, moves_remaining
    )]
    RemainingMovesExceeded {
        id: UnitID,
        src: Location,
        dest: Location,
        intended_distance: u16,
        moves_remaining: u16,
    },

    #[fail(
        display = "Cannot move unit at source location {} because there is no unit there",
        src
    )]
    SourceUnitNotAtLocation { src: Location },

    #[fail(display = "Cannot move unit with ID {:?} because none exists", id)]
    SourceUnitDoesNotExist { id: UnitID },

    #[fail(
        display = "Cannot move unit at source location {} with ID {:?} becuase no such unit exists",
        src, id
    )]
    SourceUnitWithIdNotAtLocation { id: UnitID, src: Location },

    #[fail(
        display = "No route from {} to {} for unit with ID {:?}",
        src, dest, id
    )]
    NoRoute {
        id: UnitID,
        src: Location,
        dest: Location,
    },

    // #[fail(display="Destination {} lies outside of bounds {}", dest, bounds)]
    #[fail(display = "Destination out of bounds")]
    DestinationOutOfBounds {
        // dest: Location,
        // bounds: Dims,
    },

    #[fail(display = "Insufficient fuel")]
    InsufficientFuel,
}
