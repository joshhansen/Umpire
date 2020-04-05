use failure::{
    Fail,
};

use crate::{
    game::{
        Game,
        ProposedAction,
        city::City,
        combat::CombatOutcome,
        obs::{
            LocatedObs,
            Observer,
        },
        unit::{
            UnitID,Unit,
        },
    },
    util::Location,
};

pub type MoveResult = Result<Move,MoveError>;
pub type ProposedMoveResult = Result<ProposedMove,MoveError>;


/// A move.
/// 
/// Returned by `ProposedMove::make`
#[derive(Debug,PartialEq)]
pub struct Move {
    /// The unit as it will be at the end of the proposed move
    pub unit: Unit,

    /// The unit's starting location in the proposed move
    pub starting_loc: Location,

    /// The components of the proposed move
    pub components: Vec<MoveComponent>
}
impl Move {
    /// unit represents the unit _after_ the move is completed
    pub fn new(unit: Unit, starting_loc: Location, components: Vec<MoveComponent>) -> MoveResult {
        if components.is_empty() {
            Err(MoveError::ZeroLengthMove)
        } else {
            Ok(Self{unit, starting_loc, components})
        }
    }

    /// Did the unit survive the move?
    pub fn moved_successfully(&self) -> bool {
        self.components.iter().map(MoveComponent::moved_successfully).all(|success| success)
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
}


/// A move that has been simulated and contemplated but not yet carried out
#[deprecated]
#[derive(Debug,PartialEq)]
pub struct ProposedMove(pub Move);

impl ProposedMove {
    pub fn new(unit: Unit, starting_loc: Location, components: Vec<MoveComponent>) -> ProposedMoveResult {
        Move::new(unit, starting_loc, components).map(ProposedMove)
    }
}

impl ProposedAction for ProposedMove {
    type Outcome = Move;
    fn take(self, game: &mut Game) -> Self::Outcome {
        let mut move_ = self.0;

        // Pop the unit from the game itself (not this clone)
        game.map.pop_unit_by_loc_and_id(move_.starting_loc, move_.unit.id).unwrap();

        let unit = &mut move_.unit;

        // let mut moves = Vec::new();

        // Move along the shortest path to the destination
        // At each tile along the path, check if there's a unit there
        // If so, battle it
        // If we lose, this unit is destroyed
        // If we win, the opposing unit is destroyed and this unit continues its journey
        //     battling if necessary until it is either destroyed or reaches its destination
        //
        // Observe that the unit will either make it all the way to its destination, or
        // will be destroyed somewhere along the way. There will be no stopping midway.

        // let mut conquered_city = false;

        // let mut it = shortest_path.iter();
        // let first_loc = it.next().unwrap();// skip the source location
        // debug_assert_eq!(src, *first_loc);
        // for loc in it {
        //     moves.push(MoveComponent::new(*loc));
        //     let mut move_ = moves.last_mut().unwrap();

        for move_component in &move_.components {

            let loc = move_component.loc;

            // // let mut dest_tile = &mut self.tiles[*loc];
            // // debug_assert_eq!(dest_tile.loc, *loc);
            // if let Some(ref other_unit) = self.map.toplevel_unit_by_loc(*loc) {
            //     if unit.is_friendly_to(other_unit) {
            //         // the friendly unit must have space for us in its carrying capacity or else the
            //         // path search wouldn't have included it
            //         // We won't actually insert this unit in the space yet since it might move/get destroyed later
            //         move_.carrier = Some(other_unit.id);
            //     } else {
            //         // On the other hand, we fight any unfriendly units
            //         move_.unit_combat = Some(unit.fight(other_unit));
            //     }
            // }
            if let Some(outcome) = move_component.unit_combat.as_ref() {
                if outcome.destroyed() {
                    break;
                } else {
                    game.map.pop_toplevel_unit_by_loc(loc).unwrap();// eliminate the unit we conquered
                }
            }

            if let Some(city_combat) = move_component.city_combat.as_ref() {
                
                if city_combat.victorious() {
                    game.map.set_city_alignment_by_loc(loc, unit.alignment).unwrap();
                    game.map.clear_city_production_without_ignoring_by_loc(loc).unwrap();
                    // let mut city = game.map.city_by_loc_mut(loc).unwrap();
                    // city.alignment = unit.alignment;
                    // city.clear_production_without_ignoring();
                }
            }

            // if let Some(city) = game.map.city_by_loc_mut(loc) {
            //     if city.alignment != unit.alignment {
            //         let outcome = unit.fight(city);

            //         if outcome.victorious() {
            //             city.alignment = unit.alignment;
            //             city.clear_production_without_ignoring();
            //         }

            //         move_.city_combat = Some(outcome);

            //         conquered_city = true;

            //         break;// break regardless of outcome. Either conquer a city and stop, or be destroyed
            //     }
            // }
        }

        // if move_.conquered_city().is_some() {
        //     unit.movement_complete();
        // } else {
        //     unit.record_movement(move_.len() as u16).unwrap();
        // }

        if let Some(move_component) = move_.components.last() {
            if move_component.moved_successfully() {
                if let Some(carrier_unit_id) = move_component.carrier {
                    game.map.carry_unit(carrier_unit_id, unit.clone()).unwrap();
                } else {
                    let dest = move_component.loc;
                    game.map.set_unit(dest, unit.clone());
                }
            }
        }

        for move_component in move_.components.iter() {
            if move_component.moved_successfully() {
                let obs_tracker = game.player_observations.get_mut(&game.current_player()).unwrap();
                unit.loc = move_component.loc;
                unit.observe(&game.map, game.turn, game.wrapping, obs_tracker);
            }
        }

        // Move::new(unit, src, moves)

        move_
    }
}

//FIXME The name is a misnomer---UnitAction or something would be more accurate
#[derive(Debug,PartialEq)]
pub struct MoveComponent {
    pub prev_loc: Location,
    pub loc: Location,
    /// Was the unit carried by another unit? If so, which one?
    pub carrier: Option<UnitID>,
    pub unit_combat: Option<CombatOutcome<Unit,Unit>>,
    pub city_combat: Option<CombatOutcome<Unit,City>>,
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

    #[fail(display="Cannot move unit at source location {} because there is no unit there", src)]
    SourceUnitNotAtLocation {
        src: Location,
    },

    #[fail(display="Cannot move unit with ID {:?} because none exists", id)]
    SourceUnitDoesNotExist {
        id: UnitID,
    },

    #[fail(display="Cannot move unit at source location {} with ID {:?} becuase no such unit exists", src, id)]
    SourceUnitWithIdNotAtLocation {
        id: UnitID,
        src: Location,
    },

    #[fail(display="No route from {} to {} for unit with ID {:?}", src, dest, id)]
    NoRoute {
        id: UnitID,
        src: Location,
        dest: Location,
    },

    // #[fail(display="Destination {} lies outside of bounds {}", dest, bounds)]
    #[fail(display="Destination out of bounds")]
    DestinationOutOfBounds {
        // dest: Location,
        // bounds: Dims,
    }
}