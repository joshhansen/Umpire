//! Abstract representation of units and cities and their interactions.

pub mod combat;
pub mod orders;

use std::cmp::Ordering;
use std::fmt;

use crate::{
    color::{Colorized,Colors},
    game::{
        Aligned,Alignment,
        obs::Observer,
    },
    map::{Terrain,Tile},
    map::newmap::{CityID,UnitID},
    util::Location,
};

use self::orders::Orders;


pub trait Located {
    fn loc(&self) -> Location;
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
    pub fn values() -> [UnitType;10] {
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

    fn max_hp(self) -> u16 {
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
            UnitType::Battleship => 'B',
            UnitType::Carrier => 'C'
        }
    }

    pub fn sight_distance(self) -> u16 {
        match self {
            UnitType::Infantry | UnitType::Armor | UnitType::Transport => 2,
            UnitType::Destroyer | UnitType::Submarine | UnitType::Cruiser => 3,
            UnitType::Fighter | UnitType::Bomber | UnitType::Battleship | UnitType::Carrier => 4,
        }
    }

    pub fn from_key(c: char) -> Option<UnitType> {
        for unit_type in &UnitType::values() {
            if unit_type.key() == c {
                return Some(*unit_type);
            }
        }
        None
    }

    // pub fn can_move_on_terrain(&self, terrain: &Terrain) -> bool {
    //     match *self {
    //         UnitType::Infantry | UnitType::Armor =>
    //                 *terrain==Terrain::Land,
    //         UnitType::Fighter | UnitType::Bomber =>
    //                 *terrain==Terrain::Land || *terrain==Terrain::Water,
    //         UnitType::Transport | UnitType::Destroyer | UnitType::Submarine | UnitType::Cruiser |
    //         UnitType::Battleship | UnitType::Carrier =>
    //                 *terrain==Terrain::Water,
    //     }
    // }

    /// Determine whether this unit can move onto a particular tile (potentially requiring combat to do so).
    /// 
    /// If a city is present, this will always be true. Otherwise, it will be determined by the match between
    /// the unit's capabilities and the terrain (e.g. planes over water, but not tanks over water).
    pub fn can_move_on_tile(self, tile: &Tile) -> bool {
        tile.city.is_some() || match self {
            UnitType::Infantry | UnitType::Armor =>
                    tile.terrain==Terrain::Land,
            UnitType::Fighter | UnitType::Bomber =>
                    tile.terrain==Terrain::Land || tile.terrain==Terrain::Water,
            UnitType::Transport | UnitType::Destroyer | UnitType::Submarine | UnitType::Cruiser |
            UnitType::Battleship | UnitType::Carrier =>
                    tile.terrain==Terrain::Water,
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

#[derive(Clone,Debug,PartialEq)]
pub struct Unit {
    pub id: UnitID,
    pub loc: Location,
    pub type_: UnitType,
    pub alignment: Alignment,
    hp: u16,
    max_hp: u16,
    pub moves_remaining: u16,
    name: String,
    orders: Option<Orders>
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
            orders: None
        }
    }

    pub fn movement_per_turn(&self) -> u16 {
        match self.type_ {
            UnitType::Infantry | UnitType::Battleship | UnitType::Carrier => 1,
            UnitType::Armor | UnitType::Transport | UnitType::Submarine | UnitType::Cruiser => 2,
            UnitType::Destroyer => 3,
            UnitType::Fighter | UnitType::Bomber => 5,
        }
    }

    /// Indicate whether this unit can move (if only theoretically) onto a given tile
    /// Basically, the unit can (attempt to) move to any tile that is an appropriate terrain for
    /// its unit type and that does not already contain a friendly unit.
    /// The presence of cities makes no difference, because either we'll go as a visitor to our own
    /// city, or attempt to capture a hostile city.
    pub fn can_move_on_tile(&self, tile: &Tile) -> bool {
        if !self.type_.can_move_on_tile(tile) {
            return false;
        }

        if let Some(ref unit) = tile.unit {
            return self.alignment != unit.alignment;
        }

        true
    }

    pub fn orders(&self) -> Option<&Orders> { self.orders.as_ref() }

    pub fn set_orders(&mut self, orders: Option<Orders>) {
        self.orders = orders;
    }
}

impl Aligned for Unit {
    fn alignment(&self) -> Alignment {
        self.alignment
    }
}

impl Colorized for Unit {
    fn color(&self) -> Option<Colors> {
        self.alignment.color()
    }
}

impl Observer for Unit {
    fn sight_distance(&self) -> u16 {
        self.type_.sight_distance()
    }
}

impl fmt::Display for Unit {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} \"{}\" [{}/{}]", self.type_, self.name, self.hp, self.max_hp)
    }
}

const CITY_MAX_HP: u16 = 1;

#[derive(Clone,Hash,PartialEq,Eq)]
pub struct City {
    pub id: CityID,
    pub alignment: Alignment,
    pub loc: Location,//NOTE City location is also reflected in the Game::grid matrix, so this could be stale
    hp: u16,
    pub unit_under_production: Option<UnitType>,
    pub production_progress: u16,
    name: String
}
impl City {
    pub fn new<S:Into<String>>(id: CityID, alignment: Alignment, loc: Location, name: S) -> City {
        City {
            id,
            loc,
            alignment,
            hp: CITY_MAX_HP,
            unit_under_production: None,
            production_progress: 0,
            name: name.into()
        }
    }

    pub fn name(&self) -> &String {
        &self.name
    }
}

impl fmt::Display for City {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut result = write!(f, "City \"{}\"", self.name);
        if let Some(ref produced_unit) = self.unit_under_production {
            result = result.and(write!(f, ", producing {} ({}/{})", produced_unit, self.production_progress, produced_unit.cost()));
        }
        result
    }
}

impl fmt::Debug for City {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl Located for City {
    fn loc(&self) -> Location { self.loc }
}

impl Aligned for City {
    fn alignment(&self) -> Alignment {
        self.alignment
    }
}

impl Observer for City {
    fn sight_distance(&self) -> u16 {
        3
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashSet;
    use std::convert::TryFrom;
    use std::iter::FromIterator;

    use game::obs::{Obs,ObsTracker};
    use game::unit::{Alignment,Observer,Unit,UnitType};
    use map::{LocationGrid,Terrain,Tile};
    use map::newmap::UnitID;
    use util::{Dims,Location,WRAP_BOTH};


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

                for tile in map.iter() {
                    assert_eq!(*obs_tracker.get(tile.loc), Obs::Unobserved);
                }

                let turn = 0;

                infantry.observe(infantry_loc, &map, turn, WRAP_BOTH, &mut obs_tracker);

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

                for tile in map.iter() {
                    assert_eq!(*obs_tracker.get(tile.loc), if observed_locs.contains(&tile.loc) {
                        Obs::Observed{ tile: map[tile.loc].clone(), turn: turn, current: false }
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
                let infantry_loc_2 = Location{x:5, y:2};

                infantry.observe(infantry_loc_2, &map, turn, WRAP_BOTH, &mut obs_tracker);

                let observed_locs_arr_2 = [
                    Location{x:5, y:0},
                    Location{x:6, y:1},
                    Location{x:7, y:2},
                    Location{x:6, y:3},
                    Location{x:5, y:4}
                ];
                let observed_locs_2: HashSet<&Location> = HashSet::from_iter(observed_locs_arr_2.iter());

                for tile in map.iter() {
                    assert_eq!(*obs_tracker.get(tile.loc), if observed_locs.contains(&tile.loc) || observed_locs_2.contains(&tile.loc) {
                        Obs::Observed{ tile: map[tile.loc].clone(), turn: turn, current: false }
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
