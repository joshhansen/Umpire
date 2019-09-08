//! Abstract map representation
//!
//! Data structures and algorithms for representing and working with the game map.

use std::fmt;

use crate::color::{Colors,Colorized};
use game::{Aligned,AlignedMaybe,Alignment};
use game::unit::{City,Unit};
use util::Location;


#[derive(Clone,PartialEq)]
pub enum Terrain {
    Water,
    Land,
    // CITY
    //ice, lava, river, deep sea vs shallow, etc.
}

impl Colorized for Terrain {
    fn color(&self) -> Option<Colors> {
        Some(match *self {
            Terrain::Water => Colors::Ocean,
            Terrain::Land => Colors::Land
        })
    }
}

impl fmt::Display for Terrain {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", match *self {
            Terrain::Water => "Water",
            Terrain::Land => "Land"
        })
    }
}

impl fmt::Debug for Terrain {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

#[derive(Clone,Debug,PartialEq)]
pub struct Tile {
    pub terrain: Terrain,
    pub unit: Option<Unit>,
    pub city: Option<City>,
    pub loc: Location
}

impl Tile {
    pub fn new(terrain: Terrain, loc: Location) -> Tile {
        Tile{ terrain, unit: None, city: None, loc }
    }

    pub fn pop_unit(&mut self) -> Option<Unit> {
        let unit = self.unit.clone();
        self.unit = None;
        unit
    }

    pub fn set_unit(&mut self, unit: Unit) {
        self.unit = Some(unit);
    }
}

impl Colorized for Tile {
    fn color(&self) -> Option<Colors> {
        if let Some(ref last_unit) = self.unit {
            last_unit.alignment.color()
        } else if let Some(ref city) = self.city {
            city.alignment().color()
        } else {
            None
        }
    }
}

impl AlignedMaybe for Tile {
    fn alignment_maybe(&self) -> Option<Alignment> {
        if let Some(ref city) = self.city {
            Some(city.alignment())
        } else if let Some(ref unit) = self.unit {
            Some(unit.alignment())
        } else {
            None
        }
    }
}

impl fmt::Display for Tile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(ref city) = self.city {
            if let Some(ref unit) = self.unit {
                write!(f, "{} with {} garrisoned; {}", city, unit, self.terrain)
            } else {
                write!(f, "{}; {}", city, self.terrain)
            }
        } else if let Some(ref unit) = self.unit {
            write!(f, "{} on {}", unit, self.terrain)
        } else {
            write!(f, "{}", self.terrain)
        }
    }
}


pub mod dijkstra;
pub mod gen;
mod grid;
pub mod newmap;

pub use self::grid::LocationGrid;

#[cfg(test)]
mod test {
    use crate::{
        game::{
            Alignment,
            map::{
                Terrain,
                Tile,
                newmap::UnitID,
            },
            unit::{Unit,UnitType},
        },
        util::Location
    };


    #[test]
    fn test_tile() {
        let loc = Location{x: 10, y: 10};
        let terrain = Terrain::Land;

        let tile = Tile::new(terrain, loc);

        assert_eq!(tile.unit, None);

        let mut tile = tile;

        let unit = Unit::new(UnitID::new(0), loc, UnitType::Infantry, Alignment::Neutral, "Mordai Nowhere");
        let unit2 = unit.clone();
        tile.set_unit(unit);
        assert_eq!(tile.unit, Some(unit2));
    }

}
