extern crate termion;

use termion::color::{Fg, AnsiValue};

use unit::{Alignment,City,Unit,alignment_color,Aligned,Sym};

#[derive(PartialEq)]
pub enum TerrainType {
    WATER,
    LAND,
    // CITY
    //ice, lava, river, deep sea vs shallow, etc.
}

pub struct Terrain {
    pub type_: TerrainType,
}

impl Terrain {
    pub fn water() -> Terrain {
        Terrain{ type_: TerrainType::WATER }
    }

    pub fn land() -> Terrain {
        Terrain{ type_: TerrainType::LAND }
    }

    pub fn color(&self) -> AnsiValue {
        match self.type_ {
            TerrainType::WATER => AnsiValue(12),
            TerrainType::LAND => AnsiValue(10),
            // TerrainType::CITY => AnsiValue(245)
        }
    }
}

pub struct Tile {
    pub terrain: Terrain,
    pub units: Vec<Unit>,
    pub city: Option<City>,
    pub x: u16,
    pub y: u16
}

impl Tile {
    fn new(terrain: Terrain, x:u16, y:u16) -> Tile {
        Tile{ terrain: terrain, units: Vec::new(), city: None, x: x, y: y }
    }

    pub fn sym(&self) -> char {
        match self.units.last() {
            Option::None => match self.city {
                None => ' ',
                Some(ref city) => city.sym()
            },
            Option::Some(unit) => unit.sym()
        }
    }

    pub fn alignment(&self) -> Option<Alignment> {
        match self.units.last() {
            Option::None => match self.city {
                None => None,
                Some(ref city) => Some(city.alignment())
            },
            Option::Some(unit) => Option::Some(unit.alignment)
        }
    }

    pub fn fg_color(&self) -> Option<AnsiValue> {
        match self.units.last() {
            Option::None => match self.city {
                None => None,
                Some(ref city) => Some(alignment_color(city.alignment()))
            },
            Option::Some(last_unit) => Option::Some(alignment_color(last_unit.alignment))
        }
    }

    pub fn bg_color(&self) -> AnsiValue {
        self.terrain.color()
    }
}


pub mod draw;
pub mod gen;
