extern crate termion;

use termion::color::AnsiValue;

use unit::{Alignment,City,Unit,Aligned,Sym};
use util::Location;


#[derive(Clone,PartialEq)]
pub enum Terrain {
    WATER,
    LAND,
    // CITY
    //ice, lava, river, deep sea vs shallow, etc.
}

impl Terrain {
    pub fn color(&self) -> AnsiValue {
        match *self {
            Terrain::WATER => AnsiValue(12),
            Terrain::LAND => AnsiValue(10),
            // Terrain::CITY => AnsiValue(245)
        }
    }
}

#[derive(Clone)]
pub struct Tile {
    pub terrain: Terrain,
    pub unit: Option<Unit>,
    pub city: Option<City>,
    pub loc: Location
}

impl Tile {
    fn new(terrain: Terrain, loc: Location) -> Tile {
        Tile{ terrain: terrain, unit: None, city: None, loc: loc }
    }

    pub fn sym(&self) -> char {
        match self.unit {
            None => match self.city {
                None => ' ',
                Some(ref city) => city.sym()
            },
            Some(ref unit) => unit.sym()
        }
    }

    pub fn alignment(&self) -> Option<Alignment> {
        match self.unit {
            None => match self.city {
                None => None,
                Some(ref city) => Some(city.alignment())
            },
            Some(ref unit) => Some(unit.alignment)
        }
    }

    pub fn fg_color(&self) -> Option<AnsiValue> {
        match self.unit {
            None => match self.city {
                None => None,
                Some(ref city) => Some(city.alignment().color())
            },
            Some(ref last_unit) => Some(last_unit.alignment.color())
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
}


pub mod gen;
