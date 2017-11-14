//! Abstract map representation
//!
//! Data structures and algorithms for representing and working with the game map.

use std::fmt;

use termion::color::AnsiValue;

use unit::{Alignment,City,Sym,Unit};
use util::Location;


#[derive(Clone,PartialEq)]
pub enum Terrain {
    Water,
    Land,
    // CITY
    //ice, lava, river, deep sea vs shallow, etc.
}

impl Terrain {
    pub fn color(&self) -> AnsiValue {
        match *self {
            Terrain::Water => AnsiValue(12),
            Terrain::Land => AnsiValue(10),
            // Terrain::CITY => AnsiValue(245)
        }
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
        Tile{ terrain: terrain, unit: None, city: None, loc: loc }
    }

    pub fn sym(&self) -> &'static str {
        if let Some(ref unit) = self.unit {
            unit.sym()
        } else if let Some(ref city) = self.city {
            city.sym()
        } else {
            " "
        }
    }

    pub fn fg_color(&self) -> Option<AnsiValue> {
        match self.unit {
            Some(ref last_unit) => Some(last_unit.alignment.color()),
            None => match self.city {
                Some(ref city) => Some(city.alignment().color()),
                None => None
            }
        }
    }

    pub fn bg_color(&self) -> AnsiValue {
        self.terrain.color()
    }

    pub fn pop_unit(&mut self) -> Option<Unit> {
        let unit = self.unit.clone();
        self.unit = None;
        unit
    }

    pub fn set_unit(&mut self, unit: Unit) {
        self.unit = Some(unit);
    }

    pub fn alignment(&self) -> Option<Alignment> {
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
                write!(f, "{} with {} garrisoned", city, unit)
            } else {
                write!(f, "{}", city)
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
    use map::{Terrain,Tile};
    use unit::{Alignment,Unit,UnitType};
    use util::Location;


    #[test]
    fn test_tile() {
        let loc = Location{x: 10, y: 10};
        let terrain = Terrain::Land;

        let tile = Tile::new(terrain, loc);

        assert_eq!(tile.unit, None);

        let mut tile = tile;

        let unit = Unit::new(UnitType::Infantry, Alignment::Neutral, "Mordai Nowhere");
        let unit2 = unit.clone();
        tile.set_unit(unit);
        assert_eq!(tile.unit, Some(unit2));
    }

}
