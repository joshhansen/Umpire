use std::{
    collections::HashSet,
};

use failure::{
    Fail,
};

use crate::{
    game::{
        city::City,
        combat::CombatOutcome,
        obs::LocatedObs,
        unit::{
            UnitID,Unit,
        },
    },
    util::{Dims,Location},
};

#[derive(Debug,PartialEq)]
pub struct Move {
    pub unit: Unit,
    pub starting_loc: Location,
    pub moves: Vec<MoveComponent>
}
impl Move {
    /// unit represents the unit _after_ the move is completed
    pub fn new(unit: Unit, starting_loc: Location, moves: Vec<MoveComponent>) -> MoveResult {
        if moves.is_empty() {
            Err(MoveError::ZeroLengthMove)
        } else {
            Ok(Self{unit, starting_loc, moves})
        }
    }

    pub fn moved_successfully(&self) -> bool {
        self.moves.iter().map(MoveComponent::moved_successfully).all(|success| success)
    }

    /// The city conquered at the end of this move, if any
    pub fn conquered_city(&self) -> Option<&City> {
        if let Some(move_) = self.moves.last() {
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
            self.moves.last().map(|move_| move_.loc)
            // Some(self.moves.last().unwrap().loc)
        } else {
            None
        }
    }

    /// If the unit survived to the end of the move, which (if any) unit ended up carrying it?
    pub fn ending_carrier(&self) -> Option<UnitID> {
        if self.moved_successfully() {
            if let Some(move_) = self.moves.last() {
                move_.carrier
            } else {
                None
            }
        } else {
            None
        }
    }
}

#[derive(Debug,PartialEq)]
pub struct MoveComponent {
    pub loc: Location,
    /// Was the unit carried by another unit? If so, which one?
    pub carrier: Option<UnitID>,
    pub unit_combat: Option<CombatOutcome<Unit,Unit>>,
    pub city_combat: Option<CombatOutcome<Unit,City>>,
    pub observations_after_move: Vec<LocatedObs>,
}
impl MoveComponent {
    pub fn new(loc: Location) -> Self {
        MoveComponent {
            loc,
            carrier: None,
            unit_combat: None,
            city_combat: None,
            observations_after_move: Vec::with_capacity(0),
        }
    }

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
}

#[derive(Debug,Fail,PartialEq)]
pub enum MoveError {
    #[fail(display="Cannot execute a move of length zero")]
    ZeroLengthMove,

    #[fail(display="Ordered move of unit with ID {:?} from {} to {} spans a distance ({}) greater than the number of moves remaining ({})",
                    id, src, dest, intended_distance, moves_remaining)]
    RemainingMovesExceeded {
        id: UnitID,
        src: Location,
        dest: Location,
        intended_distance: u16,
        moves_remaining: u16,
    },

    #[fail(display="Cannot move unit at source location {} with ID {:?} because none exists", src_loc, id)]
    SourceUnitDoesNotExist {
        src_loc: Location,
        id: UnitID,
    },

    #[fail(display="No route from {} to {} for unit with ID {:?}", src, dest, id)]
    NoRoute {
        id: UnitID,
        src: Location,
        dest: Location,
    },

    #[fail(display="Destination {} lies outside of bounds {}", dest, bounds)]
    DestinationOutOfBounds {
        dest: Location,
        bounds: Dims,
    }
}

pub type MoveResult = Result<Move,MoveError>;