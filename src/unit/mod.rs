pub mod combat;

use std::fmt;

use termion::color::AnsiValue;

use game::TurnNum;
use game::obs::ObsTracker;
use map::{LocationGrid,Terrain,Tile};
use util::{Location,Vec2d,Wrap2d,wrapped_add};

pub type PlayerNum = u8;

#[derive(Copy,Clone,Debug,PartialEq,Hash,Eq)]
pub enum Alignment {
    NEUTRAL,
    BELLIGERENT { player: PlayerNum }
    // active neutral, chaotic, etc.
}

impl Alignment {
    pub fn color(&self) -> AnsiValue {
        match *self {
            Alignment::NEUTRAL => AnsiValue(8),
            Alignment::BELLIGERENT{player} => AnsiValue(player + 9 + if player >= 1 { 1 } else { 0 })
        }
    }
}

pub trait Located {
    fn loc(&self) -> Location;
}

pub trait Aligned {
    fn alignment(&self) -> Alignment;
}

pub trait Named {
    fn name(&self) -> &'static str;
}

pub trait Sym {
    fn sym(&self) -> &'static str;
}

pub trait Observer {
    fn sight_distance(&self) -> u16;
    fn observe(&self, observer_loc: Location, tiles: &LocationGrid<Tile>, turn: TurnNum, wrapping: Wrap2d, obs_tracker: &mut Box<ObsTracker>) {
        let sight = self.sight_distance() as i16;

        for i in (-sight)..(sight+1) {
            for j in (-sight)..(sight+1) {
                let dist = ((i.pow(2) + j.pow(2)) as f64).sqrt();

                // println!("{},{} dist {} sight {}", i, j, dist, sight);

                if dist <= sight as f64 {
                    let inc = Vec2d::new(i, j);
                    if let Some(loc) = wrapped_add(observer_loc, inc, tiles.dims(), wrapping) {

                        // println!("\t{}", loc);
                        obs_tracker.observe(loc, &tiles[loc], turn);
                    }
                }
            }
        }
    }
}

#[derive(Clone,Copy,Debug,Hash,PartialEq,Eq)]
pub enum UnitType {
    INFANTRY,
    ARMOR,
    FIGHTER,
    BOMBER,
    TRANSPORT,
    DESTROYER,
    SUBMARINE,
    CRUISER,
    BATTLESHIP,
    CARRIER
}

impl UnitType {
    pub fn values() -> Vec<UnitType> {
        vec![
            UnitType::INFANTRY,
            UnitType::ARMOR,
            UnitType::FIGHTER,
            UnitType::BOMBER,
            UnitType::TRANSPORT,
            UnitType::DESTROYER,
            UnitType::SUBMARINE,
            UnitType::CRUISER,
            UnitType::BATTLESHIP,
            UnitType::CARRIER
        ]
    }

    fn max_hp(&self) -> u16 {
        match *self {
            UnitType::INFANTRY => 1,
            UnitType::ARMOR => 2,
            UnitType::FIGHTER => 1,
            UnitType::BOMBER => 1,
            UnitType::TRANSPORT => 3,
            UnitType::DESTROYER => 2,
            UnitType::SUBMARINE => 2,
            UnitType::CRUISER => 4,
            UnitType::BATTLESHIP => 8,
            UnitType::CARRIER => 6
        }
    }

    pub fn cost(&self) -> u16 {
        match *self {
            UnitType::INFANTRY => 6,
            UnitType::ARMOR => 12,//?
            UnitType::FIGHTER => 12,
            UnitType::BOMBER => 12,//?
            UnitType::TRANSPORT => 30,
            UnitType::DESTROYER => 24,
            UnitType::SUBMARINE => 24,
            UnitType::CRUISER => 36,
            UnitType::BATTLESHIP => 60,
            UnitType::CARRIER => 48
        }
    }

    pub fn key(&self) -> char {
        match *self {
            UnitType::INFANTRY => 'i',
            UnitType::ARMOR => 'a',
            UnitType::FIGHTER => 'f',
            UnitType::BOMBER => 'b',
            UnitType::TRANSPORT => 't',
            UnitType::DESTROYER => 'd',
            UnitType::SUBMARINE => 's',
            UnitType::CRUISER => 'c',
            UnitType::BATTLESHIP => 'B',
            UnitType::CARRIER => 'C'
        }
    }

    pub fn sight_distance(&self) -> u16 {
        match *self {
            UnitType::INFANTRY => 2,
            UnitType::ARMOR => 2,
            UnitType::FIGHTER => 4,
            UnitType::BOMBER => 4,
            UnitType::TRANSPORT => 2,
            UnitType::DESTROYER => 3,
            UnitType::SUBMARINE => 3,
            UnitType::CRUISER => 3,
            UnitType::BATTLESHIP => 4,
            UnitType::CARRIER => 4
        }
    }

    pub fn from_key(c: &char) -> Option<UnitType> {
        for unit_type in UnitType::values().iter() {
            if unit_type.key() == *c {
                return Some(*unit_type);
            }
        }
        None
    }

    pub fn can_move_on(&self, terrain: &Terrain) -> bool {
        match *self {
            UnitType::INFANTRY => *terrain==Terrain::LAND,
            UnitType::ARMOR => *terrain==Terrain::LAND,
            UnitType::FIGHTER => *terrain==Terrain::LAND || *terrain==Terrain::WATER,
            UnitType::BOMBER => *terrain==Terrain::LAND || *terrain==Terrain::WATER,
            UnitType::TRANSPORT => *terrain==Terrain::WATER,
            UnitType::DESTROYER => *terrain==Terrain::WATER,
            UnitType::SUBMARINE => *terrain==Terrain::WATER,
            UnitType::CRUISER => *terrain==Terrain::WATER,
            UnitType::BATTLESHIP => *terrain==Terrain::WATER,
            UnitType::CARRIER => *terrain==Terrain::WATER
        }
    }
}

impl Named for UnitType {
    fn name(&self) -> &'static str {
        match *self {
            UnitType::INFANTRY => "Infantry",
            UnitType::ARMOR => "Armor",
            UnitType::FIGHTER => "Fighter",
            UnitType::BOMBER => "Bomber",
            UnitType::TRANSPORT => "Transport",
            UnitType::DESTROYER => "Destroyer",
            UnitType::SUBMARINE => "Submarine",
            UnitType::CRUISER => "Cruiser",
            UnitType::BATTLESHIP => "Battleship",
            UnitType::CARRIER => "Carrier"
        }
    }
}

#[derive(Clone,Copy,Debug,PartialEq)]
pub struct Unit {
    pub type_: UnitType,
    pub alignment: Alignment,
    hp: u16,
    max_hp: u16,
    pub sentry: bool,
    pub moves_remaining: u16
}





impl Unit {
    pub fn new(type_: UnitType, alignment: Alignment) -> Self {
        let max_hp =type_.max_hp();
        Unit {
            type_: type_,
            alignment: alignment,
            hp: max_hp,
            max_hp: max_hp,
            sentry: false,
            moves_remaining: 0
        }
    }

    pub fn movement_per_turn(&self) -> u16 {
        match self.type_ {
            UnitType::INFANTRY => 1,
            UnitType::ARMOR => 2,//?
            UnitType::FIGHTER => 5,
            UnitType::BOMBER => 5,//?
            UnitType::TRANSPORT => 2,
            UnitType::DESTROYER => 3,
            UnitType::SUBMARINE => 2,
            UnitType::CRUISER => 2,
            UnitType::BATTLESHIP => 1,
            UnitType::CARRIER => 1
        }
    }

    pub fn can_move_on(&self, tile: &Tile) -> bool {
        if !self.type_.can_move_on(&tile.terrain) {
            return false;
        }

        // If the destination tile contains no unit, we're free to go as we please

        // If it contains an enemy or neutral unit, we can in theory go there (if we defeat them in combat)

        // If it contains one of our units, we can't go there

        // But, if it contains any city we can go there---either as a visitor to our own city, or
        // attacking an enemy or neutral city

        if let Some(unit) = tile.unit {
            return self.alignment != unit.alignment;
        }

        return true;
    }
}

impl Sym for Unit {
    fn sym(&self) -> &'static str {
        match self.type_ {
            UnitType::INFANTRY => "i",
            UnitType::ARMOR => "A",
            UnitType::FIGHTER => "✈",
            UnitType::BOMBER => "b",
            UnitType::TRANSPORT => "t",
            UnitType::DESTROYER => "d",
            UnitType::SUBMARINE => "—",
            UnitType::CRUISER => "c",
            UnitType::BATTLESHIP => "B",
            UnitType::CARRIER => "C"
        }
    }
}

impl Aligned for Unit {
    fn alignment(&self) -> Alignment { self.alignment }
}

impl Observer for Unit {
    fn sight_distance(&self) -> u16 {
        self.type_.sight_distance()
    }
}

impl fmt::Display for Unit {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.type_.name())
    }
}

const CITY_MAX_HP: u16 = 4;

#[derive(Clone,Debug,Hash,PartialEq,Eq)]
pub struct City {
    pub loc: Location,
    pub alignment: Alignment,
    hp: u16,
    pub unit_under_production: Option<UnitType>,
    pub production_progress: u16,
    name: String
}

impl City {
    pub fn new<N:Into<String>>(name: N, alignment: Alignment, loc: Location) -> City {
        City {
            loc: loc,
            alignment: alignment,
            hp: CITY_MAX_HP,
            unit_under_production: None,
            production_progress: 0,
            name: name.into()
        }
    }
}

impl fmt::Display for City {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "City \"{}\"@{}", self.name, self.loc)
    }
}

impl Located for City {
    fn loc(&self) -> Location { self.loc }
}

impl Aligned for City {
    fn alignment(&self) -> Alignment { self.alignment }
}

impl Observer for City {
    fn sight_distance(&self) -> u16 {
        3
    }
}

impl Sym for City {
    fn sym(&self) -> &'static str { "#" }
}

impl Named for City {
    fn name(&self) -> &'static str { "City "}
}

#[cfg(test)]
mod test {
    use std::collections::HashSet;
    use std::convert::TryFrom;
    use std::iter::FromIterator;

    use game::obs::{FogOfWarTracker,Obs,ObsTracker};
    use map::{LocationGrid,Terrain,Tile};
    use unit::{Alignment,Observer,Unit,UnitType};
    use util::{Dims,Location,WRAP_BOTH};


    #[test]
    fn test_observations() {
        let map_s = "\
x   o    x
x  ooo   x
x ooioo  x
x  ooo   x
x   o    x";

        match LocationGrid::try_from(map_s) {
            Err(err) => {
                assert!(false, "Error parsing map: {}", err);
            },
            Ok(map) => {
                assert_eq!(map.dims(), Dims{width:10, height:5});

                let width = map_s.lines().map(|line| line.len()).max();
                let height = map_s.lines().count();

                let infantry_loc = Location{x:4, y:2};

                let infantry = Unit::new(UnitType::INFANTRY, Alignment::BELLIGERENT{player:0});

                let mut obs_tracker: Box<ObsTracker> = Box::new(FogOfWarTracker::new(map.dims()));

                for tile in map.iter() {
                    assert_eq!(*obs_tracker.get(tile.loc).unwrap(), Obs::UNOBSERVED);
                }

                let turn = 0;

                infantry.observe(infantry_loc, &map, turn, &WRAP_BOTH, &mut obs_tracker);

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
                    assert_eq!(*obs_tracker.get(tile.loc).unwrap(), if observed_locs.contains(&tile.loc) {
                        Obs::OBSERVED{ tile: map[tile.loc].clone(), turn: turn }
                    } else {
                        Obs::UNOBSERVED
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

                infantry.observe(infantry_loc_2, &map, turn, &WRAP_BOTH, &mut obs_tracker);

                let observed_locs_arr_2 = [
                    Location{x:5, y:0},
                    Location{x:6, y:1},
                    Location{x:7, y:2},
                    Location{x:6, y:3},
                    Location{x:5, y:4}
                ];
                let observed_locs_2: HashSet<&Location> = HashSet::from_iter(observed_locs_arr_2.iter());

                for tile in map.iter() {
                    assert_eq!(*obs_tracker.get(tile.loc).unwrap(), if observed_locs.contains(&tile.loc) || observed_locs_2.contains(&tile.loc) {
                        Obs::OBSERVED{ tile: map[tile.loc].clone(), turn: turn }
                    } else {
                        Obs::UNOBSERVED
                    });
                }
            }
        }
    }

    #[test]
    fn test_mobility() {
        let infantry = Unit::new(UnitType::INFANTRY, Alignment::BELLIGERENT{player:0});
        let friendly_unit = Unit::new(UnitType::ARMOR, Alignment::BELLIGERENT{player:0});
        let enemy_unit = Unit::new(UnitType::ARMOR, Alignment::BELLIGERENT{player:1});

        let loc = Location{x:5, y:5};
        
        let tile1 = Tile::new(Terrain::LAND, loc);
        assert!(infantry.can_move_on(&tile1));

        let tile2 = Tile::new(Terrain::WATER, loc);
        assert!(!infantry.can_move_on(&tile2));

        let mut tile3 = Tile::new(Terrain::LAND, loc);
        tile3.unit = Some(friendly_unit);
        assert!(!infantry.can_move_on(&tile3));

        let mut tile4 = Tile::new(Terrain::LAND, loc);
        tile4.unit = Some(enemy_unit);
        assert!(infantry.can_move_on(&tile4));

    }
}
