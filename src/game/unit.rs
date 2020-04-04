//! Abstract representation of units and cities and their interactions.

pub mod orders;

use std::cmp::Ordering;
use std::fmt;

use crate::{
    color::{Colorized,Colors},
    game::{
        Aligned,
        Alignment,
        GameError,
        combat::CombatCapable,
        map::{
            Terrain,
            Tile,
        },
        obs::Observer,
    },
    name::Named,
    util::{
        Location,
        Located,
    },
};

use self::orders::Orders;

#[derive(Clone,Copy,Debug,Eq,Hash,Ord,PartialEq,PartialOrd)]
pub struct UnitID {
    id: u64
}
impl UnitID {
    pub fn new(id: u64) -> Self {
        Self{ id }
    }
    pub fn next(self) -> Self {
        UnitID{ id: self.id + 1 }
    }
}

impl Default for UnitID {
    fn default() -> Self {
        UnitID::new(0)
    }
}

#[derive(Clone,Debug,PartialEq)]
enum TransportMode {
    Land,
    Sea,
    Air,
}
impl TransportMode {
    /// Determine whether a unit with this transport mode can operate on terrain of the given type
    pub fn can_traverse(&self, terrain: Terrain) -> bool {
        match self {
            TransportMode::Land => terrain==Terrain::Land,
            TransportMode::Sea  => terrain==Terrain::Water,
            TransportMode::Air  => terrain==Terrain::Land || terrain==Terrain::Water,
        }
    }
}

#[derive(Clone)]
struct CarryingSpaceEssentials {
    capacity: usize,
    space: Vec<UnitEssentials>,
}

#[derive(Clone,Debug,PartialEq)]
struct CarryingSpace {
    owner: Alignment,
    accepted_transport_mode: TransportMode,
    capacity: usize,
    space: Vec<Unit>,
}
impl CarryingSpace {
    fn new(owner: Alignment, accepted_transport_mode: TransportMode, capacity: usize) -> Self {
        Self {
            owner,
            accepted_transport_mode,
            capacity,
            space: Vec::with_capacity(capacity),
        }
    }

    fn can_carry_unit(&self, unit: &Unit) -> bool {
        self.owner == unit.alignment                                &&
        unit.type_.transport_mode() == self.accepted_transport_mode &&
        self.space.len() < self.capacity
    }

    fn carry(&mut self, unit: Unit) -> Result<usize,GameError> {
        if !self.can_carry_unit(&unit) {
            return Err(GameError::CannotCarryUnit{carried_id: unit.id});
        }

        self.space.push(unit);
        Ok(self.space.len()-1)
    }

    fn release_by_id(&mut self, id: UnitID) -> Option<Unit> {
        self.space.iter()
            .position(|carried_unit| carried_unit.id==id)
            .map(|carried_unit_idx| self.space.remove(carried_unit_idx))
    }

    fn units_held(&self) -> usize {
        self.space.len()
    }

    fn carried_units(&self) -> impl Iterator<Item=&Unit> {
        self.space.iter()
    }

    fn carried_units_mut(&mut self) -> impl Iterator<Item=&mut Unit> {
        self.space.iter_mut()
    }
}

#[derive(Clone,Copy,Debug,Hash,PartialEq,Eq)]
pub enum UnitType {
    Infantry,
    Armor,
    Fighter,
    Bomber,
    Transport,
    Destroyer,
    Submarine,
    Cruiser,
    Battleship,
    Carrier
}

impl UnitType {
    pub const fn values() -> [UnitType;10] {
        [
            UnitType::Infantry,
            UnitType::Armor,
            UnitType::Fighter,
            UnitType::Bomber,
            UnitType::Transport,
            UnitType::Destroyer,
            UnitType::Submarine,
            UnitType::Cruiser,
            UnitType::Battleship,
            UnitType::Carrier
        ]
    }

    pub fn max_hp(self) -> u16 {
        match self {
            UnitType::Infantry | UnitType::Fighter | UnitType::Bomber => 1,
            UnitType::Armor | UnitType::Destroyer | UnitType::Submarine => 2,
            UnitType::Transport => 3,
            UnitType::Cruiser => 4,
            UnitType::Battleship => 8,
            UnitType::Carrier => 6
        }
    }

    pub fn cost(self) -> u16 {
        match self {
            UnitType::Infantry => 6,
            UnitType::Armor | UnitType::Fighter | UnitType::Bomber => 12,
            UnitType::Transport => 30,
            UnitType::Destroyer | UnitType::Submarine=> 24,
            UnitType::Cruiser => 36,
            UnitType::Battleship => 60,
            UnitType::Carrier => 48
        }
    }

    pub fn key(self) -> char {
        match self {
            UnitType::Infantry => 'i',
            UnitType::Armor => 'a',
            UnitType::Fighter => 'f',
            UnitType::Bomber => 'b',
            UnitType::Transport => 't',
            UnitType::Destroyer => 'd',
            UnitType::Submarine => 's',
            UnitType::Cruiser => 'c',
            UnitType::Battleship => 'p',
            UnitType::Carrier => 'k'
        }
    }

    pub fn sight_distance(self) -> u16 {
        match self {
            UnitType::Infantry | UnitType::Armor | UnitType::Transport => 2,
            UnitType::Destroyer | UnitType::Submarine | UnitType::Cruiser => 3,
            UnitType::Fighter | UnitType::Bomber | UnitType::Battleship | UnitType::Carrier => 4,
        }
    }

    pub fn try_from_key(c: char) -> Result<UnitType,()> {
        for unit_type in &UnitType::values() {
            if unit_type.key() == c {
                return Ok(*unit_type);
            }
        }
        Err(())
    }

    /// Determine whether a unit of this type could potentially move to a particular tile
    /// (maybe requiring combat to do so).
    /// 
    /// If a city is present, this will always be true. Otherwise, it will be determined by the match between
    /// the unit's capabilities and the terrain (e.g. planes over water, but not tanks over water).
    pub fn can_move_on_tile(self, tile: &Tile) -> bool {
        tile.city.is_some() || self.transport_mode().can_traverse(tile.terrain)
    }

    /// Determine whether a unit of this type could actually occupy a particular tile (maybe requiring combat to do so).
    pub fn can_occupy_tile(self, tile: &Tile) -> bool {
        if tile.city.is_some() {
            self.can_occupy_cities()
        } else {
            self.transport_mode().can_traverse(tile.terrain)
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            UnitType::Infantry => "Infantry",
            UnitType::Armor => "Armor",
            UnitType::Fighter => "Fighter",
            UnitType::Bomber => "Bomber",
            UnitType::Transport => "Transport",
            UnitType::Destroyer => "Destroyer",
            UnitType::Submarine => "Submarine",
            UnitType::Cruiser => "Cruiser",
            UnitType::Battleship => "Battleship",
            UnitType::Carrier => "Carrier"
        }
    }

    fn transport_mode(self) -> TransportMode {
        match self {
            UnitType::Infantry | UnitType::Armor => TransportMode::Land,
            UnitType::Fighter | UnitType::Bomber => TransportMode::Air,
            UnitType::Transport | UnitType::Destroyer | UnitType::Submarine | UnitType::Cruiser |
                UnitType::Battleship | UnitType::Carrier => TransportMode::Sea,
        }
    }

    /// Can this type of unit occupy cities?
    pub fn can_occupy_cities(self) -> bool {
        self.transport_mode() == TransportMode::Land
    }

    pub fn movement_per_turn(&self) -> u16 {
        match self {
            UnitType::Infantry | UnitType::Battleship | UnitType::Carrier => 1,
            UnitType::Armor | UnitType::Transport | UnitType::Submarine | UnitType::Cruiser => 2,
            UnitType::Destroyer => 3,
            UnitType::Fighter | UnitType::Bomber => 5,
        }
    }
}

impl fmt::Display for UnitType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl PartialOrd for UnitType {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.cost().partial_cmp(&other.cost())
    }
}

impl Ord for UnitType {
    fn cmp(&self, other: &Self) -> Ordering {
        let cmp = self.cost().cmp(&other.cost());
        if cmp == Ordering::Equal {
            self.key().cmp(&other.key())
        } else {
            cmp
        }
    }
}

#[derive(Clone)]
pub struct UnitEssentials {
    pub type_: UnitType,
    pub alignment: Alignment,
    hp: u16,
    max_hp: u16,
    moves_remaining: u16,
    // pub orders: Option<Orders>,
    carrying_space: Option<CarryingSpaceEssentials>,
}

#[derive(Clone,Debug,PartialEq)]
pub struct Unit {
    pub id: UnitID,
    pub loc: Location,
    pub type_: UnitType,
    pub alignment: Alignment,
    hp: u16,
    max_hp: u16,
    moves_remaining: u16,
    name: String,
    pub orders: Option<Orders>,
    carrying_space: Option<CarryingSpace>,
}

impl Unit {
    pub fn new<S:Into<String>>(id: UnitID, loc: Location, type_: UnitType, alignment: Alignment, name: S) -> Self {
        let max_hp =type_.max_hp();
        Unit {
            id,
            loc,
            type_,
            alignment,
            hp: max_hp,
            max_hp,
            moves_remaining: 0,
            name: name.into(),
            orders: None,
            carrying_space: Self::new_carrying_space_for(type_, alignment),
        }
    }

    pub fn movement_per_turn(&self) -> u16 {
        self.type_.movement_per_turn()
    }

    /// Indicate whether this unit can move (if only theoretically) onto a given tile.
    ///
    /// This is determined as follows:
    ///
    /// If the tile contains a unit:
    ///    if the unit is unfriendly, then defer to terrain features / city presence
    ///    if the unit is friendly:
    ///        if the unit has appropriate carrying space for this unit, then we can move on the tile
    ///        otherwise, we cannot move on the tile
    /// otherwise, defer to terrain features / city presence
    pub fn can_move_on_tile(&self, tile: &Tile) -> bool {

        if let Some(ref unit) = tile.unit {
            if self.alignment != unit.alignment {
                self.type_.can_move_on_tile(tile)
            } else {
                unit.can_carry_unit(self)
            }
        } else {
            self.type_.can_move_on_tile(tile)
        }
    }

    pub fn moves_remaining(&self) -> u16 {
        self.moves_remaining
    }

    pub(in crate::game) fn movement_complete(&mut self) {
        self.moves_remaining = 0;
    }

    pub(in crate::game) fn record_movement(&mut self, moves: u16) -> Result<u16,String> {
        if self.moves_remaining >= moves {
            self.moves_remaining -= moves;
            Ok(self.moves_remaining)
        } else {
            Err(format!("Could not move {} moves because only {} remain", moves, self.moves_remaining))
        }
    }

    pub(in crate::game) fn refresh_moves_remaining(&mut self) {
        self.moves_remaining = self.movement_per_turn();
    }
    
    fn new_carrying_space_for(type_: UnitType, alignment: Alignment) -> Option<CarryingSpace> {
        match type_ {
            UnitType::Infantry | UnitType::Armor | UnitType::Fighter | UnitType::Bomber | UnitType::Destroyer |
            UnitType::Submarine | UnitType::Cruiser | UnitType::Battleship => None,
            UnitType::Transport => Some(CarryingSpace::new(alignment, TransportMode::Land, 4)),
            UnitType::Carrier => Some(CarryingSpace::new(alignment, TransportMode::Air, 5)),
        }
    }

    fn can_carry_unit(&self, unit: &Unit) -> bool {
        if let Some(ref carrying_space) = self.carrying_space {
            carrying_space.can_carry_unit(unit)
        } else {
            false
        }
    }

    /// Carry a unit in this unit's carrying space.
    /// 
    /// For example, make a Transport carry an Armor.
    /// 
    /// This method call should only be called by MapData's carry_unit method.
    pub(in crate::game) fn carry(&mut self, mut unit: Unit) -> Result<usize,GameError> {
        if let Some(ref mut carrying_space) = self.carrying_space {
            unit.loc = self.loc;
            carrying_space.carry(unit)
        } else {
            Err(GameError::UnitHasNoCarryingSpace{id: self.id})
        }
    }

    pub(in crate::game) fn release_by_id(&mut self, carried_unit_id: UnitID) -> Option<Unit> {
        if let Some(ref mut carrying_space) = self.carrying_space {
            carrying_space.release_by_id(carried_unit_id)
        } else {
            None
        }
    }

    pub fn carried_units(&self) -> impl Iterator<Item=&Unit> {
        self.carrying_space.iter().flat_map(|carrying_space| carrying_space.carried_units())
    }

    pub(in crate::game) fn carried_units_mut(&mut self) -> impl Iterator<Item=&mut Unit> {
        self.carrying_space.iter_mut().flat_map(|carrying_space| carrying_space.carried_units_mut())
    }

    pub fn short_desc(&self) -> String {
        format!("{} \"{}\"", self.type_, self.name)
    }

    pub fn medium_desc(&self) -> String {
        format!("{} [{}/{}]", self.short_desc(), self.hp, self.max_hp)
    }

    /// Can this unit occupy cities?
    pub fn can_occupy_cities(&self) -> bool {
        self.type_.can_occupy_cities()
    }

    pub fn has_orders(&self) -> bool {
        self.orders.is_some()
    }
}

impl Aligned for Unit {
    fn alignment(&self) -> Alignment {
        self.alignment
    }
}

impl CombatCapable for Unit {
    fn hp(&self) -> u16 { self.hp }
    fn max_hp(&self) -> u16 { self.max_hp }
}

impl Colorized for Unit {
    fn color(&self) -> Option<Colors> {
        self.alignment.color()
    }
}

impl Located for Unit {
    fn loc(&self) -> Location {
        self.loc
    }
}

impl Named for Unit {
    fn name(&self) -> &String {
        &self.name
    }
}

impl Observer for Unit {
    fn sight_distance(&self) -> u16 {
        self.type_.sight_distance()
    }
}

impl fmt::Display for Unit {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut result = write!(f, "{} {}", self.alignment, self.medium_desc());
        if let Some(ref carrying_space) = self.carrying_space {
            result = result.and(write!(f, " carrying {} units", carrying_space.units_held()));
        }
        result
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashSet;
    use std::convert::TryFrom;
    use std::iter::FromIterator;

    use crate::{
        game::{
            map::{
                LocationGrid,
                Terrain,
                Tile,
                dijkstra::Source,
            },
            obs::{Obs,ObsTracker},
            unit::{Alignment,Observer,UnitID,Unit,UnitType},
        },
        util::{Dims,Location,Wrap2d},
    };


    #[test]
    fn test_observations() {
        let map_s = "\
x   o    x
x  ooo   x
x ooioo  x
x  ooo   x
x   o    x";

        match LocationGrid::<Tile>::try_from(map_s) {
            Err(err) => {
                panic!("Error parsing map: {}", err);
            },
            Ok(map) => {
                assert_eq!(map.dims(), Dims{width:10, height:5});

                let width = map_s.lines().map(|line| line.len()).max();
                let height = map_s.lines().count();

                assert_eq!(width, Some(10));
                assert_eq!(height, 5);

                let infantry_loc = Location{x:4, y:2};

                let infantry = Unit::new(UnitID::new(0), infantry_loc, UnitType::Infantry, Alignment::Belligerent{player:0}, "Lynn Stone");

                // let mut obs_tracker: ObsTracker = ObsTracker::new_fog_of_war(map.dims());
                let mut obs_tracker = ObsTracker::new(map.dims());

                for loc in map.iter_locs() {
                    assert_eq!(*obs_tracker.get(loc), Obs::Unobserved);
                }

                let turn = 0;

                infantry.observe(&map, turn, Wrap2d::BOTH, &mut obs_tracker);

                let observed_locs_arr = [
                    Location{x:4, y:0},
                    Location{x:3, y:1},
                    Location{x:4, y:1},
                    Location{x:5, y:1},
                    Location{x:2, y:2},
                    Location{x:3, y:2},
                    Location{x:4, y:2},
                    Location{x:5, y:2},
                    Location{x:6, y:2},
                    Location{x:3, y:3},
                    Location{x:4, y:3},
                    Location{x:5, y:3},
                    Location{x:4, y:4}
                ];
                let observed_locs: HashSet<&Location> = HashSet::from_iter(observed_locs_arr.iter());

                for loc in map.iter_locs() {
                    assert_eq!(*obs_tracker.get(loc), if observed_locs.contains(&loc) {
                        Obs::Observed{ tile: map[loc].clone(), turn: turn, current: true }
                    } else {
                        Obs::Unobserved
                    });
                }

                /*
                x   oo   x
                x  oooo  x
                x ooiioo x
                x  oooo  x
                x   oo   x"
                */
                let mut infantry = infantry;
                infantry.loc = Location{x:5, y:2};

                infantry.observe(&map, turn, Wrap2d::BOTH, &mut obs_tracker);

                let observed_locs_arr_2 = [
                    Location{x:5, y:0},
                    Location{x:6, y:1},
                    Location{x:7, y:2},
                    Location{x:6, y:3},
                    Location{x:5, y:4}
                ];
                let observed_locs_2: HashSet<&Location> = HashSet::from_iter(observed_locs_arr_2.iter());

                for loc in map.iter_locs() {
                    assert_eq!(*obs_tracker.get(loc), if observed_locs.contains(&loc) || observed_locs_2.contains(&loc) {
                        Obs::Observed{ tile: map[loc].clone(), turn: turn, current: true }
                    } else {
                        Obs::Unobserved
                    });
                }

                obs_tracker.archive();

                for loc in map.iter_locs() {
                    assert_eq!(*obs_tracker.get(loc), if observed_locs.contains(&loc) || observed_locs_2.contains(&loc) {
                        Obs::Observed{ tile: map[loc].clone(), turn: turn, current: false }
                    } else {
                        Obs::Unobserved
                    });
                }
            }
        }
    }

    #[test]
    fn test_mobility() {
        let loc = Location{x:5, y:5};

        let infantry = Unit::new(UnitID::new(0), loc, UnitType::Infantry, Alignment::Belligerent{player:0}, "Isabel Nash");
        let friendly_unit = Unit::new(UnitID::new(1), loc, UnitType::Armor, Alignment::Belligerent{player:0}, "Lynn Stone");
        let enemy_unit = Unit::new(UnitID::new(2), loc, UnitType::Armor, Alignment::Belligerent{player:1}, "James Lindsey");

        let tile1 = Tile::new(Terrain::Land, loc);
        assert!(infantry.can_move_on_tile(&tile1));

        let tile2 = Tile::new(Terrain::Water, loc);
        assert!(!infantry.can_move_on_tile(&tile2));

        let mut tile3 = Tile::new(Terrain::Land, loc);
        tile3.unit = Some(friendly_unit);
        assert!(!infantry.can_move_on_tile(&tile3));

        let mut tile4 = Tile::new(Terrain::Land, loc);
        tile4.unit = Some(enemy_unit);
        assert!(infantry.can_move_on_tile(&tile4));
    }
}
